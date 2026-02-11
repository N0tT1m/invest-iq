# Quick Start: Production Deployment

This guide walks you through deploying InvestIQ to production with TLS/HTTPS support.

## Prerequisites

- Linux server with Docker and Docker Compose installed
- Domain name pointed to your server IP (A record)
- Ports 80 and 443 open in firewall
- API keys from Polygon.io and Alpaca Markets

## Step 1: Clone Repository

```bash
git clone https://github.com/yourusername/invest-iq.git
cd invest-iq
```

## Step 2: Setup Secrets

Run the interactive secrets setup script:

```bash
./scripts/setup-secrets.sh
```

This will prompt you for:
- Polygon API key
- Alpaca API key and secret
- InvestIQ API keys (or auto-generate)
- Optional: Live trading key
- Optional: Finnhub API key

**Alternatively**, create secrets manually:

```bash
mkdir -p secrets
echo -n "your_polygon_api_key" > secrets/polygon_api_key.txt
echo -n "your_alpaca_api_key" > secrets/alpaca_api_key.txt
echo -n "your_alpaca_secret_key" > secrets/alpaca_secret_key.txt
echo -n "key1:admin,key2:trader" > secrets/api_keys.txt
chmod 600 secrets/*.txt
```

## Step 3: Configure Domain

Set your domain and email for Let's Encrypt:

```bash
export DOMAIN=api.yourdomain.com
export ACME_EMAIL=admin@yourdomain.com
```

Or add to `.env` file:

```bash
cat >> .env << EOF
DOMAIN=api.yourdomain.com
ACME_EMAIL=admin@yourdomain.com
EOF
```

## Step 4: Deploy

### Production Deployment (with HTTPS)

```bash
docker compose -f docker-compose.yml -f docker-compose.production.yml up -d
```

This will:
- Start Traefik reverse proxy
- Obtain Let's Encrypt certificate automatically
- Enable HTTPS with automatic HTTP redirect
- Enforce authentication
- Enable JSON structured logging

### Staging Deployment (testing)

```bash
docker compose -f docker-compose.yml -f docker-compose.staging.yml up -d
```

This uses the same setup but with resource limits suitable for testing.

## Step 5: Verify Deployment

### Check Service Health

```bash
curl https://api.yourdomain.com/health | jq
```

Expected output:
```json
{
  "status": "healthy",
  "service": "invest-iq-api",
  "checks": {
    "database": { "status": "ok", "latency_ms": 5 },
    "polygon": { "status": "ok", "latency_ms": 150 },
    "redis": { "status": "ok", "latency_ms": 2 },
    "alpaca": { "status": "ok", "latency_ms": 200 },
    "ml_service": { "status": "ok", "latency_ms": 50 }
  }
}
```

### Test Authentication

```bash
# Should return 401 Unauthorized
curl https://api.yourdomain.com/api/analyze/AAPL

# Should return analysis data
curl -H "X-API-Key: your_admin_key" https://api.yourdomain.com/api/analyze/AAPL | jq
```

### Check TLS Certificate

```bash
openssl s_client -connect api.yourdomain.com:443 -servername api.yourdomain.com < /dev/null 2>/dev/null | openssl x509 -noout -dates
```

Expected output:
```
notBefore=Feb 11 00:00:00 2026 GMT
notAfter=May 12 00:00:00 2026 GMT
```

### Verify Rate Limiting

```bash
curl -i https://api.yourdomain.com/health | grep -i ratelimit
```

Expected headers:
```
x-ratelimit-limit: 60
x-ratelimit-remaining: 59
x-ratelimit-reset: 1612972800
```

## Step 6: Monitor Logs

### View All Services

```bash
docker compose logs -f
```

### View Specific Service

```bash
docker compose logs -f api-server
docker compose logs -f traefik
```

### Check for Errors

```bash
docker compose logs api-server | grep -i error
docker compose logs api-server | grep -i "401\|403"
```

## Step 7: Access Frontend

The Dash frontend runs on the same domain, port 8050:

```bash
# Via SSH tunnel
ssh -L 8050:localhost:8050 user@your-server
# Then open: http://localhost:8050

# Or configure Traefik route for frontend (advanced)
```

## Troubleshooting

### Certificate Not Obtained

**Symptom:** Traefik logs show ACME challenge failures

**Fix:**
1. Verify DNS A record points to server IP:
   ```bash
   dig +short api.yourdomain.com
   ```
2. Ensure ports 80 and 443 are open:
   ```bash
   sudo ufw status
   sudo ufw allow 80/tcp
   sudo ufw allow 443/tcp
   ```
3. Check Traefik logs:
   ```bash
   docker compose logs traefik | grep -i acme
   ```

### Health Check Fails

**Symptom:** `/health` returns 503 Service Unavailable

**Fix:**
1. Check which dependency is down:
   ```bash
   curl https://api.yourdomain.com/health | jq '.checks'
   ```
2. Verify secrets are correct:
   ```bash
   docker compose exec api-server cat /run/secrets/polygon_api_key
   ```
3. Check service logs:
   ```bash
   docker compose logs api-server --tail 100
   ```

### Database Issues

**Symptom:** Database check shows "down" status

**Fix:**
1. Verify database volume exists:
   ```bash
   docker volume ls | grep db_data
   ```
