# Secrets Rotation Guide

This document covers rotation procedures for all secrets used in InvestIQ. All authentication changes are automatically logged in the `audit_log` database table.

## Table of Contents

1. [Secret Inventory](#secret-inventory)
2. [Key Generation](#key-generation)
3. [Rotation Procedures](#rotation-procedures)
4. [Zero-Downtime Rotation](#zero-downtime-rotation)
5. [Docker Compose Rotation](#docker-compose-rotation)
6. [Emergency Rotation](#emergency-rotation)
7. [Audit Trail](#audit-trail)

---

## Secret Inventory

| Secret | Type | Required | Rotation Frequency |
|--------|------|----------|-------------------|
| `POLYGON_API_KEY` | Third-party API | Yes | Annually or if compromised |
| `ALPACA_API_KEY` | Third-party API | Yes | Annually or if compromised |
| `ALPACA_SECRET_KEY` | Third-party API | Yes | Annually or if compromised |
| `ALPACA_BASE_URL` | Configuration | Yes | N/A (not a secret) |
| `API_KEYS` | Application Auth | Yes | Quarterly or if compromised |
| `LIVE_TRADING_KEY` | Application Auth | Conditional* | Quarterly or if compromised |
| `FINNHUB_API_KEY` | Third-party API | No | Annually or if compromised |
| `REDIS_URL` | Infrastructure | No | When Redis password changes |
| `DISCORD_WEBHOOK_URL` | Integration | No | If webhook regenerated |
| `DATABASE_URL` | Configuration | No | N/A (file path) |

*Required for live trading write operations

---

## Key Generation

### Generate Application Keys

Use `openssl` for cryptographically secure random keys:

```bash
# Generate 32-byte (256-bit) hex key
openssl rand -hex 32

# Generate 64-byte (512-bit) hex key (more secure)
openssl rand -hex 64

# Generate base64-encoded key
openssl rand -base64 32
```

**Recommendations:**
- Minimum 32 bytes (256 bits) for production keys
- Use hex encoding for easier handling in environment variables
- Never reuse keys across environments (dev/staging/prod)
- Store keys in a password manager (1Password, LastPass, etc.)

### API Key Format

For `API_KEYS` and `LIVE_TRADING_KEY`, use this format:

```bash
# Single key
API_KEYS="a1b2c3d4e5f6g7h8:admin"

# Multiple keys (comma-separated)
API_KEYS="key1:admin,key2:trader,key3:readonly"

# Live trading key (no role suffix)
LIVE_TRADING_KEY="x9y8z7w6v5u4t3s2"
```

---

## Rotation Procedures

### 1. Polygon API Key

**Where to generate:** [Polygon.io Dashboard](https://polygon.io/dashboard/api-keys)

**Steps:**
1. Log in to Polygon.io dashboard
2. Navigate to "API Keys" section
3. Generate new key (keep old key active)
4. Update environment variable:
   ```bash
   POLYGON_API_KEY="new_key_here"
   ```
5. Restart application
6. Verify data fetching works (check logs for Polygon requests)
7. Deactivate old key in Polygon dashboard after 24 hours

**Verification:**
```bash
# Test endpoint
curl "https://api.polygon.io/v2/aggs/ticker/AAPL/prev?apiKey=YOUR_NEW_KEY"
```

---

### 2. Alpaca API Credentials

**Where to generate:** [Alpaca Dashboard](https://app.alpaca.markets/paper/dashboard/overview)

**Steps:**
1. Log in to Alpaca dashboard
2. Navigate to "Your API Keys" (paper or live trading)
3. **Regenerate keys** (generates new key pair)
4. Copy new `API Key ID` and `Secret Key`
5. Update environment variables:
   ```bash
   ALPACA_API_KEY="new_api_key_id"
   ALPACA_SECRET_KEY="new_secret_key"
   ```
6. Restart application
7. Verify broker integration (check portfolio/positions endpoints)

**CRITICAL NOTES:**
- Alpaca keys come in pairs (API Key + Secret)
- Regenerating invalidates the old pair immediately
- For live trading, ensure `ALPACA_BASE_URL` is correct:
  - Paper: `https://paper-api.alpaca.markets`
  - Live: `https://api.alpaca.markets`

**Verification:**
```bash
# Test account endpoint
curl -X GET https://paper-api.alpaca.markets/v2/account \
  -H "APCA-API-KEY-ID: YOUR_NEW_API_KEY" \
  -H "APCA-API-SECRET-KEY: YOUR_NEW_SECRET_KEY"
```

---

### 3. Application API Keys

**Format:** `API_KEYS="key1:role1,key2:role2"`

**Supported roles:** `admin`, `trader`, `readonly`

**Steps:**
1. Generate new key:
   ```bash
   NEW_KEY=$(openssl rand -hex 32)
   echo "New key: $NEW_KEY"
   ```
2. Add new key to existing keys (comma-separated):
   ```bash
   API_KEYS="old_key:admin,new_key:admin"
   ```
3. Restart application
4. Test new key:
   ```bash
   curl http://localhost:3000/api/analyze/AAPL \
     -H "X-API-Key: new_key"
   ```
5. Update client applications to use new key
6. After all clients migrated, remove old key:
   ```bash
   API_KEYS="new_key:admin"
   ```
7. Restart application again

**See:** [Zero-Downtime Rotation](#zero-downtime-rotation) for detailed multi-key strategy

---

### 4. Live Trading Key

**When required:** Only if `LIVE_TRADING_KEY` env var is set (enables write operations)

**Steps:**
1. Generate new key:
   ```bash
   LIVE_TRADING_KEY=$(openssl rand -hex 32)
   ```
2. Update environment variable
3. Restart application
4. Update frontend to use new key (check `frontend/components/paper_trading.py`)
5. Test trade execution endpoint:
   ```bash
   curl -X POST http://localhost:3000/api/broker/execute \
     -H "X-API-Key: your_api_key" \
     -H "X-Live-Trading-Key: new_live_key" \
     -H "Content-Type: application/json" \
     -d '{"symbol":"AAPL","side":"buy","quantity":1,"order_type":"market"}'
   ```

**Safe Default:** If `LIVE_TRADING_KEY` is not set, all broker write endpoints return 403.

---

### 5. Finnhub API Key

**Where to generate:** [Finnhub Dashboard](https://finnhub.io/dashboard)

**Steps:**
1. Log in to Finnhub
2. Navigate to "API Keys" section
3. Generate new key
4. Update environment variable:
   ```bash
   FINNHUB_API_KEY="new_key"
   ```
5. Restart application
6. Verify news fetching (check social sentiment panel)
7. Deactivate old key after verification

**Note:** This is optional. If not set, app uses Polygon news as fallback.

---

### 6. Redis Connection String

**Format:** `redis://[user:password@]host:port[/database]`

**Steps:**
1. Connect to Redis and change password:
   ```bash
   redis-cli
   127.0.0.1:6379> CONFIG SET requirepass "new_password"
   127.0.0.1:6379> CONFIG REWRITE
   ```
2. Update environment variable:
   ```bash
   REDIS_URL="redis://:new_password@localhost:6379"
   ```
3. Restart application
4. Verify cache operations (check logs for Redis connections)

**Note:** Redis is optional. App works without it (degrades to in-memory caching).

---

### 7. Discord Webhook URL

**Where to generate:** Discord Server Settings → Integrations → Webhooks

**Steps:**
1. Open Discord server settings
2. Navigate to Integrations → Webhooks
3. Create new webhook or regenerate existing one
4. Copy new webhook URL
5. Update environment variable:
   ```bash
   DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/..."
   ```
6. Restart application
7. Test notification (trigger a trade or alert)

**Note:** Optional integration for notifications.

---

## Zero-Downtime Rotation

The `API_KEYS` format supports multiple keys simultaneously, enabling zero-downtime rotation:

### Step-by-Step

**Phase 1: Add New Key**
```bash
# Current state
API_KEYS="old_key_abc123:admin"

# Add new key
API_KEYS="old_key_abc123:admin,new_key_xyz789:admin"

# Deploy (both keys now work)
docker-compose restart backend
```

**Phase 2: Migrate Clients**
- Update frontend `.env`:
  ```bash
  API_KEY="new_key_xyz789"
  ```
- Update any external scripts/integrations
- Test with new key
- Monitor logs for any clients still using old key

**Phase 3: Remove Old Key**
```bash
# After all clients migrated (wait 24-48 hours)
API_KEYS="new_key_xyz789:admin"

# Deploy
docker-compose restart backend
```

### Monitoring During Rotation

Check audit logs to see which keys are still active:

```sql
-- Count requests by API key (last 24 hours)
SELECT
  metadata->>'$.api_key' as api_key,
  COUNT(*) as request_count,
  MAX(timestamp) as last_used
FROM audit_log
WHERE timestamp > datetime('now', '-1 day')
  AND metadata IS NOT NULL
GROUP BY metadata->>'$.api_key'
ORDER BY last_used DESC;
```

---

## Docker Compose Rotation

When running in Docker, update the `.env` file and restart services.

### Standard Rotation

1. **Update `.env` file:**
   ```bash
   vim .env
   # Update the secret values
   ```

2. **Restart services:**
   ```bash
   # Restart specific service
   docker-compose restart backend

   # Or restart all services
   docker-compose down && docker-compose up -d
   ```

3. **Verify services are healthy:**
   ```bash
   docker-compose ps
   docker-compose logs -f backend | grep -i "server started"
   ```

### Secrets in Docker Swarm/Kubernetes

If using orchestration, consider using secrets managers:

**Docker Swarm:**
```bash
# Create secret
echo "new_polygon_key" | docker secret create polygon_api_key -

# Update service
docker service update --secret-rm polygon_api_key \
  --secret-add polygon_api_key \
  invest-iq_backend
```

**Kubernetes:**
```yaml
# Create secret
kubectl create secret generic invest-iq-secrets \
  --from-literal=polygon-api-key="new_key" \
  --from-literal=alpaca-api-key="new_key" \
  --dry-run=client -o yaml | kubectl apply -f -

# Rollout restart
kubectl rollout restart deployment/invest-iq-backend
```

---

## Emergency Rotation

If a secret is compromised, follow this checklist:

### Immediate Actions (0-15 minutes)

- [ ] **Generate new keys immediately**
  ```bash
  # Generate new app keys
  NEW_API_KEY=$(openssl rand -hex 32)
  NEW_LIVE_KEY=$(openssl rand -hex 32)
  ```

- [ ] **Remove compromised key from API_KEYS**
  ```bash
  # Emergency single-key replacement
  API_KEYS="$NEW_API_KEY:admin"
  ```

- [ ] **Restart application**
  ```bash
  docker-compose restart backend
  # Or kill and restart process
  ```

- [ ] **Verify new keys work**
  ```bash
  curl http://localhost:3000/health \
    -H "X-API-Key: $NEW_API_KEY"
  ```

### Third-Party Keys (15-30 minutes)

- [ ] **Polygon:** Regenerate in dashboard immediately
- [ ] **Alpaca:** Regenerate key pair (choose paper or live carefully!)
- [ ] **Finnhub:** Regenerate in dashboard

### Documentation & Communication (30-60 minutes)

- [ ] **Update password manager** with new keys
- [ ] **Notify team members** (Slack, email)
- [ ] **Update client applications** (frontend, scripts)
- [ ] **Review audit logs** for suspicious activity:
  ```sql
  SELECT * FROM audit_log
  WHERE timestamp > datetime('now', '-1 day')
  ORDER BY timestamp DESC;
  ```

### Post-Incident (1-24 hours)

- [ ] **Review how compromise occurred**
- [ ] **Check git history** for accidentally committed secrets:
  ```bash
  git log -p | grep -i "api_key\|secret\|password"
  ```
- [ ] **Scan environment files**:
  ```bash
  # Check for secrets in tracked files
  grep -r "POLYGON_API_KEY\|ALPACA_SECRET" .env* 2>/dev/null
  ```
- [ ] **Update CI/CD secrets** (GitHub Actions, GitLab CI)
- [ ] **Document incident** in security log
- [ ] **Schedule follow-up review** (1 week later)

### If Database Compromised

If `portfolio.db` is exposed:

- [ ] **Backup database immediately**:
  ```bash
  cp portfolio.db portfolio.db.backup.$(date +%Y%m%d_%H%M%S)
  ```
- [ ] **Rotate ALL secrets** (assume attackers have everything)
- [ ] **Review positions/orders tables** for unauthorized trades
- [ ] **Check Alpaca account** directly for unexpected activity
- [ ] **Consider resetting database** if integrity questionable

---

## Audit Trail

All authentication and authorization changes are automatically logged in the `audit_log` table.

### Schema

```sql
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    action TEXT NOT NULL,           -- e.g., "api_access", "trade_executed"
    user_id TEXT,                   -- API key hash or user identifier
    resource TEXT,                  -- e.g., "/api/analyze/AAPL"
    status TEXT NOT NULL,           -- "success" or "failure"
    metadata TEXT,                  -- JSON with additional context
    ip_address TEXT
);
```

### Query Examples

**Recent API key usage:**
```sql
SELECT
  timestamp,
  resource,
  status,
  metadata->>'$.api_key' as api_key_hint
FROM audit_log
WHERE action = 'api_access'
ORDER BY timestamp DESC
LIMIT 100;
```

**Failed authentication attempts:**
```sql
SELECT
  timestamp,
  resource,
  ip_address,
  metadata
FROM audit_log
WHERE action = 'api_access'
  AND status = 'failure'
ORDER BY timestamp DESC;
```

**Suspicious activity (multiple failures from same IP):**
```sql
SELECT
  ip_address,
  COUNT(*) as failure_count,
  MIN(timestamp) as first_attempt,
  MAX(timestamp) as last_attempt
FROM audit_log
WHERE action = 'api_access'
  AND status = 'failure'
  AND timestamp > datetime('now', '-1 hour')
GROUP BY ip_address
HAVING failure_count > 5
ORDER BY failure_count DESC;
```

**All activity for specific API key:**
```sql
SELECT
  timestamp,
  action,
  resource,
  status
FROM audit_log
WHERE metadata LIKE '%your_key_hint%'
ORDER BY timestamp DESC;
```

### Log Retention

Consider implementing log rotation:

```sql
-- Archive old logs (older than 90 days)
CREATE TABLE audit_log_archive AS
SELECT * FROM audit_log
WHERE timestamp < datetime('now', '-90 days');

-- Delete archived logs
DELETE FROM audit_log
WHERE timestamp < datetime('now', '-90 days');

-- Vacuum to reclaim space
VACUUM;
```

---

## Automation

### Scheduled Rotation Script

Create a script for quarterly API key rotation:

```bash
#!/bin/bash
# rotate_api_keys.sh

set -euo pipefail

# Generate new key
NEW_KEY=$(openssl rand -hex 32)
echo "Generated new key: ${NEW_KEY:0:8}..." # Show first 8 chars

# Read current keys
CURRENT_KEYS=$(grep "API_KEYS=" .env | cut -d'=' -f2-)

# Append new key
echo "Adding new key to rotation..."
NEW_KEYS="${CURRENT_KEYS},${NEW_KEY}:admin"

# Update .env
sed -i.bak "s|API_KEYS=.*|API_KEYS=\"${NEW_KEYS}\"|" .env

# Restart service
echo "Restarting backend..."
docker-compose restart backend

# Wait for health check
echo "Waiting for service to be healthy..."
sleep 5

# Test new key
if curl -s -H "X-API-Key: ${NEW_KEY}" http://localhost:3000/health | grep -q "healthy"; then
    echo "✓ New key verified"
    echo "⚠ Remember to remove old key after client migration"
    echo "⚠ Old keys: ${CURRENT_KEYS}"
else
    echo "✗ New key verification failed - rolling back"
    mv .env.bak .env
    docker-compose restart backend
    exit 1
fi
```

### Usage

```bash
# Make executable
chmod +x rotate_api_keys.sh

# Run rotation
./rotate_api_keys.sh

# Set up cron for quarterly rotation (first day of quarter at 2am)
# crontab -e
# 0 2 1 1,4,7,10 * /path/to/rotate_api_keys.sh >> /var/log/key-rotation.log 2>&1
```

---

## Best Practices

1. **Never commit secrets to git**
   - Use `.env` files (add to `.gitignore`)
   - Use `.env.example` for documentation
   - Scan with tools like `git-secrets` or `truffleHog`

2. **Use environment-specific keys**
   - Different keys for dev/staging/prod
   - Never use production keys in development

3. **Principle of least privilege**
   - Use `readonly` role for monitoring tools
   - Use `trader` role for automated bots
   - Restrict `admin` role to human operators

4. **Monitor usage**
   - Review audit logs weekly
   - Set up alerts for failed auth attempts
   - Track key usage patterns

5. **Document everything**
   - Keep this document updated
   - Record rotation dates in team wiki
   - Maintain key inventory spreadsheet

6. **Test before deploying**
   - Verify new keys work before removing old ones
   - Use health check endpoint to validate
   - Monitor logs for errors after rotation

7. **Secure storage**
   - Use password manager (1Password, LastPass)
   - Encrypt `.env` files in backups
   - Never send keys via unencrypted channels (email, Slack)

---

## Troubleshooting

### "Unauthorized" after rotation

**Cause:** Old key still in use by client

**Fix:**
```bash
# Add old key back temporarily
API_KEYS="new_key:admin,old_key:admin"

# Identify which client is using old key
grep "old_key" /var/log/invest-iq/backend.log

# Update that client, then remove old key
```

### Service won't start after rotation

**Cause:** Invalid key format or missing required secret

**Fix:**
```bash
# Check environment variables
docker-compose config | grep -A5 "environment:"

# Validate .env syntax
cat .env | grep -v "^#" | grep "="

# Check startup logs
docker-compose logs backend | tail -50
```

### Third-party API returns 401

**Cause:** Key not updated in third-party dashboard or network issue

**Fix:**
```bash
# Test key directly
curl "https://api.polygon.io/v2/aggs/ticker/AAPL/prev?apiKey=YOUR_KEY"

# Check if old key still works (maybe rotation didn't complete)
curl "https://api.polygon.io/v2/aggs/ticker/AAPL/prev?apiKey=OLD_KEY"

# Regenerate key in third-party dashboard
```

---

## Related Documentation

- [Production Deployment Guide](./deployment.md) *(if exists)*
- [Security Best Practices](./security.md) *(if exists)*
- [API Authentication](../README.md#authentication)

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-02-10 | Initial documentation | System |

---

**Questions or issues?** Open an issue in the repository or contact the security team.