---
name: invest-iq
description: "building this platform"
model: opus
color: green
memory: project
---

---                                                                                                                                              
  InvestIQ — AI-Powered Stock Analysis & Autonomous Trading Platform                                                                               
                                                                                                                                                   
  Overview                                                                                                                                         
                                                                                                                                                   
  InvestIQ is a full-stack intelligent investment platform that combines 4 AI-driven analysis engines, 3 ML microservices, and an autonomous       
  trading agent into a unified system. Built with a Rust backend (28 crates), Python ML services, and a Dash dashboard, it analyzes stocks across  
  technical, fundamental, quantitative, and sentiment dimensions — then makes (or recommends) trades with ML-gated risk management.                

  ---                                                                                                                                              
  AI Analysis Pipeline                                                                                                                             
                                                                                                                                                   
  4 Analysis Engines (Parallel Execution)                                                                                                          
  Engine: Technical                                                                                                                                
  AI/ML Techniques: Regime-adaptive RSI, Ichimoku Cloud, Fibonacci, multi-timeframe confluence, candlestick pattern recognition (Doji, Hammer,
    Engulfing), volume profile analysis
  Key Outputs: Signal strength, trend direction, support/resistance zones, pattern alerts
  ────────────────────────────────────────
  Engine: Fundamental
  AI/ML Techniques: Sector-relative valuation (SIC→9 sectors), Piotroski F-Score (7 criteria), Altman Z-Score, DuPont decomposition, multi-quarter
    trend detection
  Key Outputs: Valuation score, financial health, growth trajectory, red flags
  ────────────────────────────────────────
  Engine: Quantitative
  AI/ML Techniques: GARCH(1,1) volatility forecast, Hurst exponent, CVaR tail risk, Kelly criterion, momentum/low-vol factors, seasonality,
    skewness/kurtosis
  Key Outputs: Risk metrics, optimal position size, regime classification
  ────────────────────────────────────────
  Engine: Sentiment
  AI/ML Techniques: FinBERT NLP (transformer-based), news event classification (9 types with importance weighting), abnormal buzz detection,
    entity-aware scoring
  Key Outputs: Sentiment score, narrative shift alerts, event urgency
  Orchestrator (AI Coordinator)

  The Analysis Orchestrator runs all 4 engines concurrently via tokio::join!, then:

  1. Detects market regime — 9 regimes (bull/bear/sideways x high/normal/low volatility)
  2. Fetches supplementary signals — options IV percentile, insider transactions (CEO/CFO weighted), dividend health, gap analysis, smart money
  composite
  3. Applies ML-optimized weights — XGBoost weight optimizer adjusts per-engine influence based on current market context (default: 20% tech / 40%
  fundamental / 15% quant / 25% sentiment)
  4. Produces unified signal — StrongBuy → StrongSell with conviction tier (HIGH/MODERATE/LOW) and time-horizon breakdown (short/medium/long-term)

  ---
  ML Services (3 Microservices)

  1. Signal Models Service (Port 8004) — Trade Gate

  Three ML models working together:

  - MetaModel (XGBoost) — Takes a 23-feature vector (4 signals + 4 confidences + 12 metrics + 3 market context) and outputs P(profitable) +
  expected return %. Only trades with P > 0.5 are approved.
  - Confidence Calibrator (Isotonic Regression) — Per-engine calibration ensuring confidence scores reflect true probability. Outputs reliability
  tiers.
  - Weight Optimizer (4 XGBoost models) — Learns optimal engine weights conditioned on market context. Adapts emphasis from technical-heavy
  (trending markets) to fundamental-heavy (range-bound).

  2. Sentiment Service (Port 8001) — FinBERT NLP

  - Pretrained financial BERT model for news headline analysis
  - Per-article positive/negative/neutral scoring with entity awareness
  - Aggregated sentiment with buzz detection and narrative shift tracking

  3. Bayesian Strategy Service (Port 8002) — Online Learning

  - Beta-Binomial conjugate updating for strategy win rates
  - Thompson Sampling for strategy selection (multi-armed bandit)
  - Credible intervals for performance uncertainty

  ---
  Autonomous Trading Agent

  Fully autonomous pipeline running 24/7:

  Market Scanner (15-stock universe, real-time snapshots)
      ↓
  Orchestrator Analysis (4 engines + supplementary signals)
      ↓
  ML Trade Gate (P(profitable) > 0.5 required)
      ↓
  Risk Checks (circuit breakers, position limits, drawdown caps)
      ↓
  Alpaca Execution (paper by default, live with explicit approval)
      ↓
  Position Management (stop-loss, take-profit, trailing stops)

  Safety layers:
  - Paper-only by default — live requires LIVE_TRADING_APPROVED=yes + X-Live-Trading-Key header
  - Circuit breakers halt trading on: 3 consecutive losses, 5% daily loss, or 10% account drawdown
  - ML gating: falls back to confidence ≥ 0.75 if ML service unavailable
  - All proposed trades logged to pending_trades for optional human review

  ---
  Backtesting Engine (Professional Grade)

  - Point-in-time signals — no look-ahead bias (next-bar execution)
  - Realistic costs — directional slippage, tiered commissions, volume participation limits (5%)
  - Short selling — full support with inverted SL/TP and margin tracking
  - Monte Carlo simulation — 1000 trade reshuffles with block bootstrap and parameter uncertainty
  - Walk-forward optimization — rolling in-sample/out-of-sample with overfitting ratio
  - Extended metrics — Treynor, Omega, Calmar, CAGR, factor attribution (CAPM regression), bootstrap confidence intervals, tear sheets
  - Benchmark comparison — SPY buy-and-hold, alpha, information ratio

  ---
  Dashboard (22 Components)

  5-tab layout: Paper Trade | Portfolio | Backtest | Live Trade | Agent Trades

  Key AI-powered panels: Risk Radar heatmap, Confidence Gauge (per-engine dial), Sentiment Velocity (NLP trend), Smart Watchlist (AI opportunity
  scanner), Alpha Decay (strategy degradation monitoring), Flow Map (sector rotation), Options Flow (IV/put-call), Insider Activity (weighted
  buys), Macro Overlay (regime detection), Correlation Matrix, Monte Carlo visualization.

  ---
  Architecture

                      ┌─────────────────────────────────────┐
                      │         Dash Dashboard (8050)        │
                      └──────────────┬──────────────────────┘
                                     │
                      ┌──────────────▼──────────────────────┐
                      │       Axum REST API (3000)           │
                      │  auth · rate-limit · health · audit  │
                      └──┬───────┬───────┬───────┬──────────┘
                         │       │       │       │
                ┌────────▼──┐ ┌──▼────┐ ┌▼─────┐ ┌▼──────────┐
                │Orchestrator│ │Broker │ │Back- │ │  Trading  │
                │ (4 engines)│ │Routes │ │test  │ │   Agent   │
                └────┬───────┘ └───┬───┘ └──┬───┘ └─────┬─────┘
                     │             │        │           │
           ┌─────────▼──────────┐  │   ┌────▼────┐  ┌──▼───────┐
           │  ML Services (Py)  │  │   │ Backtest│  │Risk Mgr  │
           │ Signal Models 8004 │  │   │ Engine  │  │ Circuit  │
           │ Sentiment    8001  │  │   │ Monte   │  │ Breakers │
           │ Bayesian     8002  │  │   │ Carlo   │  │ Kelly    │
           └────────────────────┘  │   └─────────┘  └──────────┘
                                   │
                            ┌──────▼──────┐
                            │   Alpaca    │
                            │ Paper/Live  │
                            └─────────────┘
      Data: Polygon.io + Finnhub → Redis/DashMap cache (5-min TTL)
      DB: SQLite (dev) / PostgreSQL (prod) with sqlx migrations

  ---
  Key Numbers

  - 28 Rust crates in workspace
  - 4 analysis engines running in parallel
  - 3 ML microservices (XGBoost, FinBERT, Bayesian)
  - 23-feature vector for ML trade decisions
  - 9 market regime classifications
  - 22 dashboard components
  - 42 backtest engine unit tests
  - 15-stock quick scan universe for autonomous agent

