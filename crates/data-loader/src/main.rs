//! data-loader: Fetch historical data from Polygon and generate ML training features.
//!
//! For each symbol, fetches 365 days of bars + financials, then slides a window
//! to produce (features, forward_return) pairs written to the `analysis_features` table.
//!
//! Usage:
//!   cargo run -p data-loader -- --symbols AAPL MSFT GOOGL
//!   cargo run -p data-loader -- --all          # all 150 default symbols
//!   cargo run -p data-loader -- --all --dry-run
//!   cargo run -p data-loader -- --fetch-tickers --limit 3000

use analysis_core::{
    AnalysisResult, AnalystConsensusData, Bar,
    SignalStrength,
};
use chrono::{Duration, Utc};
use fundamental_analysis::FundamentalAnalysisEngine;
use polygon_client::PolygonClient;
use quant_analysis::QuantAnalysisEngine;
use technical_analysis::TechnicalAnalysisEngine;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Semaphore;

const DEFAULT_SYMBOLS: &[&str] = &[
    // Technology (20)
    "AAPL", "MSFT", "GOOGL", "NVDA", "META", "AVGO", "TSM", "ORCL", "CRM", "AMD",
    "ADBE", "INTC", "CSCO", "QCOM", "TXN", "NOW", "IBM", "AMAT", "MU", "SNPS",
    // Healthcare (15)
    "JNJ", "UNH", "PFE", "ABBV", "MRK", "LLY", "TMO", "ABT", "DHR", "BMY",
    "AMGN", "GILD", "MDT", "ISRG", "VRTX",
    // Financials (15)
    "JPM", "BAC", "GS", "V", "MA", "BRK.B", "WFC", "MS", "AXP", "SCHW",
    "BLK", "C", "CB", "MMC", "ICE",
    // Energy (10)
    "XOM", "CVX", "COP", "SLB", "EOG", "MPC", "PSX", "VLO", "OXY", "HAL",
    // Consumer Discretionary (15)
    "AMZN", "TSLA", "HD", "NKE", "SBUX", "MCD", "LOW", "TJX", "BKNG", "CMG",
    "ORLY", "ROST", "DHI", "LEN", "GM",
    // Industrials (15)
    "CAT", "BA", "HON", "UPS", "GE", "RTX", "DE", "LMT", "UNP", "ETN",
    "WM", "EMR", "ITW", "FDX", "NSC",
    // Utilities (8)
    "NEE", "DUK", "SO", "AEP", "D", "SRE", "EXC", "XEL",
    // Materials (8)
    "LIN", "APD", "ECL", "SHW", "NEM", "FCX", "DOW", "NUE",
    // Real Estate (8)
    "AMT", "PLD", "CCI", "EQIX", "SPG", "PSA", "O", "DLR",
    // Communications (10)
    "NFLX", "DIS", "CMCSA", "T", "VZ", "TMUS", "CHTR", "EA", "TTWO", "WBD",
    // Consumer Staples (10)
    "PG", "KO", "PEP", "COST", "WMT", "PM", "MO", "CL", "KHC", "GIS",
    // Mid-caps / high-vol names for diversity (16)
    "SQ", "SHOP", "SNAP", "ROKU", "DKNG", "COIN", "PLTR", "CRWD",
    "PANW", "ZS", "NET", "DDOG", "SNOW", "MELI", "SE", "UBER",
];

