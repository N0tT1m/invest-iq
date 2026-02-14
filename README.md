# InvestIQ

A high-performance stock analysis and trading platform built in Rust. Combines four analysis engines (technical, fundamental, quantitative, sentiment) with ML-powered signal models, an autonomous trading agent, paper/live trading via Alpaca, and a full-featured Dash dashboard.

## Architecture

```
invest-iq/
├── crates/                        # 29 Rust crates
│   ├── analysis-core/             # Shared types, traits, adaptive regime logic
│   ├── polygon-client/            # Polygon.io + Finnhub API client (rate-limited, cached)
│   ├── technical-analysis/        # 20+ indicators, patterns, volume profile, Ichimoku, Fibonacci
│   ├── fundamental-analysis/      # Valuation, Piotroski, Altman Z, DuPont, sector-relative
│   ├── quant-analysis/            # Risk metrics, GARCH, Hurst, CVaR, Kelly, factor models
│   ├── sentiment-analysis/        # FinBERT NLP, event classification, buzz detection
│   ├── analysis-orchestrator/     # 9-regime detection, ML weights, supplementary signals
│   ├── api-server/                # Axum REST API (port 3000), 30+ route modules
│   ├── portfolio-manager/         # Positions, trades, alerts (SQLite, rust_decimal)
│   ├── alpaca-broker/             # Alpaca paper/live trading client
│   ├── backtest-engine/           # Full-featured backtester (see Backtesting section)
│   ├── risk-manager/              # Position sizing, stop-losses, risk radar, circuit breakers
│   ├── trading-agent/             # Autonomous agent with ML gate, portfolio guard, ATR stops
│   ├── ml-client/                 # HTTP client for ML services (sentiment, signals)
│   ├── invest-iq-data/            # PyO3 Python extension for high-perf bulk data fetching
│   ├── data-loader/               # Bulk Polygon data loading for ML training
│   ├── validation/                # Analysis accuracy vs Alpha Vantage / Yahoo
│   ├── analytics/                 # Strategy performance tracking, signal quality
│   ├── confidence-calibrator/     # Platt scaling, isotonic regression
│   ├── kelly-position-sizer/      # Kelly Criterion optimal allocation
│   ├── multi-timeframe/           # Cross-timeframe trend alignment
│   ├── market-regime-detector/    # Bullish/bearish/sideways regime detection
│   ├── news-trading/              # News-driven signal generation (Polygon + Finnhub)
│   ├── alpha-decay/               # Strategy degradation monitoring
│   ├── smart-watchlist/           # AI-curated personalized opportunity feed
│   ├── flow-map/                  # Sector rotation and money flow tracking
│   ├── time-machine/              # Historical replay for learning
│   ├── tax-optimizer/             # Tax-loss harvesting, wash sale rules
│   └── discord-bot/               # Discord integration with rich embeds
├── frontend/
│   ├── app.py                     # Main Dash dashboard (port 8050)
│   └── components/                # 21 modular dashboard panels
├── ml-services/                   # 4 Python ML microservices
│   ├── sentiment/                 # FinBERT sentiment (port 8001)
│   ├── bayesian/                  # Bayesian strategy weights (port 8002)
│   ├── price_predictor/           # PatchTST price forecasting (port 8003)
│   └── signal_models/             # Meta-model, calibrator, weight optimizer (port 8004)
├── migrations/                    # SQLite migrations (sqlx)
└── scripts/                       # Backup, deployment utilities
```

## Analysis Engines

### Technical Analysis
RSI (regime-adaptive dynamic thresholds), MACD, Bollinger Bands (width + %B), ADX, SMA (20/50/200), pattern recognition (4 patterns), support/resistance levels, volume confirmation, Ichimoku cloud, Fibonacci retracement, VWAP signals, volume profile (20-bucket volume-at-price), accumulation/distribution (CLV-weighted), relative strength vs SPY, multi-timeframe confluence (weekly alignment).

### Fundamental Analysis
P/E, P/B, ROE, ROIC, FCF yield, PEG, DCF, EV/EBITDA, debt ratios, quality of earnings, sector-relative valuation (SIC-based, per-sector P/E/D/E/EV-EBITDA thresholds), Piotroski F-Score (7 criteria), Altman Z-Score, DuPont decomposition (margin x turnover x leverage), multi-quarter trends (revenue acceleration, margin expansion, consecutive growth), financing cash flow analysis (buyback intensity, capital raise detection).