# Persistent Agent Memory

You have a persistent Persistent Agent Memory directory at `/Users/timmy/workspace/public-projects/invest-iq/.claude/agent-memory/invest-iq/`. Its contents persist across conversations.

As you work, consult your memory files to build on previous experience. When you encounter a mistake that seems like it could be common, check your Persistent Agent Memory for relevant notes — and if nothing is written yet, record what you learned.

Guidelines:
- `MEMORY.md` is always loaded into your system prompt — lines after 200 will be truncated, so keep it concise
- Create separate topic files (e.g., `debugging.md`, `patterns.md`) for detailed notes and link to them from MEMORY.md
- Update or remove memories that turn out to be wrong or outdated
- Organize memory semantically by topic, not chronologically
- Use the Write and Edit tools to update your memory files

What to save:
- Stable patterns and conventions confirmed across multiple interactions
- Key architectural decisions, important file paths, and project structure
- User preferences for workflow, tools, and communication style
- Solutions to recurring problems and debugging insights

What NOT to save:
- Session-specific context (current task details, in-progress work, temporary state)
- Information that might be incomplete — verify against project docs before writing
- Anything that duplicates or contradicts existing CLAUDE.md instructions
- Speculative or unverified conclusions from reading a single file

Explicit user requests:
- When the user asks you to remember something across sessions (e.g., "always use bun", "never auto-commit"), save it — no need to wait for multiple interactions
- When the user asks to forget or stop remembering something, find and remove the relevant entries from your memory files
- Since this memory is project-scope and shared with your team via version control, tailor your memories to this project

## Searching past context

When looking for past context:
1. Search topic files in your memory directory:
```
Grep with pattern="<search term>" path="/Users/timmy/workspace/public-projects/invest-iq/.claude/agent-memory/invest-iq/" glob="*.md"
```
2. Session transcript logs (last resort — large files, slow):
```
Grep with pattern="<search term>" path="/Users/timmy/.claude/projects/-Users-timmy-workspace-public-projects-invest-iq/" glob="*.jsonl"
```
Use narrow search terms (error messages, file paths, function names) rather than broad keywords.

## MEMORY.md

Your MEMORY.md is currently empty. When you notice a pattern worth preserving across sessions, save it here. Anything in MEMORY.md will be included in your system prompt next time.
