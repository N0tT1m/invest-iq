# InvestIQ

A high-performance stock analysis and trading platform built in Rust. Combines four analysis engines (technical, fundamental, quantitative, sentiment) with ML-powered signal models, paper/live trading via Alpaca, and a full-featured Dash dashboard.

## Architecture

```
invest-iq/
├── crates/                        # 28 Rust crates
│   ├── analysis-core/             # Shared types, traits, error handling
│   ├── polygon-client/            # Polygon.io API client (rate-limited, cached)
│   ├── technical-analysis/        # RSI, MACD, Bollinger, ADX, SMA, patterns
│   ├── fundamental-analysis/      # P/E, ROE, ROIC, FCF, PEG, DCF, debt ratios
│   ├── quant-analysis/            # Sharpe, Sortino, VaR, drawdown, beta, volatility
│   ├── sentiment-analysis/        # News sentiment with entity awareness
│   ├── analysis-orchestrator/     # Combines engines, dynamic ML weights, conflict penalty
│   ├── api-server/                # Axum REST API (port 3000)
│   ├── portfolio-manager/         # Positions, trades, alerts (SQLite)
│   ├── alpaca-broker/             # Alpaca paper/live trading client
│   ├── backtest-engine/           # Point-in-time backtesting with commission/slippage
│   ├── risk-manager/              # Position sizing, stop-losses, risk radar, circuit breakers
│   ├── trading-agent/             # Autonomous trading agent
│   ├── ml-client/                 # HTTP client for ML services
│   ├── data-loader/               # Bulk Polygon data loading for ML training
│   ├── validation/                # Analysis accuracy vs Alpha Vantage / Yahoo
│   ├── analytics/                 # Strategy performance tracking, signal quality
│   ├── confidence-calibrator/     # Platt scaling, isotonic regression
│   ├── kelly-position-sizer/      # Kelly Criterion optimal allocation
│   ├── multi-timeframe/           # Cross-timeframe trend alignment
│   ├── market-regime-detector/    # Bullish/bearish/sideways regime detection
│   ├── news-trading/              # News-driven signal generation
│   ├── alpha-decay/               # Strategy degradation monitoring
│   ├── smart-watchlist/           # AI-curated personalized opportunity feed
│   ├── flow-map/                  # Sector rotation and money flow tracking
│   ├── time-machine/              # Historical replay for learning
│   ├── tax-optimizer/             # Tax-loss harvesting, wash sale rules
│   └── discord-bot/               # Discord integration
├── frontend/
│   ├── app.py                     # Main Dash dashboard (port 8050)
│   └── components/                # 20 modular dashboard panels
├── ml-services/                   # 4 Python ML microservices
│   ├── sentiment/                 # FinBERT sentiment (port 8001)
│   ├── bayesian/                  # Bayesian strategy weights (port 8002)
│   ├── price_predictor/           # PatchTST price forecasting (port 8003)
│   └── signal_models/             # Meta-model, calibrator, weight optimizer (port 8004)
```

## Prerequisites