### Quantitative Analysis
Sharpe, Sortino, VaR, CVaR (5% tail), max drawdown, beta, volatility, Hurst exponent (trending/mean-reverting classification), autocorrelation lag-1, GARCH(1,1) volatility forecast, Kelly criterion, momentum factor (12mo-1mo), low-volatility factor ratio, skewness (crash risk), excess kurtosis (fat tails), correlation regime shifts (rolling beta stability), seasonality (month-of-year effects).

### Sentiment Analysis
FinBERT NLP via ML service (falls back to word-list scoring), entity-aware sentiment extraction, news event classification (9 types: Earnings 2x, M&A 2.5x, FDA 2x, Lawsuit 1.5x, etc.), abnormal buzz detection (article volume spike vs baseline), conflict penalty between engines.

### Orchestrator
Combines all four engines with ML-optimized dynamic weights. Enhanced 9-regime detection (bull/bear/sideways x high_vol/low_vol/normal), regime-conditional weight defaults, conviction tiers (HIGH/MODERATE/LOW), time-horizon signal breakdown (short/medium/long-term). Supplementary signals computed during analysis:

- **Options-implied**: IV percentile, put/call ratio, IV skew, max pain convergence
- **Insider transactions**: Title-weighted scoring (CEO/CFO buys weighted higher), net buy/sell
- **Dividend health**: Cut/suspension detection, yield, special dividend detection
- **Intraday**: Gap analysis (gap up/down, fade/reverse detection)
- **Smart money composite**: Combined insider + options + volume decline score
- **Sector rotation**: 20-day relative performance vs SPY

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
- Starter/Developer: `POLYGON_RATE_LIMIT=3000` (default) — safe up to `6000`
- Data loader defaults to `5500` req/min for bulk operations

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

### Optional — API Server
| Variable | Default | Description |
|----------|---------|-------------|
| `API_KEYS` | - | Comma-separated API keys with roles (`key1:admin,key2:trader,key3:viewer`) |
| `REQUIRE_AUTH` | - | Set to `true` to enforce auth (exits if `API_KEYS` empty) |
| `APCA_API_KEY_ID` | - | Alpaca trading API key (also accepts `ALPACA_API_KEY`) |
| `APCA_API_SECRET_KEY` | - | Alpaca secret key (also accepts `ALPACA_SECRET_KEY`) |
| `ALPACA_BASE_URL` | paper endpoint | Alpaca base URL (paper or live) |
| `LIVE_TRADING_KEY` | - | Extra auth key for broker write endpoints (if unset, writes blocked) |
| `LIVE_TRADING_APPROVED` | - | Set to `yes` to allow live (non-paper) trading |
| `DISCORD_BOT_TOKEN` | - | Discord bot token |
| `ALPHA_VANTAGE_API_KEY` | - | For validation features |
| `FINNHUB_API_KEY` | - | Finnhub API key for supplemental news (free tier: 60 calls/min) |
| `POLYGON_RATE_LIMIT` | `3000` | Max Polygon requests per minute (free tier: set to 5) |
| `RATE_LIMIT_PER_MINUTE` | `120` | HTTP request rate limit per IP |
| `ALLOWED_ORIGINS` | `localhost:3000,...` | Comma-separated CORS origins |
| `ML_SIGNAL_MODELS_URL` | `http://localhost:8004` | Signal models service URL |
| `REDIS_URL` | - | Redis cache URL (falls back to in-memory) |
| `DATABASE_URL` | `sqlite:portfolio.db` | SQLite database path |
| `RUST_LOG` | `info` | Log level (trace/debug/info/warn/error) |
| `RUST_LOG_FORMAT` | `text` | Set to `json` for structured JSON logging |