/// Minimum bars needed for technical analysis (50) + forward window (20)
const MIN_WINDOW: usize = 50;
const FORWARD_5D: usize = 5;
const FORWARD_20D: usize = 20;
/// Max concurrent symbol processing tasks
const DEFAULT_CONCURRENCY: usize = 20;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "data_loader=info,polygon_client=warn".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let use_all = args.iter().any(|a| a == "--all");
    let fetch_tickers = args.iter().any(|a| a == "--fetch-tickers");

    let limit: usize = args
        .iter()
        .position(|a| a == "--limit")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);

    let concurrency: usize = args
        .iter()
        .position(|a| a == "--concurrency")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CONCURRENCY);

    let db_path = args
        .iter()
        .position(|a| a == "--db")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("portfolio.db");

    let api_key = std::env::var("POLYGON_API_KEY")
        .expect("POLYGON_API_KEY must be set");

    // Default to 5000 req/min for bulk loading (Polygon recommends <6000/min on paid plans).
    // Free tier users should set POLYGON_RATE_LIMIT=5.
    if std::env::var("POLYGON_RATE_LIMIT").is_err() {
        std::env::set_var("POLYGON_RATE_LIMIT", "5000");
    }
    let polygon = Arc::new(PolygonClient::new(api_key));

    let symbols: Vec<String> = if fetch_tickers {
        tracing::info!("Fetching active US tickers from Polygon (limit: {})...", limit);
        let tickers = polygon.list_tickers(limit).await?;
        tracing::info!("Fetched {} tickers", tickers.len());
        tickers
    } else if use_all {
        DEFAULT_SYMBOLS.iter().map(|s| s.to_string()).collect()
    } else if let Some(idx) = args.iter().position(|a| a == "--symbols") {
        args[idx + 1..]
            .iter()
            .take_while(|a| !a.starts_with("--"))
            .cloned()
            .collect()
    } else {
        eprintln!("Usage:");
        eprintln!("  data-loader --fetch-tickers            Fetch all active US stocks from Polygon");
        eprintln!("  data-loader --fetch-tickers --limit N  Fetch up to N tickers (default 5000)");
        eprintln!("  data-loader --all                      Use built-in 150 symbols");
        eprintln!("  data-loader --symbols AAPL MSFT ...    Specific symbols");
        eprintln!("");
        eprintln!("Options:");
        eprintln!("  --dry-run          Print stats without writing to DB");
        eprintln!("  --db PATH          SQLite DB path (default: portfolio.db)");
        eprintln!("  --concurrency N    Max parallel symbols (default: {})", DEFAULT_CONCURRENCY);
        std::process::exit(1);
    };

    let total_symbols = symbols.len();
    tracing::info!(
        "data-loader: {} symbols, db={}, dry_run={}, concurrency={}",
        total_symbols, db_path, dry_run, concurrency
    );

    // Open DB (migrations handle table schema via sqlx::migrate!())
    let pool = Arc::new(SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path)).await?);

    // Enable WAL mode for concurrent writes from parallel tasks
    sqlx::query("PRAGMA journal_mode=WAL").execute(pool.as_ref()).await?;

    let tech_engine = Arc::new(TechnicalAnalysisEngine::new());
    let fund_engine = Arc::new(FundamentalAnalysisEngine::new());
    let quant_engine = Arc::new(QuantAnalysisEngine::new());

    // Fetch SPY bars once for benchmark
    tracing::info!("Fetching SPY bars for benchmark...");
    let now = Utc::now();
    let spy_bars = Arc::new(polygon
        .get_aggregates("SPY", 1, "day", now - Duration::days(400), now)
        .await
        .unwrap_or_default());
    tracing::info!("SPY: {} bars", spy_bars.len());

    let total_rows = Arc::new(AtomicU64::new(0));
    let completed = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let semaphore = Arc::new(Semaphore::new(concurrency));

    let mut handles = Vec::with_capacity(total_symbols);

    for symbol in symbols {
        let polygon = Arc::clone(&polygon);
        let tech_engine = Arc::clone(&tech_engine);
        let fund_engine = Arc::clone(&fund_engine);
        let quant_engine = Arc::clone(&quant_engine);
        let spy_bars = Arc::clone(&spy_bars);
        let pool = Arc::clone(&pool);
        let total_rows = Arc::clone(&total_rows);
        let completed = Arc::clone(&completed);
        let failed = Arc::clone(&failed);
        let semaphore = Arc::clone(&semaphore);

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let rows = process_symbol(
                &polygon, &tech_engine, &fund_engine, &quant_engine,
                &symbol, &spy_bars, pool.as_ref(), dry_run,
            ).await;

            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;

            match rows {
                Ok(n) => {
                    total_rows.fetch_add(n, Ordering::Relaxed);
                    tracing::info!("[{}/{}] {} => {} rows", done, total_symbols, symbol, n);
                }
                Err(e) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!("[{}/{}] {} failed: {}", done, total_symbols, symbol, e);
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        let _ = handle.await;
    }

    let rows = total_rows.load(Ordering::Relaxed);
    let fails = failed.load(Ordering::Relaxed);
    tracing::info!(
        "Done! {} total rows across {} symbols ({} failed)",
        rows, total_symbols, fails
    );
    Ok(())
}

