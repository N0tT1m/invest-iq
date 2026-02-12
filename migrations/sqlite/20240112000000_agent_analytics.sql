-- Agent analytics: rich trade context and daily snapshots

CREATE TABLE IF NOT EXISTS agent_trade_context_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pending_trade_id INTEGER,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    entry_price REAL,
    stop_loss REAL,
    take_profit REAL,
    entry_regime TEXT,
    conviction_tier TEXT,
    entry_confidence REAL,
    entry_atr REAL,
    ml_probability REAL,
    ml_reasoning TEXT,
    ml_features_json TEXT,
    technical_reason TEXT,
    fundamental_reason TEXT,
    sentiment_score REAL,
    signal_adjustments TEXT,
    supplementary_signals TEXT,
    engine_signals_json TEXT,
    time_horizon_signals TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    -- Exit fields (filled post-close)
    exit_regime TEXT,
    exit_reason TEXT,
    exit_price REAL,
    exit_date TEXT,
    pnl REAL,
    pnl_percent REAL,
    outcome TEXT
);

CREATE INDEX IF NOT EXISTS idx_atc2_pending ON agent_trade_context_v2(pending_trade_id);
CREATE INDEX IF NOT EXISTS idx_atc2_symbol ON agent_trade_context_v2(symbol);
CREATE INDEX IF NOT EXISTS idx_atc2_outcome ON agent_trade_context_v2(outcome);
CREATE INDEX IF NOT EXISTS idx_atc2_created ON agent_trade_context_v2(created_at);

CREATE TABLE IF NOT EXISTS agent_daily_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_date TEXT NOT NULL UNIQUE,
    cycles_run INTEGER DEFAULT 0,
    signals_generated INTEGER DEFAULT 0,
    signals_filtered INTEGER DEFAULT 0,
    signals_ml_approved INTEGER DEFAULT 0,
    signals_ml_rejected INTEGER DEFAULT 0,
    trades_proposed INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_pnl REAL DEFAULT 0,
    regime TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
