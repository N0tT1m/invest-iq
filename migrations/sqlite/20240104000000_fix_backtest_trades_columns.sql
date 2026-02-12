-- Fix backtest_trades column names to match Rust backtest-engine INSERT statements.
-- Migration: signal_type -> signal, duration_days -> holding_period_days, add commission/slippage.

ALTER TABLE backtest_trades RENAME COLUMN signal_type TO signal;
ALTER TABLE backtest_trades RENAME COLUMN duration_days TO holding_period_days;
ALTER TABLE backtest_trades ADD COLUMN commission_cost REAL NOT NULL DEFAULT 0.0;
ALTER TABLE backtest_trades ADD COLUMN slippage_cost REAL NOT NULL DEFAULT 0.0;