async fn process_symbol(
    polygon: &PolygonClient,
    tech_engine: &TechnicalAnalysisEngine,
    fund_engine: &FundamentalAnalysisEngine,
    quant_engine: &QuantAnalysisEngine,
    symbol: &str,
    spy_bars: &[Bar],
    pool: &SqlitePool,
    dry_run: bool,
) -> anyhow::Result<u64> {
    let now = Utc::now();
    let start = now - Duration::days(400);

    // Fetch bars + financials concurrently
    let (bars_result, fin_result) = tokio::join!(
        polygon.get_aggregates(symbol, 1, "day", start, now),
        polygon.get_financials(symbol),
    );

    let bars = bars_result?;
    let financials = fin_result.unwrap_or_default();

    if bars.len() < MIN_WINDOW + FORWARD_20D {
        tracing::warn!("  {} only has {} bars, need {}", symbol, bars.len(), MIN_WINDOW + FORWARD_20D);
        return Ok(0);
    }

    // Slide a window: for each position t (from MIN_WINDOW to len - FORWARD_20D),
    // run engines on bars[..t], compute forward returns from bars[t+5] and bars[t+20]
    let max_t = bars.len() - FORWARD_20D;
    let mut count = 0u64;

    // Step by 5 days to avoid excessive overlap while still getting good coverage
    let step = 5;
    let mut t = MIN_WINDOW;

    while t < max_t {
        let window = &bars[..t];
        let current_price = bars[t - 1].close;
        let date = bars[t - 1].timestamp.format("%Y-%m-%dT%H:%M:%S").to_string();

        // Forward returns
        let fwd_5d_idx = (t + FORWARD_5D - 1).min(bars.len() - 1);
        let fwd_20d_idx = (t + FORWARD_20D - 1).min(bars.len() - 1);
        let return_5d = (bars[fwd_5d_idx].close - current_price) / current_price * 100.0;
        let return_20d = (bars[fwd_20d_idx].close - current_price) / current_price * 100.0;

        // Run analysis engines
        let tech = tech_engine.analyze_enhanced(symbol, window).ok();
        let quant = quant_engine
            .analyze_with_benchmark_and_rate(symbol, window, Some(spy_bars), None)
            .ok();

        // Fundamental: uses financials + current price (same for whole period)
        let empty_consensus = AnalystConsensusData {
            consensus: None,
            recent_ratings: Vec::new(),
        };
        let fund = if !financials.is_empty() {
            fund_engine
                .analyze_with_consensus(symbol, &financials, Some(current_price), None, &empty_consensus, None)
                .ok()
        } else {
            None
        };

        // Build feature vector
        let features = build_features(&tech, &fund, &quant);
        let features_json = serde_json::to_string(&features)?;

        // Combine for overall signal (simplified weighted average)
        let (overall_signal, overall_confidence) = combine_simple(&tech, &fund, &quant);

        if !dry_run {
            sqlx::query(
                "INSERT INTO analysis_features (symbol, analysis_date, features_json, overall_signal, overall_confidence, actual_return_5d, actual_return_20d, evaluated) VALUES (?, ?, ?, ?, ?, ?, ?, 1)"
            )
            .bind(symbol)
            .bind(&date)
            .bind(&features_json)
            .bind(format!("{:?}", overall_signal))
            .bind(overall_confidence)
            .bind(return_5d)
            .bind(return_20d)
            .execute(pool)
            .await?;
        }

        count += 1;
        t += step;
    }

    Ok(count)
}

