-- InvestIQ: Initial schema (baseline migration)
-- Consolidates schema.sql + inline table creation from crates

-- =====================================================
-- Core Portfolio Tables
-- =====================================================

-- Positions: Current holdings
CREATE TABLE IF NOT EXISTS positions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    shares REAL NOT NULL,
    entry_price REAL NOT NULL,
    entry_date TEXT NOT NULL,
    notes TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(symbol)
);

-- Trades: Historical trade log
CREATE TABLE IF NOT EXISTS trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL CHECK(action IN ('buy', 'sell')),
    shares REAL NOT NULL,
    price REAL NOT NULL,
    trade_date TEXT NOT NULL,
    commission REAL DEFAULT 0.0,
    notes TEXT,
    profit_loss REAL,
    profit_loss_percent REAL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Alerts: Tracked signals and actions
CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    alert_type TEXT NOT NULL CHECK(alert_type IN ('buy', 'sell', 'stop_loss', 'take_profit', 'watch')),
    signal TEXT NOT NULL,
    confidence REAL NOT NULL,
    current_price REAL,
    target_price REAL,
    stop_loss_price REAL,
    reason TEXT,
    status TEXT DEFAULT 'active' CHECK(status IN ('active', 'completed', 'ignored', 'expired')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    expires_at TEXT,
    completed_at TEXT
);

-- Watchlist: Stocks to monitor
CREATE TABLE IF NOT EXISTS watchlist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL UNIQUE,
    notes TEXT,
    added_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Portfolio snapshots for equity curve
CREATE TABLE IF NOT EXISTS portfolio_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    total_value REAL NOT NULL,
    total_cost REAL NOT NULL,
    total_pnl REAL NOT NULL,
    total_pnl_percent REAL NOT NULL,
    snapshot_date TEXT DEFAULT CURRENT_TIMESTAMP
);

-- =====================================================
-- Backtest Tables (from schema.sql)
-- =====================================================

CREATE TABLE IF NOT EXISTS backtest_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_name TEXT NOT NULL,
    symbol TEXT,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    initial_capital REAL NOT NULL,
    final_capital REAL NOT NULL,
    total_return REAL NOT NULL,
    total_return_percent REAL NOT NULL,
    total_trades INTEGER NOT NULL,
    winning_trades INTEGER NOT NULL,
    losing_trades INTEGER NOT NULL,
    win_rate REAL NOT NULL,
    profit_factor REAL,
    sharpe_ratio REAL,
    max_drawdown REAL,
    max_drawdown_percent REAL,
    avg_win REAL,
    avg_loss REAL,
    largest_win REAL,
    largest_loss REAL,
    avg_trade_duration_days REAL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    config_json TEXT
);

CREATE TABLE IF NOT EXISTS backtest_trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    backtest_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    entry_date TEXT NOT NULL,
    entry_price REAL NOT NULL,
    exit_date TEXT,
    exit_price REAL,
    shares REAL NOT NULL,
    signal_type TEXT NOT NULL,
    confidence REAL,
    profit_loss REAL,
    profit_loss_percent REAL,
    duration_days INTEGER,
    exit_reason TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (backtest_id) REFERENCES backtest_results(id)
);

-- Strategy performance: Track live strategy performance
CREATE TABLE IF NOT EXISTS strategy_performance (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_name TEXT NOT NULL,
    symbol TEXT,
    total_signals INTEGER DEFAULT 0,
    signals_taken INTEGER DEFAULT 0,
    signals_ignored INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_profit_loss REAL DEFAULT 0.0,
    win_rate REAL DEFAULT 0.0,
    avg_win REAL DEFAULT 0.0,
    avg_loss REAL DEFAULT 0.0,
    profit_factor REAL DEFAULT 0.0,
    last_updated TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(strategy_name, symbol)
);

-- =====================================================
-- Risk Management Tables
-- =====================================================

CREATE TABLE IF NOT EXISTS risk_parameters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    max_risk_per_trade_percent REAL DEFAULT 2.0,
    max_portfolio_risk_percent REAL DEFAULT 10.0,
    max_position_size_percent REAL DEFAULT 20.0,
    default_stop_loss_percent REAL DEFAULT 5.0,
    default_take_profit_percent REAL DEFAULT 10.0,
    trailing_stop_enabled INTEGER DEFAULT 0,
    trailing_stop_percent REAL DEFAULT 3.0,
    min_confidence_threshold REAL DEFAULT 0.70,
    min_win_rate_threshold REAL DEFAULT 0.55,
    daily_loss_limit_percent REAL NOT NULL DEFAULT 5.0,
    max_consecutive_losses INTEGER NOT NULL DEFAULT 3,
    account_drawdown_limit_percent REAL NOT NULL DEFAULT 10.0,
    trading_halted INTEGER NOT NULL DEFAULT 0,
    halt_reason TEXT,
    halted_at TEXT,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS active_risk_positions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    shares REAL NOT NULL,
    entry_price REAL NOT NULL,
    entry_date TEXT NOT NULL,
    stop_loss_price REAL,
    take_profit_price REAL,
    trailing_stop_enabled INTEGER DEFAULT 0,
    trailing_stop_percent REAL,
    max_price_seen REAL,
    risk_amount REAL,
    position_size_percent REAL,
    status TEXT DEFAULT 'active' CHECK(status IN ('active', 'stopped_out', 'target_hit', 'manual_close')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    closed_at TEXT,
    UNIQUE(symbol, status)
);

