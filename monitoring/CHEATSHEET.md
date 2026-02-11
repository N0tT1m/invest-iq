# InvestIQ Monitoring Cheatsheet

Quick reference for common monitoring operations.

## Starting/Stopping Services

```bash
# Start monitoring stack
docker compose --profile monitoring up -d

# Stop monitoring stack
docker compose --profile monitoring down

# Restart services
docker compose restart prometheus grafana

# View logs
docker logs investiq-prometheus
docker logs investiq-grafana
docker logs -f investiq-prometheus  # Follow logs
```

## Accessing Dashboards

| Service | URL | Default Credentials |
|---------|-----|---------------------|
| Grafana | http://localhost:3001 | admin / admin |
| Prometheus | http://localhost:9090 | None |
| API Metrics | http://localhost:3000/metrics | None |

## Common PromQL Queries

```promql
# Request rate (per second)
rate(investiq_requests_total[5m])

# Error rate percentage
rate(investiq_errors_total[5m]) / rate(investiq_requests_total[5m]) * 100

# P95 latency
histogram_quantile(0.95, rate(investiq_request_duration_milliseconds_bucket[5m]))

# Total trades today
increase(investiq_trades_total[24h])

# Average active connections
avg_over_time(investiq_active_connections[5m])

# Service uptime percentage
avg_over_time(up{job="investiq-api"}[24h]) * 100

# Is trading halted?
investiq_trading_halted > 0

# Which dependencies are down?
investiq_dependency_health < 1
```

## Grafana Operations

```bash
# Reset admin password
docker exec investiq-grafana grafana-cli admin reset-admin-password NEW_PASSWORD

# List installed plugins
docker exec investiq-grafana grafana-cli plugins ls

# Install plugin
docker exec investiq-grafana grafana-cli plugins install grafana-piechart-panel
docker restart investiq-grafana

# Backup dashboards
docker cp investiq-grafana:/var/lib/grafana ./grafana-backup

# Export dashboard JSON
# In Grafana UI: Dashboard settings → JSON Model → Copy to clipboard
```

## Prometheus Operations

```bash
# Check Prometheus config is valid
docker exec investiq-prometheus promtool check config /etc/prometheus/prometheus.yml

# Check alert rules
docker exec investiq-prometheus promtool check rules /etc/prometheus/alert_rules.yml

# Reload configuration (without restart)
curl -X POST http://localhost:9090/-/reload

# Check targets status
curl http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job:.labels.job, health:.health}'

# Query metrics via API
curl 'http://localhost:9090/api/v1/query?query=up' | jq '.data.result'

# Check TSDB stats
curl http://localhost:9090/api/v1/status/tsdb | jq
```

## Debugging

```bash
# Test API metrics endpoint
curl http://localhost:3000/metrics | head -20

# Check if Prometheus can reach API
docker exec investiq-prometheus wget -O- http://api-server:3000/metrics

# Check Grafana datasource
docker exec investiq-grafana curl http://prometheus:9090/api/v1/query?query=up

# Verify network connectivity
docker network inspect investiq-network

# Check service health
curl http://localhost:3000/health | jq
curl http://localhost:9090/-/healthy
curl http://localhost:3001/api/health | jq
```

## Alert Operations

```bash
# View active alerts in Prometheus
curl http://localhost:9090/api/v1/alerts | jq '.data.alerts[] | select(.state=="firing")'

# Silence an alert (requires Alertmanager)
curl -X POST http://localhost:9093/api/v2/silences \
  -H "Content-Type: application/json" \
  -d '{
    "matchers": [{"name": "alertname", "value": "HighErrorRate", "isRegex": false}],
    "startsAt": "2024-01-01T00:00:00Z",
    "endsAt": "2024-01-01T01:00:00Z",
    "comment": "Maintenance window"
  }'

# View silence status
curl http://localhost:9093/api/v2/silences | jq
```

## Backup & Restore

```bash
# Backup Prometheus data
docker run --rm \
  -v investiq_prometheus_data:/data:ro \
  -v $(pwd):/backup \
  alpine tar czf /backup/prometheus-$(date +%Y%m%d).tar.gz /data

# Restore Prometheus data
docker run --rm \
  -v investiq_prometheus_data:/data \
  -v $(pwd):/backup \
  alpine tar xzf /backup/prometheus-20240101.tar.gz -C /

# Backup Grafana
docker run --rm \
  -v investiq_grafana_data:/data:ro \
  -v $(pwd):/backup \
  alpine tar czf /backup/grafana-$(date +%Y%m%d).tar.gz /data

# Export single dashboard
curl -u admin:password http://localhost:3001/api/dashboards/uid/investiq-overview | jq '.dashboard' > dashboard-backup.json
```

## Performance Tuning

```bash
# Check Prometheus memory usage
docker stats investiq-prometheus --no-stream

# Check database size
docker exec investiq-prometheus du -sh /prometheus

# Compact Prometheus data
docker exec investiq-prometheus promtool tsdb analyze /prometheus

# Clean up old data
docker exec investiq-prometheus rm -rf /prometheus/wal

# Reduce scrape interval (in prometheus.yml)
# scrape_interval: 30s  # Instead of 10s
```

## Metrics Instrumentation Examples

### Rust (Axum)

