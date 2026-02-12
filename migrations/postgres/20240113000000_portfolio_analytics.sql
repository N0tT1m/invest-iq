-- Portfolio Analytics: alert execution tracking, reconciliation, target allocations

-- Alert execution tracking
CREATE TABLE IF NOT EXISTS alert_executions (
    id BIGSERIAL PRIMARY KEY,
    alert_id BIGINT NOT NULL,
    trade_id BIGINT,
    symbol TEXT NOT NULL,
    alert_signal TEXT NOT NULL,
    alert_confidence DOUBLE PRECISION NOT NULL,
    alert_price DOUBLE PRECISION,
    execution_price DOUBLE PRECISION,
    outcome TEXT DEFAULT 'open',
    outcome_pnl DOUBLE PRECISION,
    outcome_pnl_percent DOUBLE PRECISION,
    executed_at TIMESTAMPTZ,
    closed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Reconciliation log
CREATE TABLE IF NOT EXISTS reconciliation_log (
    id BIGSERIAL PRIMARY KEY,
    reconciliation_date TEXT NOT NULL,
    total_positions INTEGER NOT NULL DEFAULT 0,
    matches INTEGER NOT NULL DEFAULT 0,
    discrepancies INTEGER NOT NULL DEFAULT 0,
    auto_resolved INTEGER NOT NULL DEFAULT 0,
    details_json TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Target allocations for rebalancing
CREATE TABLE IF NOT EXISTS target_allocations (
    id BIGSERIAL PRIMARY KEY,
    symbol TEXT,
    sector TEXT,
    target_weight_percent DOUBLE PRECISION NOT NULL,
    drift_tolerance_percent DOUBLE PRECISION NOT NULL DEFAULT 5.0,
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(symbol, sector)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_alert_exec_alert ON alert_executions(alert_id);
CREATE INDEX IF NOT EXISTS idx_alert_exec_trade ON alert_executions(trade_id);
CREATE INDEX IF NOT EXISTS idx_alert_exec_outcome ON alert_executions(outcome);
CREATE INDEX IF NOT EXISTS idx_recon_date ON reconciliation_log(reconciliation_date);
CREATE INDEX IF NOT EXISTS idx_target_alloc_symbol ON target_allocations(symbol);
