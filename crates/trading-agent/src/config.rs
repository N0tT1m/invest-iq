use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    // Risk parameters (Conservative settings)
    pub max_risk_per_trade_percent: f64,  // 2%
    pub max_position_size: f64,            // $500
    pub max_portfolio_risk_percent: f64,   // 10%
    pub min_confidence: f64,               // 0.75 (75%)
    pub min_win_rate: f64,                 // 0.60 (60%)

    // Kelly Criterion settings
    pub use_kelly_sizing: bool,            // Enable Kelly Criterion
    pub kelly_mode: String,                // "conservative", "default", "aggressive"
    pub kelly_multiplier: f64,             // Fractional Kelly (0.25 = quarter-Kelly)

    // Trading parameters
    pub scan_interval_seconds: u64,        // 300 (5 minutes)
    pub trading_enabled: bool,             // true/false
    pub paper_trading: bool,               // true for testing

    // Extended hours trading
    pub enable_extended_hours: bool,       // Trade pre/post market
    pub regular_hours_min_volume: i64,     // Min volume during regular hours
    pub extended_hours_min_volume: i64,    // Higher min volume for extended hours

    // Multi-timeframe settings
    pub enable_multi_timeframe: bool,      // Enable MTF analysis
    pub primary_timeframe: String,         // "5min", "15min", "1hour", "4hour", "daily"

    // Market regime detection
    pub enable_regime_detection: bool,     // Enable regime-based strategy switching
    pub regime_ml_service_url: Option<String>,  // Optional ML service URL

    // News trading
    pub enable_news_trading: bool,         // Enable news-based trading
    pub news_sentiment_service_url: Option<String>,  // Optional FinBERT service
    pub news_scan_interval_seconds: u64,   // How often to check news

    // Market scanner
    pub watchlist: Vec<String>,
    pub scan_top_n_stocks: usize,          // Scan top 100 stocks
    pub scan_top_movers: bool,             // Scan gainers/losers

    // Strategy weights (must sum to 1.0)
    pub momentum_weight: f64,              // 0.40
    pub mean_reversion_weight: f64,        // 0.25
    pub breakout_weight: f64,              // 0.20
    pub sentiment_weight: f64,             // 0.10
    pub high_risk_weight: f64,             // 0.05

    // ML Signal Models
    pub ml_signal_models_url: String,      // http://localhost:8004

    // External APIs
    pub polygon_api_key: String,
    pub alpaca_api_key: String,
    pub alpaca_secret_key: String,
    pub alpaca_base_url: String,

    // Discord notifications
    pub discord_webhook_url: String,

    // Database
    pub database_url: String,
}

impl AgentConfig {
    pub fn from_env() -> Result<Self> {
        let config = Self {
            // Conservative risk settings
            max_risk_per_trade_percent: env::var("MAX_RISK_PER_TRADE")
                .unwrap_or_else(|_| "2.0".to_string())
                .parse()?,
            max_position_size: env::var("MAX_POSITION_SIZE")
                .unwrap_or_else(|_| "500.0".to_string())
                .parse()?,
            max_portfolio_risk_percent: env::var("MAX_PORTFOLIO_RISK")
                .unwrap_or_else(|_| "10.0".to_string())
                .parse()?,
            min_confidence: env::var("MIN_CONFIDENCE")
                .unwrap_or_else(|_| "0.75".to_string())
                .parse()?,
            min_win_rate: env::var("MIN_WIN_RATE")
                .unwrap_or_else(|_| "0.60".to_string())
                .parse()?,

            // Kelly Criterion
            use_kelly_sizing: env::var("USE_KELLY_SIZING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            kelly_mode: env::var("KELLY_MODE")
                .unwrap_or_else(|_| "conservative".to_string()),
            kelly_multiplier: env::var("KELLY_MULTIPLIER")
                .unwrap_or_else(|_| "0.25".to_string())
                .parse()?,

            // Trading parameters
            scan_interval_seconds: env::var("SCAN_INTERVAL")
                .unwrap_or_else(|_| "300".to_string())
                .parse()?,
            trading_enabled: env::var("TRADING_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            paper_trading: env::var("PAPER_TRADING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,

            // Extended hours
            enable_extended_hours: env::var("ENABLE_EXTENDED_HOURS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            regular_hours_min_volume: env::var("REGULAR_HOURS_MIN_VOLUME")
                .unwrap_or_else(|_| "1000000".to_string())
                .parse()?,
            extended_hours_min_volume: env::var("EXTENDED_HOURS_MIN_VOLUME")
                .unwrap_or_else(|_| "500000".to_string())
                .parse()?,

            // Multi-timeframe
            enable_multi_timeframe: env::var("ENABLE_MULTI_TIMEFRAME")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            primary_timeframe: env::var("PRIMARY_TIMEFRAME")
                .unwrap_or_else(|_| "15min".to_string()),

            // Regime detection
            enable_regime_detection: env::var("ENABLE_REGIME_DETECTION")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            regime_ml_service_url: env::var("REGIME_ML_SERVICE_URL").ok(),

            // News trading
            enable_news_trading: env::var("ENABLE_NEWS_TRADING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            news_sentiment_service_url: env::var("NEWS_SENTIMENT_SERVICE_URL").ok(),
            news_scan_interval_seconds: env::var("NEWS_SCAN_INTERVAL")
                .unwrap_or_else(|_| "60".to_string())
                .parse()?,

            // Watchlist
            watchlist: env::var("WATCHLIST")
                .unwrap_or_else(|_| "AAPL,MSFT,GOOGL,AMZN,NVDA,TSLA,META,AMD,NFLX,SPY".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            scan_top_n_stocks: env::var("SCAN_TOP_N")
                .unwrap_or_else(|_| "100".to_string())
                .parse()?,
            scan_top_movers: env::var("SCAN_TOP_MOVERS")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,

            // Strategy weights
            momentum_weight: 0.40,
            mean_reversion_weight: 0.25,
            breakout_weight: 0.20,
            sentiment_weight: 0.10,
            high_risk_weight: 0.05,

            // ML Signal Models
            ml_signal_models_url: env::var("ML_SIGNAL_MODELS_URL")
                .unwrap_or_else(|_| "http://localhost:8004".to_string()),

            // APIs
            polygon_api_key: env::var("POLYGON_API_KEY")
                .context("POLYGON_API_KEY not set")?,
            alpaca_api_key: env::var("ALPACA_API_KEY")
                .context("ALPACA_API_KEY not set")?,
            alpaca_secret_key: env::var("ALPACA_SECRET_KEY")
                .context("ALPACA_SECRET_KEY not set")?,
            alpaca_base_url: env::var("ALPACA_BASE_URL")
                .unwrap_or_else(|_| "https://paper-api.alpaca.markets".to_string()),

            // Discord
            discord_webhook_url: env::var("DISCORD_WEBHOOK_URL")
                .unwrap_or_else(|_| String::new()),

            // Database
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:portfolio.db".to_string()),
        };

        Ok(config)
    }
}
