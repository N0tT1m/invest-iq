mod auth;
mod brute_force;
mod embedded_frontend;
mod ip_allowlist;
mod portfolio_routes;
mod broker_routes;
mod request_id;
mod risk_routes;
mod security_headers;
mod backtest_routes;
mod analytics_routes;
mod sentiment_routes;
mod calibration_routes;
mod alpha_decay_routes;
mod watchlist_routes;
mod flow_routes;
mod time_machine_routes;
mod tax_routes;
mod earnings_routes;
mod dividend_routes;
mod options_routes;
mod short_interest_routes;
mod insider_routes;
mod correlation_routes;
mod macro_routes;
mod agent_trade_routes;
mod audit;
mod retention_routes;
mod symbol_routes;
mod portfolio_analytics_routes;
mod python_manager;

use analysis_core::{Bar, Timeframe, UnifiedAnalysis};
use analysis_orchestrator::{AnalysisOrchestrator, StockScreener, StockUniverse, ScreenerFilters, ScreenerResult};
use alpaca_broker::AlpacaClient;
use backtest_engine::{BacktestConfig, BacktestDb, BacktestEngine as BtEngine, BacktestResult as BtResult, HistoricalBar, Signal as BtSignal};
use risk_manager::RiskManager;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use analytics::{PerformanceTracker, SignalAnalyzer};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use portfolio_manager::{PortfolioDb, PortfolioManager, TradeLogger, AlertManager};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use axum::error_handling::HandleErrorLayer;
use std::time::Duration;
use tower_governor::{
    governor::GovernorConfigBuilder,
    GovernorLayer,
    key_extractor::SmartIpKeyExtractor,
};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use validation::{ComparisonEngine, ComparisonResult};