- **Rust 1.70+** - [rustup.rs](https://rustup.rs/)
- **Python 3.10+** - For dashboard and ML services
- **Polygon.io API key** - Starter plan ($29/mo) or free tier (5 req/min)
- **Alpaca API key** (optional) - For paper/live trading
- **Discord bot token** (optional) - For Discord integration

### Polygon.io Plan Notes

| Plan | Rate Limit | Data Delay | History | Price |
|------|-----------|------------|---------|-------|
| Free | 5 req/min | 15-min | 2 years | $0 |
| Starter | ~100 req/sec* | 15-min | 5 years | $29/mo |
| Developer | ~100 req/sec* | Real-time | 5+ years | $79/mo |

*Paid plans are marketed as "unlimited" but Polygon recommends staying under 100 req/sec to avoid throttling.

The `POLYGON_RATE_LIMIT` env var controls the client-side rate limiter (requests per minute). Set it to match your plan:
- Free: `POLYGON_RATE_LIMIT=5`
- Starter/Developer: `POLYGON_RATE_LIMIT=100` (default) — increase up to `6000` if needed
- Data loader defaults to `3000` req/min for bulk operations

## Quick Start

```bash
# 1. Clone and configure
git clone <repo-url> && cd invest-iq
cp .env.example .env
# Edit .env with your POLYGON_API_KEY

# 2. Build
cargo build --release

# 3. Start API server
cargo run --release --bin api-server
# API running at http://localhost:3000

# 4. Start dashboard (separate terminal)
cd frontend && pip install -r requirements.txt
python app.py
# Dashboard at http://localhost:8050
```

## Environment Variables

### Required
| Variable | Description |
|----------|-------------|
| `POLYGON_API_KEY` | Polygon.io API key |

### Optional
| Variable | Default | Description |
|----------|---------|-------------|
| `API_KEYS` | - | Comma-separated API keys for auth |
| `ALPACA_API_KEY` | - | Alpaca trading API key |
| `ALPACA_SECRET_KEY` | - | Alpaca secret key |
| `ALPACA_BASE_URL` | paper endpoint | Alpaca base URL (paper or live) |
| `LIVE_TRADING_KEY` | - | Extra auth key for broker write endpoints (if unset, writes blocked) |
| `LIVE_TRADING_APPROVED` | - | Set to `yes` to allow live (non-paper) trading |
| `DISCORD_BOT_TOKEN` | - | Discord bot token |
| `ALPHA_VANTAGE_API_KEY` | - | For validation features |
| `POLYGON_RATE_LIMIT` | `100` | Max Polygon requests per minute (free tier: set to 5) |
| `ML_SIGNAL_MODELS_URL` | `http://localhost:8004` | Signal models service URL |
| `REDIS_URL` | `redis://localhost:6379` | Redis cache (optional) |
| `API_PORT` | `3000` | API server port |
| `API_BASE_URL` | `http://localhost:3000` | Backend URL for the frontend |
| `API_TIMEOUT` | `30` | Frontend HTTP request timeout (seconds) |
| `RUST_LOG` | `info` | Log level (trace/debug/info/warn/error) |
| `RUST_LOG_FORMAT` | `text` | Set to `json` for structured JSON logging |
| `PRODUCTION` | - | Set to `true` to enforce API key requirement in frontend |

## API Endpoints

All endpoints except `/health` and `/metrics` require API key auth via `X-API-Key` header.
Broker write endpoints (execute, close, cancel) additionally require `X-Live-Trading-Key` header when using a live (non-paper) Alpaca account. Paper trading is exempt.

### Core Analysis
```
GET  /api/analyze/:symbol          Full 4-engine analysis
GET  /api/bars/:symbol             Historical bars (?timeframe=1d&days=90)
GET  /api/ticker/:symbol           Ticker details
GET  /api/suggest                  Ranked stock suggestions (?universe=tech&limit=10)
GET  /api/backtest/:symbol         Backtest signals (?days=365)
```

### Trading & Portfolio
```
GET  /api/broker/account           Alpaca account info
POST /api/broker/execute           Execute trade
GET  /api/broker/positions         Open positions
GET  /api/broker/orders            Active orders
GET  /api/portfolio                Portfolio summary
GET  /api/portfolio/positions      All tracked positions
POST /api/trades                   Log a trade
GET  /api/trades/performance       Trade performance metrics
```

### Risk & Analytics
```
GET  /api/risk/radar/:symbol       Risk radar (6-axis)
POST /api/risk/position-size       Calculate position size
POST /api/risk/check               Pre-trade risk check
GET  /api/risk/circuit-breakers    Circuit breaker status
POST /api/risk/trading-halt        Set/clear trading halt
GET  /api/analytics/overview       Performance overview
GET  /api/analytics/signals/quality Signal quality report
```

### Observability
```
GET  /health                       Dependency-aware health check (DB, Polygon, Redis, Alpaca, ML)
GET  /metrics                      Request counters, error rates, latency histogram
```

### Market Intelligence
```
GET  /api/sentiment/:symbol/velocity    Sentiment velocity
GET  /api/sentiment/:symbol/social      Social sentiment
GET  /api/earnings/:symbol              Earnings data
GET  /api/dividends/:symbol             Dividend data
GET  /api/options/:symbol               Options flow
GET  /api/short-interest/:symbol        Short interest
GET  /api/insiders/:symbol              Insider activity
GET  /api/correlation/:symbol           Benchmark correlations
GET  /api/macro/indicators              Macro overlay
GET  /api/flows/sectors                 Sector flows
GET  /api/flows/rotations               Sector rotation patterns
```

### Strategy Tools
```
GET  /api/strategies/health             All strategy health
GET  /api/strategies/:name/decay        Alpha decay analysis
GET  /api/watchlist/personalized        AI-curated watchlist
GET  /api/calibration/stats             Confidence calibration
POST /api/backtest/run                  Run backtest (POST)
```

### Agent Trades
```
GET  /api/agent-trades/pending            Pending agent-proposed trades
POST /api/agent-trades/:id/review         Approve or reject a trade
```

### Tax Optimization
```
GET  /api/tax/harvest-opportunities     Tax-loss harvesting candidates
GET  /api/tax/wash-sales                Wash sale monitoring
GET  /api/tax/year-end-summary          Year-end tax summary
```

### Time Machine
```
POST /api/time-machine/start            Start historical replay
POST /api/time-machine/session/:id/decide  Make a decision
GET  /api/time-machine/leaderboard      Leaderboard
```

## Dashboard

The Dash frontend (`frontend/app.py`) provides a single-page dashboard with 20 integrated panels:

- **Stock search** with timeframe and lookback controls
- **Overall signal** combining all 4 engines (ML-weighted)
- **Trading tabs** - Paper trade, portfolio positions, backtest runner, live trade, agent trades (5 tabs)
- **Price chart** with technical overlays + multi-timeframe mini-charts
- **RSI / MACD** indicator panels
- **Risk radar** (6-axis) and **confidence compass** (calibrated vs raw)
- **Sentiment velocity** gauge with historical tracking
- **Earnings, dividends, options flow, short interest, insider activity** panels
- **Correlation matrix** (SPY/QQQ/DIA/IWM benchmarks)
- **Social sentiment** and **macro overlay** (ETF-derived regime detection)
- **Alpha decay** - Strategy health monitoring
- **Flow map** - Sector rotation and money flow
- **Smart watchlist** - AI-ranked opportunity feed
- **Tax dashboard** - Tax-loss harvesting candidates

### Dashboard Components

| Component | Description |
|-----------|-------------|
| `risk_radar.py` | 6-axis risk visualization |
| `confidence_gauge.py` | Calibrated confidence compass |
| `sentiment_velocity.py` | Sentiment momentum tracking |
| `paper_trading.py` | Buy/sell interface with Alpaca |
| `portfolio_dashboard.py` | Positions, allocation, orders |
| `backtest_panel.py` | Equity curve, metrics, trade log |
| `earnings_panel.py` | Earnings dates and estimates |
| `dividend_panel.py` | Dividend yields and ex-dates |
| `options_flow.py` | Options analysis, IV, P/C ratio |
| `short_interest.py` | Short squeeze scoring |
| `insider_activity.py` | Insider transaction tracking |
| `correlation_matrix.py` | Rolling benchmark correlations |
| `social_sentiment.py` | News-powered sentiment breakdown |
| `macro_overlay.py` | Market regime, rates, volatility |
| `alpha_decay.py` | Strategy health monitoring |
| `flow_map.py` | Sector rotation heatmap |
| `smart_watchlist.py` | AI-ranked opportunity feed |
| `tax_dashboard.py` | Tax-loss harvesting UI |
| `live_trading.py` | Live trading with confirmation safeguards |
| `agent_trades.py` | Agent-proposed trade review (approve/reject) |

## ML Services

Four Python microservices providing ML capabilities. All degrade gracefully when unavailable.

### Service Overview

| Service | Port | Model | Purpose |
|---------|------|-------|---------|
| Sentiment | 8001 | FinBERT | News sentiment classification |
| Bayesian | 8002 | Beta-Bernoulli | Strategy weight optimization via Thompson sampling |
| Price Predictor | 8003 | PatchTST | 3-hour price direction forecasting |
| Signal Models | 8004 | XGBoost + Isotonic | Trade gating, confidence calibration, dynamic weights |

### Signal Models (Port 8004)

Three models trained on historical analysis data:

1. **Meta-Model** (XGBoost) - Predicts P(profitable) for trade gating. Replaces the LLM gatekeeper.
2. **Confidence Calibrator** (Isotonic regression, one per engine) - Maps raw confidence to calibrated probability.
3. **Weight Optimizer** (XGBoost, 4 outputs) - Learns optimal engine weights per market condition (replaces hardcoded 20/40/15/25).

23-feature input vector: 4 engine signals + 4 confidences + 12 key metrics + 3 market context.

Cold start returns safe defaults (0.5 probability, raw confidence passthrough, hardcoded weights).

### Managing ML Services

```bash
cd ml-services

# Start all
./start_all_services.sh

# Stop all
./stop_all_services.sh

# Retrain all models
./retrain_all.sh
```

## Data Loader

Rust binary for bulk-loading historical data from Polygon to train ML models. Fetches bars + financials, runs all analysis engines on sliding windows, computes forward returns, and writes labeled feature vectors to SQLite.

```bash
# Fetch all active US stocks from Polygon (dynamic discovery)
cargo run -p data-loader --release -- --fetch-tickers --limit 3000

# Use built-in 150 curated symbols
cargo run -p data-loader --release -- --all

# Specific symbols
cargo run -p data-loader --release -- --symbols AAPL MSFT NVDA TSLA

# Dry run (no DB writes)
cargo run -p data-loader --release -- --all --dry-run

# Custom DB path
cargo run -p data-loader --release -- --all --db ./portfolio.db
```

Produces ~40 labeled rows per symbol (5-day step over 365 days). At 3,000 symbols that's **~120,000 training samples**.

The data loader sets `POLYGON_RATE_LIMIT=500` by default. Override with the env var if needed.

## Training Pipeline

```bash
# 1. Load data (Rust, fast)
cargo run -p data-loader --release -- --fetch-tickers --limit 2000

# 2. Train models (Python)
cd ml-services
python signal_models/train.py --db-path ../portfolio.db --output-dir ./models/signal_models

# 3. Restart signal models service to pick up new models
./stop_all_services.sh && ./start_all_services.sh
```

## Development

```bash
# Run all tests
cargo test

# Run specific crate tests
cargo test -p api-server          # 16 tests: auth, integration, combine_pit_signals
cargo test -p risk-manager        # 10 tests: circuit breakers, drawdown, risk radar
cargo test -p technical-analysis

# Debug logging
RUST_LOG=debug cargo run --bin api-server

# Structured JSON logging (for production)
RUST_LOG_FORMAT=json RUST_LOG=info cargo run --bin api-server

# Check compilation
cargo check
```

## Docker

The API server and Dash dashboard run in a single container (the Rust binary manages the Python frontend automatically).

```bash
cp .env.example .env
# Edit .env with API keys

docker-compose up -d                    # API + Dashboard + Redis
docker-compose --profile discord up -d  # + Discord bot

# Health check (dependency-aware: DB, Polygon, Redis, Alpaca, ML)
curl http://localhost:3000/health

# API at :3000, Dashboard at :8050
```

### Production Safety

- If `ALPACA_BASE_URL` points to live trading (not `paper-api`), the server requires `LIVE_TRADING_APPROVED=yes` to start
- Broker write endpoints (execute, close, cancel) require `X-Live-Trading-Key` header; if `LIVE_TRADING_KEY` env is unset, all writes are blocked
- Circuit breakers auto-halt trading on consecutive losses, daily loss limits, or account drawdown thresholds
- Request body size limited to 1 MB

## Disclaimer

This software is for educational and informational purposes only. It is not financial advice. Always do your own research and consult with a qualified financial advisor before making investment decisions.

## License

MIT
