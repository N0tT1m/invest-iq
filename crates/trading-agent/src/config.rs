use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    // Risk parameters (Conservative settings)
    pub max_risk_per_trade_percent: f64,  // 2%
    pub max_position_size: f64,            // $500
    pub max_portfolio_risk_percent: f64,   // 10%
    pub min_confidence: f64,               // 0.60 (60%)
    pub min_win_rate: f64,                 // 0.50 (50%)

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
    pub scan_top_n_stocks: usize,          // Scan top 20 movers (beyond watchlist)
    pub scan_top_movers: bool,             // Scan gainers/losers
    pub scanner_min_price: f64,            // $10 min for non-watchlist
    pub scanner_min_dollar_volume: f64,    // $10M daily dollar volume for non-watchlist

    // Strategy weights (must sum to ~1.0)
    pub momentum_weight: f64,              // 0.40
    pub mean_reversion_weight: f64,        // 0.25
    pub breakout_weight: f64,              // 0.20
    pub sentiment_weight: f64,             // 0.10
    pub high_risk_weight: f64,             // 0.05

    // ATR stop parameters (P2)
    pub atr_sl_multiplier: f64,            // 2.0 (overridden by regime)
    pub atr_tp_multiplier: f64,            // 3.0 (overridden by regime)

    // Order management (P3)
    pub order_timeout_seconds: u64,        // 30

    // Auto-execution: when true, execute trades immediately instead of pending review
    pub auto_execute: bool,                // false (safe default — requires explicit opt-in)
    pub max_concurrent_executions: usize,  // 5 (concurrent order submissions)

    // Portfolio guard (P4)
    pub max_open_positions: usize,         // 50 (absolute hard cap)
    pub min_position_value: f64,           // $2500 (dynamic: max_positions = portfolio / min_position_value)
    pub max_sector_concentration: f64,     // 0.30 (30%)
    pub max_gross_exposure: f64,           // 0.80 (80%)
    pub daily_loss_halt_percent: f64,      // 3.0%

    // Regime thresholds for ML gate (P6)
    pub regime_bear_threshold: f64,        // 0.60
    pub regime_bull_threshold: f64,        // 0.45

    // Concurrency
    pub max_concurrent_analyses: usize,    // 20 (symbols analyzed concurrently)

    // Metrics (P7)
    pub metrics_log_interval_cycles: u64,  // 12 (hourly at 5min intervals)

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
                .unwrap_or_else(|_| "0.60".to_string())
                .parse()?,
            min_win_rate: env::var("MIN_WIN_RATE")
                .unwrap_or_else(|_| "0.50".to_string())
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
                .unwrap_or_else(|_| "20".to_string())
                .parse()?,
            scan_top_movers: env::var("SCAN_TOP_MOVERS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            scanner_min_price: env::var("SCANNER_MIN_PRICE")
                .unwrap_or_else(|_| "10.0".to_string())
                .parse()?,
            scanner_min_dollar_volume: env::var("SCANNER_MIN_DOLLAR_VOLUME")
                .unwrap_or_else(|_| "10000000.0".to_string())
                .parse()?,

            // Strategy weights (P10: now env-configurable)
            momentum_weight: env::var("MOMENTUM_WEIGHT")
                .unwrap_or_else(|_| "0.40".to_string())
                .parse()?,
            mean_reversion_weight: env::var("MEAN_REVERSION_WEIGHT")
                .unwrap_or_else(|_| "0.25".to_string())
                .parse()?,
            breakout_weight: env::var("BREAKOUT_WEIGHT")
                .unwrap_or_else(|_| "0.20".to_string())
                .parse()?,
            sentiment_weight: env::var("SENTIMENT_WEIGHT")
                .unwrap_or_else(|_| "0.10".to_string())
                .parse()?,
            high_risk_weight: env::var("HIGH_RISK_WEIGHT")
                .unwrap_or_else(|_| "0.05".to_string())
                .parse()?,

            // ATR stop parameters (P2)
            atr_sl_multiplier: env::var("ATR_SL_MULTIPLIER")
                .unwrap_or_else(|_| "2.0".to_string())
                .parse()?,
            atr_tp_multiplier: env::var("ATR_TP_MULTIPLIER")
                .unwrap_or_else(|_| "3.0".to_string())
                .parse()?,

            // Order management (P3)
            order_timeout_seconds: env::var("ORDER_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()?,

            // Auto-execution
            auto_execute: env::var("AUTO_EXECUTE_TRADES")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            max_concurrent_executions: env::var("MAX_CONCURRENT_EXECUTIONS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()?,

            // Portfolio guard (P4)
            max_open_positions: env::var("MAX_OPEN_POSITIONS")
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,
            min_position_value: env::var("MIN_POSITION_VALUE")
                .unwrap_or_else(|_| "2500.0".to_string())
                .parse()?,
            max_sector_concentration: env::var("MAX_SECTOR_CONCENTRATION")
                .unwrap_or_else(|_| "0.30".to_string())
                .parse()?,
            max_gross_exposure: env::var("MAX_GROSS_EXPOSURE")
                .unwrap_or_else(|_| "0.80".to_string())
                .parse()?,
            daily_loss_halt_percent: env::var("DAILY_LOSS_HALT_PERCENT")
                .unwrap_or_else(|_| "3.0".to_string())
                .parse()?,

            // Regime thresholds (P6)
            regime_bear_threshold: env::var("REGIME_BEAR_THRESHOLD")
                .unwrap_or_else(|_| "0.60".to_string())
                .parse()?,
            regime_bull_threshold: env::var("REGIME_BULL_THRESHOLD")
                .unwrap_or_else(|_| "0.45".to_string())
                .parse()?,

            // Concurrency
            max_concurrent_analyses: env::var("MAX_CONCURRENT_ANALYSES")
                .unwrap_or_else(|_| "20".to_string())
                .parse()?,

            // Metrics (P7)
            metrics_log_interval_cycles: env::var("METRICS_LOG_INTERVAL_CYCLES")
                .unwrap_or_else(|_| "12".to_string())
                .parse()?,

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

        config.validate()?;
        Ok(config)
    }

    /// Validate config values are in sensible ranges.
    pub fn validate(&self) -> Result<()> {
        // Strategy weights should sum to ~1.0
        let weight_sum = self.momentum_weight
            + self.mean_reversion_weight
            + self.breakout_weight
            + self.sentiment_weight
            + self.high_risk_weight;
        if (weight_sum - 1.0).abs() > 0.05 {
            bail!(
                "Strategy weights sum to {:.3} (expected ~1.0): momentum={}, mean_reversion={}, breakout={}, sentiment={}, high_risk={}",
                weight_sum,
                self.momentum_weight, self.mean_reversion_weight, self.breakout_weight,
                self.sentiment_weight, self.high_risk_weight
            );
        }

        // Thresholds in valid ranges
        if self.min_confidence < 0.0 || self.min_confidence > 1.0 {
            bail!("MIN_CONFIDENCE must be 0.0-1.0, got {}", self.min_confidence);
        }
        if self.min_win_rate < 0.0 || self.min_win_rate > 1.0 {
            bail!("MIN_WIN_RATE must be 0.0-1.0, got {}", self.min_win_rate);
        }
        if self.max_sector_concentration < 0.0 || self.max_sector_concentration > 1.0 {
            bail!("MAX_SECTOR_CONCENTRATION must be 0.0-1.0, got {}", self.max_sector_concentration);
        }
        if self.max_gross_exposure < 0.0 || self.max_gross_exposure > 2.0 {
            bail!("MAX_GROSS_EXPOSURE must be 0.0-2.0, got {}", self.max_gross_exposure);
        }
        if self.atr_sl_multiplier <= 0.0 {
            bail!("ATR_SL_MULTIPLIER must be positive, got {}", self.atr_sl_multiplier);
        }
        if self.atr_tp_multiplier <= 0.0 {
            bail!("ATR_TP_MULTIPLIER must be positive, got {}", self.atr_tp_multiplier);
        }
        if self.order_timeout_seconds == 0 {
            bail!("ORDER_TIMEOUT_SECONDS must be > 0");
        }

        // Required API keys are present (already checked by Context above)
        if self.polygon_api_key.is_empty() {
            bail!("POLYGON_API_KEY is empty");
        }
        if self.alpaca_api_key.is_empty() {
            bail!("ALPACA_API_KEY is empty");
        }

        Ok(())
    }
}
