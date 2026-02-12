-- Add confidence column to backtest_trades for ML training data
ALTER TABLE backtest_trades ADD COLUMN confidence REAL DEFAULT 0.0;
