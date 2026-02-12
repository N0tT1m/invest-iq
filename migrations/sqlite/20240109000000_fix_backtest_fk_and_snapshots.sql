-- Fix backtest_trades: FK references backtest_results(id) but save_backtest() inserts into backtests table.
-- Also fix strategy_health_snapshots column names: rolling_win_rate -> win_rate, rolling_profit_factor -> profit_factor.

-- Step 1: Recreate backtest_trades with correct FK pointing to backtests(id)
-- SQLite doesn't support ALTER FOREIGN KEY, so we must drop and recreate.

CREATE TABLE IF NOT EXISTS backtest_trades_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    backtest_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    signal TEXT NOT NULL,
    entry_date TEXT NOT NULL,
    exit_date TEXT,
    entry_price REAL NOT NULL,
    exit_price REAL,
    shares REAL NOT NULL,
    profit_loss REAL,
    profit_loss_percent REAL,
    holding_period_days INTEGER,
    commission_cost REAL NOT NULL DEFAULT 0.0,
    slippage_cost REAL NOT NULL DEFAULT 0.0,
    exit_reason TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (backtest_id) REFERENCES backtests(id)
);

-- Copy any existing data (columns were already renamed by migration 20240104)
INSERT OR IGNORE INTO backtest_trades_new (
    id, backtest_id, symbol, signal, entry_date, exit_date,
    entry_price, exit_price, shares, profit_loss, profit_loss_percent,
    holding_period_days, commission_cost, slippage_cost, exit_reason, created_at
)
SELECT
    id, backtest_id, symbol, signal, entry_date, exit_date,
    entry_price, exit_price, shares, profit_loss, profit_loss_percent,
    holding_period_days, commission_cost, slippage_cost, exit_reason, created_at
FROM backtest_trades;

DROP TABLE backtest_trades;

ALTER TABLE backtest_trades_new RENAME TO backtest_trades;

-- Step 2: Fix strategy_health_snapshots column names to match code
ALTER TABLE strategy_health_snapshots RENAME COLUMN rolling_win_rate TO win_rate;
ALTER TABLE strategy_health_snapshots RENAME COLUMN rolling_profit_factor TO profit_factor;
