# InvestIQ Monitoring Infrastructure

This directory contains the complete monitoring and alerting infrastructure for InvestIQ, built on Prometheus and Grafana.

## Architecture

The monitoring stack consists of:

- **Prometheus**: Metrics collection, storage, and alerting engine
- **Grafana**: Visualization dashboards and metric exploration
- **Alert Rules**: Pre-configured alerts for critical system conditions

## Quick Start

### Start Monitoring Stack

```bash
# Start monitoring services (Prometheus + Grafana)
docker compose --profile monitoring up -d

# Verify services are running
docker ps | grep -E "(prometheus|grafana)"
```

### Access Dashboards

- **Grafana**: http://localhost:3001
  - Default credentials: `admin` / `admin` (change on first login)
  - Pre-configured dashboard: "InvestIQ Overview"

- **Prometheus**: http://localhost:9090
  - Query interface for raw metrics
  - Alert status at http://localhost:9090/alerts

## Metrics Exposed

InvestIQ API server should expose the following metrics at `/metrics`:

### Request Metrics
- `investiq_requests_total` (counter): Total number of requests by endpoint
- `investiq_request_duration_milliseconds` (histogram): Request latency distribution
- `investiq_errors_total` (counter): Total number of errors by type
- `investiq_active_connections` (gauge): Current active connections

### Business Metrics
- `investiq_trades_total` (counter): Total trades executed
- `investiq_analyses_total` (counter): Total symbol analyses performed

### Health Metrics
- `investiq_dependency_health` (gauge): Health status per dependency (0=down, 1=up)
  - Labels: `name` (database, redis, polygon, alpaca, ml_service)
- `investiq_trading_halted` (gauge): Trading circuit breaker status (0=active, 1=halted)

## Alert Rules

Pre-configured alerts in `alert_rules.yml`:

### Warning Alerts

**HighErrorRate**
- Condition: Error rate > 0.1 errors/sec for 2+ minutes
- Action: Investigate error logs, check downstream dependencies

**HighLatency**
- Condition: p95 request latency > 1 second for 5+ minutes
- Action: Check database query performance, Polygon API rate limits

**MLServiceDown**
- Condition: Signal models service unreachable for 5+ minutes
- Action: Check ML service logs, verify model loading

### Critical Alerts

**ServiceDown**
- Condition: Service unreachable for 1+ minute
- Action: Immediate investigation, check container health

**CircuitBreakerTripped**
- Condition: Trading halted by risk management
- Action: Review circuit breaker reason, assess market conditions

## Grafana Dashboard

The "InvestIQ Overview" dashboard (`investiq-overview.json`) provides:

### Performance Panel (Top Row)
- **Request Rate**: Real-time requests per second
- **Error Rate**: Errors per second with threshold visualization

### Latency Panel (Second Row)
- **Latency Histogram**: p50, p95, p99 latencies over time
- **Active Connections**: Current WebSocket/HTTP connections

### Business Metrics (Third Row)
- **Trades (24h)**: Total trades executed in last 24 hours
- **Analyses (24h)**: Total symbol analyses in last 24 hours

### Health Status (Bottom Row)
Six status panels showing real-time health of:
1. Database (SQLite)
2. Redis cache
3. Polygon API
4. Alpaca broker
5. ML Signal Models service
6. Trading status (Active/Halted)

Color coding:
- Green = Healthy/Active
- Red = Down/Halted

## Configuration

### Prometheus Configuration

**Scrape Intervals**:
- API server: 10 seconds
- ML service: 30 seconds (less critical)

**Evaluation Interval**: 10 seconds for alert rules

### Custom Environment Variables

Set in your `.env` file:

```bash
# Grafana admin credentials (defaults to admin/admin)
GRAFANA_ADMIN_USER=admin
GRAFANA_ADMIN_PASSWORD=your_secure_password
```

### Data Retention

**Prometheus**: Defaults to 15 days of metric retention
- Modify with `--storage.tsdb.retention.time=30d` in docker-compose.yml

**Grafana**: Dashboard data stored in `grafana_data` volume
- Persists across container restarts

## Implementing Metrics in Rust API

To expose Prometheus metrics, add to your Rust API:

```toml
# Cargo.toml
[dependencies]
prometheus = "0.13"
axum-prometheus = "0.6"
```

```rust
// main.rs
use axum_prometheus::PrometheusMetricLayer;

let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

let app = Router::new()
    .route("/metrics", get(|| async move { metric_handle.render() }))
    .layer(prometheus_layer);
```

For custom business metrics:

```rust
use prometheus::{register_counter, register_gauge, Counter, Gauge};

lazy_static! {
    static ref TRADES_TOTAL: Counter =
        register_counter!("investiq_trades_total", "Total trades executed").unwrap();

    static ref TRADING_HALTED: Gauge =
        register_gauge!("investiq_trading_halted", "Trading circuit breaker status").unwrap();
}

// In your trade execution code
TRADES_TOTAL.inc();

// In circuit breaker code
TRADING_HALTED.set(if halted { 1.0 } else { 0.0 });
```

