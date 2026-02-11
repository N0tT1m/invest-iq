# InvestIQ On-Call Runbook

**Last Updated**: 2026-02-11
**Platform**: Rust/Axum API (port 3000) + Python Dash (port 8050) + FastAPI ML (port 8004)

---

## Quick Reference

### Service Ports
- **API Server**: 3000 (Rust/Axum)
- **Frontend**: 8050 (Python Dash)
- **ML Service**: 8004 (FastAPI signal-models)
- **Database**: SQLite at `/data/investiq.db` (or Postgres)
- **Redis**: 6379 (optional cache)

### Critical Environment Variables
```bash
DATABASE_URL          # SQLite or Postgres connection
POLYGON_API_KEY       # Market data (REQUIRED)
ALPACA_API_KEY        # Brokerage auth
ALPACA_SECRET_KEY     # Brokerage auth
ALPACA_BASE_URL       # paper-api or api (PRODUCTION CHECK)
REDIS_URL             # Cache connection
API_KEYS              # Comma-separated auth keys
LIVE_TRADING_KEY      # Extra gate for live orders
ML_SERVICE_URL        # Signal models endpoint (default: http://localhost:8004)
```

### Log Locations
- **Docker**: `docker compose logs -f [service]`
- **Systemd**: `journalctl -u investiq-api -f`
- **Direct**: `RUST_LOG=info cargo run`

---

## Alert Response Procedures

### 1. HighErrorRate
**Alert**: `rate(investiq_errors_total[5m]) > 0.1` for 2 minutes

**Severity**: High
**Impact**: User-facing errors, potential data loss, degraded service quality

#### First Response Checklist
```bash
# 1. Check current error rate and types
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health

# 2. Check recent logs for stack traces
docker compose logs --tail=100 api-server | grep -i error

# 3. Identify error patterns
docker compose logs --tail=500 api-server | grep "ERROR" | cut -d'"' -f4 | sort | uniq -c | sort -rn
```

#### Common Root Causes
1. **Polygon API rate limiting** (HTTP 429)
   - Symptom: "too many requests" in logs
   - Check: `grep "429" logs` or Polygon dashboard usage

2. **Alpaca API errors** (authentication, paper vs live mismatch)
   - Symptom: "401 Unauthorized" or "forbidden" in broker routes
   - Check: Verify `ALPACA_BASE_URL` matches API keys (paper vs live)

3. **Database connection pool exhaustion**
   - Symptom: "unable to open database file" or "connection pool timeout"
   - Check: `lsof | grep investiq.db | wc -l` (should be < 100)

4. **ML service unreachable**
   - Symptom: "connection refused" to port 8004
   - Check: `curl http://localhost:8004/health`

5. **Invalid data from external APIs**
   - Symptom: "failed to deserialize" or "NoneError"
   - Check: Response payloads in debug logs

#### Resolution Steps

**For Polygon Rate Limiting**:
```bash
# 1. Check current rate limit setting
grep POLYGON_RATE_LIMIT .env

# 2. Temporarily reduce rate (requires restart)
echo "POLYGON_RATE_LIMIT=300" >> .env
docker compose restart api-server

# 3. Clear ETF bar cache to reduce immediate load
docker compose exec api-server redis-cli FLUSHDB  # if using Redis
```

**For Alpaca API Errors**:
```bash
# 1. Verify credentials are valid
curl -H "APCA-API-KEY-ID: $ALPACA_API_KEY" \
     -H "APCA-API-SECRET-KEY: $ALPACA_SECRET_KEY" \
     https://paper-api.alpaca.markets/v2/account

# 2. Check base URL matches keys
grep ALPACA_BASE_URL .env
# Should be "https://paper-api.alpaca.markets" for paper keys

# 3. Restart with corrected config
docker compose restart api-server
```

**For Database Issues**:
```bash
# 1. Check database file integrity
sqlite3 /data/investiq.db "PRAGMA integrity_check;"

# 2. Check disk space
df -h /data

# 3. Restart with connection pool adjustment
# Add to .env: DATABASE_MAX_CONNECTIONS=25
docker compose restart api-server

# 4. If corrupted, restore from backup
./scripts/restore-db.sh /backups/investiq.db.YYYYMMDD
```

**For ML Service Down**:
```bash
# 1. Check if service is running
docker compose ps signal-models

# 2. Restart ML service
docker compose restart signal-models

# 3. Verify models loaded
curl http://localhost:8004/health
# Should return: {"status": "healthy", "models_loaded": true}

# 4. If models missing, retrain
docker compose exec signal-models python signal_models/train.py
```

#### Escalation Path
1. **If errors persist after 15 minutes**: Page senior engineer
2. **If data corruption suspected**: Page database administrator + stop writes
3. **If external API outage**: Check Polygon/Alpaca status pages, notify stakeholders

---

### 2. HighLatency
**Alert**: `histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 1` for 5 minutes

**Severity**: Medium
**Impact**: Slow user experience, potential timeouts (frontend has 30s timeout)

#### First Response Checklist
```bash
# 1. Check health endpoint latency breakdown
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health | jq

# 2. Identify slow endpoints
docker compose logs --tail=200 api-server | grep "latency=" | awk '{print $NF}' | sort -rn | head -20

# 3. Check CPU and memory usage
docker stats --no-stream

# 4. Check active database connections
sqlite3 /data/investiq.db "PRAGMA database_list;"
# Or for Postgres: SELECT count(*) FROM pg_stat_activity;
```

#### Common Root Causes
1. **Cold Polygon cache** (first request after ETF cache expiry)
   - Symptom: Spike in `/api/analyze` latency, 15+ second response
   - Time pattern: Every 15 minutes when ETF cache expires