2. Check database file:
   ```bash
   docker compose exec api-server ls -la /app/data/
   ```
3. Restart service:
   ```bash
   docker compose restart api-server
   ```

### Rate Limiting Too Strict

**Symptom:** Getting 429 Too Many Requests errors

**Fix:**
1. Increase rate limit in `.env`:
   ```bash
   RATE_LIMIT_PER_MINUTE=120
   ```
2. Restart api-server:
   ```bash
   docker compose restart api-server
   ```

## Configuration Options

### Environment Variables

All configuration is via environment variables (set in `.env` or export):

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DOMAIN` | Yes* | localhost | Your domain name |
| `ACME_EMAIL` | Yes* | - | Email for Let's Encrypt |
| `REQUIRE_AUTH` | No | false | Enforce API authentication |
| `RATE_LIMIT_PER_MINUTE` | No | 60 | API rate limit |
| `RUST_LOG` | No | info | Log level (debug/info/warn/error) |
| `RUST_LOG_FORMAT` | No | - | Set to "json" for structured logs |
| `LIVE_TRADING_KEY` | No | - | Key for live trading operations |

*Required for production deployment with TLS

### Resource Limits (docker-compose.staging.yml)

The staging/production configs include resource limits:

```yaml
api-server:
  deploy:
    resources:
      limits:
        cpus: "2"
        memory: 2G
```

Adjust based on your server capacity and load.

## Backup Strategy

### Database Backups

The `db-backup` service runs daily backups:

```bash
# Enable backup service
docker compose --profile backup up -d

# Manual backup
docker compose exec api-server cp /app/data/portfolio.db /app/data/portfolio.backup.$(date +%Y%m%d).db

# List backups
docker compose exec db-backup ls -lh /backups/
```

### Configuration Backups

Backup these files regularly:

```bash
tar -czf investiq-config-$(date +%Y%m%d).tar.gz \
  .env \
  secrets/ \
  docker-compose.yml \
  docker-compose.production.yml \
  monitoring/
```

Store encrypted backups off-site (S3, Backblaze, etc.)

## Scaling Considerations

### Single Server (Current Setup)
- 2-4 vCPU, 4-8GB RAM recommended
- Handles ~1000 req/min comfortably
- Cost: $20-40/month VPS

### Multi-Server (Future)
- Use Docker Swarm or Kubernetes
- Load balance across multiple API servers
- Separate database server (PostgreSQL recommended)
- Redis cluster for shared cache
- Cost: $100-200/month

## Security Checklist

Before going live:

- [ ] Secrets stored in `secrets/` directory (not `.env` in production)
- [ ] Secret files have 600 permissions
- [ ] `REQUIRE_AUTH=true` set
- [ ] Strong API keys generated (32+ bytes)
- [ ] Rate limiting enabled
- [ ] HTTPS configured and working
- [ ] Firewall configured (only 22, 80, 443 open)
- [ ] SSH key-based authentication (disable password auth)
- [ ] Regular backups scheduled
- [ ] Monitoring and alerting configured
- [ ] Secrets rotation schedule established (90 days)
- [ ] Incident response plan documented

## Monitoring

### Built-in Metrics

```bash
# Prometheus format
curl https://api.yourdomain.com/metrics

# JSON format
curl https://api.yourdomain.com/metrics/json | jq
```

### Key Metrics to Monitor

- `investiq_requests_total` - Total requests
- `investiq_errors_total` - Error count
- `investiq_active_connections` - Current connections
- `investiq_analysis_total` - Analysis requests
- `investiq_request_duration_milliseconds` - Latency histogram

### External Monitoring (Recommended)

- **UptimeRobot**: Free uptime monitoring
- **Prometheus + Grafana**: Enable with `--profile monitoring`
- **DataDog / New Relic**: APM for production

## Cost Breakdown (Monthly)

### Infrastructure
- **VPS (4 vCPU, 8GB RAM)**: $40-80
- **Domain**: $1 (amortized)
- **Backups (optional)**: $5-10
- **Monitoring (optional)**: $0-50

### APIs
- **Polygon Starter**: $29
- **Alpaca Paper**: Free
- **Finnhub**: Free tier

**Total**: ~$70-170/month for production setup

## Next Steps

1. **Enable Monitoring**:
   ```bash
   docker compose --profile monitoring up -d
   ```
   Access Grafana at `http://your-server-ip:3001`

2. **Setup Alerting**: Configure Prometheus alerts (see `monitoring/alert_rules.yml`)

3. **Enable Trading Agent**:
   ```bash
   docker compose --profile agent up -d
   ```

4. **Configure Discord Notifications**: Set `DISCORD_WEBHOOK_URL` in `.env`

5. **Setup Automated Backups**: Enable backup profile and configure cron

## Support

- **Documentation**: See `docs/` directory
- **Security**: See `docs/secrets-rotation.md`
- **API Docs**: See `README.md#API`
- **Issues**: Open GitHub issue

## Related Documentation

- [Infrastructure Hardening](./INFRASTRUCTURE_HARDENING.md) - Implementation details
- [Secrets Rotation Guide](./docs/secrets-rotation.md) - Rotate API keys and secrets
- [Deployment Guide](./docs/deployment.md) - Detailed deployment options
- [RBAC Guide](./RBAC.md) - Role-based access control

---

**Last Updated**: 2026-02-11
