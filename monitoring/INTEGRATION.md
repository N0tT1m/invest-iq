# Monitoring Integration Guide

This guide shows how to integrate the Prometheus/Grafana monitoring stack with the existing InvestIQ production deployment.

## Prerequisites

Before integrating monitoring, ensure you have:

1. Working InvestIQ deployment (API server + frontend)
2. Docker and Docker Compose installed
3. Sufficient disk space for metrics storage (recommend 10GB+)
4. Port 9090 (Prometheus) and 3001 (Grafana) available

## Architecture Overview

```
┌─────────────────┐      Scrape       ┌──────────────┐
│   API Server    │◄──────────────────│  Prometheus  │
│  (port 3000)    │    /metrics       │  (port 9090) │
│                 │                   │              │
│  Expose metrics │                   │  Collect &   │
│  via /metrics   │                   │  Store data  │
└─────────────────┘                   └──────┬───────┘
                                             │
┌─────────────────┐                          │ Query
│  ML Service     │◄──────────────────┐      │
│  (port 8004)    │    /health        │      │
└─────────────────┘                   │      ▼
                                      │  ┌──────────┐
┌─────────────────┐                   │  │ Grafana  │
│   Frontend      │                   │  │(port 3001│
│  (port 8050)    │                   │  │          │
└─────────────────┘                   │  │Dashboards│
                                      └──┤& Alerts  │
                                         └──────────┘
```

## Step 1: Add Prometheus Metrics to Rust API

### 1.1 Add Dependencies

Add to `crates/api-server/Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...
prometheus = "0.13"
lazy_static = "1.4"
```

### 1.2 Create Metrics Module

Create `crates/api-server/src/metrics.rs`:

```rust
use prometheus::{
    register_counter, register_gauge, register_histogram_vec, Counter, Gauge, HistogramVec,
};
use lazy_static::lazy_static;

lazy_static! {
    // Request metrics
    pub static ref REQUESTS_TOTAL: Counter =
        register_counter!("investiq_requests_total", "Total HTTP requests").unwrap();

    pub static ref ERRORS_TOTAL: Counter =
        register_counter!("investiq_errors_total", "Total errors").unwrap();

    pub static ref REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "investiq_request_duration_milliseconds",
        "HTTP request duration in milliseconds",
        &["endpoint", "method", "status"],
        vec![10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0]
    )
    .unwrap();

    // Business metrics
    pub static ref TRADES_TOTAL: Counter =
        register_counter!("investiq_trades_total", "Total trades executed").unwrap();

    pub static ref ANALYSES_TOTAL: Counter =
        register_counter!("investiq_analyses_total", "Total analyses performed").unwrap();

    // System metrics
    pub static ref ACTIVE_CONNECTIONS: Gauge =
        register_gauge!("investiq_active_connections", "Active connections").unwrap();

    pub static ref TRADING_HALTED: Gauge =
        register_gauge!("investiq_trading_halted", "Trading circuit breaker status").unwrap();

    // Dependency health (0=down, 1=up)
    pub static ref DB_HEALTH: Gauge =
        register_gauge!("investiq_dependency_health", "Database health").unwrap();
}

// Helper functions for dependency health tracking
pub fn set_dependency_health(name: &str, healthy: bool) {
    use prometheus::core::GenericGauge;
    let gauge = match name {
        "database" => &DB_HEALTH,
        _ => return,
    };
    gauge.set(if healthy { 1.0 } else { 0.0 });
}
```

### 1.3 Add Metrics Endpoint

In `crates/api-server/src/main.rs`:

```rust
mod metrics;

use axum::{routing::get, Router};
use prometheus::{Encoder, TextEncoder};

async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();

    match encoder.encode(&metric_families, &mut buffer) {
        Ok(()) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, encoder.format_type())],
            buffer,
        ),
        Err(e) => {
            tracing::error!("Failed to encode metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain")],
                Vec::new(),
            )
        }
    }
}

// Add to router
let app = Router::new()
    .route("/metrics", get(metrics_handler))
    // ... existing routes
```

### 1.4 Instrument Request Middleware

Add request tracking middleware:

```rust
use tower_http::trace::TraceLayer;
use std::time::Instant;

async fn metrics_middleware(
    method: Method,
    uri: Uri,
    req: Request<Body>,
    next: Next<Body>,
) -> Response {
    let start = Instant::now();
    metrics::REQUESTS_TOTAL.inc();

    let response = next.run(req).await;

    let duration = start.elapsed().as_millis() as f64;
    let status = response.status().as_u16().to_string();

    metrics::REQUEST_DURATION
        .with_label_values(&[uri.path(), method.as_str(), &status])
        .observe(duration);

    response
}
```

### 1.5 Instrument Business Logic

Add metrics to key operations:

```rust
// In analysis routes
use crate::metrics;

async fn analyze_symbol(symbol: String) -> Result<Json<Analysis>> {
    let result = orchestrator.analyze(&symbol).await?;
    metrics::ANALYSES_TOTAL.inc();
    Ok(Json(result))
}

// In trade execution
async fn execute_trade(order: ExecuteTradeRequest) -> Result<Json<Trade>> {
    let result = alpaca.submit_order(order).await?;
    metrics::TRADES_TOTAL.inc();
    Ok(Json(result))
}

// In health check
async fn health_check() -> Json<HealthStatus> {
    let db_healthy = check_database().await;
    metrics::set_dependency_health("database", db_healthy);

    // Return health status
    Json(HealthStatus { /* ... */ })
}

// In circuit breaker
fn set_trading_halt(halted: bool, reason: Option<String>) {
    metrics::TRADING_HALTED.set(if halted { 1.0 } else { 0.0 });
}
```

## Step 2: Deploy Monitoring Stack

### 2.1 Start Monitoring Services

```bash
cd /Users/timmy/workspace/public-projects/invest-iq

# Start Prometheus and Grafana
docker compose --profile monitoring up -d

# Verify services started
docker ps | grep -E "(prometheus|grafana)"
```

### 2.2 Verify Metrics Collection

1. Check Prometheus targets: http://localhost:9090/targets
2. Both targets should show "UP":
   - `investiq-api` (api-server:3000)
   - `signal-models` (signal-models:8004)

3. Test metrics endpoint:
```bash
curl http://localhost:3000/metrics
```

Expected output should include:
```
# HELP investiq_requests_total Total HTTP requests
# TYPE investiq_requests_total counter
investiq_requests_total 123

# HELP investiq_analyses_total Total analyses performed
# TYPE investiq_analyses_total counter
investiq_analyses_total 45
```

### 2.3 Access Grafana Dashboard

1. Open http://localhost:3001
2. Login with default credentials (admin/admin)
3. Navigate to Dashboards → InvestIQ Overview
4. Verify panels are showing data

## Step 3: Configure Production Alerts

### 3.1 Set Up Alertmanager (Optional but Recommended)

Add to `docker-compose.yml`:

```yaml
  alertmanager:
    image: prom/alertmanager:latest
    container_name: investiq-alertmanager
    ports:
      - "9093:9093"
    volumes:
      - ./monitoring/alertmanager.yml:/etc/alertmanager/alertmanager.yml:ro
      - alertmanager_data:/alertmanager
    command:
      - '--config.file=/etc/alertmanager/alertmanager.yml'
      - '--storage.path=/alertmanager'
    networks:
      - investiq-network
    restart: unless-stopped
    profiles:
      - monitoring
```

### 3.2 Configure Notification Channels

Create `monitoring/alertmanager.yml`:

```yaml
global:
  resolve_timeout: 5m

route:
  group_by: ['alertname', 'severity']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h
  receiver: 'default'

  routes:
    # Critical alerts go to PagerDuty + Discord
    - match:
        severity: critical
      receiver: 'critical-alerts'
      continue: false

    # Warning alerts only to Discord
    - match:
        severity: warning
      receiver: 'warning-alerts'

receivers:
  - name: 'default'
    webhook_configs:
      - url: 'http://localhost:5001/webhook'

  - name: 'critical-alerts'
    discord_configs:
      - webhook_url: '${DISCORD_WEBHOOK_URL}'
        title: '🚨 CRITICAL: {{ .GroupLabels.alertname }}'
    pagerduty_configs:
      - service_key: '${PAGERDUTY_KEY}'

  - name: 'warning-alerts'
    discord_configs:
      - webhook_url: '${DISCORD_WEBHOOK_URL}'
        title: '⚠️ WARNING: {{ .GroupLabels.alertname }}'
```