2. **Slow database queries** (missing indexes, large table scans)
   - Symptom: Queries in EXPLAIN plan show "SCAN TABLE"
   - Check: Enable query logging, run EXPLAIN on slow queries

3. **ML service overload** (model inference backlog)
   - Symptom: `/predict` endpoint slow, queue buildup
   - Check: ML service logs for processing times

4. **Network issues to external APIs**
   - Symptom: High latency on Polygon/Alpaca calls
   - Check: `curl -w "@curl-format.txt"` timing breakdown

5. **Memory pressure causing GC pauses** (Python frontend)
   - Symptom: Dash callbacks slow, memory usage climbing
   - Check: `docker stats frontend` shows high memory

#### Resolution Steps

**For Cold Cache Issues**:
```bash
# 1. Pre-warm ETF cache (if Redis available)
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/flow/map/SPY

# 2. Increase cache TTL (requires code change + redeploy)
# In orchestrator.rs, change ETF_CACHE_TTL from 900s to 1800s

# 3. Temporary: Accept degraded performance during cache refresh
# No action needed, will resolve in 1-2 minutes
```

**For Database Performance**:
```bash
# 1. Identify slow queries
sqlite3 /data/investiq.db "PRAGMA query_log;"

# 2. Check for missing indexes
sqlite3 /data/investiq.db ".indexes"

# 3. Analyze table stats
sqlite3 /data/investiq.db "ANALYZE;"

# 4. Vacuum database if bloated
sqlite3 /data/investiq.db "VACUUM;"

# 5. If Postgres, check slow query log
tail -f /var/log/postgresql/postgresql-*.log | grep "duration:"
```

**For ML Service Bottleneck**:
```bash
# 1. Check request queue depth
docker compose logs signal-models | grep "queue_depth"

# 2. Scale ML service replicas (if using orchestrator)
docker compose up -d --scale signal-models=3

# 3. Temporarily disable ML gating (requires restart)
# Set ML_GATING_ENABLED=false in .env
docker compose restart trading-agent
```

**For Network Latency**:
```bash
# 1. Test Polygon API latency
curl -w "time_total: %{time_total}\n" \
     "https://api.polygon.io/v2/aggs/ticker/SPY/range/1/day/2025-01-01/2025-01-31?apiKey=$POLYGON_API_KEY"

# 2. Test Alpaca API latency
curl -w "time_total: %{time_total}\n" \
     -H "APCA-API-KEY-ID: $ALPACA_API_KEY" \
     -H "APCA-API-SECRET-KEY: $ALPACA_SECRET_KEY" \
     https://paper-api.alpaca.markets/v2/account

# 3. If consistently slow, check for regional routing issues
traceroute api.polygon.io
```

**For Memory Pressure**:
```bash
# 1. Restart frontend to clear memory
docker compose restart frontend

# 2. Check for memory leaks in Dash callbacks
docker compose logs frontend | grep "MemoryError"

# 3. Increase container memory limit
# In docker-compose.yml, add: mem_limit: 2g
docker compose up -d frontend
```

#### Escalation Path
1. **If latency > 5s for 10+ minutes**: Page infrastructure team
2. **If external API slow**: Check status pages, notify stakeholders
3. **If database performance degraded**: Page database administrator

---

### 3. ServiceDown
**Alert**: `up == 0` for 1 minute

**Severity**: Critical
**Impact**: Complete service outage, no user access

#### First Response Checklist
```bash
# 1. Identify which service is down
docker compose ps

# 2. Check if process crashed or killed
docker compose logs --tail=50 [service-name]

# 3. Check system resources
df -h           # Disk space
free -h         # Memory
top -bn1        # CPU/processes

# 4. Check if port is bound by another process
lsof -i :3000   # API server
lsof -i :8050   # Frontend
lsof -i :8004   # ML service
```

#### Common Root Causes
1. **Out of disk space**
   - Symptom: "No space left on device" in logs
   - Check: `df -h` shows 100% usage

2. **OOM killer terminated process**
   - Symptom: "Killed" message in logs, dmesg shows OOM
   - Check: `dmesg | grep -i "out of memory"`

3. **Configuration error after deploy**
   - Symptom: "failed to parse config" or "missing required env var"
   - Check: Container exits immediately on start

4. **Database file locked or corrupted**
   - Symptom: "database is locked" or "malformed database"
   - Check: Multiple processes accessing SQLite

5. **Port already in use**
   - Symptom: "Address already in use" in logs
   - Check: `lsof -i :[port]`

#### Resolution Steps

**For Disk Space Issues**:
```bash
# 1. Check what's consuming space
du -sh /var/lib/docker/* | sort -rh | head -10

# 2. Clean up Docker artifacts
docker system prune -a --volumes -f

# 3. Rotate/compress logs
docker compose logs > /tmp/logs-$(date +%Y%m%d).txt
docker compose restart

# 4. Delete old backups
find /backups -name "*.db.*" -mtime +7 -delete
```

**For OOM Issues**:
```bash
# 1. Check which service was killed
dmesg -T | grep -i "killed process"

# 2. Increase memory limits in docker-compose.yml
# api-server: mem_limit: 2g
# frontend: mem_limit: 1g
# signal-models: mem_limit: 1g

# 3. Restart with new limits
docker compose up -d
```

**For Configuration Errors**:
```bash
# 1. Check required env vars are set
docker compose config

# 2. Validate .env file syntax
cat .env | grep -v "^#" | grep -v "^$"

# 3. Check for typos in recent changes
git diff HEAD~1 .env docker-compose.yml

# 4. Rollback to last known good config
git checkout HEAD~1 .env
docker compose up -d
```

