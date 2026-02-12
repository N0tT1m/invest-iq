-- Agent analytics: rich trade context and daily snapshots -- PostgreSQL version

CREATE TABLE IF NOT EXISTS agent_trade_context_v2 (
    id BIGSERIAL PRIMARY KEY,
    pending_trade_id INTEGER,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    entry_price DOUBLE PRECISION,
    stop_loss DOUBLE PRECISION,
    take_profit DOUBLE PRECISION,
    entry_regime TEXT,
    conviction_tier TEXT,
    entry_confidence DOUBLE PRECISION,
    entry_atr DOUBLE PRECISION,
    ml_probability DOUBLE PRECISION,
    ml_reasoning TEXT,
    ml_features_json TEXT,
    technical_reason TEXT,
    fundamental_reason TEXT,
    sentiment_score DOUBLE PRECISION,
    signal_adjustments TEXT,
    supplementary_signals TEXT,
    engine_signals_json TEXT,
    time_horizon_signals TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Exit fields (filled post-close)
    exit_regime TEXT,
    exit_reason TEXT,
    exit_price DOUBLE PRECISION,
    exit_date TEXT,
    pnl DOUBLE PRECISION,
    pnl_percent DOUBLE PRECISION,
    outcome TEXT
);

CREATE INDEX IF NOT EXISTS idx_atc2_pending ON agent_trade_context_v2(pending_trade_id);
CREATE INDEX IF NOT EXISTS idx_atc2_symbol ON agent_trade_context_v2(symbol);
CREATE INDEX IF NOT EXISTS idx_atc2_outcome ON agent_trade_context_v2(outcome);
CREATE INDEX IF NOT EXISTS idx_atc2_created ON agent_trade_context_v2(created_at);

CREATE TABLE IF NOT EXISTS agent_daily_snapshots (
    id BIGSERIAL PRIMARY KEY,
    snapshot_date TEXT NOT NULL UNIQUE,
    cycles_run INTEGER DEFAULT 0,
    signals_generated INTEGER DEFAULT 0,
    signals_filtered INTEGER DEFAULT 0,
    signals_ml_approved INTEGER DEFAULT 0,
    signals_ml_rejected INTEGER DEFAULT 0,
    trades_proposed INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_pnl DOUBLE PRECISION DEFAULT 0,
    regime TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