### 3.3 Update Prometheus Configuration

Update `monitoring/prometheus.yml`:

```yaml
alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']
```

## Step 4: Secure Production Deployment

### 4.1 Change Default Passwords

```bash
# Change Grafana admin password
docker exec investiq-grafana grafana-cli admin reset-admin-password "YOUR_SECURE_PASSWORD"

# Or set via environment variable in docker-compose.yml
GRAFANA_ADMIN_PASSWORD=your_secure_password
```

### 4.2 Add Reverse Proxy (Recommended)

Add nginx reverse proxy configuration:

```nginx
# /etc/nginx/sites-available/investiq-monitoring
server {
    listen 443 ssl http2;
    server_name grafana.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;

    location / {
        proxy_pass http://localhost:3001;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 443 ssl http2;
    server_name prometheus.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;

    # Basic auth for Prometheus
    auth_basic "Prometheus";
    auth_basic_user_file /etc/nginx/.htpasswd;

    location / {
        proxy_pass http://localhost:9090;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### 4.3 Restrict Metrics Endpoint

Add authentication to `/metrics` endpoint:

```rust
// In api-server/src/auth.rs
pub async fn metrics_auth_middleware(
    headers: HeaderMap,
    req: Request<Body>,
    next: Next<Body>,
) -> Result<Response, StatusCode> {
    // Allow Prometheus scraper with Bearer token
    if let Some(auth) = headers.get(header::AUTHORIZATION) {
        if auth.to_str().ok() == Some(&format!("Bearer {}", env::var("METRICS_TOKEN").unwrap_or_default())) {
            return Ok(next.run(req).await);
        }
    }

    // Or allow from trusted networks only
    if is_internal_network(&peer_ip) {
        return Ok(next.run(req).await);
    }

    Err(StatusCode::UNAUTHORIZED)
}
```

## Step 5: Set Up Backup and Retention

### 5.1 Configure Prometheus Retention

Update `docker-compose.yml`:

```yaml
prometheus:
  command:
    - '--config.file=/etc/prometheus/prometheus.yml'
    - '--storage.tsdb.path=/prometheus'
    - '--storage.tsdb.retention.time=30d'  # Keep 30 days
    - '--storage.tsdb.retention.size=10GB' # Or 10GB max
```

### 5.2 Backup Prometheus Data

Add to your backup script:

```bash
#!/bin/bash
# Backup Prometheus data
docker run --rm \
  -v investiq_prometheus_data:/data:ro \
  -v $(pwd)/backups:/backup \
  alpine tar czf /backup/prometheus-$(date +%Y%m%d).tar.gz /data

# Backup Grafana dashboards and settings
docker run --rm \
  -v investiq_grafana_data:/data:ro \
  -v $(pwd)/backups:/backup \
  alpine tar czf /backup/grafana-$(date +%Y%m%d).tar.gz /data

# Clean up old backups (keep 30 days)
find ./backups -name "prometheus-*.tar.gz" -mtime +30 -delete
find ./backups -name "grafana-*.tar.gz" -mtime +30 -delete
```

## Step 6: Monitoring the Monitors

Set up meta-monitoring to ensure the monitoring stack itself is healthy:

### 6.1 Add Blackbox Exporter

Add to `docker-compose.yml`:

```yaml
  blackbox-exporter:
    image: prom/blackbox-exporter:latest
    container_name: investiq-blackbox
    volumes:
      - ./monitoring/blackbox.yml:/etc/blackbox/blackbox.yml:ro
    command:
      - '--config.file=/etc/blackbox/blackbox.yml'
    networks:
      - investiq-network
    profiles:
      - monitoring