**For Database Lock**:
```bash
# 1. Find processes with DB file open
lsof | grep investiq.db

# 2. Kill stale connections
# If multiple services, stop all then start one by one
docker compose down
docker compose up -d api-server
docker compose up -d frontend

# 3. If corrupted, restore from backup
cp /backups/investiq.db.latest /data/investiq.db
docker compose up -d
```

**For Port Conflicts**:
```bash
# 1. Find process using port
lsof -i :3000

# 2. Kill process (if stale)
kill -9 [PID]

# 3. If app.py stale process (common)
pkill -9 -f "python.*app.py"

# 4. Restart service
docker compose up -d
```

#### Escalation Path
1. **If service won't start after 5 minutes**: Page senior engineer + infrastructure
2. **If data corruption suspected**: Page database administrator IMMEDIATELY
3. **If infrastructure issue (disk, network)**: Page infrastructure team

---

### 4. CircuitBreakerTripped
**Alert**: Trading halt detected in `/api/risk/circuit-breakers` response

**Severity**: High
**Impact**: All automated trading stopped, manual intervention required

#### First Response Checklist
```bash
# 1. Check circuit breaker status
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/risk/circuit-breakers | jq

# 2. Check what triggered the halt
# Response includes: halt_reason, consecutive_losses, daily_loss_percent, drawdown_percent, halted_at

# 3. Review recent trade outcomes
sqlite3 /data/investiq.db "SELECT * FROM trade_outcomes ORDER BY timestamp DESC LIMIT 10;"

# 4. Check current positions and P/L
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/broker/positions | jq
```

#### Common Root Causes
1. **Consecutive losses threshold** (3+ losses in a row)
   - Symptom: `halt_reason: "3 consecutive losses"`
   - Root cause: Strategy malfunction or adverse market conditions

2. **Daily loss limit** (5% account drawdown in one day)
   - Symptom: `halt_reason: "Daily loss limit exceeded"`
   - Root cause: Large losing trades or high volatility

3. **Account drawdown** (10% from peak equity)
   - Symptom: `halt_reason: "Account drawdown limit"`
   - Root cause: Sustained losing period or position sizing issue

4. **Manual halt** (triggered by operator)
   - Symptom: `halt_reason: "Manual halt"`
   - Root cause: Preventive action during investigation

#### Resolution Steps

**Investigation Phase** (DO NOT RESET YET):
```bash
# 1. Export recent trade history for analysis
curl -H "X-API-Key: $API_KEY" \
     "http://localhost:3000/api/portfolio/trades?days=7" | jq > trades_$(date +%Y%m%d).json

# 2. Check if issue is strategy-specific
sqlite3 /data/investiq.db <<EOF
SELECT symbol, COUNT(*), SUM(profit_loss)
FROM trade_outcomes
WHERE timestamp > datetime('now', '-1 day')
GROUP BY symbol;
EOF

# 3. Review ML predictions vs outcomes
sqlite3 /data/investiq.db <<EOF
SELECT predicted_profitable, confidence, actual_profitable, COUNT(*)
FROM trade_outcomes
WHERE timestamp > datetime('now', '-7 days')
GROUP BY predicted_profitable, actual_profitable;
EOF

# 4. Check for market regime change
curl -H "X-API-Key: $API_KEY" \
     "http://localhost:3000/api/flow/map/SPY" | jq '.market_summary'
```

**Resolution Decision Tree**:

**If consecutive losses from same symbol**:
```bash
# Add symbol to blacklist (requires code change)
# Or manually close positions
curl -X DELETE -H "X-API-Key: $API_KEY" \
     -H "X-Live-Trading-Key: $LIVE_TRADING_KEY" \
     "http://localhost:3000/api/broker/close/SYMBOL"

# Reset circuit breaker
curl -X POST -H "X-API-Key: $API_KEY" \
     "http://localhost:3000/api/risk/trading-halt" \
     -H "Content-Type: application/json" \
     -d '{"halt": false, "reason": "Investigated - bad symbol removed"}'
```

**If ML predictions inaccurate**:
```bash
# 1. Check ML service health
curl http://localhost:8004/health

# 2. Retrain models with recent data
docker compose exec signal-models python signal_models/train.py

# 3. Verify calibration improved
curl http://localhost:8004/health | jq '.model_performance'

# 4. If still bad, disable ML gating temporarily
# Set ML_GATING_ENABLED=false in .env
docker compose restart trading-agent

# 5. DO NOT reset breaker until models validated
```

**If market regime changed**:
```bash
# 1. Check if VIX spiked or major news
curl -H "X-API-Key: $API_KEY" \
     "http://localhost:3000/api/analyze/SPY" | jq '.sentiment'

# 2. Adjust position sizing temporarily
# Requires risk parameter update (future enhancement)

# 3. Reset breaker with caution
curl -X POST -H "X-API-Key: $API_KEY" \
     "http://localhost:3000/api/risk/trading-halt" \
     -H "Content-Type: application/json" \
     -d '{"halt": false, "reason": "Market regime assessed - reducing position size"}'

# 4. Monitor closely for 1 hour
watch -n 60 'curl -s -H "X-API-Key: $API_KEY" http://localhost:3000/api/broker/positions'
```