CREATE TABLE IF NOT EXISTS portfolio_peak (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    peak_value REAL NOT NULL,
    peak_date TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Signal quality metrics
CREATE TABLE IF NOT EXISTS signal_quality (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    signal_type TEXT NOT NULL,
    confidence_range TEXT NOT NULL,
    total_signals INTEGER DEFAULT 0,
    signals_taken INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    actual_win_rate REAL DEFAULT 0.0,
    avg_return REAL DEFAULT 0.0,
    calibration_error REAL DEFAULT 0.0,
    last_updated TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(signal_type, confidence_range)
);

-- =====================================================
-- Sentiment & Analytics Tables
-- =====================================================

CREATE TABLE IF NOT EXISTS sentiment_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    sentiment_score REAL NOT NULL,
    article_count INTEGER DEFAULT 0,
    velocity REAL,
    acceleration REAL,
    signal TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS user_preferences (
    user_id TEXT PRIMARY KEY,
    preferred_sectors TEXT,
    risk_tolerance REAL DEFAULT 0.5,
    preferred_market_cap TEXT DEFAULT 'all',
    preferred_volatility TEXT DEFAULT 'medium',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS symbol_interactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT DEFAULT 'default',
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    context TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS calibration_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    prediction_date TEXT NOT NULL,
    predicted_signal TEXT NOT NULL,
    predicted_confidence REAL NOT NULL,
    actual_outcome TEXT,
    actual_return REAL,
    evaluation_date TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS strategy_health_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_name TEXT NOT NULL,
    snapshot_date TEXT NOT NULL,
    rolling_sharpe REAL,
    rolling_win_rate REAL,
    rolling_profit_factor REAL,
    trades_count INTEGER DEFAULT 0,
    decay_score REAL,
    status TEXT DEFAULT 'healthy',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- =====================================================
-- Time Machine Tables
-- =====================================================

CREATE TABLE IF NOT EXISTS time_machine_sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT DEFAULT 'default',
    scenario_id TEXT,
    symbol TEXT NOT NULL,
    start_date TEXT NOT NULL,
    current_date TEXT NOT NULL,
    end_date TEXT,
    portfolio_value REAL DEFAULT 10000,
    initial_value REAL DEFAULT 10000,
    status TEXT DEFAULT 'active',
    score REAL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS time_machine_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    decision_date TEXT NOT NULL,
    action TEXT NOT NULL,
    shares REAL,
    price REAL,
    ai_action TEXT,
    ai_confidence REAL,
    actual_return REAL,
    cumulative_pnl REAL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (session_id) REFERENCES time_machine_sessions(id)
);

-- =====================================================
-- Tax Optimization Tables
-- =====================================================

CREATE TABLE IF NOT EXISTS tax_lots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    shares REAL NOT NULL,
    cost_basis REAL NOT NULL,
    purchase_date TEXT NOT NULL,
    sale_date TEXT,
    sale_price REAL,
    realized_gain_loss REAL,
    is_short_term INTEGER,
    wash_sale_adjustment REAL DEFAULT 0,
    tax_jurisdiction TEXT DEFAULT 'US',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS wash_sale_windows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    sale_date TEXT NOT NULL,
    window_start TEXT NOT NULL,
    window_end TEXT NOT NULL,
    loss_amount REAL NOT NULL,
    disallowed_amount REAL DEFAULT 0,
    sale_lot_id INTEGER REFERENCES tax_lots(id),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Risk Target Profile
CREATE TABLE IF NOT EXISTS risk_target_profile (
    user_id TEXT PRIMARY KEY DEFAULT 'default',
    market_risk_target REAL DEFAULT 50,
    volatility_risk_target REAL DEFAULT 50,
    liquidity_risk_target REAL DEFAULT 30,
    event_risk_target REAL DEFAULT 40,
    concentration_risk_target REAL DEFAULT 40,
    sentiment_risk_target REAL DEFAULT 50,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- =====================================================
-- Backtest Engine Tables (from backtest-engine crate)
-- =====================================================

CREATE TABLE IF NOT EXISTS backtests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_name TEXT NOT NULL,
    symbols TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    initial_capital REAL NOT NULL,
    final_capital REAL NOT NULL,
    total_return REAL NOT NULL DEFAULT 0,
    total_return_percent REAL NOT NULL,
    total_trades INTEGER NOT NULL,
    winning_trades INTEGER NOT NULL,
    losing_trades INTEGER NOT NULL,
    win_rate REAL NOT NULL,
    profit_factor REAL,
    sharpe_ratio REAL,
    sortino_ratio REAL,
    max_drawdown REAL,
    calmar_ratio REAL,
    max_consecutive_wins INTEGER NOT NULL DEFAULT 0,
    max_consecutive_losses INTEGER NOT NULL DEFAULT 0,
    avg_holding_period_days REAL,
    exposure_time_percent REAL,
    recovery_factor REAL,
    average_win REAL,
    average_loss REAL,
    largest_win REAL,
    largest_loss REAL,
    avg_trade_return_percent REAL,
    total_commission_paid REAL NOT NULL DEFAULT 0,
    total_slippage_cost REAL NOT NULL DEFAULT 0,
    equity_curve_json TEXT,
    benchmark_json TEXT,
    per_symbol_results_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Note: This is the backtest_trades used by backtest-engine (different schema from the one in schema.sql).
-- The schema.sql version (above) is used by the old system; this version has richer fields.
-- Both can coexist since they reference different backtest tables.
-- TODO: Unify these in a future migration.

-- =====================================================
-- Agent Trading Tables
-- =====================================================

CREATE TABLE IF NOT EXISTS pending_trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    shares REAL NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.5,
    reason TEXT NOT NULL DEFAULT '',
    signal_type TEXT NOT NULL DEFAULT 'Neutral',
    proposed_at TEXT NOT NULL DEFAULT (datetime('now')),
    status TEXT NOT NULL DEFAULT 'pending',
    reviewed_at TEXT,
    price REAL,
    order_id TEXT
);

-- =====================================================
-- ML Training Features (from data-loader)
-- =====================================================

CREATE TABLE IF NOT EXISTS analysis_features (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    analysis_date TEXT NOT NULL,
    features_json TEXT NOT NULL,
    overall_signal TEXT NOT NULL,
    overall_confidence REAL NOT NULL,
    actual_return_5d REAL,
    actual_return_20d REAL,
    evaluated INTEGER DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- =====================================================
-- Indexes
-- =====================================================

CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);
CREATE INDEX IF NOT EXISTS idx_trades_date ON trades(trade_date);
CREATE INDEX IF NOT EXISTS idx_alerts_status ON alerts(status);
CREATE INDEX IF NOT EXISTS idx_alerts_symbol ON alerts(symbol);
CREATE INDEX IF NOT EXISTS idx_snapshots_date ON portfolio_snapshots(snapshot_date);
CREATE INDEX IF NOT EXISTS idx_backtest_results_strategy ON backtest_results(strategy_name);
CREATE INDEX IF NOT EXISTS idx_backtest_results_symbol ON backtest_results(symbol);
CREATE INDEX IF NOT EXISTS idx_backtest_trades_backtest ON backtest_trades(backtest_id);
CREATE INDEX IF NOT EXISTS idx_backtest_trades_symbol ON backtest_trades(symbol);
CREATE INDEX IF NOT EXISTS idx_strategy_perf_name ON strategy_performance(strategy_name);
CREATE INDEX IF NOT EXISTS idx_active_risk_symbol ON active_risk_positions(symbol, status);
CREATE INDEX IF NOT EXISTS idx_signal_quality_type ON signal_quality(signal_type);
CREATE INDEX IF NOT EXISTS idx_sentiment_symbol_time ON sentiment_history(symbol, timestamp);
CREATE INDEX IF NOT EXISTS idx_interactions_user ON symbol_interactions(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_interactions_symbol ON symbol_interactions(symbol);
CREATE INDEX IF NOT EXISTS idx_calibration_symbol ON calibration_history(symbol);
CREATE INDEX IF NOT EXISTS idx_calibration_outcome ON calibration_history(actual_outcome);
CREATE INDEX IF NOT EXISTS idx_strategy_health_name ON strategy_health_snapshots(strategy_name, snapshot_date);
CREATE INDEX IF NOT EXISTS idx_tm_sessions_user ON time_machine_sessions(user_id, status);
CREATE INDEX IF NOT EXISTS idx_tm_decisions_session ON time_machine_decisions(session_id);
CREATE INDEX IF NOT EXISTS idx_tax_lots_symbol ON tax_lots(symbol);
CREATE INDEX IF NOT EXISTS idx_tax_lots_open ON tax_lots(symbol, sale_date);
CREATE INDEX IF NOT EXISTS idx_wash_sale_symbol ON wash_sale_windows(symbol, window_end);
CREATE INDEX IF NOT EXISTS idx_analysis_feat_symbol ON analysis_features(symbol, analysis_date);
CREATE INDEX IF NOT EXISTS idx_pending_trades_status ON pending_trades(status);
CREATE INDEX IF NOT EXISTS idx_pending_trades_symbol ON pending_trades(symbol);
CREATE INDEX IF NOT EXISTS idx_backtests_strategy ON backtests(strategy_name);
CREATE INDEX IF NOT EXISTS idx_backtests_created ON backtests(created_at);
CREATE INDEX IF NOT EXISTS idx_portfolio_peak_date ON portfolio_peak(peak_date);
