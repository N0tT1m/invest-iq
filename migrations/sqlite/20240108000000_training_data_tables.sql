-- Training data tables for ML model training pipeline.
-- Populated by data-loader, consumed by price_predictor and sentiment trainers.

-- OHLCV bars for price predictor training
CREATE TABLE IF NOT EXISTS training_bars (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    timespan TEXT NOT NULL DEFAULT 'day',
    open REAL NOT NULL,
    high REAL NOT NULL,
    low REAL NOT NULL,
    close REAL NOT NULL,
    volume REAL NOT NULL,
    vwap REAL,
    UNIQUE(symbol, timestamp_ms, timespan)
);

CREATE INDEX IF NOT EXISTS idx_training_bars_symbol_timespan ON training_bars(symbol, timespan);

-- News articles for sentiment model training
CREATE TABLE IF NOT EXISTS training_news (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    published_utc TEXT NOT NULL,
    tickers_json TEXT,
    price_change_5d REAL,
    UNIQUE(symbol, title)
);

CREATE INDEX IF NOT EXISTS idx_training_news_symbol ON training_news(symbol);
