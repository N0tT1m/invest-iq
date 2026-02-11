# Monitoring Quick Start Guide

Get InvestIQ monitoring up and running in 5 minutes.

## Step 1: Start the Monitoring Stack

```bash
# From the project root
cd /Users/timmy/workspace/public-projects/invest-iq

# Start Prometheus and Grafana
docker compose --profile monitoring up -d

# Verify services are running
docker ps | grep -E "(prometheus|grafana)"
```

Expected output:
```
investiq-prometheus   Up 10 seconds   0.0.0.0:9090->9090/tcp
investiq-grafana      Up 10 seconds   0.0.0.0:3001->3000/tcp
```

## Step 2: Access Grafana Dashboard

1. Open browser to http://localhost:3001
2. Login with default credentials:
   - Username: `admin`
   - Password: `admin`
3. Change password when prompted
4. Navigate to **Dashboards** > **InvestIQ Overview**

## Step 3: Verify Metrics Collection

Check Prometheus is scraping metrics:

1. Open http://localhost:9090/targets
2. Verify both targets show "UP" status:
   - `investiq-api` (api-server:3000/metrics)
   - `signal-models` (signal-models:8004/health)

If targets show "DOWN":
```bash
# Check API server is running
docker ps | grep investiq-api

# Test metrics endpoint manually
curl http://localhost:3000/metrics
```

## Step 4: Implement Metrics in API Server

Add Prometheus instrumentation to your Rust API:

```toml
# crates/api-server/Cargo.toml
[dependencies]
prometheus = "0.13"
lazy_static = "1.4"
```

```rust
// crates/api-server/src/metrics.rs
use prometheus::{
    register_counter, register_gauge, register_histogram, Counter, Gauge, Histogram,
};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref REQUESTS_TOTAL: Counter =
        register_counter!("investiq_requests_total", "Total HTTP requests").unwrap();

    pub static ref ERRORS_TOTAL: Counter =
        register_counter!("investiq_errors_total", "Total errors").unwrap();

    pub static ref TRADES_TOTAL: Counter =
        register_counter!("investiq_trades_total", "Total trades executed").unwrap();

    pub static ref ANALYSES_TOTAL: Counter =
        register_counter!("investiq_analyses_total", "Total analyses performed").unwrap();

    pub static ref ACTIVE_CONNECTIONS: Gauge =
        register_gauge!("investiq_active_connections", "Active WebSocket connections").unwrap();

    pub static ref TRADING_HALTED: Gauge =
        register_gauge!("investiq_trading_halted", "Trading circuit breaker status").unwrap();

    pub static ref REQUEST_DURATION: Histogram = register_histogram!(
        "investiq_request_duration_milliseconds",
        "HTTP request duration in milliseconds"
    )
    .unwrap();

    pub static ref DEPENDENCY_HEALTH: Gauge = register_gauge!(
        "investiq_dependency_health",
        "Health status of dependencies (0=down, 1=up)",
        &["name"]
    )
    .unwrap();
}
```

```rust
// crates/api-server/src/main.rs
mod metrics;

use axum::{routing::get, Router};
use prometheus::{Encoder, TextEncoder};

// Add metrics endpoint
async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

// Add to your router
let app = Router::new()
    .route("/metrics", get(metrics_handler))
    // ... other routes
```

## Step 5: Instrument Your Code

Add metrics to key operations:

```rust
// In analysis routes
use crate::metrics::{ANALYSES_TOTAL, REQUEST_DURATION};

async fn analyze_symbol(symbol: String) -> Result<Analysis> {
    let timer = REQUEST_DURATION.start_timer();

    // Your analysis logic
    let result = perform_analysis(&symbol).await?;

    ANALYSES_TOTAL.inc();
    timer.observe_duration();

    Ok(result)
}

// In trade execution
use crate::metrics::TRADES_TOTAL;

async fn execute_trade(order: Order) -> Result<Trade> {
    let result = alpaca_client.submit_order(order).await?;
    TRADES_TOTAL.inc();
    Ok(result)
}

// In health check
use crate::metrics::DEPENDENCY_HEALTH;

async fn health_check() -> HealthStatus {
    let db_healthy = check_database().await;
    DEPENDENCY_HEALTH.with_label_values(&["database"]).set(if db_healthy { 1.0 } else { 0.0 });

    let redis_healthy = check_redis().await;
    DEPENDENCY_HEALTH.with_label_values(&["redis"]).set(if redis_healthy { 1.0 } else { 0.0 });

    // ... repeat for other dependencies
}

// In circuit breaker
use crate::metrics::TRADING_HALTED;

fn set_trading_halt(halted: bool) {
    TRADING_HALTED.set(if halted { 1.0 } else { 0.0 });
}
```

