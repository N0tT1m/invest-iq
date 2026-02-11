# InvestIQ Load Testing

Load tests using [Locust](https://locust.io/).

## Setup

```bash
pip install -r requirements.txt
```

## Usage

### Web UI (interactive)

```bash
locust -f locustfile.py --host=http://localhost:3000
```

Then open http://localhost:8089 in your browser.

### Headless (CI)

```bash
# Quick smoke test: 10 users, 2 users/sec spawn rate, 30 seconds
locust -f locustfile.py --headless -u 10 -r 2 -t 30s --host=http://localhost:3000

# Medium load: 50 users, 60 seconds
locust -f locustfile.py --headless -u 50 -r 5 -t 60s --host=http://localhost:3000

# Stress test: 200 users, 5 minutes
locust -f locustfile.py --headless -u 200 -r 10 -t 5m --host=http://localhost:3000
```

### Environment Variables

- `API_KEY` — API key for authenticated endpoints (default: `test_key`)

### User Types

| User | Weight | Description |
|------|--------|-------------|
| HealthCheckUser | 1 | Hits /health and /metrics only |
| AnalysisUser | 3 | Stock analysis, bars, backtests |
| TradingUser | 2 | Portfolio reads, risk endpoints |
| MixedUser | 5 | Simulates real dashboard navigation |

### Tags

Filter by endpoint type:

```bash
locust -f locustfile.py --tags analysis --headless -u 20 -r 5 -t 30s --host=http://localhost:3000
```

Available tags: `health`, `metrics`, `analysis`, `bars`, `ticker`, `backtest`, `broker`, `portfolio`, `risk`
