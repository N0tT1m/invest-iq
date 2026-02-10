# Production-Grade Backtesting Roadmap

## Current State (2026-02-10)

Two separate engines exist:
- `validation::BacktestEngine` (crates/validation/src/backtesting.rs) - used by `GET /api/backtest/:symbol`
  - Has commission/slippage (0.1% / 0.05%)
  - Hardcoded 2% risk-free rate for Sharpe
  - Equity curve only at signal dates
  - Confidence threshold: 0.5 for buys
- `backtest_engine::BacktestEngine` (crates/backtest-engine/src/engine.rs) - used by `POST /api/backtest/run`
  - NO commission/slippage
  - Sharpe ratio has NO risk-free rate subtraction (bug)
  - Has configurable stop-loss/take-profit
  - Has DB persistence (backtest-engine/src/db.rs)

**CRITICAL BUG**: Both routes call `get_default_analysis()` with CURRENT market data for ALL historical bars. Signals are forward-looking but backdated to historical bar timestamps. This inflates performance drastically.

Frontend: `frontend/components/backtest_panel.py` in Backtest tab, uses `GET /api/backtest/:symbol?days=N`.

---

## Phase 1: Fix Broken Fundamentals (no dependencies)

### [ ] 1. Eliminate look-ahead bias (CRITICAL)
- Create `generate_historical_signals()` in validation crate
- Takes full bars vec, computes indicators from bars[0..i] at each bar i
- Technical only (RSI, MACD, SMA, Bollinger from historical slice)
- Fundamental/sentiment: exclude or hold constant (can't reconstruct)
- Replace `get_default_analysis()` loop in both `main.rs:backtest_symbol` and `backtest_routes.rs:run_backtest`
- Files: `crates/validation/src/backtesting.rs`, `crates/api-server/src/main.rs`, `crates/api-server/src/backtest_routes.rs`

### [ ] 2. Unify two engines into one
- Keep `backtest-engine` crate as the single engine
- Port commission/slippage from validation crate into it
- Port DB persistence (already in backtest-engine)
- Merge config: commission_rate, slippage_rate, stop_loss_pct, take_profit_pct, confidence_threshold, position_size_pct
- Update `GET /api/backtest/:symbol` to use unified engine
- Deprecate `validation::BacktestEngine` (keep validation crate for comparison/data providers)
- Files: `crates/backtest-engine/src/engine.rs`, `crates/backtest-engine/src/models.rs`, `crates/api-server/src/main.rs`

### [ ] 3. Equity curve at every bar
- Iterate all bars chronologically, mark-to-market open positions at each close
- Record EquityPoint for every bar (not just signal dates)
- Accurate max drawdown from continuous equity series
- Files: `crates/backtest-engine/src/engine.rs`

### [ ] 4. Additional risk metrics
- Sortino ratio (downside deviation only)
- Calmar ratio (annualized return / max drawdown)
- Max consecutive wins/losses
- Average holding period
- Exposure time (% of bars with open position)
- Recovery factor (total return / max drawdown)
- Update BacktestResult struct + frontend backtest_panel.py
- Files: `crates/backtest-engine/src/engine.rs`, `crates/backtest-engine/src/models.rs`, `frontend/components/backtest_panel.py`

---

## Phase 2: After Phase 1

### [ ] 5. Fix Sharpe ratio (needs #2)
- backtest-engine: add risk-free rate subtraction `(avg_return - rf_daily) / std_dev * sqrt(252)`
- Use dynamic rate from TLT via orchestrator (already fetched for quant engine)
- Add `risk_free_rate` field to BacktestConfig (optional, falls back to TLT-derived)
- Files: `crates/backtest-engine/src/engine.rs`

### [ ] 6. Commission/slippage in unified engine (needs #2)
- Configurable commission_rate (default 0.1%) and slippage_rate (default 0.05%)
- Apply on both entry and exit
- Track total_commission_paid, total_slippage_cost in result
- Files: `crates/backtest-engine/src/engine.rs`, `crates/backtest-engine/src/models.rs`

### [ ] 7. Walk-forward validation (needs #1)
- Split bars into N rolling windows: train_window + test_window
- Generate signals from train window only, test on out-of-sample window
- Report in-sample vs out-of-sample metrics
- Add `walk_forward: bool`, `train_window_days`, `test_window_days` to config
- Files: `crates/backtest-engine/src/engine.rs`, `crates/backtest-engine/src/models.rs`

### [ ] 8. Benchmark comparison (needs #3)
- Buy-and-hold same stock over same period
- SPY return over same period (fetch via orchestrator)
- Alpha = strategy return - benchmark return
- Information ratio
- Frontend: dashed benchmark line on equity curve, alpha in metrics
- Files: `crates/backtest-engine/src/engine.rs`, `crates/backtest-engine/src/models.rs`, `frontend/components/backtest_panel.py`

---

## Phase 3: Advanced

### [ ] 9. Multi-symbol portfolio backtesting (needs #2)
- Capital allocation across symbols (equal weight or configurable)
- Rebalancing at configurable intervals
- Portfolio-level metrics (portfolio Sharpe, portfolio max DD, diversification ratio)
- Per-symbol + aggregate performance tracking
- Files: `crates/backtest-engine/src/engine.rs`, new portfolio module

### [ ] 10. Monte Carlo simulation (needs #2, #4)
- Reshuffle trade sequence N times (default 1000)
- Confidence intervals on returns, drawdown, Sharpe
- Report: median return, 5th/95th percentile, probability of ruin, expected max DD at 95%
- Frontend: distribution chart
- Files: `crates/backtest-engine/src/engine.rs`, `frontend/components/backtest_panel.py`