/// Read a secret from environment variable or Docker secret file.
/// Checks env var first, then falls back to /run/secrets/{name}.
fn read_secret(env_var: &str) -> Option<String> {
    if let Ok(val) = std::env::var(env_var) {
        if !val.is_empty() {
            return Some(val);
        }
    }
    let secret_name = env_var.to_lowercase();
    let path = format!("/run/secrets/{}", secret_name);
    match std::fs::read_to_string(&path) {
        Ok(val) => {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                Some(trimmed)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

/// Simple in-process metrics counters.
pub(crate) struct Metrics {
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    /// Latency histogram buckets (ms): <10, <50, <100, <250, <500, <1000, >=1000
    pub latency_buckets: [AtomicU64; 7],
    pub analysis_count: AtomicU64,
    pub trade_count: AtomicU64,
    pub active_connections: AtomicU64,
}

impl Metrics {
    fn new() -> Self {
        Self {
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            latency_buckets: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            analysis_count: AtomicU64::new(0),
            trade_count: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
        }
    }

    fn record(&self, latency_ms: u64, is_error: bool) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        if is_error {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
        let bucket = match latency_ms {
            0..=9 => 0,
            10..=49 => 1,
            50..=99 => 2,
            100..=249 => 3,
            250..=499 => 4,
            500..=999 => 5,
            _ => 6,
        };
        self.latency_buckets[bucket].fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Clone)]
enum CacheBackend {
    Redis(ConnectionManager),
    Memory(Arc<DashMap<String, CachedAnalysis>>),
}

#[derive(Clone)]
struct AppState {
    orchestrator: Arc<AnalysisOrchestrator>,
    screener: Arc<StockScreener>,
    cache: CacheBackend,
    etf_bar_cache: Arc<DashMap<String, (DateTime<Utc>, Vec<Bar>)>>,
    comparison_engine: Option<Arc<ComparisonEngine>>,
    portfolio_manager: Option<Arc<PortfolioManager>>,
    trade_logger: Option<Arc<TradeLogger>>,
    alert_manager: Option<Arc<AlertManager>>,
    alpaca_client: Option<Arc<AlpacaClient>>,
    risk_manager: Option<Arc<RiskManager>>,
    backtest_db: Option<Arc<BacktestDb>>,
    performance_tracker: Option<Arc<PerformanceTracker>>,
    signal_analyzer: Option<Arc<SignalAnalyzer>>,
    metrics: Arc<Metrics>,
    brute_force_guard: Arc<brute_force::BruteForceGuard>,
    #[allow(dead_code)] // Accessed via middleware's own state, not through AppState
    ip_allowlist: Option<ip_allowlist::IpAllowlist>,
}

/// Get cached ETF bars, fetching and caching on miss.
/// TTL is in minutes. Returns empty vec on failure.
pub(crate) async fn get_cached_etf_bars(
    state: &AppState,
    symbol: &str,
    days: i64,
    ttl_minutes: i64,
) -> Vec<Bar> {
    let cache_key = format!("{}:{}", symbol, days);
    if let Some(entry) = state.etf_bar_cache.get(&cache_key) {
        let (cached_at, bars) = entry.value();
        let age = (Utc::now() - *cached_at).num_minutes();
        if age < ttl_minutes {
            return bars.clone();
        }
    }
    match state.orchestrator.get_bars(symbol, Timeframe::Day1, days).await {
        Ok(bars) => {
            state.etf_bar_cache.insert(cache_key, (Utc::now(), bars.clone()));
            bars
        }
        Err(e) => {
            tracing::warn!("Failed to fetch ETF bars for {}: {}", symbol, e);
            Vec::new()
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct CachedAnalysis {
    analysis: UnifiedAnalysis,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct AnalyzeQuery {
    #[serde(default = "default_cache_duration")]
    #[allow(dead_code)]
    cache_ttl: u64,
    timeframe: Option<String>,
    days: Option<i64>,
}

fn default_cache_duration() -> u64 {
    300 // 5 minutes
}

/// Parse a timeframe string (from query params) into a Timeframe enum.
fn parse_timeframe(s: Option<&str>) -> Timeframe {
    match s {
        Some("1m") => Timeframe::Minute1,
        Some("5m") => Timeframe::Minute5,
        Some("15m") => Timeframe::Minute15,
        Some("30m") => Timeframe::Minute30,
        Some("1h") => Timeframe::Hour1,
        Some("4h") => Timeframe::Hour4,
        Some("1w") => Timeframe::Week1,
        Some("1M") => Timeframe::Month1,
        _ => Timeframe::Day1,
    }
}

/// Get analysis with cache (used by all route handlers).
/// Returns cached result if available and fresh, otherwise runs full analysis and caches it.
pub(crate) async fn get_cached_analysis(
    state: &AppState,
    symbol: &str,
    timeframe: Timeframe,
    days: i64,
) -> Result<UnifiedAnalysis, analysis_core::AnalysisError> {
    let symbol = symbol.to_uppercase();
    let cache_ttl = default_cache_duration();
    let cache_key = format!("analysis:{}:{:?}:{}", symbol, timeframe, days);

    // Check cache
    let cached: Option<CachedAnalysis> = match &state.cache {
        CacheBackend::Redis(conn) => {
            let mut conn = conn.clone();
            match conn.get::<_, String>(&cache_key).await {
                Ok(json_str) => serde_json::from_str(&json_str).ok(),
                Err(_) => None,
            }
        }
        CacheBackend::Memory(map) => {
            map.get(&cache_key).map(|entry| entry.clone())
        }
    };

    if let Some(cached) = cached {
        let age = (chrono::Utc::now() - cached.timestamp).num_seconds() as u64;
        if age < cache_ttl {
            tracing::info!("💾 [get_cached_analysis] cache hit for {} (age: {}s)", cache_key, age);
            return Ok(cached.analysis);
        }
    }

    // Cache miss — run full analysis
    tracing::info!("🔍 [get_cached_analysis] cache miss for {}, running analysis", cache_key);
    let analysis = state.orchestrator.analyze(&symbol, timeframe, days).await?;

    let cached_analysis = CachedAnalysis {
        analysis: analysis.clone(),
        timestamp: chrono::Utc::now(),
    };

    match &state.cache {
        CacheBackend::Redis(conn) => {
            let mut conn = conn.clone();
            if let Ok(json_str) = serde_json::to_string(&cached_analysis) {
                let _: Result<(), _> = conn
                    .set_ex(&cache_key, json_str, cache_ttl)
                    .await;
            }
        }
        CacheBackend::Memory(map) => {
            map.insert(cache_key, cached_analysis);
        }
    }

    Ok(analysis)
}

/// Convenience wrapper: run analysis with default timeframe (Day1) and lookback (365 days).
/// Used by internal routes (backtest, validate, screener, etc.) that don't expose timeframe controls.
pub(crate) async fn get_default_analysis(
    state: &AppState,
    symbol: &str,
) -> Result<UnifiedAnalysis, analysis_core::AnalysisError> {
    get_cached_analysis(state, symbol, Timeframe::Day1, 365).await
}

#[derive(Deserialize)]
struct BarsQuery {
    timeframe: Option<String>,
    days: Option<i64>,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown...");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // On Windows, catch panics so the console window stays open
    #[cfg(windows)]
    std::panic::set_hook(Box::new(|info| {
        eprintln!("\n[FATAL] {}", info);
        eprintln!("\nPress Enter to exit...");
        let _ = std::io::stdin().read_line(&mut String::new());
    }));

    println!("InvestIQ API Server starting...");

    // Load environment variables
    let dotenv_result = dotenvy::dotenv();

    // Initialize tracing (JSON format when RUST_LOG_FORMAT=json)
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "api_server=info,tower_http=debug".into());

    let use_json = std::env::var("RUST_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    if use_json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    match &dotenv_result {
        Ok(path) => tracing::info!("Loaded .env from {}", path.display()),
        Err(e) => tracing::warn!("No .env file loaded: {}", e),
    }

    // Get Polygon API key from environment or Docker secret
    let polygon_api_key = match read_secret("POLYGON_API_KEY") {
        Some(key) => key,
        None => {
            tracing::error!("POLYGON_API_KEY must be set in environment or .env file");
            #[cfg(windows)]
            {
                eprintln!("\nPress Enter to exit...");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
            std::process::exit(1);
        }
    };

    // Create orchestrator
    tracing::info!("Initializing orchestrator...");
    let orchestrator = Arc::new(AnalysisOrchestrator::new(polygon_api_key));

    // Create stock screener
    let screener = Arc::new(StockScreener::new(Arc::clone(&orchestrator)));

    // Try to connect to Redis, fall back to in-memory cache
    let cache = match std::env::var("REDIS_URL") {
        Ok(redis_url) if !redis_url.is_empty() => {
            tracing::info!("Connecting to Redis at {}...", redis_url);
            match redis::Client::open(redis_url.as_str()) {
                Ok(client) => {
                    // Timeout after 5s so we don't hang if Redis is unreachable
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        ConnectionManager::new(client),
                    ).await {
                        Ok(Ok(conn)) => {
                            tracing::info!("Connected to Redis");
                            CacheBackend::Redis(conn)
                        }
                        Ok(Err(e)) => {
                            tracing::warn!("Failed to connect to Redis: {}. Using in-memory cache.", e);
                            CacheBackend::Memory(Arc::new(DashMap::new()))
                        }
                        Err(_) => {
                            tracing::warn!("Redis connection timed out. Using in-memory cache.");
                            CacheBackend::Memory(Arc::new(DashMap::new()))
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Invalid Redis URL: {}. Using in-memory cache.", e);
                    CacheBackend::Memory(Arc::new(DashMap::new()))
                }
            }
        }
        _ => {
            tracing::info!("REDIS_URL not set. Using in-memory cache.");
            CacheBackend::Memory(Arc::new(DashMap::new()))
        }
    };

    // Create validation engines (optional - requires Alpha Vantage key)
    let comparison_engine = std::env::var("ALPHA_VANTAGE_API_KEY")
        .ok()
        .map(|key| {
            tracing::info!("✅ Alpha Vantage API key found. Validation enabled.");
            Arc::new(ComparisonEngine::new(key))
        });

    if comparison_engine.is_none() {
        tracing::info!("ℹ️  ALPHA_VANTAGE_API_KEY not set. Validation features disabled.");
    }

    // Initialize portfolio database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:portfolio.db".to_string());
    tracing::info!("Initializing database at {}...", database_url);

    let (portfolio_manager, trade_logger, alert_manager, risk_manager, backtest_db, performance_tracker, signal_analyzer) = match PortfolioDb::new(&database_url).await {
        Ok(db) => {
            tracing::info!("✅ Portfolio database initialized at {}", database_url);
            let pm = Arc::new(PortfolioManager::new(db.clone()));
            let tl = Arc::new(TradeLogger::new(db.clone()));
            let am = Arc::new(AlertManager::new(db.clone()));
            let rm = Arc::new(RiskManager::new(db.pool().clone()));
            let bd = Arc::new(BacktestDb::new(db.pool().clone()));
            let pt = Arc::new(PerformanceTracker::new(db.pool().clone()));
            let sa = Arc::new(SignalAnalyzer::new(db.pool().clone()));
            // Initialize circuit breaker tables
            if let Err(e) = rm.init_circuit_breaker_tables().await {
                tracing::warn!("Failed to initialize circuit breaker tables: {}", e);
            }
            // Initialize agent pending trades table
            if let Err(e) = agent_trade_routes::init_pending_trades_table(db.pool()).await {
                tracing::warn!("Failed to initialize pending trades table: {}", e);
            }
            tracing::info!("✅ Risk manager initialized");
            tracing::info!("✅ Backtest database initialized");
            tracing::info!("✅ Performance tracker initialized");
            tracing::info!("✅ Signal analyzer initialized");
            (Some(pm), Some(tl), Some(am), Some(rm), Some(bd), Some(pt), Some(sa))
        }
        Err(e) => {
            tracing::warn!("⚠️  Failed to initialize portfolio database: {}. Portfolio features disabled.", e);
            (None, None, None, None, None, None, None)
        }
    };

    // Initialize Alpaca broker client
    let alpaca_client = match AlpacaClient::from_env() {
        Ok(client) => {
            // Safety gate: if pointed at live Alpaca, require explicit approval
            if !client.is_paper() {
                let approved = std::env::var("LIVE_TRADING_APPROVED")
                    .map(|v| v.eq_ignore_ascii_case("yes"))
                    .unwrap_or(false);
                if !approved {
                    tracing::error!(
                        "🛑 ALPACA_BASE_URL points to LIVE trading ({}) but LIVE_TRADING_APPROVED=yes is not set. Refusing to start.",
                        client.base_url()
                    );
                    std::process::exit(1);
                }
                tracing::warn!("⚠️  Alpaca broker connected to LIVE trading: {}", client.base_url());
            } else {
                tracing::info!("✅ Alpaca broker connected (Paper Trading Mode)");
            }
            tracing::info!("   Using: {}", client.base_url());
            Some(Arc::new(client))
        }
        Err(e) => {
            tracing::warn!("⚠️  Alpaca broker not configured: {}. Trading features disabled.", e);
            tracing::info!("   Set APCA_API_KEY_ID and APCA_API_SECRET_KEY to enable broker integration.");
            None
        }
    };

    let metrics = Arc::new(Metrics::new());
    let brute_force_guard = Arc::new(brute_force::BruteForceGuard::new());
    let ip_allowlist = ip_allowlist::IpAllowlist::from_env();

    let state = AppState {
        orchestrator,
        screener,
        cache,
        etf_bar_cache: Arc::new(DashMap::new()),
        comparison_engine,
        portfolio_manager,
        trade_logger,
        alert_manager,
        alpaca_client,
        risk_manager,
        backtest_db,
        performance_tracker,
        signal_analyzer,
        metrics,
        brute_force_guard: brute_force_guard.clone(),
        ip_allowlist: ip_allowlist.clone(),
    };

    // --- Startup environment validation ---
    {
        let api_keys = auth::get_valid_api_keys();
        let require_auth = std::env::var("REQUIRE_AUTH")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);

        if api_keys.is_empty() {
            if require_auth {
                tracing::error!("REQUIRE_AUTH is set but API_KEYS is empty. Cannot start without authentication in production mode.");
                std::process::exit(1);
            }
            tracing::warn!("API_KEYS is not set. API authentication is DISABLED. Set API_KEYS=<key1>,<key2> for production.");
            tracing::warn!("Set REQUIRE_AUTH=true to enforce authentication.");
        } else {
            tracing::info!("API authentication enabled ({} key(s) configured)", api_keys.len());
        }

        if state.portfolio_manager.is_none() {
            tracing::warn!("Portfolio features disabled (database unavailable)");
        }
        if state.alpaca_client.is_none() {
            tracing::warn!("Trading features disabled (APCA_API_KEY_ID/APCA_API_SECRET_KEY not set)");
        }
        if std::env::var("FINNHUB_API_KEY").is_err() {
            tracing::info!("FINNHUB_API_KEY not set — Finnhub news source disabled");
        }
        if std::env::var("LIVE_TRADING_KEY").is_err() && state.alpaca_client.is_some() {
            tracing::info!("LIVE_TRADING_KEY not set — broker write endpoints disabled (safe default)");
        }

        // Warn if CORS is wide open in production
        if require_auth {
            let origins_raw = std::env::var("ALLOWED_ORIGINS").unwrap_or_default();
            if origins_raw.contains('*') || origins_raw.is_empty() {
                tracing::warn!("REQUIRE_AUTH=true but ALLOWED_ORIGINS is wildcard or empty — consider restricting CORS origins");
            }
        }
    }

    // Get allowed origins from environment (comma-separated)
    let allowed_origins = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:3000,http://localhost:8050,http://localhost:8051,http://localhost:8052".to_string());

    let origins: Vec<_> = allowed_origins
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    tracing::info!("✅ CORS allowed origins: {:?}", origins);

    // Build CORS layer with specific origins
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderName::from_static("x-api-key"),
            axum::http::HeaderName::from_static("x-live-trading-key"),
        ])
        .max_age(Duration::from_secs(3600));

    // Rate limiting: uses SmartIpKeyExtractor (X-Forwarded-For → X-Real-Ip → Forwarded → peer IP)
    let rate_limit_per_minute = std::env::var("RATE_LIMIT_PER_MINUTE")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(120);

    // period = time between token replenishments; burst_size = max tokens (burst capacity)
    // Burst should be generous — the frontend fires 20+ concurrent requests on analyze
    let replenish_interval_ms = 60_000u64.checked_div(rate_limit_per_minute.max(1)).unwrap_or(1000);
    let burst = rate_limit_per_minute.min(200) as u32;

    let mut governor_builder = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor);
    governor_builder.per_millisecond(replenish_interval_ms);
    governor_builder.burst_size(burst);
    let governor_conf = std::sync::Arc::new(
        governor_builder
            .use_headers()
            .finish()
            .expect("Invalid rate limit configuration"),
    );

    tracing::info!(
        "Rate limiting enabled: {} req/min, burst size {}, replenish every {}ms",
        rate_limit_per_minute, burst, replenish_interval_ms
    );

    // Background brute-force guard cleanup (every 5 minutes)
    {
        let guard = brute_force_guard.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                guard.cleanup();
            }
        });
    }

    // Request timeout
    let request_timeout_secs = std::env::var("REQUEST_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30);
    let request_timeout = Duration::from_secs(request_timeout_secs);
    tracing::info!("Request timeout: {}s", request_timeout_secs);

    // Pre-warm ETF bar caches in the background — all 6 fire in parallel.
    // With Starter plan (unlimited API calls), this completes in ~1-2s.
    {
        let warm_state = state.clone();
        tokio::spawn(async move {
            tracing::info!("🔄 Pre-warming ETF bar caches (SPY, TLT, GLD, QQQ, DIA, IWM)...");
            let (spy, tlt, gld, qqq, dia, iwm) = tokio::join!(
                warm_state.orchestrator.get_bars("SPY", Timeframe::Day1, 365),
                warm_state.orchestrator.get_bars("TLT", Timeframe::Day1, 365),
                warm_state.orchestrator.get_bars("GLD", Timeframe::Day1, 365),
                warm_state.orchestrator.get_bars("QQQ", Timeframe::Day1, 365),
                warm_state.orchestrator.get_bars("DIA", Timeframe::Day1, 365),
                warm_state.orchestrator.get_bars("IWM", Timeframe::Day1, 365),
            );
            let results = [("SPY", spy), ("TLT", tlt), ("GLD", gld), ("QQQ", qqq), ("DIA", dia), ("IWM", iwm)];
            for (etf, result) in results {
                match result {
                    Ok(bars) => {
                        let cache_key = format!("{}:{}", etf, 365);
                        warm_state.etf_bar_cache.insert(cache_key, (Utc::now(), bars.clone()));
                        let cache_key_90 = format!("{}:{}", etf, 90);
                        let cutoff = Utc::now() - chrono::Duration::days(90);
                        let subset: Vec<_> = bars.iter().filter(|b| b.timestamp >= cutoff).cloned().collect();
                        warm_state.etf_bar_cache.insert(cache_key_90, (Utc::now(), subset));
                        tracing::info!("✅ Pre-warmed {} ({} bars)", etf, bars.len());
                    }
                    Err(e) => tracing::warn!("⚠️ Failed to pre-warm {}: {}", etf, e),
                }
            }
            tracing::info!("🔄 ETF pre-warming complete");
        });
    }

    // Build routes
    let app = Router::new()
        .route("/", get(health_check))
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_endpoint))
        .route("/metrics/json", get(metrics_json_endpoint))
        .route("/api/analyze/:symbol", get(analyze_symbol))
        .route("/api/bars/:symbol", get(get_bars))
        .route("/api/ticker/:symbol", get(get_ticker))
        .route("/api/suggest", get(suggest_stocks))
        .route("/api/validate/:symbol", get(validate_analysis))
        .route("/api/backtest/:symbol", get(backtest_symbol))
        .merge(portfolio_routes::portfolio_routes())
        .merge(broker_routes::broker_read_routes())
        .merge(
            broker_routes::broker_write_routes()
                .layer(middleware::from_fn(auth::require_trader_middleware))
        )
        .merge(agent_trade_routes::agent_trade_routes())
        .merge(
            risk_routes::risk_routes()
                .layer(middleware::from_fn_with_state(
                    ip_allowlist.clone(),
                    ip_allowlist::ip_allowlist_middleware,
                ))
        )
        .merge(backtest_routes::backtest_routes())
        .merge(analytics_routes::analytics_routes())
        .merge(sentiment_routes::sentiment_routes())
        .merge(calibration_routes::calibration_routes())
        .merge(alpha_decay_routes::alpha_decay_routes())
        .merge(watchlist_routes::watchlist_routes())
        .merge(flow_routes::flow_routes())
        .merge(time_machine_routes::time_machine_routes())
        .merge(tax_routes::tax_routes())
        .merge(earnings_routes::earnings_routes())
        .merge(dividend_routes::dividend_routes())
        .merge(options_routes::options_routes())
        .merge(short_interest_routes::short_interest_routes())
        .merge(insider_routes::insider_routes())
        .merge(correlation_routes::correlation_routes())
        .merge(macro_routes::macro_routes())
        .merge(symbol_routes::symbol_routes())
        .merge(portfolio_analytics_routes::portfolio_analytics_routes())
        .merge(
            retention_routes::retention_routes()
                .layer(middleware::from_fn(auth::require_admin_middleware))
                .layer(middleware::from_fn_with_state(
                    ip_allowlist.clone(),
                    ip_allowlist::ip_allowlist_middleware,
                ))
        )
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|_: tower::BoxError| async {
                    (StatusCode::REQUEST_TIMEOUT, Json(ApiResponse::<()>::error("Request timeout".to_string())))
                }))
                .layer(TimeoutLayer::new(request_timeout))
                .layer(axum::extract::DefaultBodyLimit::max(1_048_576)) // 1MB
                .layer(TraceLayer::new_for_http())
                .layer(middleware::from_fn(request_id::request_id_middleware))
                .layer(middleware::from_fn(security_headers::security_headers_middleware))
                .layer(GovernorLayer { config: governor_conf })
                .layer(middleware::from_fn_with_state(state.clone(), metrics_middleware))
                .layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware))
                .layer(cors)
        )
        .with_state(state.clone());

    // Parse CLI flags
    let no_frontend = std::env::args().any(|a| a == "--no-frontend");

    // Start frontend manager (unless --no-frontend)
    let frontend_handle = if no_frontend {
        tracing::info!("Frontend disabled (--no-frontend flag)");
        None
    } else {
        match python_manager::PythonManager::new() {
            Ok(mgr) => match mgr.start() {
                Ok((handle, _status_rx)) => Some(handle),
                Err(e) => {
                    tracing::error!("Failed to start frontend: {}. Continuing API-only.", e);
                    None
                }
            },
            Err(e) => {
                tracing::error!("Failed to initialize frontend manager: {}. Continuing API-only.", e);
                None
            }
        }
    };

    // Start server with optional TLS and graceful shutdown
    let addr = "0.0.0.0:3000";
    let shutdown_timeout_secs = std::env::var("SHUTDOWN_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30);

    if let (Ok(cert_path), Ok(key_path)) = (
        std::env::var("TLS_CERT_PATH"),
        std::env::var("TLS_KEY_PATH"),
    ) {
        tracing::info!("🔒 TLS enabled: cert={}, key={}", cert_path, key_path);
        tracing::info!("🚀 API Server starting on https://{}", addr);
        let config =
            axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_path, &key_path).await?;
        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            shutdown_handle
                .graceful_shutdown(Some(Duration::from_secs(shutdown_timeout_secs)));
        });
        axum_server::bind_rustls(addr.parse::<SocketAddr>()?, config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await?;
    } else {
        tracing::info!("🚀 API Server starting on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    }

    // Shutdown: abort the frontend supervisor (which kills the child)
    if let Some(handle) = frontend_handle {
        tracing::info!("Shutting down frontend...");
        handle.abort();
        let _ = handle.await;
        tracing::info!("Frontend stopped");
    }

    Ok(())
}

async fn metrics_middleware(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: middleware::Next,
) -> Response {
    // Track active connections
    state.metrics.active_connections.fetch_add(1, Ordering::Relaxed);

    // Track analysis requests
    let uri_path = request.uri().path();
    if uri_path.starts_with("/api/analyze/") {
        state.metrics.analysis_count.fetch_add(1, Ordering::Relaxed);
    }

    let start = std::time::Instant::now();
    let response = next.run(request).await;
    let latency_ms = start.elapsed().as_millis() as u64;
    let is_error = response.status().is_server_error();
    state.metrics.record(latency_ms, is_error);

    // Decrement active connections
    state.metrics.active_connections.fetch_sub(1, Ordering::Relaxed);

    response
}

/// Prometheus exposition format endpoint
async fn metrics_endpoint(
    State(state): State<AppState>,
) -> impl IntoResponse {
    use axum::http::{HeaderName, HeaderValue};

    let m = &state.metrics;

    // Load all metrics once
    let request_count = m.request_count.load(Ordering::Relaxed);
    let error_count = m.error_count.load(Ordering::Relaxed);
    let active_connections = m.active_connections.load(Ordering::Relaxed);
    let analysis_count = m.analysis_count.load(Ordering::Relaxed);
    let trade_count = m.trade_count.load(Ordering::Relaxed);

    // Calculate cumulative bucket counts
    let bucket_counts: Vec<u64> = (0..7)
        .map(|i| m.latency_buckets[i].load(Ordering::Relaxed))
        .collect();

    let cumulative_10 = bucket_counts[0];
    let cumulative_50 = cumulative_10 + bucket_counts[1];
    let cumulative_100 = cumulative_50 + bucket_counts[2];
    let cumulative_250 = cumulative_100 + bucket_counts[3];
    let cumulative_500 = cumulative_250 + bucket_counts[4];
    let cumulative_1000 = cumulative_500 + bucket_counts[5];
    let cumulative_inf = cumulative_1000 + bucket_counts[6];

    // Total request count equals sum of all buckets
    let histogram_count = cumulative_inf;

    // Build Prometheus exposition format
    let output = format!(
        r#"# HELP investiq_requests_total Total HTTP requests
# TYPE investiq_requests_total counter
investiq_requests_total {}

# HELP investiq_errors_total Total HTTP 5xx errors
# TYPE investiq_errors_total counter
investiq_errors_total {}

# HELP investiq_active_connections Current active HTTP connections
# TYPE investiq_active_connections gauge
investiq_active_connections {}

# HELP investiq_analysis_total Total analysis requests
# TYPE investiq_analysis_total counter
investiq_analysis_total {}

# HELP investiq_trades_total Total trade executions
# TYPE investiq_trades_total counter
investiq_trades_total {}

# HELP investiq_request_duration_milliseconds HTTP request latency histogram
# TYPE investiq_request_duration_milliseconds histogram
investiq_request_duration_milliseconds_bucket{{le="10"}} {}
investiq_request_duration_milliseconds_bucket{{le="50"}} {}
investiq_request_duration_milliseconds_bucket{{le="100"}} {}
investiq_request_duration_milliseconds_bucket{{le="250"}} {}
investiq_request_duration_milliseconds_bucket{{le="500"}} {}
investiq_request_duration_milliseconds_bucket{{le="1000"}} {}
investiq_request_duration_milliseconds_bucket{{le="+Inf"}} {}
investiq_request_duration_milliseconds_count {}
investiq_request_duration_milliseconds_sum 0
"#,
        request_count,
        error_count,
        active_connections,
        analysis_count,
        trade_count,
        cumulative_10,
        cumulative_50,
        cumulative_100,
        cumulative_250,
        cumulative_500,
        cumulative_1000,
        cumulative_inf,
        histogram_count,
    );

    (
        StatusCode::OK,
        [(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        output,
    )
}

/// JSON metrics endpoint for dashboard
async fn metrics_json_endpoint(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let m = &state.metrics;
    let buckets = ["<10ms", "<50ms", "<100ms", "<250ms", "<500ms", "<1000ms", ">=1000ms"];
    let latency: serde_json::Value = buckets.iter().enumerate().map(|(i, label)| {
        (label.to_string(), serde_json::Value::from(m.latency_buckets[i].load(Ordering::Relaxed)))
    }).collect::<serde_json::Map<String, serde_json::Value>>().into();

    Json(serde_json::json!({
        "request_count": m.request_count.load(Ordering::Relaxed),
        "error_count": m.error_count.load(Ordering::Relaxed),
        "active_connections": m.active_connections.load(Ordering::Relaxed),
        "analysis_count": m.analysis_count.load(Ordering::Relaxed),
        "trade_count": m.trade_count.load(Ordering::Relaxed),
        "latency_histogram": latency,
    }))
}

async fn health_check(
    State(state): State<AppState>,
) -> impl IntoResponse {
    #[derive(Serialize)]
    struct DepCheck {
        status: String,
        latency_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    }

    #[derive(Serialize)]
    struct HealthResponse {
        status: String,
        service: String,
        checks: serde_json::Value,
    }

    let check_timeout = std::time::Duration::from_secs(5);
    let mut checks = serde_json::Map::new();
    let mut any_critical_down = false;
    let mut any_optional_down = false;

    // Critical: Database
    {
        let start = std::time::Instant::now();
        let db_ok = if let Some(pm) = state.portfolio_manager.as_ref() {
            match tokio::time::timeout(check_timeout, async {
                sqlx::query("SELECT 1")
                    .execute(pm.db().pool())
                    .await
            }).await {
                Ok(Ok(_)) => true,
                _ => false,
            }
        } else {
            false
        };
        let latency = start.elapsed().as_millis() as u64;
        if !db_ok { any_critical_down = true; }
        checks.insert("database".to_string(), serde_json::to_value(DepCheck {
            status: if db_ok { "ok".to_string() } else { "down".to_string() },
            latency_ms: latency,
            error: if db_ok { None } else { Some("Database unreachable".to_string()) },
        }).unwrap());
    }

    // Critical: Polygon (SPY snapshot)
    {
        let start = std::time::Instant::now();
        let polygon_ok = match tokio::time::timeout(check_timeout,
            state.orchestrator.polygon_client.get_snapshot("SPY")
        ).await {
            Ok(Ok(_)) => true,
            _ => false,
        };
        let latency = start.elapsed().as_millis() as u64;
        if !polygon_ok { any_critical_down = true; }
        checks.insert("polygon".to_string(), serde_json::to_value(DepCheck {
            status: if polygon_ok { "ok".to_string() } else { "down".to_string() },
            latency_ms: latency,
            error: if polygon_ok { None } else { Some("Polygon API unreachable".to_string()) },
        }).unwrap());
    }

    // Optional: Redis
    {
        let start = std::time::Instant::now();
        let redis_ok = match &state.cache {
            CacheBackend::Redis(conn) => {
                let mut conn = conn.clone();
                match tokio::time::timeout(check_timeout, async {
                    redis::cmd("PING").query_async::<String>(&mut conn).await
                }).await {
                    Ok(Ok(_)) => true,
                    _ => false,
                }
            }
            CacheBackend::Memory(_) => true, // In-memory is always "up"
        };
        let latency = start.elapsed().as_millis() as u64;
        if !redis_ok { any_optional_down = true; }
        let status_str = match &state.cache {
            CacheBackend::Memory(_) => "ok (in-memory)".to_string(),
            _ => if redis_ok { "ok".to_string() } else { "down".to_string() },
        };
        checks.insert("redis".to_string(), serde_json::to_value(DepCheck {
            status: status_str,
            latency_ms: latency,
            error: if redis_ok { None } else { Some("Redis unreachable".to_string()) },
        }).unwrap());
    }

    // Optional: Alpaca
    {
        let start = std::time::Instant::now();
        let alpaca_ok = if let Some(alpaca) = state.alpaca_client.as_ref() {
            match tokio::time::timeout(check_timeout, alpaca.get_account()).await {
                Ok(Ok(_)) => true,
                _ => false,
            }
        } else {
            true // Not configured = not a problem
        };
        let latency = start.elapsed().as_millis() as u64;
        if !alpaca_ok { any_optional_down = true; }
        let status_str = if state.alpaca_client.is_none() {
            "not_configured".to_string()
        } else if alpaca_ok {
            "ok".to_string()
        } else {
            "down".to_string()
        };
        checks.insert("alpaca".to_string(), serde_json::to_value(DepCheck {
            status: status_str,
            latency_ms: latency,
            error: if alpaca_ok || state.alpaca_client.is_none() { None } else { Some("Alpaca unreachable".to_string()) },
        }).unwrap());
    }

    // Optional: ML service
    {
        let start = std::time::Instant::now();
        let ml_url = std::env::var("ML_SIGNAL_MODELS_URL")
            .unwrap_or_else(|_| "http://localhost:8004".to_string());
        let ml_ok = match tokio::time::timeout(
            std::time::Duration::from_secs(3),
            reqwest::get(format!("{}/health", ml_url)),
        ).await {
            Ok(Ok(resp)) => resp.status().is_success(),
            _ => false,
        };
        let latency = start.elapsed().as_millis() as u64;
        if !ml_ok { any_optional_down = true; }
        checks.insert("ml_service".to_string(), serde_json::to_value(DepCheck {
            status: if ml_ok { "ok".to_string() } else { "down".to_string() },
            latency_ms: latency,
            error: if ml_ok { None } else { Some("ML service unreachable".to_string()) },
        }).unwrap());
    }

    let overall_status = if any_critical_down {
        "unhealthy"
    } else if any_optional_down {
        "degraded"
    } else {
        "healthy"
    };

    let status_code = if any_critical_down {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };

    let response = HealthResponse {
        status: overall_status.to_string(),
        service: "invest-iq-api".to_string(),
        checks: serde_json::Value::Object(checks),
    };

    (status_code, Json(response))
}

async fn analyze_symbol(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<AnalyzeQuery>,
) -> Result<Json<ApiResponse<UnifiedAnalysis>>, AppError> {
    let timeframe = parse_timeframe(query.timeframe.as_deref());
    let days = query.days.unwrap_or(365);
    let analysis = get_cached_analysis(&state, &symbol, timeframe, days).await?;
    Ok(Json(ApiResponse::success(analysis)))
}

async fn get_bars(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<BarsQuery>,
) -> Result<Json<ApiResponse<Vec<analysis_core::Bar>>>, AppError> {
    let symbol = symbol.to_uppercase();

    let timeframe = match query.timeframe.as_deref() {
        Some("1m") => Timeframe::Minute1,
        Some("5m") => Timeframe::Minute5,
        Some("15m") => Timeframe::Minute15,
        Some("30m") => Timeframe::Minute30,
        Some("1h") => Timeframe::Hour1,
        Some("4h") => Timeframe::Hour4,
        Some("1w") => Timeframe::Week1,
        Some("1M") => Timeframe::Month1,
        _ => Timeframe::Day1,
    };

    let days = query.days.unwrap_or(90);

    tracing::info!(
        "📊 Fetching bars for {} (timeframe: {:?}, days: {})",
        symbol,
        timeframe,
        days
    );

    let bars = state.orchestrator.get_bars(&symbol, timeframe, days).await?;

    Ok(Json(ApiResponse::success(bars)))
}

async fn get_ticker(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<polygon_client::TickerDetails>>, AppError> {
    let symbol = symbol.to_uppercase();

    tracing::info!("🔍 Fetching ticker details for {}", symbol);

    let details = state.orchestrator.get_ticker_details(&symbol).await?;

    Ok(Json(ApiResponse::success(details)))
}

#[derive(Deserialize)]
struct BacktestQuery {
    days: Option<i64>,
}

#[derive(Deserialize)]
struct SuggestQuery {
    universe: Option<String>,
    min_confidence: Option<f64>,
    min_signal: Option<i32>,
    limit: Option<usize>,
}

async fn suggest_stocks(
    State(state): State<AppState>,
    Query(query): Query<SuggestQuery>,
) -> Result<Json<ApiResponse<ScreenerResult>>, AppError> {
    // Parse universe
    let universe = match query.universe.as_deref() {
        Some("tech") => StockUniverse::TechStocks,
        Some("bluechip") => StockUniverse::BlueChips,
        Some("popular") | None => StockUniverse::PopularStocks,
        Some(custom) => {
            // Support custom comma-separated list
            let symbols: Vec<String> = custom
                .split(',')
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty())
                .collect();

            if symbols.is_empty() {
                StockUniverse::PopularStocks
            } else {
                StockUniverse::Custom(symbols)
            }
        }
    };

    // Build filters
    let filters = ScreenerFilters {
        min_confidence: query.min_confidence.unwrap_or(0.5),
        min_signal_strength: query.min_signal.unwrap_or(0), // 0 = Neutral or better
        limit: query.limit.unwrap_or(10),
    };

    tracing::info!(
        "📊 Stock screening request - Universe: {:?}, Filters: min_conf={}, min_signal={}, limit={}",
        universe,
        filters.min_confidence,
        filters.min_signal_strength,
        filters.limit
    );

    // Run screener
    let result = state.screener.screen(universe, filters).await?;

    Ok(Json(ApiResponse::success(result)))
}

async fn validate_analysis(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<ComparisonResult>>, AppError> {
    let symbol = symbol.to_uppercase();

    // Check if validation is enabled
    let comparison_engine = state.comparison_engine.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Validation not enabled. Set ALPHA_VANTAGE_API_KEY."))?;

    tracing::info!("🔍 Validating analysis for {}", symbol);

    // Get our analysis (uses shared cache)
    let analysis = get_default_analysis(&state, &symbol).await?;

    // Extract values for comparison
    let our_rsi = analysis.technical.as_ref()
        .and_then(|t| t.metrics.get("rsi"))
        .and_then(|v| v.as_f64());

    let our_macd = analysis.technical.as_ref()
        .and_then(|t| {
            let macd = t.metrics.get("macd")?.as_f64()?;
            let signal = t.metrics.get("macd_signal")?.as_f64()?;
            let hist = t.metrics.get("macd_histogram")?.as_f64()?;
            Some((macd, signal, hist))
        });

    let our_sma_20 = analysis.technical.as_ref()
        .and_then(|t| t.metrics.get("sma_20"))
        .and_then(|v| v.as_f64());

    let our_pe = analysis.fundamental.as_ref()
        .and_then(|f| f.metrics.get("pe_ratio"))
        .and_then(|v| v.as_f64());

    let our_roe = analysis.fundamental.as_ref()
        .and_then(|f| f.metrics.get("roe"))
        .and_then(|v| v.as_f64());

    let our_profit_margin = analysis.fundamental.as_ref()
        .and_then(|f| f.metrics.get("profit_margin"))
        .and_then(|v| v.as_f64());

    let our_debt_to_equity = analysis.fundamental.as_ref()
        .and_then(|f| f.metrics.get("debt_to_equity"))
        .and_then(|v| v.as_f64());

    let our_beta = analysis.quantitative.as_ref()
        .and_then(|q| q.metrics.get("beta"))
        .and_then(|v| v.as_f64());

    // Perform comparison
    let comparison = comparison_engine.full_comparison(
        &symbol,
        our_rsi,
        our_macd,
        our_sma_20,
        our_pe,
        our_roe,
        our_profit_margin,
        our_debt_to_equity,
        our_beta,
    ).await?;

    Ok(Json(ApiResponse::success(comparison)))
}

/// Combine point-in-time signals from technical and quant engines.
/// Weighted: technical 60%, quant 40%.
pub(crate) fn combine_pit_signals(
    tech: &Option<analysis_core::AnalysisResult>,
    quant: &Option<analysis_core::AnalysisResult>,
) -> (analysis_core::SignalStrength, f64) {
    let (w_tech, w_quant) = (60i32, 40i32);
    let mut total_score = 0i32;
    let mut total_weight = 0i32;
    let mut combined_confidence = 0.0f64;

    if let Some(t) = tech {
        total_score += t.signal.to_score() * w_tech;
        total_weight += w_tech;
        combined_confidence += t.confidence * (w_tech as f64 / 100.0);
    }
    if let Some(q) = quant {
        total_score += q.signal.to_score() * w_quant;
        total_weight += w_quant;
        combined_confidence += q.confidence * (w_quant as f64 / 100.0);
    }

    let signal = if total_weight > 0 {
        analysis_core::SignalStrength::from_score((total_score as f64 / total_weight as f64) as i32)
    } else {
        analysis_core::SignalStrength::Neutral
    };

    (signal, combined_confidence)
}

/// Convert SignalStrength to a display name for trades.
fn signal_to_display(signal: &analysis_core::SignalStrength) -> &'static str {
    match signal {
        analysis_core::SignalStrength::StrongBuy => "StrongBuy",
        analysis_core::SignalStrength::Buy => "Buy",
        analysis_core::SignalStrength::WeakBuy => "WeakBuy",
        analysis_core::SignalStrength::StrongSell => "StrongSell",
        analysis_core::SignalStrength::Sell => "Sell",
        analysis_core::SignalStrength::WeakSell => "WeakSell",
        analysis_core::SignalStrength::Neutral => "Neutral",
    }
}

/// Convert SignalStrength to buy/sell/hold action.
fn signal_to_action(signal: &analysis_core::SignalStrength) -> &'static str {
    match signal {
        analysis_core::SignalStrength::StrongBuy
        | analysis_core::SignalStrength::Buy
        | analysis_core::SignalStrength::WeakBuy => "buy",
        analysis_core::SignalStrength::StrongSell
        | analysis_core::SignalStrength::Sell
        | analysis_core::SignalStrength::WeakSell => "sell",
        analysis_core::SignalStrength::Neutral => "hold",
    }
}

async fn backtest_symbol(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<BacktestQuery>,
) -> Result<Json<ApiResponse<BtResult>>, AppError> {
    let symbol = symbol.to_uppercase();
    let days = query.days.unwrap_or(365);

    tracing::info!("Running point-in-time backtest for {} ({} days)", symbol, days);

    // Get historical bars
    let bars = state.orchestrator.get_bars(&symbol, Timeframe::Day1, days).await?;

    if bars.len() < 50 {
        return Err(anyhow::anyhow!("Not enough historical data for backtesting (need >= 50 bars)").into());
    }

    // Fetch SPY bars for benchmark (quant engine needs them)
    let spy_bars = get_cached_etf_bars(&state, "SPY", 365, 15).await;

    // Convert bars to HistoricalBar format for the engine
    let hist_bars: Vec<HistoricalBar> = bars.iter().map(|bar| HistoricalBar {
        date: bar.timestamp.format("%Y-%m-%d").to_string(),
        open: Decimal::from_f64(bar.open).unwrap_or_default(),
        high: Decimal::from_f64(bar.high).unwrap_or_default(),
        low: Decimal::from_f64(bar.low).unwrap_or_default(),
        close: Decimal::from_f64(bar.close).unwrap_or_default(),
        volume: bar.volume,
    }).collect();

    let first_date = hist_bars.first().map(|b| b.date.clone()).unwrap_or_default();
    let last_date = hist_bars.last().map(|b| b.date.clone()).unwrap_or_default();

    // Point-in-time signal generation
    let mut signals: Vec<BtSignal> = Vec::new();
    let sample_interval: usize = if days > 180 { 5 } else { 1 };
    let tech_engine = state.orchestrator.technical_engine();
    let quant_engine = state.orchestrator.quant_engine();

    for i in (50..bars.len()).step_by(sample_interval) {
        let bar_slice = &bars[..i];
        let bar = &bars[i];

        let tech_result = tech_engine.analyze_enhanced(&symbol, bar_slice, None).ok();
        let quant_result = quant_engine.analyze_with_benchmark_and_rate(
            &symbol,
            bar_slice,
            if spy_bars.len() >= 30 { Some(&spy_bars) } else { None },
            None,
        ).ok();

        let (signal_strength, confidence) = combine_pit_signals(&tech_result, &quant_result);
        let action = signal_to_action(&signal_strength);

        if action != "hold" {
            signals.push(BtSignal {
                date: bar.timestamp.format("%Y-%m-%d").to_string(),
                symbol: symbol.clone(),
                signal_type: signal_to_display(&signal_strength).to_string(),
                confidence,
                price: Decimal::from_f64(bar.close).unwrap_or_default(),
                reason: format!("{:?} signal at {:.0}% confidence (point-in-time)",
                    signal_strength, confidence * 100.0),
                order_type: None,
                limit_price: None,
                limit_expiry_bars: None,
            });
        }
    }

    if signals.is_empty() {
        return Err(anyhow::anyhow!("No signals generated during backtest period").into());
    }

    // Convert SPY bars for benchmark comparison
    let benchmark_bars = if spy_bars.len() >= 30 {
        Some(spy_bars.iter().map(|b| HistoricalBar {
            date: b.timestamp.format("%Y-%m-%d").to_string(),
            open: Decimal::from_f64(b.open).unwrap_or_default(),
            high: Decimal::from_f64(b.high).unwrap_or_default(),
            low: Decimal::from_f64(b.low).unwrap_or_default(),
            close: Decimal::from_f64(b.close).unwrap_or_default(),
            volume: b.volume,
        }).collect())
    } else {
        None
    };

    // Build config and run the unified backtest engine
    let config = BacktestConfig {
        strategy_name: format!("Backtest-{}", symbol),
        symbols: vec![symbol.clone()],
        start_date: first_date,
        end_date: last_date,
        initial_capital: Decimal::new(10000, 0),
        position_size_percent: 95.0,
        stop_loss_percent: Some(0.05),
        take_profit_percent: Some(0.15),
        confidence_threshold: 0.5,
        commission_rate: Some(0.001),
        slippage_rate: Some(0.0005),
        max_volume_participation: Some(0.05),
        benchmark_bars,
        allocation_strategy: None,
        symbol_weights: None,
        rebalance_interval_days: None,
        allow_short_selling: None,
        margin_multiplier: None,
        signal_timeframe: None,
        trailing_stop_percent: None,
        max_drawdown_halt_percent: None,
        regime_config: None,
        commission_model: None,
        allow_fractional_shares: None,
        cash_sweep_rate: None,
        incremental_rebalance: None,
        param_search_space: None,
        market_impact: None,
    };

    let mut historical_data = std::collections::HashMap::new();
    historical_data.insert(symbol.clone(), hist_bars);

    let mut engine = BtEngine::new(config);
    let mut result = engine.run(historical_data, signals)
        .map_err(|e| anyhow::anyhow!("Backtest engine error: {}", e))?;

    // Save to database if available
    if let Some(backtest_db) = state.backtest_db.as_ref() {
        match backtest_db.save_backtest(&result).await {
            Ok(id) => {
                tracing::info!("Backtest saved to DB (id: {}). Setting result.id.", id);
                result.id = Some(id);
            }
            Err(e) => tracing::warn!("Failed to save backtest to DB: {}", e),
        }
    } else {
        tracing::warn!("No backtest_db available — result.id will be null");
    }
    tracing::info!("Returning backtest result with id: {:?}", result.id);

    // Auto-record strategy snapshot for alpha decay monitoring
    if let Some(pm) = state.portfolio_manager.as_ref() {
        let monitor = alpha_decay::AlphaDecayMonitor::new(pm.db().pool().clone());
        let snapshot = alpha_decay::PerformanceSnapshot {
            id: None,
            strategy_name: result.strategy_name.clone(),
            snapshot_date: chrono::Utc::now().date_naive(),
            rolling_sharpe: result.sharpe_ratio.unwrap_or(0.0),
            win_rate: result.win_rate,
            profit_factor: result.profit_factor.unwrap_or(0.0),
            trades_count: result.total_trades,
            cumulative_return: result.total_return_percent,
            max_drawdown: result.max_drawdown.unwrap_or(0.0),
            created_at: None,
        };
        if let Err(e) = monitor.record_snapshot(&snapshot).await {
            tracing::warn!("Failed to record strategy snapshot: {}", e);
        }
    }

    Ok(Json(ApiResponse::success(result)))
}

// Error handling
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("API error: {:#}", self.0);

        let is_production = std::env::var("RUST_ENV")
            .map(|v| v == "production")
            .unwrap_or(false);
        let message = if is_production {
            "Internal server error".to_string()
        } else {
            self.0.to_string()
        };

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(message)),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    /// Build a minimal test router with in-memory SQLite
    async fn create_test_app() -> Router<()> {
        // Set a test API key
        std::env::set_var("API_KEYS", "test-key-123");

        let orchestrator = Arc::new(AnalysisOrchestrator::new("dummy-polygon-key".to_string()));
        let screener = Arc::new(StockScreener::new(Arc::clone(&orchestrator)));

        // In-memory SQLite
        sqlx::any::install_default_drivers();
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");

        // Create minimal tables
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS risk_parameters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                max_risk_per_trade_percent REAL NOT NULL DEFAULT 2.0,
                max_portfolio_risk_percent REAL NOT NULL DEFAULT 10.0,
                max_position_size_percent REAL NOT NULL DEFAULT 20.0,
                default_stop_loss_percent REAL NOT NULL DEFAULT 5.0,
                default_take_profit_percent REAL NOT NULL DEFAULT 10.0,
                trailing_stop_enabled INTEGER NOT NULL DEFAULT 0,
                trailing_stop_percent REAL NOT NULL DEFAULT 3.0,
                min_confidence_threshold REAL NOT NULL DEFAULT 0.70,
                min_win_rate_threshold REAL NOT NULL DEFAULT 0.55,
                daily_loss_limit_percent REAL NOT NULL DEFAULT 5.0,
                max_consecutive_losses INTEGER NOT NULL DEFAULT 3,
                account_drawdown_limit_percent REAL NOT NULL DEFAULT 10.0,
                trading_halted INTEGER NOT NULL DEFAULT 0,
                halt_reason TEXT,
                halted_at TEXT,
                updated_at TEXT DEFAULT (datetime('now'))
            )"
        ).execute(&pool).await.unwrap();

        // Migrate: add circuit breaker columns if missing (pre-existing DBs)
        for col_def in &[
            ("daily_loss_limit_percent", "REAL NOT NULL DEFAULT 5.0"),
            ("max_consecutive_losses", "INTEGER NOT NULL DEFAULT 3"),
            ("account_drawdown_limit_percent", "REAL NOT NULL DEFAULT 10.0"),
            ("trading_halted", "INTEGER NOT NULL DEFAULT 0"),
            ("halt_reason", "TEXT"),
            ("halted_at", "TEXT"),
        ] {
            let sql = format!(
                "ALTER TABLE risk_parameters ADD COLUMN {} {}",
                col_def.0, col_def.1
            );
            // Ignore "duplicate column" errors — means column already exists
            let _ = sqlx::query(&sql).execute(&pool).await;
        }

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS portfolio_peak (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                peak_value REAL NOT NULL,
                peak_date TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        ).execute(&pool).await.unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS trades (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                action TEXT NOT NULL,
                shares REAL NOT NULL,
                price REAL NOT NULL,
                timestamp TEXT DEFAULT (datetime('now'))
            )"
        ).execute(&pool).await.unwrap();

        let rm = Arc::new(RiskManager::new(pool.clone()));
        let metrics = Arc::new(Metrics::new());

        let state = AppState {
            orchestrator,
            screener,
            cache: CacheBackend::Memory(Arc::new(DashMap::new())),
            etf_bar_cache: Arc::new(DashMap::new()),
            comparison_engine: None,
            portfolio_manager: None,
            trade_logger: None,
            alert_manager: None,
            alpaca_client: None,
            risk_manager: Some(rm),
            backtest_db: None,
            performance_tracker: None,
            signal_analyzer: None,
            metrics,
            brute_force_guard: Arc::new(brute_force::BruteForceGuard::new()),
            ip_allowlist: None,
        };

        let cors = CorsLayer::permissive();

        Router::new()
            .route("/", get(health_check))
            .route("/health", get(health_check))
            .route("/metrics", get(metrics_endpoint))
            .route("/metrics/json", get(metrics_json_endpoint))
            .route("/api/analyze/:symbol", get(analyze_symbol))
            .merge(broker_routes::broker_read_routes())
            .merge(broker_routes::broker_write_routes())
            .layer(
                ServiceBuilder::new()
                    .layer(axum::extract::DefaultBodyLimit::max(1_048_576))
                    .layer(TraceLayer::new_for_http())
                    .layer(middleware::from_fn_with_state(state.clone(), metrics_middleware))
                    .layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware))
                    .layer(cors)
            )
            .with_state(state)
    }

    #[tokio::test]
    async fn health_bypasses_auth() {
        let app = create_test_app().await;
        let resp = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        // Health endpoint may return 503 if deps are down, but it should NOT
        // return 401/403 (auth should be bypassed).
        assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_ne!(resp.status(), StatusCode::FORBIDDEN);
        // Verify it returns valid JSON with a status field
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("status").is_some());
    }

    #[tokio::test]
    async fn metrics_bypasses_auth() {
        let app = create_test_app().await;
        let resp = app
            .oneshot(Request::builder().uri("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        // Verify Prometheus format
        assert!(text.contains("# HELP investiq_requests_total"));
        assert!(text.contains("# TYPE investiq_requests_total counter"));
        assert!(text.contains("investiq_active_connections"));
    }

    #[tokio::test]
    async fn api_requires_auth() {
        let app = create_test_app().await;
        let resp = app
            .oneshot(Request::builder().uri("/api/analyze/AAPL").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn invalid_key_rejected() {
        let app = create_test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/analyze/AAPL")
                    .header("X-API-Key", "wrong-key-456")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn broker_write_requires_live_key() {
        // Set ALPACA_BASE_URL to a non-paper URL so live_trading_auth_middleware
        // checks for LIVE_TRADING_KEY (which is not set → 403).
        std::env::set_var("ALPACA_BASE_URL", "https://api.alpaca.markets");
        std::env::remove_var("LIVE_TRADING_KEY");
        let app = create_test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/broker/execute")
                    .header("X-API-Key", "test-key-123")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"symbol":"AAPL","action":"buy","shares":1}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        // 403 because LIVE_TRADING_KEY is not set and URL is live
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        // Clean up
        std::env::remove_var("ALPACA_BASE_URL");
    }

    #[tokio::test]
    async fn combine_pit_signals_buy_sell() {
        use analysis_core::{AnalysisResult, SignalStrength};

        let now = chrono::Utc::now();

        // StrongBuy tech (weight 60) + Sell quant (weight 40)
        let tech = Some(AnalysisResult {
            symbol: "TEST".into(),
            timestamp: now,
            signal: SignalStrength::StrongBuy,
            confidence: 0.9,
            reason: "test".into(),
            metrics: serde_json::json!({}),
        });
        let quant = Some(AnalysisResult {
            symbol: "TEST".into(),
            timestamp: now,
            signal: SignalStrength::Sell,
            confidence: 0.7,
            reason: "test".into(),
            metrics: serde_json::json!({}),
        });

        let (signal, confidence) = combine_pit_signals(&tech, &quant);
        assert!(confidence > 0.0);
        // StrongBuy=2*60=120, Sell=-1*40=-40 → net 80/100=0.8 → WeakBuy (score 1)
        assert!(signal != SignalStrength::Sell && signal != SignalStrength::StrongSell);

        // Both Neutral
        let tech_n = Some(AnalysisResult {
            symbol: "TEST".into(),
            timestamp: now,
            signal: SignalStrength::Neutral,
            confidence: 0.5,
            reason: "test".into(),
            metrics: serde_json::json!({}),
        });
        let quant_n = Some(AnalysisResult {
            symbol: "TEST".into(),
            timestamp: now,
            signal: SignalStrength::Neutral,
            confidence: 0.5,
            reason: "test".into(),
            metrics: serde_json::json!({}),
        });
        let (signal2, _) = combine_pit_signals(&tech_n, &quant_n);
        assert_eq!(signal2, SignalStrength::Neutral);

        // None inputs
        let (signal3, conf3) = combine_pit_signals(&None, &None);
        assert_eq!(signal3, SignalStrength::Neutral);
        assert_eq!(conf3, 0.0);
    }
}