### Optional — API Server Security
| Variable | Default | Description |
|----------|---------|-------------|
| `TLS_CERT_PATH` | - | Path to PEM certificate (enables native HTTPS) |
| `TLS_KEY_PATH` | - | Path to PEM private key (requires `TLS_CERT_PATH`) |
| `ENABLE_HSTS` | `false` | Add HSTS header (set `true` when TLS is terminated) |
| `REQUEST_TIMEOUT_SECS` | `30` | Hard request timeout |
| `AUTH_MAX_FAILURES` | `5` | Auth failures before IP lockout |
| `AUTH_FAILURE_WINDOW_SECS` | `300` | Window for counting auth failures |
| `AUTH_LOCKOUT_SECS` | `900` | IP lockout duration after brute-force |
| `ADMIN_IP_ALLOWLIST` | - | Comma-separated CIDRs for admin/risk endpoints (e.g. `10.0.0.0/8,::1/128`) |
| `RUST_ENV` | - | Set to `production` to sanitize error responses |
| `SHUTDOWN_TIMEOUT_SECS` | `30` | Graceful shutdown drain timeout |

### Optional — Frontend
| Variable | Default | Description |
|----------|---------|-------------|
| `API_BASE_URL` | `http://localhost:3000` | Backend URL for the frontend |
| `API_TIMEOUT` | `30` | Frontend HTTP request timeout (seconds) |
| `PRODUCTION` | - | Set to `true` to enforce API key requirement in frontend |

### Optional — Trading Agent
| Variable | Default | Description |
|----------|---------|-------------|
| `SCAN_INTERVAL` | `300` | Scan interval in seconds |
| `TRADING_ENABLED` | `true` | Enable/disable trading |
| `PAPER_TRADING` | `true` | Paper vs live mode |
| `MIN_CONFIDENCE` | `0.60` | Minimum confidence threshold |
| `MAX_RISK_PER_TRADE` | `2.0` | Max risk per trade (%) |
| `MAX_POSITION_SIZE` | `500.0` | Max position size ($) |
| `MAX_OPEN_POSITIONS` | `50` | Max concurrent open positions |
| `MAX_SECTOR_CONCENTRATION` | `0.30` | Max allocation to one sector |
| `MAX_GROSS_EXPOSURE` | `0.80` | Max gross portfolio exposure |
| `DAILY_LOSS_HALT_PERCENT` | `3.0` | Daily loss threshold to halt trading |
| `ATR_SL_MULTIPLIER` | `2.0` | ATR-based stop-loss multiplier |
| `ATR_TP_MULTIPLIER` | `3.0` | ATR-based take-profit multiplier |
| `ORDER_TIMEOUT_SECONDS` | `30` | Cancel unfilled orders after N seconds |
| `WATCHLIST` | built-in list | Comma-separated symbols to scan |
| `DISCORD_WEBHOOK_URL` | - | Discord webhook for agent notifications |

## API Endpoints

All endpoints except `/health` and `/metrics` require API key auth via `X-API-Key` header (skipped when `API_KEYS` not set).
Broker write endpoints additionally require `X-Live-Trading-Key` header for live accounts.

### Core Analysis
```
GET  /api/analyze/:symbol          Full 4-engine analysis with supplementary signals
GET  /api/bars/:symbol             Historical bars (?timeframe=1d&days=90)
GET  /api/ticker/:symbol           Ticker details
GET  /api/suggest                  Ranked stock suggestions (?universe=tech&limit=10)
GET  /api/validate/:symbol         Validate analysis vs Alpha Vantage
```

### Symbol Search
```
GET  /api/symbols/search           Search tickers (?q=apple&limit=20)
GET  /api/symbols/:symbol          Ticker detail lookup
```

### Trading & Portfolio
```
GET  /api/broker/account           Alpaca account info
POST /api/broker/execute           Execute trade (idempotency key supported)
DELETE /api/broker/close/:symbol   Close position
POST /api/broker/cancel/:order_id  Cancel order
GET  /api/broker/positions         Open positions
GET  /api/broker/orders            Active orders
GET  /api/portfolio                Portfolio summary
GET  /api/portfolio/positions      All tracked positions
POST /api/trades                   Log a trade
GET  /api/trades/performance       Trade performance metrics
```