**If drawdown from position sizing error**:
```bash
# 1. Check current risk parameters
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/risk/parameters | jq

# 2. Review position sizes vs limits
sqlite3 /data/investiq.db <<EOF
SELECT symbol, quantity, entry_price, quantity * entry_price as position_value
FROM trades
WHERE exit_date IS NULL;
EOF

# 3. Close oversized positions
curl -X DELETE -H "X-API-Key: $API_KEY" \
     -H "X-Live-Trading-Key: $LIVE_TRADING_KEY" \
     "http://localhost:3000/api/broker/close/[SYMBOL]"

# 4. Update risk limits if needed
curl -X PUT -H "X-API-Key: $API_KEY" \
     "http://localhost:3000/api/risk/parameters" \
     -H "Content-Type: application/json" \
     -d '{"max_position_size": 0.05}'  # 5% instead of 10%

# 5. Reset breaker
curl -X POST -H "X-API-Key: $API_KEY" \
     "http://localhost:3000/api/risk/trading-halt" \
     -H "Content-Type: application/json" \
     -d '{"halt": false, "reason": "Position sizing corrected"}'
```

#### Post-Reset Monitoring
```bash
# 1. Watch for immediate re-trigger (indicates unresolved issue)
watch -n 30 'curl -s -H "X-API-Key: $API_KEY" http://localhost:3000/api/risk/circuit-breakers'

# 2. Monitor next 5 trades closely
tail -f /var/log/investiq/api-server.log | grep "trade_executed"

# 3. Set up alert for next trigger
# Alert should wake someone immediately if re-triggered within 1 hour
```

#### Escalation Path
1. **If root cause unclear after 30 minutes**: Page senior engineer + risk manager
2. **If ML models degraded**: Page ML engineer
3. **If market event (flash crash, news)**: Notify trading manager BEFORE reset
4. **If re-triggers within 1 hour**: STOP investigation, page entire team

---

### 5. MLServiceDown
**Alert**: Signal-models service unreachable for 5 minutes

**Severity**: Medium
**Impact**: Analysis falls back to default weights, reduced confidence, agent may skip trades

#### First Response Checklist
```bash
# 1. Check if service is running
docker compose ps signal-models

# 2. Check health endpoint
curl http://localhost:8004/health

# 3. Check recent logs
docker compose logs --tail=100 signal-models

# 4. Verify port is listening
lsof -i :8004
```

#### Common Root Causes
1. **Model files missing** (not loaded at startup)
   - Symptom: "model not found" or "FileNotFoundError" in logs
   - Check: `/ml-services/signal_models/models/` directory

2. **Python dependencies broken** (package conflict)
   - Symptom: "ImportError" or "ModuleNotFoundError"
   - Check: `pip list` inside container

3. **OOM during inference** (large batch request)
   - Symptom: Container exits with code 137
   - Check: `docker stats` memory usage

4. **Training script still running** (locks model files)
   - Symptom: "Resource temporarily unavailable"
   - Check: `ps aux | grep train.py`

5. **FastAPI crashed** (unhandled exception)
   - Symptom: Traceback in logs, service exits
   - Check: Last exception in logs

#### Resolution Steps

**For Missing Models**:
```bash
# 1. Check model files exist
docker compose exec signal-models ls -lh /app/signal_models/models/

# 2. If missing, retrain
docker compose exec signal-models python signal_models/train.py

# 3. Verify models loaded
curl http://localhost:8004/health
# Should show: {"models_loaded": true}

# 4. Restart service
docker compose restart signal-models
```

**For Dependency Issues**:
```bash
# 1. Check for conflicting packages
docker compose exec signal-models pip check

# 2. Reinstall requirements
docker compose exec signal-models pip install -r signal_models/requirements.txt --force-reinstall

# 3. If still broken, rebuild image
docker compose build --no-cache signal-models
docker compose up -d signal-models
```

**For OOM**:
```bash
# 1. Increase memory limit
# In docker-compose.yml, update:
# signal-models:
#   mem_limit: 2g  # was 1g

# 2. Restart with new limit
docker compose up -d signal-models

# 3. If recurring, reduce batch size in client
# In ml-client/src/signal_models.rs, reduce BATCH_SIZE
```

**For Training Lock**:
```bash
# 1. Find training process
docker compose exec signal-models ps aux | grep train.py

# 2. Kill if stale (running > 30 minutes)
docker compose exec signal-models kill [PID]

# 3. Remove lock files if exist
docker compose exec signal-models rm -f /app/signal_models/models/.lock

# 4. Restart service
docker compose restart signal-models
```

**For FastAPI Crash**:
```bash
# 1. Check full traceback
docker compose logs signal-models | grep -A 50 "Traceback"

# 2. If input validation error, check recent API calls
docker compose logs api-server | grep "/predict"

# 3. If code bug, rollback to previous version
git log --oneline ml-services/signal_models/
git checkout [PREVIOUS_COMMIT] ml-services/signal_models/
docker compose build signal-models
docker compose up -d signal-models

# 4. File bug with stack trace for fix
```

#### Fallback Behavior
When ML service is down, the system automatically falls back:
- **Meta prediction**: Returns probability 0.5 (neutral)
- **Confidence calibration**: Returns raw confidence values
- **Weight optimization**: Uses default weights (20/40/15/25)
- **Trading agent**: Skips ML gating if confidence < 0.75

**This is safe** — system continues with reduced intelligence, not dangerous behavior.

#### Escalation Path
1. **If service down > 15 minutes**: Page ML engineer
2. **If model performance degraded**: Page ML engineer + provide recent analysis_features data
3. **If training data corruption**: Page database administrator + ML engineer

---

## Common Operational Tasks

### Restart Services

**Restart all services**:
```bash
docker compose restart
```

**Restart specific service**:
```bash
docker compose restart api-server
docker compose restart frontend
docker compose restart signal-models
docker compose restart trading-agent
```

