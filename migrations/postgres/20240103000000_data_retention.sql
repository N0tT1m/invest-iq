-- Archive tables for data retention -- PostgreSQL version

CREATE TABLE IF NOT EXISTS trades_archive (
    id INTEGER PRIMARY KEY,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    shares DOUBLE PRECISION NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    total_value DOUBLE PRECISION,
    order_id TEXT,
    status TEXT DEFAULT 'filled',
    trade_date TIMESTAMPTZ DEFAULT NOW(),
    notes TEXT,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS audit_log_archive (
    id INTEGER PRIMARY KEY,
    event_type TEXT NOT NULL,
    symbol TEXT,
    action TEXT,
    details TEXT,
    user_id TEXT NOT NULL DEFAULT 'system',
    order_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Track retention runs
CREATE TABLE IF NOT EXISTS retention_runs (
    id BIGSERIAL PRIMARY KEY,
    run_date TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    trades_archived INTEGER NOT NULL DEFAULT 0,
    audit_entries_archived INTEGER NOT NULL DEFAULT 0,
    backtest_results_archived INTEGER NOT NULL DEFAULT 0,
    completed_at TEXT
);