### Backtesting
```
GET  /api/backtest/:symbol         Quick PIT backtest (?days=365)
POST /api/backtest/run             Full backtest with custom config
GET  /api/backtest/results         List all backtest results
GET  /api/backtest/results/:id     Get specific backtest result
DELETE /api/backtest/results/:id   Delete backtest result
GET  /api/backtest/results/:id/trades       Trades for a backtest
GET  /api/backtest/results/:id/monte-carlo  Monte Carlo simulation
GET  /api/backtest/strategy/:name  Get backtests by strategy
POST /api/backtest/walk-forward    Walk-forward validation
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

### Market Intelligence
```
GET  /api/sentiment/:symbol/velocity    Sentiment velocity
GET  /api/sentiment/:symbol/social      Social sentiment (news-powered)
GET  /api/earnings/:symbol              Earnings data
GET  /api/dividends/:symbol             Dividend data
GET  /api/options/:symbol               Options flow, IV, P/C ratio
GET  /api/short-interest/:symbol        Short interest scoring
GET  /api/insiders/:symbol              Insider activity
GET  /api/correlation/:symbol           Benchmark correlations (SPY/QQQ/DIA/IWM)
GET  /api/macro/indicators              Macro overlay (ETF-derived regime)
GET  /api/flows/sectors                 Sector flows (11 sector ETFs)
GET  /api/flows/rotations               Sector rotation patterns
```

### Strategy Tools
```
GET  /api/strategies/health             All strategy health
GET  /api/strategies/:name/decay        Alpha decay analysis
GET  /api/watchlist/personalized        AI-curated watchlist
GET  /api/calibration/stats             Confidence calibration
```

### Agent Trades
```
GET  /api/agent-trades/pending          Pending agent-proposed trades
POST /api/agent-trades/:id/review       Approve or reject a trade
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

### Observability
```
GET  /health                       Dependency-aware health check (DB, Polygon, Redis, Alpaca, ML)
GET  /metrics                      Prometheus-format metrics
GET  /metrics/json                 JSON metrics for dashboard
```

## Backtesting

The backtest engine supports point-in-time signal generation with next-bar execution (no look-ahead bias). Signals on day[i] execute at day[i+1]'s open price.

### Features

- **Directional slippage**: Buys fill at `open * (1 + slippage)`, sells at `open * (1 - slippage)`
- **Volume participation limit**: Caps position at configurable % of bar volume (default 5%)
- **Gap-through stops**: SL/TP triggers at open if gap exceeds stop level
- **Short selling**: Sell with no position opens short; inverted SL/TP
- **Trailing stops**: Ratchets up on new highs, overrides fixed SL when higher
- **Circuit breaker**: Halts new entries when drawdown exceeds threshold
- **Limit orders**: With expiry, triggered limits execute as market
- **Margin**: Configurable leverage multiplier
- **Fractional shares**: Optional sub-share position sizing
- **Cash sweep**: Daily interest accrual on idle cash
- **Regime detection**: Volatility-based position size adjustment every 20 bars
- **Tiered commissions**: Volume-based per-share rates with min/max bounds
- **Benchmark comparison**: SPY buy-and-hold equity curve, alpha, information ratio
- **Multi-symbol**: Equal weight or custom allocation with periodic rebalancing
- **Monte Carlo**: Block bootstrap (preserves streaks) + parameter uncertainty simulation
- **Walk-forward validation**: Rolling train/test folds, overfitting ratio, combined OOS curve
- **Walk-forward optimization**: Grid search over parameter space per fold

### Analytics

- **Extended metrics**: Treynor, Jensen's alpha, Omega, tail ratio, skewness, kurtosis, top-5 drawdowns, monthly returns, rolling Sharpe, max DD duration
- **Factor attribution**: CAPM regression (OLS) — beta, alpha, R-squared, tracking error
- **Bootstrap confidence intervals**: 1000 resamples, 95% CI on Sharpe/win rate/profit factor
- **CPCV**: Combinatorially purged cross-validation with embargo bars
- **Data quality checks**: OHLC consistency, zero volume, price spikes, date gaps, possible splits
- **Tear sheet**: Structured JSON summary of all analytics

## Autonomous Trading Agent

The trading agent (`crates/trading-agent/`) runs as a standalone binary that scans the market, generates signals, and executes trades through Alpaca.

### Architecture

