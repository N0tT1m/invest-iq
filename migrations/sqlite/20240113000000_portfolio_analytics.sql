-- Portfolio Analytics: alert execution tracking, reconciliation, target allocations

-- Alert execution tracking
CREATE TABLE IF NOT EXISTS alert_executions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alert_id INTEGER NOT NULL,
    trade_id INTEGER,
    symbol TEXT NOT NULL,
    alert_signal TEXT NOT NULL,
    alert_confidence REAL NOT NULL,
    alert_price REAL,
    execution_price REAL,
    outcome TEXT DEFAULT 'open',
    outcome_pnl REAL,
    outcome_pnl_percent REAL,
    executed_at TEXT,
    closed_at TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Reconciliation log
CREATE TABLE IF NOT EXISTS reconciliation_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    reconciliation_date TEXT NOT NULL,
    total_positions INTEGER NOT NULL DEFAULT 0,
    matches INTEGER NOT NULL DEFAULT 0,
    discrepancies INTEGER NOT NULL DEFAULT 0,
    auto_resolved INTEGER NOT NULL DEFAULT 0,
    details_json TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Target allocations for rebalancing
CREATE TABLE IF NOT EXISTS target_allocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT,
    sector TEXT,
    target_weight_percent REAL NOT NULL,
    drift_tolerance_percent REAL NOT NULL DEFAULT 5.0,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(symbol, sector)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_alert_exec_alert ON alert_executions(alert_id);
CREATE INDEX IF NOT EXISTS idx_alert_exec_trade ON alert_executions(trade_id);
CREATE INDEX IF NOT EXISTS idx_alert_exec_outcome ON alert_executions(outcome);
CREATE INDEX IF NOT EXISTS idx_recon_date ON reconciliation_log(reconciliation_date);
CREATE INDEX IF NOT EXISTS idx_target_alloc_symbol ON target_allocations(symbol);
