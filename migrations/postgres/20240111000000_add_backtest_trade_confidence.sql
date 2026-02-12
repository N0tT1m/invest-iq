-- Add confidence column to backtest_trades for ML training data -- PostgreSQL version
ALTER TABLE backtest_trades ADD COLUMN confidence DOUBLE PRECISION DEFAULT 0.0;