1. **Market Scanner** — Scans watchlist + top movers on configurable interval
2. **Strategy Manager** — Momentum, mean-reversion, breakout, sentiment, high-risk strategies with configurable weights
3. **ML Gate** — Dual-gate system: XGBoost meta-model P(profitable) + calibrated confidence average. Regime-conditional thresholds (bear=0.6, bull_low_vol=0.45). Uses real SPY returns and VIX proxy for 9-regime encoding.
4. **Supplementary Signal Adjustments** — Smart money composite, insider buying, IV percentile penalty, put/call ratio, gap analysis
5. **Portfolio Guard** — Max open positions, sector concentration limits (GICS), gross exposure cap, daily loss halt
6. **Trade Executor** — ATR-based stops (regime-adaptive multipliers), order timeout with cancellation, partial fill handling
7. **Position Manager** — Stop-loss monitoring, trailing stops
8. **State Persistence** — Metrics, trade context, and last report date persisted to SQLite across restarts
9. **Daily Report** — Automated summary at market close (4:05 PM ET) via Discord webhook

### Running

```bash
# Paper trading (default)
cargo run --release --bin trading-agent

# With Discord notifications
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/... cargo run --release --bin trading-agent
```

## Dashboard

The Dash frontend (`frontend/app.py`) provides a single-page dashboard with 21 integrated panels:

- **Symbol search** with autocomplete, popular symbol badges, timeframe and lookback controls
- **Overall signal** combining all 4 engines (ML-weighted) with conviction tier and time-horizon breakdown
- **Trading tabs** — Paper trade, portfolio positions, backtest runner, live trade, agent trades (5 tabs)
- **Price chart** with technical overlays + multi-timeframe mini-charts
- **RSI / MACD** indicator panels
- **Risk radar** (6-axis) and **confidence compass** (calibrated vs raw)
- **Sentiment velocity** gauge with historical tracking
- **Earnings, dividends, options flow, short interest, insider activity** panels
- **Correlation matrix** (SPY/QQQ/DIA/IWM benchmarks, 30-day rolling)
- **Social sentiment** and **macro overlay** (ETF-derived regime detection)
- **Alpha decay** — Strategy health monitoring with auto-recording from backtests
- **Flow map** — Sector rotation heatmap (11 sector ETFs)
- **Smart watchlist** — Quick-signal opportunity cards (SMA/RSI from cached bars)
- **Tax dashboard** — Tax-loss harvesting candidates with Alpaca cost basis

### Dashboard Components

| Component | Description |
|-----------|-------------|
| `symbol_search.py` | Ticker search with autocomplete and popular badges |
| `risk_radar.py` | 6-axis risk visualization |
| `confidence_gauge.py` | Calibrated confidence compass |
| `sentiment_velocity.py` | Sentiment momentum tracking |
| `paper_trading.py` | Buy/sell interface with Alpaca |
| `portfolio_dashboard.py` | Positions, allocation donut, order history |
| `backtest_panel.py` | Equity curve, metric cards, trade log |
| `earnings_panel.py` | Earnings dates and estimates |
| `dividend_panel.py` | Dividend yields and ex-dates |
| `options_flow.py` | Options analysis, IV, P/C ratio |
| `short_interest.py` | Short squeeze scoring (6-component) |
| `insider_activity.py` | Insider transaction tracking |
| `correlation_matrix.py` | Rolling benchmark correlations |
| `social_sentiment.py` | News-powered sentiment breakdown |
| `macro_overlay.py` | Market regime, rates, volatility |
| `alpha_decay.py` | Strategy health monitoring |
| `flow_map.py` | Sector rotation heatmap |
| `smart_watchlist.py` | AI-ranked opportunity cards |
| `tax_dashboard.py` | Tax-loss harvesting UI |
| `live_trading.py` | Live trading with confirmation safeguards |
| `agent_trades.py` | Agent-proposed trade review (approve/reject) |

## ML Services

Four Python microservices providing ML capabilities. All degrade gracefully when unavailable. Each service includes shared production hardening middleware (`ml-services/shared/middleware.py`): request timeouts, body size limits, request-ID propagation, Prometheus `/metrics` endpoint, and error sanitization.

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

## Data Loading

### Rust Data Loader

Rust binary for bulk-loading historical data from Polygon to train ML models. Fetches bars + financials, runs all analysis engines on sliding windows, computes forward returns, and writes labeled feature vectors to SQLite. Supports graceful shutdown (SIGINT/SIGTERM), retry with exponential backoff for Polygon API calls, and progress logging every 10 symbols.

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