```rust
// Increment counter
use crate::metrics::TRADES_TOTAL;
TRADES_TOTAL.inc();

// Set gauge
use crate::metrics::ACTIVE_CONNECTIONS;
ACTIVE_CONNECTIONS.set(42.0);

// Observe histogram
use crate::metrics::REQUEST_DURATION;
let timer = REQUEST_DURATION.start_timer();
// ... do work ...
timer.observe_duration();

// Or manually
REQUEST_DURATION.observe(duration_ms);

// Gauge with labels
use crate::metrics::DEPENDENCY_HEALTH;
DEPENDENCY_HEALTH.with_label_values(&["database"]).set(1.0);
```

## Testing Scenarios

```bash
# Generate test traffic
for i in {1..100}; do
  curl -s http://localhost:3000/api/analyze/AAPL > /dev/null
done

# Simulate error condition
for i in {1..50}; do
  curl -s http://localhost:3000/api/analyze/INVALID_SYMBOL > /dev/null
done

# Check metrics updated
curl http://localhost:3000/metrics | grep investiq_analyses_total
curl http://localhost:3000/metrics | grep investiq_errors_total

# Trigger ServiceDown alert (wait 1 min after)
docker stop investiq-api

# Check alert fired
curl http://localhost:9090/api/v1/alerts | jq '.data.alerts[] | select(.labels.alertname=="ServiceDown")'

# Restart service
docker start investiq-api
```

## Validation

```bash
# Run validation script
cd /Users/timmy/workspace/public-projects/invest-iq/monitoring
./validate.sh

# Check all services healthy
docker ps --filter "name=investiq" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# Verify metrics being collected
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job:.labels.job, up:.health}'

# Test Grafana datasource
curl -s -u admin:admin http://localhost:3001/api/datasources | jq '.[] | select(.type=="prometheus") | .name'
```

## Common Issues & Fixes

### "No data" in Grafana
```bash
# Check Prometheus is scraping
curl http://localhost:9090/api/v1/targets

# Verify metrics exist
curl http://localhost:3000/metrics | grep investiq

# Test Prometheus query
curl 'http://localhost:9090/api/v1/query?query=investiq_requests_total'

# Check Grafana datasource
# Grafana → Configuration → Data Sources → Prometheus → Test
```

### High memory usage
```bash
# Check metric cardinality
curl http://localhost:9090/api/v1/status/tsdb | jq '.data.seriesCountByMetricName'

# Reduce retention
# In docker-compose.yml: --storage.tsdb.retention.time=7d

# Restart with new config
docker compose --profile monitoring restart prometheus
```

### Prometheus not starting
```bash
# Check config syntax
docker exec investiq-prometheus promtool check config /etc/prometheus/prometheus.yml

# Check logs
docker logs investiq-prometheus

# Verify volume permissions
ls -la $(docker volume inspect investiq_prometheus_data -f '{{.Mountpoint}}')
```

### Can't login to Grafana
```bash
# Reset admin password
docker exec investiq-grafana grafana-cli admin reset-admin-password admin

# Check logs
docker logs investiq-grafana

# Restart container
docker restart investiq-grafana
```

## Environment Variables

```bash
# Set in .env or docker-compose.yml

# Grafana
GRAFANA_ADMIN_USER=admin
GRAFANA_ADMIN_PASSWORD=your_password

# Prometheus retention
PROMETHEUS_RETENTION=30d

# Alert notification
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...
SLACK_WEBHOOK_URL=https://hooks.slack.com/services/...
```

## Useful URLs

| Purpose | URL |
|---------|-----|
| Grafana Dashboard | http://localhost:3001/d/investiq-overview |
| Prometheus Targets | http://localhost:9090/targets |
| Prometheus Alerts | http://localhost:9090/alerts |
| Prometheus Graph | http://localhost:9090/graph |
| API Metrics | http://localhost:3000/metrics |
| API Health | http://localhost:3000/health |
| Grafana API | http://localhost:3001/api/datasources |

## Keyboard Shortcuts (Grafana)

| Key | Action |
|-----|--------|
| `g h` | Go to Home |
| `g d` | Go to Dashboard |
| `g e` | Go to Explore |
| `s` | Open search |
| `f` | Open dashboard finder |
| `?` | Show keyboard shortcuts |
| `t z` | Zoom out time range |
| `t →` | Move time range forward |
| `t ←` | Move time range back |

## Production Checklist

```bash
# Security
[ ] Changed Grafana admin password
[ ] Added auth to /metrics endpoint
[ ] Configured reverse proxy with TLS
[ ] Restricted network access

# Reliability
[ ] Configured Alertmanager
[ ] Set up notification channels
[ ] Tested alert firing
[ ] Configured backups

# Performance
[ ] Set appropriate retention periods
[ ] Monitored memory usage
[ ] Limited metric cardinality
[ ] Configured scrape intervals

# Operations
[ ] Documented runbooks
[ ] Trained team on dashboards
[ ] Set up log rotation
[ ] Configured monitoring of monitors
```

## Quick Links

- [Full Documentation](./README.md)
- [Quick Start Guide](./QUICKSTART.md)
- [Integration Guide](./INTEGRATION.md)
- [Summary](./SUMMARY.md)
- [Prometheus Docs](https://prometheus.io/docs/)
- [Grafana Docs](https://grafana.com/docs/)
- [PromQL Basics](https://prometheus.io/docs/prometheus/latest/querying/basics/)

## Support

```bash
# Get help
docker logs investiq-prometheus
docker logs investiq-grafana
./validate.sh

# Check configuration
promtool check config prometheus.yml
promtool check rules alert_rules.yml

# Community
# Prometheus: https://prometheus.io/community/
# Grafana: https://community.grafana.com/
```