**Full stop/start (clears state)**:
```bash
docker compose down
docker compose up -d
```

**Restart with new configuration**:
```bash
# Edit .env or docker-compose.yml
docker compose down
docker compose up -d --build  # Rebuild if code changed
```

**Graceful restart (zero downtime)**:
```bash
# Start new container before stopping old
docker compose up -d --no-deps --build api-server
```

---

### Check Logs

**View live logs**:
```bash
# All services
docker compose logs -f

# Specific service
docker compose logs -f api-server
docker compose logs -f frontend
docker compose logs -f signal-models
docker compose logs -f trading-agent

# Last N lines
docker compose logs --tail=200 api-server

# Specific time range (requires JSON logging)
docker compose logs --since="2026-02-11T03:00:00" --until="2026-02-11T03:30:00" api-server
```

**Search logs**:
```bash
# Find errors
docker compose logs api-server | grep -i error

# Find slow requests
docker compose logs api-server | grep "latency=" | awk -F'latency=' '{print $2}' | sort -rn

# Find specific symbol trades
docker compose logs trading-agent | grep "AAPL"

# Count error types
docker compose logs api-server | grep ERROR | cut -d'"' -f4 | sort | uniq -c | sort -rn
```

**Export logs for analysis**:
```bash
# Last 24 hours
docker compose logs --since="24h" > logs-$(date +%Y%m%d).txt

# Compressed
docker compose logs --since="24h" | gzip > logs-$(date +%Y%m%d).txt.gz
```

---

### Database Backup and Restore

**Manual backup**:
```bash
# Using backup script (recommended)
./scripts/backup-db.sh

# Manual SQLite backup
sqlite3 /data/investiq.db ".backup /backups/investiq.db.$(date +%Y%m%d_%H%M%S)"

# Verify backup integrity
sqlite3 /backups/investiq.db.YYYYMMDD "PRAGMA integrity_check;"
```

**Restore from backup**:
```bash
# Using restore script
./scripts/restore-db.sh /backups/investiq.db.20260211

# Manual restore
docker compose down api-server trading-agent  # Stop services using DB
cp /backups/investiq.db.20260211 /data/investiq.db
docker compose up -d api-server trading-agent

# Verify restore
sqlite3 /data/investiq.db "SELECT COUNT(*) FROM trades;"
```

**Automated backup check**:
```bash
# Verify backup service is running
docker compose ps db-backup

# Check recent backups exist
ls -lh /backups/investiq.db.* | tail -7  # Should have 7 days

# Test backup integrity
for backup in /backups/investiq.db.*; do
  echo "Checking $backup"
  sqlite3 "$backup" "PRAGMA integrity_check;"
done
```

---

### Rotate Secrets

**Rotate API keys** (API_KEYS for API server):
```bash
# 1. Generate new key
NEW_KEY=$(openssl rand -hex 32)
echo "New API key: $NEW_KEY"

# 2. Add to .env (keep old key temporarily)
echo "API_KEYS=$OLD_KEY,$NEW_KEY" >> .env

# 3. Restart API server
docker compose restart api-server

# 4. Update frontend .env
cd frontend
sed -i "s/API_KEY=.*/API_KEY=$NEW_KEY/" .env

# 5. Restart frontend
docker compose restart frontend

# 6. Test new key works
curl -H "X-API-Key: $NEW_KEY" http://localhost:3000/health

# 7. Remove old key from API_KEYS after 24h grace period
```

**Rotate live trading key**:
```bash
# 1. Generate new key
NEW_KEY=$(openssl rand -hex 32)

# 2. Update .env
sed -i "s/LIVE_TRADING_KEY=.*/LIVE_TRADING_KEY=$NEW_KEY/" .env

# 3. Update frontend .env
sed -i "s/LIVE_TRADING_KEY=.*/LIVE_TRADING_KEY=$NEW_KEY/" frontend/.env

# 4. Restart services
docker compose restart api-server frontend

# 5. Test (should fail without new key)
curl -X POST -H "X-API-Key: $API_KEY" \
     http://localhost:3000/api/broker/execute \
     -d '{"symbol":"SPY","action":"buy","quantity":1}'
# Should return 403

# 6. Test with new key
curl -X POST -H "X-API-Key: $API_KEY" \
     -H "X-Live-Trading-Key: $NEW_KEY" \
     http://localhost:3000/api/broker/execute \
     -d '{"symbol":"SPY","action":"buy","quantity":1}'
```

**Rotate Polygon API key**:
```bash
# 1. Get new key from Polygon.io dashboard

# 2. Update .env
sed -i "s/POLYGON_API_KEY=.*/POLYGON_API_KEY=$NEW_KEY/" .env

# 3. Restart all services (orchestrator is embedded)
docker compose restart

# 4. Test
curl -H "X-API-Key: $API_KEY" "http://localhost:3000/api/analyze/SPY"
```

**Rotate Alpaca credentials**:
```bash
# 1. Get new keys from Alpaca dashboard

# 2. Update .env
sed -i "s/ALPACA_API_KEY=.*/ALPACA_API_KEY=$NEW_KEY_ID/" .env
sed -i "s/ALPACA_SECRET_KEY=.*/ALPACA_SECRET_KEY=$NEW_SECRET/" .env

# 3. Restart services using Alpaca
docker compose restart api-server trading-agent

# 4. Test
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/broker/account
```

