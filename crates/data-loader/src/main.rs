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
//!   cargo run -p data-loader -- --all --all-data   # bars + news + features
//!   cargo run -p data-loader -- --all --bars --news # bars + news only

use analysis_core::{
    AnalysisResult, AnalystConsensusData, Bar, NewsArticle,
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
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use tokio::signal::unix::SignalKind;
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
/// How many years of history to fetch (more = more training samples)
const HISTORY_DAYS: i64 = 1500;
/// Max concurrent symbol processing tasks (defaults to CPU count for CPU-bound feature gen)
const DEFAULT_CONCURRENCY: usize = 0; // 0 = auto-detect

/// What data to store for each symbol
#[derive(Clone, Copy)]
struct StoreFlags {
    bars: bool,
    news: bool,
    features: bool,
}

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

    // New data storage flags
    let flag_bars = args.iter().any(|a| a == "--bars");
    let flag_news = args.iter().any(|a| a == "--news");
    let flag_all_data = args.iter().any(|a| a == "--all-data");

    // If no new flags are set, default to features-only (backward compatible)
    let store_flags = if flag_all_data {
        StoreFlags { bars: true, news: true, features: true }
    } else if flag_bars || flag_news {
        StoreFlags { bars: flag_bars, news: flag_news, features: false }
    } else {
        StoreFlags { bars: false, news: false, features: true }
    };

    let timespan: String = args
        .iter()
        .position(|a| a == "--timespan")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "day".to_string());

    let news_limit: u32 = args
        .iter()
        .position(|a| a == "--news-limit")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    let limit: usize = args
        .iter()
        .position(|a| a == "--limit")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(usize::MAX);

    let concurrency: usize = args
        .iter()
        .position(|a| a == "--concurrency")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CONCURRENCY);
    let concurrency = if concurrency == 0 {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8)
    } else {
        concurrency
    };

    let db_path = args
        .iter()
        .position(|a| a == "--db")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("portfolio.db");

    let api_key = std::env::var("POLYGON_API_KEY")
        .expect("POLYGON_API_KEY must be set");

    // Default to 5500 req/min for bulk loading (Polygon Starter allows ~6000/min).
    // Free tier users should set POLYGON_RATE_LIMIT=5.
    if std::env::var("POLYGON_RATE_LIMIT").is_err() {
        std::env::set_var("POLYGON_RATE_LIMIT", "5500");
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
        eprintln!("Data flags (default: features only):");
        eprintln!("  --bars             Store OHLCV bars into training_bars");
        eprintln!("  --news             Fetch news + compute price labels into training_news");
        eprintln!("  --all-data         Everything: bars + news + analysis features");
        eprintln!("");
        eprintln!("Options:");
        eprintln!("  --dry-run          Print stats without writing to DB");
        eprintln!("  --db PATH          SQLite DB path (default: portfolio.db)");
        eprintln!("  --concurrency N    Max parallel symbols (default: {})", DEFAULT_CONCURRENCY);
        eprintln!("  --timespan SPAN    Bar timespan (default: day)");
        eprintln!("  --news-limit N     Max articles per symbol (default: 100)");
        std::process::exit(1);
    };

    let total_symbols = symbols.len();
    tracing::info!(
        "data-loader: {} symbols, db={}, dry_run={}, concurrency={}, bars={}, news={}, features={}",
        total_symbols, db_path, dry_run, concurrency,
        store_flags.bars, store_flags.news, store_flags.features
    );

    // Open DB (migrations handle table schema via sqlx::migrate!())
    let pool = Arc::new(SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path)).await?);

    // Bulk-load SQLite optimizations — use NORMAL sync to avoid corruption risk
    sqlx::query("PRAGMA journal_mode=WAL").execute(pool.as_ref()).await?;
    sqlx::query("PRAGMA synchronous=NORMAL").execute(pool.as_ref()).await?;
    sqlx::query("PRAGMA temp_store=MEMORY").execute(pool.as_ref()).await?;
    sqlx::query("PRAGMA cache_size=-64000").execute(pool.as_ref()).await?; // 64MB cache

    // Run migrations to ensure training tables exist
    sqlx::migrate!("../../migrations").run(pool.as_ref()).await?;

    // Graceful shutdown: SIGINT + SIGTERM set a flag so in-flight tasks finish
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    {
        let flag = Arc::clone(&shutdown_flag);
        tokio::spawn(async move {
            let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate())
                .expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
            tracing::info!("Shutdown signal received — finishing in-flight tasks...");
            flag.store(true, Ordering::SeqCst);
        });
    }

    let tech_engine = Arc::new(TechnicalAnalysisEngine::new());
    let fund_engine = Arc::new(FundamentalAnalysisEngine::new());
    let quant_engine = Arc::new(QuantAnalysisEngine::new());

    // Fetch SPY bars once for benchmark (only needed for features)
    let spy_bars = if store_flags.features {
        tracing::info!("Fetching SPY bars for benchmark...");
        let now = Utc::now();
        let bars = polygon
            .get_aggregates("SPY", 1, "day", now - Duration::days(HISTORY_DAYS), now)
            .await
            .unwrap_or_default();
        tracing::info!("SPY: {} bars", bars.len());
        Arc::new(bars)
    } else {
        Arc::new(Vec::new())
    };

    let total_rows = Arc::new(AtomicU64::new(0));
    let completed = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let timespan = Arc::new(timespan);

    let mut handles = Vec::with_capacity(total_symbols);

    for symbol in symbols {
        // Check shutdown before spawning new tasks
        if shutdown_flag.load(Ordering::SeqCst) {
            tracing::info!("Shutdown requested — skipping remaining symbols");
            break;
        }

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
        let timespan = Arc::clone(&timespan);
        let shutdown_flag = Arc::clone(&shutdown_flag);

        let handle = tokio::spawn(async move {
            // Abort early if shutdown was requested
            if shutdown_flag.load(Ordering::SeqCst) {
                return;
            }

            let _permit = semaphore.acquire().await.unwrap();

            let rows = process_symbol(
                &polygon, &tech_engine, &fund_engine, &quant_engine,
                &symbol, &spy_bars, pool.as_ref(), dry_run,
                store_flags, &timespan, news_limit,
            ).await;

            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;

            match rows {
                Ok(n) => {
                    total_rows.fetch_add(n, Ordering::Relaxed);
                    if n > 0 {
                        tracing::info!("[{}/{}] {} => {} rows", done, total_symbols, symbol, n);
                    } else {
                        tracing::info!("[{}/{}] {} => 0 new rows (data already exists)", done, total_symbols, symbol, );
                    }
                }
                Err(e) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!("[{}/{}] {} failed: {}", done, total_symbols, symbol, e);
                }
            }

            // Progress summary every 10 symbols
            if done % 10 == 0 {
                tracing::info!(
                    "Progress: {}/{} completed, {} rows so far, {} failed",
                    done, total_symbols,
                    total_rows.load(Ordering::Relaxed),
                    failed.load(Ordering::Relaxed),
                );
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        let _ = handle.await;
    }

    let done = completed.load(Ordering::Relaxed);
    let rows = total_rows.load(Ordering::Relaxed);
    let fails = failed.load(Ordering::Relaxed);
    let remaining = total_symbols as u64 - done;
    if remaining > 0 {
        tracing::info!(
            "Shutdown summary: {} completed, {} remaining (skipped), {} total rows, {} failed",
            done, remaining, rows, fails
        );
    } else {
        tracing::info!(
            "Done! {} total rows across {} symbols ({} failed)",
            rows, total_symbols, fails
        );
    }
    Ok(())
}

/// Retry an async operation with exponential backoff. Rate-limit (429-like) errors
/// get extra delay. Returns the result of the first successful attempt.
async fn retry_with_backoff<F, Fut, T>(label: &str, max_retries: u32, mut f: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut attempt = 0u32;
    loop {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                attempt += 1;
                if attempt > max_retries {
                    return Err(e);
                }
                // Check for rate-limit hint in the error message
                let err_str = format!("{e}");
                let is_rate_limit = err_str.contains("429") || err_str.contains("rate");
                let base_delay = if is_rate_limit {
                    std::time::Duration::from_secs(2u64.pow(attempt) * 2)
                } else {
                    std::time::Duration::from_secs(2u64.pow(attempt - 1))
                };
                tracing::warn!(
                    "{} failed (attempt {}/{}): {} — retrying in {:?}",
                    label, attempt, max_retries, e, base_delay
                );
                tokio::time::sleep(base_delay).await;
            }
        }
    }
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
    store_flags: StoreFlags,
    timespan: &str,
    news_limit: u32,
) -> anyhow::Result<u64> {
    let now = Utc::now();
    let start = now - Duration::days(HISTORY_DAYS);

    // Fetch bars + financials + news concurrently
    let need_financials = store_flags.features;
    let need_news = store_flags.news;

    let bars_label = format!("{symbol}/bars");
    let fin_label = format!("{symbol}/financials");
    let news_label = format!("{symbol}/news");

    let bars_fut = retry_with_backoff(
        &bars_label, 3,
        || async { polygon.get_aggregates(symbol, 1, timespan, start, now).await.map_err(Into::into) },
    );
    let fin_fut = async {
        if need_financials {
            retry_with_backoff(
                &fin_label, 3,
                || async { polygon.get_financials(symbol).await.map_err(Into::into) },
            ).await.unwrap_or_default()
        } else {
            Vec::new()
        }
    };
    let news_fut = async {
        if need_news {
            retry_with_backoff(
                &news_label, 3,
                || async { polygon.get_news(Some(symbol), news_limit).await.map_err(Into::into) },
            ).await.unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    let (bars_result, financials, news) = tokio::join!(bars_fut, fin_fut, news_fut);

    let bars = bars_result?;
    tracing::debug!("{}: {} bars fetched, {} news", symbol, bars.len(), news.len());

    let mut count = 0u64;

    // Store training bars
    if store_flags.bars && !dry_run && !bars.is_empty() {
        let stored = store_training_bars(pool, symbol, &bars, timespan).await?;
        count += stored;
        if stored > 0 {
            tracing::debug!("  {} bars stored for {}", stored, symbol);
        }
    }

    // Store training news with price labels
    if store_flags.news && !dry_run && !news.is_empty() {
        let stored = store_training_news(pool, symbol, &bars, &news).await?;
        count += stored;
        if stored > 0 {
            tracing::debug!("  {} news articles stored for {}", stored, symbol);
        }
    }

    // Generate analysis features (existing behavior)
    if store_flags.features {
        if bars.len() < MIN_WINDOW + FORWARD_20D {
            tracing::warn!("  {} only has {} bars, need {}", symbol, bars.len(), MIN_WINDOW + FORWARD_20D);
            return Ok(count);
        }

        // Compute fundamentals once (they don't change per window)
        let empty_consensus = AnalystConsensusData {
            consensus: None,
            recent_ratings: Vec::new(),
        };
        let fund = if !financials.is_empty() {
            let price = bars.last().map(|b| b.close).unwrap_or(0.0);
            fund_engine
                .analyze_with_consensus(symbol, &financials, Some(price), None, &empty_consensus, None, None)
                .ok()
        } else {
            None
        };

        let max_t = bars.len() - FORWARD_20D;
        let step = 1;
        let mut t = MIN_WINDOW;

        // Collect all rows, then batch-insert in a single transaction
        struct FeatureRow {
            date: String,
            features_json: String,
            signal: String,
            confidence: f64,
            return_5d: f64,
            return_20d: f64,
        }
        let mut rows: Vec<FeatureRow> = Vec::with_capacity(max_t - MIN_WINDOW);

        while t < max_t {
            let window = &bars[..t];
            let current_price = bars[t - 1].close;
            let date = bars[t - 1].timestamp.format("%Y-%m-%dT%H:%M:%S").to_string();

            let fwd_5d_idx = (t + FORWARD_5D - 1).min(bars.len() - 1);
            let fwd_20d_idx = (t + FORWARD_20D - 1).min(bars.len() - 1);
            let return_5d = (bars[fwd_5d_idx].close - current_price) / current_price * 100.0;
            let return_20d = (bars[fwd_20d_idx].close - current_price) / current_price * 100.0;

            let tech = tech_engine.analyze_enhanced(symbol, window, None).ok();
            let quant = quant_engine
                .analyze_with_benchmark_and_rate(symbol, window, Some(spy_bars), None)
                .ok();

            let features = build_features(&tech, &fund, &quant);
            let features_json = serde_json::to_string(&features)?;
            let (overall_signal, overall_confidence) = combine_simple(&tech, &fund, &quant);

            rows.push(FeatureRow {
                date,
                features_json,
                signal: format!("{:?}", overall_signal),
                confidence: overall_confidence,
                return_5d,
                return_20d,
            });

            t += step;
        }

        // Batch insert in a single transaction
        if !dry_run && !rows.is_empty() {
            let mut tx = pool.begin().await?;
            for row in &rows {
                sqlx::query(
                    "INSERT OR IGNORE INTO analysis_features (symbol, analysis_date, features_json, overall_signal, overall_confidence, actual_return_5d, actual_return_20d, evaluated) VALUES (?, ?, ?, ?, ?, ?, ?, 1)"
                )
                .bind(symbol)
                .bind(&row.date)
                .bind(&row.features_json)
                .bind(&row.signal)
                .bind(row.confidence)
                .bind(row.return_5d)
                .bind(row.return_20d)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
        }

        count += rows.len() as u64;
    }

    Ok(count)
}

/// Store OHLCV bars into `training_bars` table using a transaction for efficiency.
async fn store_training_bars(
    pool: &SqlitePool,
    symbol: &str,
    bars: &[Bar],
    timespan: &str,
) -> anyhow::Result<u64> {
    let mut tx = pool.begin().await?;
    let mut inserted = 0u64;

    for bar in bars {
        let timestamp_ms = bar.timestamp.timestamp_millis();
        let result = sqlx::query(
            "INSERT OR IGNORE INTO training_bars (symbol, timestamp_ms, timespan, open, high, low, close, volume, vwap) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(symbol)
        .bind(timestamp_ms)
        .bind(timespan)
        .bind(bar.open)
        .bind(bar.high)
        .bind(bar.low)
        .bind(bar.close)
        .bind(bar.volume)
        .bind(bar.vwap)
        .execute(&mut *tx)
        .await?;

        inserted += result.rows_affected();
    }

    tx.commit().await?;
    Ok(inserted)
}

/// Fetch news and compute 5-day price change labels, then store into `training_news`.
/// Uses the already-fetched bars array to compute labels without extra API calls.
async fn store_training_news(
    pool: &SqlitePool,
    symbol: &str,
    bars: &[Bar],
    news: &[NewsArticle],
) -> anyhow::Result<u64> {
    if bars.is_empty() || news.is_empty() {
        return Ok(0);
    }

    // Build a date->index lookup from bars for efficient price label computation
    let bar_dates: Vec<(i64, usize)> = bars
        .iter()
        .enumerate()
        .map(|(i, b)| (b.timestamp.timestamp(), i))
        .collect();

    let mut tx = pool.begin().await?;
    let mut inserted = 0u64;

    for article in news {
        let pub_ts = article.published_utc.timestamp();

        // Find the bar on or just after the publish date
        let base_idx = bar_dates
            .iter()
            .find(|(ts, _)| *ts >= pub_ts)
            .map(|(_, idx)| *idx);

        // Compute 5-day forward price change if we have enough bars
        let price_change_5d = base_idx.and_then(|idx| {
            let fwd_idx = idx + 5;
            if fwd_idx < bars.len() {
                let base_price = bars[idx].close;
                let fwd_price = bars[fwd_idx].close;
                if base_price > 0.0 {
                    Some((fwd_price - base_price) / base_price * 100.0)
                } else {
                    None
                }
            } else {
                None
            }
        });

        let tickers_json = serde_json::to_string(&article.tickers).unwrap_or_default();

        let result = sqlx::query(
            "INSERT OR IGNORE INTO training_news (symbol, title, description, published_utc, tickers_json, price_change_5d) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(symbol)
        .bind(&article.title)
        .bind(&article.description)
        .bind(article.published_utc.to_rfc3339())
        .bind(&tickers_json)
        .bind(price_change_5d)
        .execute(&mut *tx)
        .await?;

        inserted += result.rows_affected();
    }

    tx.commit().await?;
    Ok(inserted)
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
