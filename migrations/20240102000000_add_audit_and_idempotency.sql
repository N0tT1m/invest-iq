-- Migration: Add audit logging table and trade idempotency support

-- Audit log: Track all trade and risk management actions
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,       -- 'trade_executed', 'trade_logged', 'position_opened', 'position_closed',
                                    -- 'stop_loss_triggered', 'take_profit_triggered', 'circuit_breaker_triggered',
                                    -- 'risk_check_failed', 'order_submitted', 'order_canceled', 'agent_trade_proposed',
                                    -- 'agent_trade_approved', 'agent_trade_rejected', 'trading_halted', 'trading_resumed'
    symbol TEXT,
    action TEXT,                    -- 'buy', 'sell', 'close', etc.
    details TEXT,                   -- JSON blob with event-specific data
    user_id TEXT DEFAULT 'system',  -- 'system', 'agent', 'user', or specific user ID
    order_id TEXT,                  -- Alpaca order ID if applicable
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_audit_log_type ON audit_log(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_log_symbol ON audit_log(symbol);
CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_at);
CREATE INDEX IF NOT EXISTS idx_audit_log_order ON audit_log(order_id);

-- Trade idempotency: prevent duplicate trade submissions
-- The idempotency_key is a unique token sent by the client to prevent double-execution
CREATE TABLE IF NOT EXISTS trade_idempotency (
    idempotency_key TEXT PRIMARY KEY,
    order_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    shares REAL NOT NULL,
    status TEXT NOT NULL,           -- 'submitted', 'filled', 'failed'
    response_json TEXT,             -- cached response for replay
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL        -- auto-expire after 24h
);

CREATE INDEX IF NOT EXISTS idx_idempotency_expires ON trade_idempotency(expires_at);

-- Circuit breaker loss tracking: persist trade outcomes for consecutive loss counting
CREATE TABLE IF NOT EXISTS trade_outcomes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    order_id TEXT,
    action TEXT NOT NULL,
    outcome TEXT NOT NULL,          -- 'win', 'loss', 'breakeven'
    pnl REAL NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_trade_outcomes_created ON trade_outcomes(created_at);
