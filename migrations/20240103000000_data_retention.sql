-- Archive tables for data retention
CREATE TABLE IF NOT EXISTS trades_archive (
    id INTEGER PRIMARY KEY,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    shares REAL NOT NULL,
    price REAL NOT NULL,
    total_value REAL,
    order_id TEXT,
    status TEXT DEFAULT 'filled',
    trade_date TEXT DEFAULT (datetime('now')),
    notes TEXT,
    archived_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS audit_log_archive (
    id INTEGER PRIMARY KEY,
    event_type TEXT NOT NULL,
    symbol TEXT,
    action TEXT,
    details TEXT,
    user_id TEXT NOT NULL DEFAULT 'system',
    order_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    archived_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Track retention runs
CREATE TABLE IF NOT EXISTS retention_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_date TEXT NOT NULL DEFAULT (datetime('now')),
    trades_archived INTEGER NOT NULL DEFAULT 0,
    audit_entries_archived INTEGER NOT NULL DEFAULT 0,
    backtest_results_archived INTEGER NOT NULL DEFAULT 0,
    completed_at TEXT
);