The data loader sets `POLYGON_RATE_LIMIT=5500` by default. Override with the env var if needed.

### Python Data Extension (invest-iq-data)

PyO3 native Python extension for high-performance concurrent data fetching. Runs 100 concurrent requests at 5500 req/min through a shared Tokio runtime.

```python
import invest_iq_data

# Fetch all active US stock tickers
tickers = invest_iq_data.fetch_active_tickers(api_key)

# Fetch bars for multiple symbols concurrently
bars = invest_iq_data.fetch_bars_multi(api_key, ["AAPL", "MSFT", "NVDA"], days=365)

# Fetch news for multiple symbols concurrently
news = invest_iq_data.fetch_news_multi(api_key, ["AAPL", "MSFT"], limit_per_symbol=50)

# Compute N-day forward returns
changes = invest_iq_data.fetch_price_changes(api_key, ["AAPL", "MSFT"], days=5)
```

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

## Database

SQLite with sqlx migrations (`migrations/` at workspace root). Financial fields use `rust_decimal` for precision, serialized as JSON numbers.

Key tables: `trades`, `positions`, `alerts`, `backtest_results`, `backtest_trades`, `strategy_health_snapshots`, `analysis_features`, `trade_outcomes`, `portfolio_peak`, `audit_log`, `trade_idempotency`, `agent_state`, `agent_trade_context`.

Trade execution uses database transactions (trade log + risk position + portfolio update atomic). Idempotency keys prevent duplicate order submission (24h expiry).

## Releases & Distribution

Pre-built binaries are published as GitHub Releases. The API server binary has the frontend embedded — no separate frontend setup required.

### Creating a Release

```bash
# 1. Tag the commit
git tag v1.0.1

# 2. Push the tag (triggers the release workflow)
git push origin v1.0.1

# 3. Attach ML models (run locally — models are gitignored)
./scripts/upload-models.sh v1.0.1
```

The release workflow automatically builds the API server for:
- **Windows** — `api-server-windows-x64.exe`
- **macOS (ARM)** — `api-server-macos-arm64`
- **macOS (Intel)** — `api-server-macos-x64`
- **Linux** — `api-server-linux-x64`

It also builds the Tauri desktop app (requires signing secrets).

### Release Assets

| Asset | Contents |
|-------|----------|
| `api-server-*` | Standalone API server with embedded frontend |
| `ml-models.zip` | Signal models + price predictor (~10 MB, uploaded manually) |
| Desktop installers | `.exe`/`.msi` (Windows), `.dmg` (macOS) — if signing secrets configured |

The FinBERT sentiment model (~836 MB) is not included in releases. It downloads automatically on first run via HuggingFace.

### Downloading a Release (Testers)