```

Create `monitoring/blackbox.yml`:

```yaml
modules:
  http_2xx:
    prober: http
    timeout: 5s
    http:
      valid_http_versions: ["HTTP/1.1", "HTTP/2.0"]
      valid_status_codes: [200]
      method: GET
      preferred_ip_protocol: "ip4"
```

### 6.2 Monitor Monitoring Stack

Add to `monitoring/prometheus.yml`:

```yaml
scrape_configs:
  # Self-monitoring
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']

  - job_name: 'grafana'
    static_configs:
      - targets: ['grafana:3000']

  # Blackbox monitoring
  - job_name: 'blackbox'
    metrics_path: /probe
    params:
      module: [http_2xx]
    static_configs:
      - targets:
        - http://api-server:3000/health
        - http://grafana:3000/api/health
    relabel_configs:
      - source_labels: [__address__]
        target_label: __param_target
      - source_labels: [__param_target]
        target_label: instance
      - target_label: __address__
        replacement: blackbox-exporter:9115
```

## Step 7: Testing and Validation

### 7.1 Run Validation Script

```bash
cd /Users/timmy/workspace/public-projects/invest-iq/monitoring
./validate.sh
```

This checks:
- Configuration file syntax
- Docker services running
- Endpoints accessible
- Metrics being collected

### 7.2 Manual Testing

```bash
# Test metrics endpoint
curl http://localhost:3000/metrics | grep investiq

# Generate test traffic
for i in {1..10}; do
  curl http://localhost:3000/api/analyze/AAPL
  sleep 1
done

# Check metrics updated
curl http://localhost:3000/metrics | grep investiq_analyses_total
```

### 7.3 Test Alerts

Trigger a test alert:

```bash
# Stop API service to trigger ServiceDown alert
docker stop investiq-api

# Wait 1 minute, check Prometheus alerts
open http://localhost:9090/alerts

# Should see ServiceDown firing

# Restart service
docker start investiq-api
```

## Integration Checklist

Production deployment checklist:

- [ ] Prometheus metrics implemented in API server
- [ ] `/metrics` endpoint exposed and tested
- [ ] Monitoring services started with `--profile monitoring`
- [ ] Grafana dashboard accessible and showing data
- [ ] Default passwords changed
- [ ] Alert rules reviewed and customized
- [ ] Alertmanager configured with notification channels
- [ ] Reverse proxy configured with TLS
- [ ] Metrics endpoint secured (auth or network restrictions)
- [ ] Backup strategy implemented
- [ ] Retention policies configured
- [ ] Meta-monitoring set up
- [ ] Validation script passes
- [ ] Load testing performed
- [ ] Alerts tested and verified
- [ ] Team trained on dashboard usage

## Troubleshooting

### Prometheus Not Scraping

Check Prometheus logs:
```bash
docker logs investiq-prometheus
```

Verify network connectivity:
```bash
docker exec investiq-prometheus wget -O- http://api-server:3000/metrics
```

### No Data in Grafana

1. Check Prometheus datasource connection in Grafana
2. Verify queries in dashboard panels
3. Check time range (default: last 1 hour)
4. Ensure metrics are actually being exposed

### High Memory Usage

Reduce cardinality:
```rust
// Avoid high-cardinality labels
// BAD: label per user ID
metrics.with_label_values(&[&user_id])

// GOOD: label per user type
metrics.with_label_values(&["free"|"premium"])
```

Reduce retention:
```yaml
command:
  - '--storage.tsdb.retention.time=7d'
```

## Next Steps

1. **Custom Dashboards**: Create dashboards for specific teams
2. **SLO Tracking**: Define and monitor service level objectives
3. **Log Integration**: Add Loki for unified logs + metrics
4. **Distributed Tracing**: Add Jaeger for request tracing
5. **Cost Monitoring**: Track infrastructure costs in Grafana

## Support

For integration issues:

1. Review monitoring logs
2. Check validation script output
3. Consult troubleshooting section
4. Review Prometheus/Grafana documentation

## References

- [InvestIQ Deployment Guide](../docs/deployment.md)
- [Monitoring README](./README.md)
- [Quick Start Guide](./QUICKSTART.md)
- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