**Emergency key revocation** (compromised key):
```bash
# 1. Remove compromised key from API_KEYS immediately
sed -i "s/COMPROMISED_KEY,*//" .env

# 2. Restart API server
docker compose restart api-server

# 3. Check audit log for recent usage
sqlite3 /data/investiq.db "SELECT * FROM audit_log WHERE api_key LIKE '%COMPROMISED%' ORDER BY timestamp DESC LIMIT 50;"

# 4. Review suspicious activity
sqlite3 /data/investiq.db "SELECT * FROM trades WHERE created_at > datetime('now', '-1 hour');"

# 5. If unauthorized trades, close positions
curl -X DELETE -H "X-API-Key: $VALID_KEY" \
     -H "X-Live-Trading-Key: $LIVE_KEY" \
     "http://localhost:3000/api/broker/close/[SYMBOL]"
```

---

### Clear Cache

**Clear Redis cache** (if using Redis):
```bash
# Clear all cache
docker compose exec api-server redis-cli FLUSHDB

# Clear specific keys
docker compose exec api-server redis-cli DEL "etf_bars:SPY"

# Check cache hit rate
docker compose exec api-server redis-cli INFO stats | grep hit_rate
```

**Clear ETF bar cache** (in-memory):
```bash
# Only option is restart
docker compose restart api-server

# Cache auto-expires every 15 minutes
```

**Clear ML model cache**:
```bash
# Models are loaded at startup, not cached
# To reload models, restart service
docker compose restart signal-models
```

---

## Dependency Health Checks

### Database (SQLite)

**Check database is accessible**:
```bash
# Read-only check
sqlite3 /data/investiq.db "SELECT 1;"

# Write check
sqlite3 /data/investiq.db "INSERT INTO audit_log (timestamp, action, user_id, details) VALUES (datetime('now'), 'health_check', 'system', 'test');"

# Check integrity
sqlite3 /data/investiq.db "PRAGMA integrity_check;"

# Check file size (should be < 10GB)
ls -lh /data/investiq.db
```

**Check for locks**:
```bash
# Find processes with DB open
lsof | grep investiq.db

# Check for WAL files (indicates active writes)
ls -lh /data/investiq.db-wal
```

**Check table counts**:
```bash
sqlite3 /data/investiq.db <<EOF
SELECT 'trades', COUNT(*) FROM trades
UNION ALL SELECT 'positions', COUNT(*) FROM positions
UNION ALL SELECT 'backtest_results', COUNT(*) FROM backtest_results
UNION ALL SELECT 'trade_outcomes', COUNT(*) FROM trade_outcomes;
EOF
```

---

### Polygon.io

**Check API connectivity**:
```bash
# Test endpoint directly
curl -w "\nStatus: %{http_code}\nTime: %{time_total}s\n" \
     "https://api.polygon.io/v2/aggs/ticker/SPY/range/1/day/2025-01-01/2025-01-31?apiKey=$POLYGON_API_KEY"

# Check rate limit status (headers)
curl -I "https://api.polygon.io/v2/aggs/ticker/SPY/range/1/day/2025-01-01/2025-01-31?apiKey=$POLYGON_API_KEY"
# Look for: X-RateLimit-Remaining

# Via InvestIQ API
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health | jq '.dependencies.polygon'
```

**Check usage limits**:
```bash
# Check Polygon dashboard: https://polygon.io/dashboard/api-keys
# Look for: Requests today, Daily limit

# Check local rate limiter
docker compose logs api-server | grep "rate_limit" | tail -20
```

**Common error codes**:
- `401`: Invalid API key
- `403`: Insufficient permissions (need paid plan)
- `429`: Rate limit exceeded
- `500`: Polygon service issue

---

### Alpaca

**Check API connectivity**:
```bash
# Test account endpoint
curl -H "APCA-API-KEY-ID: $ALPACA_API_KEY" \
     -H "APCA-API-SECRET-KEY: $ALPACA_SECRET_KEY" \
     https://paper-api.alpaca.markets/v2/account

# Check if market is open
curl -H "APCA-API-KEY-ID: $ALPACA_API_KEY" \
     -H "APCA-API-SECRET-KEY: $ALPACA_SECRET_KEY" \
     https://paper-api.alpaca.markets/v2/clock

# Via InvestIQ API
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health | jq '.dependencies.alpaca'
```

**Verify paper vs live**:
```bash
# Check which environment is configured
grep ALPACA_BASE_URL .env

# Should be "https://paper-api.alpaca.markets" for paper
# Should be "https://api.alpaca.markets" for live (DANGER)

# Verify in logs
docker compose logs api-server | grep "Alpaca client initialized"
```

**Check account status**:
```bash
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/broker/account | jq
# Look for: buying_power, pattern_day_trader, account_blocked
```

---

### Redis (optional)

**Check connectivity**:
```bash
# Ping
docker compose exec api-server redis-cli ping
# Should return: PONG

# Check memory usage
docker compose exec api-server redis-cli INFO memory | grep "used_memory_human"

# Check connected clients
docker compose exec api-server redis-cli CLIENT LIST
```

**Check if Redis is configured**:
```bash
grep REDIS_URL .env
# If not set, Redis features are disabled (graceful degradation)
```

---

### ML Service (signal-models)