## Step 6: Test the Dashboard

Generate some traffic to see metrics:

```bash
# Run some analyses
curl http://localhost:3000/api/analyze/AAPL
curl http://localhost:3000/api/analyze/TSLA
curl http://localhost:3000/api/analyze/NVDA

# Check metrics are updating
curl http://localhost:3000/metrics | grep investiq_analyses_total
```

Return to Grafana dashboard - you should see:
- Request rate increasing
- Analysis count incrementing
- Latency measurements appearing

## Step 7: Test Alerts

Trigger a test alert:

```bash
# Simulate service down
docker stop investiq-api

# Wait 1 minute, then check Prometheus alerts
# Open http://localhost:9090/alerts
# Should see "ServiceDown" alert firing
```

Restart service:
```bash
docker start investiq-api
```

## Step 8: Configure Alert Notifications (Optional)

To receive alerts via Discord:

1. Create Discord webhook in your server
2. Add Alertmanager service to docker-compose.yml
3. Configure webhook in alertmanager.yml:

```yaml
receivers:
  - name: 'discord'
    webhook_configs:
      - url: 'YOUR_DISCORD_WEBHOOK_URL'
        send_resolved: true
```

## Common Issues

### Dashboard shows "No data"

**Cause**: Metrics not being exposed or Prometheus not scraping

**Fix**:
```bash
# Check API metrics endpoint
curl http://localhost:3000/metrics

# Check Prometheus targets
open http://localhost:9090/targets

# Restart monitoring stack
docker compose --profile monitoring restart
```

### High memory usage

**Cause**: Prometheus storing too much historical data

**Fix**: Reduce retention period in docker-compose.yml:
```yaml
prometheus:
  command:
    - '--storage.tsdb.retention.time=7d'  # Reduce from default 15d
```

### Grafana login fails

**Fix**: Reset admin password:
```bash
docker exec investiq-grafana grafana-cli admin reset-admin-password newpassword
```

## Next Steps

1. **Customize Dashboard**: Add panels for your specific metrics
2. **Set Up Alerting**: Configure Alertmanager for notifications
3. **Add Logging**: Integrate Loki for unified logs + metrics
4. **Create SLOs**: Define and track service level objectives
5. **Monitor ML Models**: Add metrics for prediction accuracy, confidence

## Production Checklist

Before deploying to production:

- [ ] Change Grafana admin password
- [ ] Configure external Alertmanager
- [ ] Set up notification channels (email, Slack, PagerDuty)
- [ ] Enable HTTPS for Grafana (reverse proxy)
- [ ] Configure backup for Prometheus data
- [ ] Set appropriate retention periods
- [ ] Add authentication to /metrics endpoint
- [ ] Configure rate limiting on Prometheus queries
- [ ] Set up monitoring for the monitoring stack itself

## Learn More

- Full documentation: `monitoring/README.md`
- Alert rules: `monitoring/alert_rules.yml`
- Prometheus config: `monitoring/prometheus.yml`
- Dashboard JSON: `monitoring/grafana/dashboards/investiq-overview.json`

## Getting Help

1. Check container logs:
   ```bash
   docker logs investiq-prometheus
   docker logs investiq-grafana
   ```

2. Verify configuration:
   ```bash
   # Check Prometheus config is valid
   docker exec investiq-prometheus promtool check config /etc/prometheus/prometheus.yml

   # Check alert rules
   docker exec investiq-prometheus promtool check rules /etc/prometheus/alert_rules.yml
   ```

3. Access Prometheus console for debugging:
   ```bash
   docker exec -it investiq-prometheus /bin/sh
   ```

## Clean Up

To stop and remove monitoring services:

```bash
# Stop services
docker compose --profile monitoring down

# Remove volumes (WARNING: deletes all metric data)
docker volume rm investiq_prometheus_data investiq_grafana_data
```