Install the [GitHub CLI](https://cli.github.com/), then:

```bash
# Install gh
# Windows: winget install GitHub.cli
# macOS:   brew install gh
# Login once
gh auth login

# Download the latest release
gh release download --repo N0tT1m/invest-iq --dir .

# Or download a specific version / platform
gh release download v1.0.1 --repo N0tT1m/invest-iq --pattern "api-server-windows*" --dir .
gh release download v1.0.1 --repo N0tT1m/invest-iq --pattern "ml-models*" --dir .
```

### Running a Release

```bash
# 1. Unzip ML models (if downloaded)
unzip ml-models.zip

# 2. Create .env file with at minimum:
#    POLYGON_API_KEY=your_key_here

# 3. Run the server
./api-server-linux-x64          # Linux
./api-server-macos-arm64        # macOS Apple Silicon
api-server-windows-x64.exe      # Windows

# Server starts at http://localhost:3000 (frontend + API)
```

### Distributing the .env File

The `.env` file contains secrets and should never be committed or included in releases. Options for sharing with testers:

- **1Password / Bitwarden Send** — Generate a time-limited, one-use link
- **age encryption** — `age -e -r <recipient-pubkey> .env > .env.age`, share `.env.age` in the release
- **Tailscale file send** — `tailscale file cp .env <tester-machine>:` (direct, encrypted, no cloud)

### Uploading ML Models

The `scripts/upload-models.sh` script packages signal models and price predictor into `ml-models.zip` and attaches it to a release:

```bash
./scripts/upload-models.sh v1.0.1
# Packages: ml-services/models/signal_models/ + ml-services/models/price_predictor/
# Excludes: FinBERT sentiment model (836 MB, auto-downloads at runtime)
# Uploads:  ml-models.zip (~10 MB) to the specified release
```

### Monitoring Releases

```bash
# Watch the build in progress
gh run watch

# List releases
gh release list --repo N0tT1m/invest-iq

# View a specific release
gh release view v1.0.1 --repo N0tT1m/invest-iq
```

## Development

```bash
# Run all tests
cargo test

# Run specific crate tests
cargo test -p api-server
cargo test -p risk-manager
cargo test -p backtest-engine     # 42 tests covering all features
cargo test -p technical-analysis

# Debug logging
RUST_LOG=debug cargo run --bin api-server

# Structured JSON logging (for production)
RUST_LOG_FORMAT=json RUST_LOG=info cargo run --bin api-server

# Check compilation
cargo check
```

## Docker

```bash
cp .env.example .env
# Edit .env with API keys

docker-compose up -d                    # API + Dashboard + Redis
docker-compose --profile discord up -d  # + Discord bot
docker-compose --profile agent up -d    # + Trading agent
docker-compose --profile backup up -d   # + DB backup sidecar

# Health check (dependency-aware: DB, Polygon, Redis, Alpaca, ML)
curl http://localhost:3000/health

# API at :3000, Dashboard at :8050, Signal Models at :8004
```

### Production Safety

**Authentication & Authorization**
- 3-tier RBAC: `viewer`, `trader`, `admin` roles via `API_KEYS=key1:admin,key2:trader`
- Auth enforced when `REQUIRE_AUTH=true` (exits if `API_KEYS` empty)
- Broker write endpoints require `X-Live-Trading-Key` header; if `LIVE_TRADING_KEY` env is unset, all writes are blocked
- If `ALPACA_BASE_URL` points to live trading (not `paper-api`), the server requires `LIVE_TRADING_APPROVED=yes` to start
- Brute-force protection: IP lockout after configurable failed auth attempts (default 5 in 5 min, 15 min lockout)

**Network Security**
- Native TLS via `axum-server` + rustls (set `TLS_CERT_PATH` + `TLS_KEY_PATH`)
- OWASP security headers on all responses (CSP, X-Frame-Options, X-Content-Type-Options, HSTS, Cache-Control: no-store, Referrer-Policy, Permissions-Policy)
- CORS with explicit origin allowlist and 1-hour preflight cache
- Rate limiting per IP via `RATE_LIMIT_PER_MINUTE` (default 120)
- Request body size limited to 1 MB
- Request timeouts (default 30s) prevent slow-loris attacks
- `X-Request-Id` propagation for distributed tracing

**Access Control**
- IP allowlist for admin and risk management endpoints (`ADMIN_IP_ALLOWLIST` CIDR ranges)
- Error messages sanitized in production (`RUST_ENV=production` hides internal details)

**Operational**
- Graceful shutdown with configurable drain timeout (SIGINT/SIGTERM across all services)
- Circuit breakers auto-halt trading on consecutive losses, daily loss limits, or account drawdown thresholds
- Hash-chained audit logging for tamper detection
- Structured JSON logging with `RUST_LOG_FORMAT=json`
- DB backup via `scripts/backup-db.sh` (SQLite backup + 7-day rotation)
- Parameterized SQL everywhere (no string interpolation)

## Discord Bot

Rich Discord integration with slash commands and formatted embeds. Includes a circuit breaker for API server calls (5 consecutive failures triggers 30s cooldown) and graceful shutdown via shard manager.

- `/analyze AAPL` — Full 4-engine analysis with conviction tier, time horizons, supplementary signals
- `/price AAPL` — Quick price snapshot with day change and volume
- `/portfolio` — Portfolio overview with equity, P&L, positions
- `/backtest AAPL` — Backtest summary (return, Sharpe, Sortino, max DD, win rate)
- `/compare AAPL` — Side-by-side engine comparison
- `/help` — Command reference

Signal-based color coding (green=buy, red=sell, gold=neutral) on all embeds.

## Disclaimer

This software is for educational and informational purposes only. It is not financial advice. Always do your own research and consult with a qualified financial advisor before making investment decisions.

## License

MIT