**Check service health**:
```bash
# Health endpoint
curl http://localhost:8004/health
# Expected: {"status": "healthy", "models_loaded": true, "model_names": [...]}

# Test prediction
curl -X POST http://localhost:8004/predict \
     -H "Content-Type: application/json" \
     -d '{
       "technical_signal": 1.0,
       "fundamental_signal": 0.5,
       "quantitative_signal": 0.8,
       "sentiment_signal": 0.6,
       "technical_confidence": 0.9,
       "fundamental_confidence": 0.7,
       "quantitative_confidence": 0.85,
       "sentiment_confidence": 0.6,
       "rsi": 55.0,
       "macd": 0.5,
       "volume_ratio": 1.2,
       "pe_ratio": 25.0,
       "pb_ratio": 3.0,
       "sharpe_ratio": 1.5,
       "sortino_ratio": 2.0,
       "var_95": -0.02,
       "sentiment_score": 0.3,
       "sentiment_velocity": 0.1,
       "news_count": 5,
       "spy_return": 0.01,
       "vix": 18.0,
       "market_regime": 1
     }'
# Expected: {"predicted_profitable": true/false, "probability": 0.0-1.0}

# Via InvestIQ API health check
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health | jq '.dependencies.ml_service'
```

**Check model files**:
```bash
docker compose exec signal-models ls -lh /app/signal_models/models/
# Should show: meta_model.pkl, confidence_calibrator_*.pkl, weight_optimizer_*.pkl
```

**Check training status**:
```bash
docker compose logs signal-models | grep "Training"
# Should show recent training completion
```

---

## Circuit Breaker Recovery

### Inspect Current State

```bash
# Check if circuit breaker is active
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/risk/circuit-breakers | jq

# Response includes:
# - trading_halted: true/false
# - halt_reason: string explanation
# - consecutive_losses: current count
# - daily_loss_percent: % loss today
# - drawdown_percent: % from peak equity
# - halted_at: timestamp
# - daily_loss_limit_percent: threshold (5%)
# - max_consecutive_losses: threshold (3)
# - account_drawdown_limit_percent: threshold (10%)
```

### Analyze Trade History

```bash
# Last 10 trades with outcomes
sqlite3 /data/investiq.db <<EOF
SELECT
  symbol,
  entry_date,
  exit_date,
  profit_loss,
  outcome,
  predicted_profitable,
  confidence
FROM trade_outcomes
ORDER BY timestamp DESC
LIMIT 10;
EOF

# Daily P/L summary
sqlite3 /data/investiq.db <<EOF
SELECT
  DATE(timestamp) as date,
  COUNT(*) as trades,
  SUM(CASE WHEN outcome='win' THEN 1 ELSE 0 END) as wins,
  SUM(profit_loss) as total_pl
FROM trade_outcomes
WHERE timestamp > datetime('now', '-7 days')
GROUP BY DATE(timestamp)
ORDER BY date DESC;
EOF

# Check for specific patterns
sqlite3 /data/investiq.db <<EOF
SELECT
  symbol,
  COUNT(*) as occurrences,
  SUM(profit_loss) as total_pl
FROM trade_outcomes
WHERE timestamp > datetime('now', '-7 days')
GROUP BY symbol
HAVING total_pl < 0
ORDER BY total_pl ASC;
EOF
```

### Reset Circuit Breaker

**Only reset after investigation and fix!**

```bash
# Reset with reason (logged in audit)
curl -X POST -H "X-API-Key: $API_KEY" \
     http://localhost:3000/api/risk/trading-halt \
     -H "Content-Type: application/json" \
     -d '{
       "halt": false,
       "reason": "[DESCRIBE WHAT WAS FIXED]"
     }'

# Verify reset
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/risk/circuit-breakers | jq '.trading_halted'
# Should return: false

# Check audit log
sqlite3 /data/investiq.db "SELECT * FROM audit_log WHERE action='trading_halt_reset' ORDER BY timestamp DESC LIMIT 1;"
```

### Monitor Post-Reset

```bash
# Watch for re-trigger (poll every 30 seconds)
watch -n 30 'curl -s -H "X-API-Key: $API_KEY" http://localhost:3000/api/risk/circuit-breakers | jq ".trading_halted"'

# Monitor trading agent activity
docker compose logs -f trading-agent

# Check new trades
watch -n 60 'curl -s -H "X-API-Key: $API_KEY" http://localhost:3000/api/portfolio/trades?days=1 | jq "length"'
```

### Manual Trading Halt

**To halt trading preventively**:

```bash
# Set halt with reason
curl -X POST -H "X-API-Key: $API_KEY" \
     http://localhost:3000/api/risk/trading-halt \
     -H "Content-Type: application/json" \
     -d '{
       "halt": true,
       "reason": "Manual halt: investigating strategy anomaly"
     }'

# Stop trading agent to prevent new trades
docker compose stop trading-agent

# Close all open positions (if needed)
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/broker/positions | jq -r '.[].symbol' | while read symbol; do
  echo "Closing $symbol"
  curl -X DELETE -H "X-API-Key: $API_KEY" \
       -H "X-Live-Trading-Key: $LIVE_TRADING_KEY" \
       "http://localhost:3000/api/broker/close/$symbol"
done
```

---

## Rollback Procedures

### Rollback Docker Images

**Identify current version**:
```bash
docker compose images
# Note image tags/hashes
```

**Rollback to previous image**:
```bash
# 1. Check available tags
docker images | grep investiq

# 2. Update docker-compose.yml to previous tag
# Before: image: investiq-api:latest
# After:  image: investiq-api:v1.2.3

# 3. Deploy previous version
docker compose up -d --no-deps api-server

# 4. Verify health
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health
```

---

### Rollback Code (Git)

**Identify bad commit**:
```bash
git log --oneline -10
# Find last known good commit
```

**Rollback entire codebase**:
```bash
# 1. Create rollback branch
git checkout -b rollback-$(date +%Y%m%d)

# 2. Revert to good commit
git reset --hard [GOOD_COMMIT_HASH]

# 3. Rebuild and redeploy
docker compose build
docker compose up -d

# 4. Verify health
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health

# 5. Push rollback branch for record
git push origin rollback-$(date +%Y%m%d)
```

