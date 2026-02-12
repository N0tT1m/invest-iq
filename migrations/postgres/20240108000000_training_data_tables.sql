-- Training data tables for ML model training pipeline. -- PostgreSQL version
-- Populated by data-loader, consumed by price_predictor and sentiment trainers.

-- OHLCV bars for price predictor training
CREATE TABLE IF NOT EXISTS training_bars (
    id BIGSERIAL PRIMARY KEY,
    symbol TEXT NOT NULL,
    timestamp_ms BIGINT NOT NULL,
    timespan TEXT NOT NULL DEFAULT 'day',
    open DOUBLE PRECISION NOT NULL,
    high DOUBLE PRECISION NOT NULL,
    low DOUBLE PRECISION NOT NULL,
    close DOUBLE PRECISION NOT NULL,
    volume DOUBLE PRECISION NOT NULL,
    vwap DOUBLE PRECISION,
    UNIQUE(symbol, timestamp_ms, timespan)
);

CREATE INDEX IF NOT EXISTS idx_training_bars_symbol_timespan ON training_bars(symbol, timespan);

-- News articles for sentiment model training
CREATE TABLE IF NOT EXISTS training_news (
    id BIGSERIAL PRIMARY KEY,
    symbol TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    published_utc TEXT NOT NULL,
    tickers_json TEXT,
    price_change_5d DOUBLE PRECISION,
    UNIQUE(symbol, title)
);

CREATE INDEX IF NOT EXISTS idx_training_news_symbol ON training_news(symbol);