fn build_features(
    tech: &Option<AnalysisResult>,
    fund: &Option<AnalysisResult>,
    quant: &Option<AnalysisResult>,
) -> HashMap<String, f64> {
    let mut f = HashMap::new();

    // Helper to extract metrics
    fn metric(result: &Option<AnalysisResult>, key: &str) -> f64 {
        result
            .as_ref()
            .and_then(|r| r.metrics.as_object())
            .and_then(|m| m.get(key))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    }

    // Engine signals (4) — sentiment is 0 since we don't have historical news
    f.insert("technical_score".into(), tech.as_ref().map(|r| r.signal.to_score() as f64).unwrap_or(0.0));
    f.insert("fundamental_score".into(), fund.as_ref().map(|r| r.signal.to_score() as f64).unwrap_or(0.0));
    f.insert("quant_score".into(), quant.as_ref().map(|r| r.signal.to_score() as f64).unwrap_or(0.0));
    f.insert("sentiment_score".into(), 0.0);

    // Engine confidences (4)
    f.insert("technical_confidence".into(), tech.as_ref().map(|r| r.confidence).unwrap_or(0.0));
    f.insert("fundamental_confidence".into(), fund.as_ref().map(|r| r.confidence).unwrap_or(0.0));
    f.insert("quant_confidence".into(), quant.as_ref().map(|r| r.confidence).unwrap_or(0.0));
    f.insert("sentiment_confidence".into(), 0.0);

    // Technical metrics (4)
    f.insert("rsi".into(), metric(tech, "rsi"));
    f.insert("bb_percent_b".into(), metric(tech, "bb_percent_b"));
    f.insert("adx".into(), metric(tech, "adx"));
    let sma20 = metric(tech, "sma_20");
    let sma50 = metric(tech, "sma_50");
    f.insert("sma_20_vs_50".into(), if sma20 > sma50 { 1.0 } else if sma50 > sma20 { -1.0 } else { 0.0 });

    // Fundamental metrics (4)
    f.insert("pe_ratio".into(), metric(fund, "pe_ratio"));
    f.insert("debt_to_equity".into(), metric(fund, "debt_to_equity"));
    f.insert("revenue_growth".into(), metric(fund, "revenue_growth"));
    f.insert("roic".into(), metric(fund, "roic"));

    // Quant metrics (4)
    f.insert("sharpe_ratio".into(), metric(quant, "sharpe_ratio"));
    f.insert("volatility".into(), metric(quant, "volatility"));
    f.insert("max_drawdown".into(), metric(quant, "max_drawdown"));
    f.insert("beta".into(), metric(quant, "beta"));

    // Sentiment metrics (3) — zeroed, no historical news
    f.insert("normalized_sentiment_score".into(), 0.0);
    f.insert("article_count".into(), 0.0);
    f.insert("direct_mention_ratio".into(), 0.0);

    // Market context (3)
    let vol = metric(quant, "volatility");
    f.insert("market_regime_encoded".into(), if vol > 0.3 { 1.0 } else if vol < 0.1 { -1.0 } else { 0.0 });

    let scores = [
        f["technical_score"], f["fundamental_score"], f["quant_score"],
    ];
    let active: Vec<f64> = scores.iter().copied().filter(|s| *s != 0.0).collect();
    let agreement = if active.len() >= 2 {
        let mean = active.iter().sum::<f64>() / active.len() as f64;
        (active.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / active.len() as f64).sqrt()
    } else {
        0.0
    };
    f.insert("inter_engine_agreement".into(), agreement);
    f.insert("vix_proxy".into(), vol);

    f
}

fn combine_simple(
    tech: &Option<AnalysisResult>,
    fund: &Option<AnalysisResult>,
    quant: &Option<AnalysisResult>,
) -> (SignalStrength, f64) {
    let mut total_score = 0i64;
    let mut total_weight = 0i64;
    let mut confidence = 0.0f64;

    if let Some(t) = tech {
        total_score += t.signal.to_score() as i64 * 20;
        total_weight += 20;
        confidence += t.confidence * 0.20;
    }
    if let Some(f) = fund {
        total_score += f.signal.to_score() as i64 * 40;
        total_weight += 40;
        confidence += f.confidence * 0.40;
    }
    if let Some(q) = quant {
        total_score += q.signal.to_score() as i64 * 15;
        total_weight += 15;
        confidence += q.confidence * 0.15;
    }

    let signal = if total_weight > 0 {
        SignalStrength::from_score((total_score as f64 / total_weight as f64) as i32)
    } else {
        SignalStrength::Neutral
    };

    (signal, confidence.max(0.05))
}