## Alerting Integration

### Alertmanager (Optional)

For production deployments, integrate Prometheus Alertmanager:

```yaml
# prometheus.yml
alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']
```

### Notification Channels

Configure in Alertmanager for:
- Email notifications
- Slack/Discord webhooks
- PagerDuty integration
- Webhook to custom endpoints

Example Alertmanager config:

```yaml
# alertmanager.yml
route:
  group_by: ['alertname']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h
  receiver: 'discord-webhook'

receivers:
  - name: 'discord-webhook'
    webhook_configs:
      - url: 'https://discord.com/api/webhooks/YOUR_WEBHOOK'
```

## Troubleshooting

### Prometheus Not Scraping Metrics

1. Check API server is exposing `/metrics`:
   ```bash
   curl http://localhost:3000/metrics
   ```

2. Check Prometheus targets:
   http://localhost:9090/targets

3. Verify network connectivity:
   ```bash
   docker exec investiq-prometheus wget -O- http://api-server:3000/metrics
   ```

### Grafana Dashboard Shows No Data

1. Verify Prometheus datasource:
   - Grafana > Configuration > Data Sources > Prometheus
   - Test connection should succeed

2. Check metric names in dashboard queries match exposed metrics

3. Verify time range in dashboard (default: last 1 hour)

### High Memory Usage

Prometheus memory usage scales with:
- Number of metrics × cardinality × retention period

Optimize by:
- Reducing scrape intervals for less critical services
- Lowering retention period
- Limiting metric label cardinality

### Alerts Not Firing

1. Check alert rule syntax:
   ```bash
   docker exec investiq-prometheus promtool check rules /etc/prometheus/alert_rules.yml
   ```

2. Verify alerts in Prometheus UI:
   http://localhost:9090/alerts

3. Check evaluation interval in prometheus.yml

## Production Deployment

### Security Hardening

1. **Change default Grafana password**:
   ```bash
   docker exec investiq-grafana grafana-cli admin reset-admin-password NEW_PASSWORD
   ```

2. **Enable Grafana authentication**:
   - Configure OAuth (Google, GitHub, etc.)
   - Set up LDAP integration
   - Enable anonymous access restrictions

3. **Secure Prometheus**:
   - Add basic auth with nginx reverse proxy
   - Restrict `/metrics` endpoint to monitoring network
   - Use network policies in Kubernetes

### Scaling Considerations

For high-traffic deployments:

1. **Prometheus Federation**: Multiple Prometheus instances with aggregation
2. **Thanos/Cortex**: Long-term storage and horizontal scaling
3. **Remote Write**: Send metrics to cloud providers (Grafana Cloud, Datadog)

### Backup Strategy

**Prometheus Data**:
```bash
# Backup Prometheus data directory
docker run --rm -v investiq_prometheus_data:/data -v $(pwd):/backup \
  alpine tar czf /backup/prometheus-backup.tar.gz /data
```

**Grafana Dashboards**:
```bash
# Backup Grafana data
docker run --rm -v investiq_grafana_data:/data -v $(pwd):/backup \
  alpine tar czf /backup/grafana-backup.tar.gz /data
```

## Custom Dashboards

Create additional dashboards in Grafana for:

- **Trading Performance**: Win rate, profit/loss, Sharpe ratio trends
- **ML Model Metrics**: Prediction accuracy, confidence distribution
- **Market Data**: Polygon API latency, rate limit utilization
- **Risk Metrics**: Circuit breaker triggers, position exposure

Export dashboards as JSON and add to `monitoring/grafana/dashboards/` for version control.

## Integration with Existing Logging

Correlate metrics with logs:

1. **Loki for Log Aggregation**:
   - Parse JSON logs from containers
   - Link Loki datasource in Grafana
   - Create unified dashboard with logs + metrics

2. **Trace Context**:
   - Add trace IDs to log lines
   - Use Jaeger/Tempo for distributed tracing
   - Link traces to metric anomalies

## Monitoring Best Practices

1. **Four Golden Signals**:
   - Latency (covered: request_duration_milliseconds)
   - Traffic (covered: requests_total)
   - Errors (covered: errors_total)
   - Saturation (add: CPU/memory usage, connection pool saturation)

2. **SLO/SLA Tracking**:
   - Define SLOs (e.g., 99.9% uptime, p95 < 500ms)
   - Create recording rules for SLI metrics
   - Set up burn rate alerts

3. **Capacity Planning**:
   - Monitor growth trends
   - Set up predictive alerts for resource exhaustion
   - Track seasonal patterns (market hours vs. after-hours)

## Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
- [Grafana Dashboard Best Practices](https://grafana.com/docs/grafana/latest/best-practices/)

## Support

For issues with the monitoring infrastructure:

1. Check container logs:
   ```bash
   docker logs investiq-prometheus
   docker logs investiq-grafana
   ```

2. Verify configuration files:
   ```bash
   docker exec investiq-prometheus cat /etc/prometheus/prometheus.yml
   ```

3. Review Prometheus TSDB status:
   http://localhost:9090/tsdb-status