**Rollback specific file**:
```bash
# 1. Identify bad file/commit
git log --oneline -- path/to/file.rs

# 2. Checkout previous version
git checkout [GOOD_COMMIT] -- path/to/file.rs

# 3. Rebuild affected service
docker compose build api-server

# 4. Deploy
docker compose up -d api-server
```

---

### Rollback Configuration

**Rollback .env**:
```bash
# 1. Check recent changes
git diff HEAD~1 .env

# 2. Restore previous version
git checkout HEAD~1 .env

# 3. Restart with old config
docker compose down
docker compose up -d

# 4. Test
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health
```

**Rollback docker-compose.yml**:
```bash
# 1. Restore previous version
git checkout HEAD~1 docker-compose.yml

# 2. Redeploy
docker compose down
docker compose up -d
```

---

### Rollback Database Schema

**WARNING: Complex, avoid if possible!**

```bash
# 1. Stop services using database
docker compose down api-server trading-agent

# 2. Backup current state
sqlite3 /data/investiq.db ".backup /backups/before-rollback-$(date +%Y%m%d_%H%M%S).db"

# 3. Restore pre-migration backup
cp /backups/investiq.db.BEFORE_MIGRATION /data/investiq.db

# 4. Verify integrity
sqlite3 /data/investiq.db "PRAGMA integrity_check;"

# 5. Start services
docker compose up -d api-server trading-agent

# 6. Verify health
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health
```

---

### Emergency Full Rollback

**Nuclear option: restore everything from backup**

```bash
# 1. Stop all services
docker compose down

# 2. Restore database
cp /backups/investiq.db.LAST_GOOD /data/investiq.db

# 3. Restore code
git checkout [LAST_GOOD_TAG]

# 4. Restore config
git checkout [LAST_GOOD_TAG] .env docker-compose.yml

# 5. Rebuild
docker compose build

# 6. Deploy
docker compose up -d

# 7. Verify ALL dependencies
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health | jq

# 8. Manually test critical path
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/analyze/SPY
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/broker/account
```

---

## Contact and Escalation Matrix

### On-Call Rotation
- **Primary**: [NAME] - [PHONE] - [EMAIL]
- **Secondary**: [NAME] - [PHONE] - [EMAIL]
- **Manager**: [NAME] - [PHONE] - [EMAIL]

### Specialized Contacts

**Infrastructure Team**
- **Lead**: [NAME] - [PHONE] - [EMAIL]
- **Escalation**: For disk/network/host issues

**Database Administrator**
- **Lead**: [NAME] - [PHONE] - [EMAIL]
- **Escalation**: For corruption, performance, migration issues

**ML Engineer**
- **Lead**: [NAME] - [PHONE] - [EMAIL]
- **Escalation**: For model degradation, training issues

**Risk Manager**
- **Lead**: [NAME] - [PHONE] - [EMAIL]
- **Escalation**: For circuit breaker, unauthorized trades, compliance

**Trading Manager**
- **Lead**: [NAME] - [PHONE] - [EMAIL]
- **Escalation**: For market events, strategy failures, P/L issues

### Escalation Criteria

**Immediate Page** (wake anyone, any time):
- Complete service outage > 5 minutes
- Data corruption detected
- Unauthorized trading activity
- Security breach suspected
- Circuit breaker re-triggers within 1 hour

**Page During Business Hours**:
- Degraded performance > 30 minutes
- Single dependency down > 15 minutes
- ML model performance degradation
- Repeated alerts (3+ in 1 hour)

**Email/Slack Notification**:
- Single alert resolved quickly
- Routine configuration changes needed
- Non-urgent questions

### External Vendor Contacts

**Polygon.io**
- **Status Page**: https://status.polygon.io/
- **Support**: support@polygon.io
- **Documentation**: https://polygon.io/docs/

**Alpaca**
- **Status Page**: https://status.alpaca.markets/
- **Support**: support@alpaca.markets
- **Documentation**: https://docs.alpaca.markets/

**Redis (if cloud-hosted)**
- **Provider**: [AWS ElastiCache/Redis Cloud/etc]
- **Support**: [LINK]

---

## Appendix: Quick Command Reference

```bash
# Service status
docker compose ps

# View logs
docker compose logs -f [service]

# Restart service
docker compose restart [service]

# Check health
curl -H "X-API-Key: $API_KEY" http://localhost:3000/health | jq

# Check circuit breakers
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/risk/circuit-breakers | jq

# Backup database
./scripts/backup-db.sh

# Check disk space
df -h

# Check memory
free -h

# Check database integrity
sqlite3 /data/investiq.db "PRAGMA integrity_check;"

# Kill stale processes
pkill -9 -f "python.*app.py"

# Clear Redis cache
docker compose exec api-server redis-cli FLUSHDB

# Retrain ML models
docker compose exec signal-models python signal_models/train.py

# Export trade history
curl -H "X-API-Key: $API_KEY" http://localhost:3000/api/portfolio/trades?days=7 > trades.json

# Manual trading halt
curl -X POST -H "X-API-Key: $API_KEY" \
     http://localhost:3000/api/risk/trading-halt \
     -H "Content-Type: application/json" \
     -d '{"halt": true, "reason": "Manual investigation"}'

# Reset circuit breaker
curl -X POST -H "X-API-Key: $API_KEY" \
     http://localhost:3000/api/risk/trading-halt \
     -H "Content-Type: application/json" \
     -d '{"halt": false, "reason": "Issue resolved"}'
```

---

**End of Runbook**
