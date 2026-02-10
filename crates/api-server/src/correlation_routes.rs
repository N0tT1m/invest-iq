use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::{get_cached_etf_bars, ApiResponse, AppError, AppState};

#[derive(Serialize)]
pub struct RollingCorrelationPoint {
    pub date: String,
    pub correlation: f64,
}

#[derive(Serialize)]
pub struct CorrelationData {
    pub symbol: String,
    pub correlations: Vec<CorrelationPair>,
    pub beta_spy: Option<f64>,
    pub beta_qqq: Option<f64>,
    pub diversification_score: Option<f64>,
    pub highest_correlation: Option<CorrelationPair>,
    pub lowest_correlation: Option<CorrelationPair>,
    pub rolling_correlation_spy: Vec<RollingCorrelationPoint>,
}

#[derive(Serialize, Clone)]
pub struct CorrelationPair {
    pub benchmark: String,
    pub correlation: f64,
    pub period_days: i64,
}

pub fn correlation_routes() -> Router<AppState> {
    Router::new()
        .route("/api/correlation/:symbol", get(get_correlations))
}

fn calculate_returns(bars: &[analysis_core::Bar]) -> Vec<f64> {
    bars.windows(2)
        .map(|w| (w[1].close - w[0].close) / w[0].close)
        .collect()
}

fn pearson_correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 2 { return 0.0; }

    let a = &a[..n];
    let b = &b[..n];

    let mean_a: f64 = a.iter().sum::<f64>() / n as f64;
    let mean_b: f64 = b.iter().sum::<f64>() / n as f64;

    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;

    for i in 0..n {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom == 0.0 { 0.0 } else { cov / denom }
}

fn calculate_beta(stock_returns: &[f64], benchmark_returns: &[f64]) -> f64 {
    let n = stock_returns.len().min(benchmark_returns.len());
    if n < 2 { return 1.0; }

    let sr = &stock_returns[..n];
    let br = &benchmark_returns[..n];

    let mean_b: f64 = br.iter().sum::<f64>() / n as f64;
    let mean_s: f64 = sr.iter().sum::<f64>() / n as f64;

    let mut cov = 0.0;
    let mut var_b = 0.0;
    for i in 0..n {
        let db = br[i] - mean_b;
        cov += (sr[i] - mean_s) * db;
        var_b += db * db;
    }

    if var_b == 0.0 { 1.0 } else { cov / var_b }
}

fn compute_rolling_correlation(
    stock_returns: &[f64],
    benchmark_returns: &[f64],
    bars: &[analysis_core::Bar],
    window: usize,
) -> Vec<RollingCorrelationPoint> {
    let n = stock_returns.len().min(benchmark_returns.len());
    if n < window { return vec![]; }

    let mut points = Vec::new();
    // bars has one more element than returns (returns = bars.windows(2))
    // So returns[i] corresponds to bars[i+1]
    for i in window..=n {
        let sr = &stock_returns[i - window..i];
        let br = &benchmark_returns[i - window..i];
        let corr = pearson_correlation(sr, br);
        // The date for returns[i-1] corresponds to bars[i] (since returns are offset by 1)
        let bar_idx = i; // i in returns space maps to i+1 in bars space, but we use i since bars offset
        if bar_idx < bars.len() {
            points.push(RollingCorrelationPoint {
                date: bars[bar_idx].timestamp.format("%Y-%m-%d").to_string(),
                correlation: (corr * 1000.0).round() / 1000.0,
            });
        }
    }
    points
}

async fn get_correlations(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<CorrelationData>>, AppError> {
    let days_back = 90i64;

    // Fetch stock bars directly, ETF bars from cache
    let (stock_bars, spy_bars, qqq_bars, dia_bars, iwm_bars) = tokio::join!(
        state.orchestrator.get_bars(&symbol, analysis_core::Timeframe::Day1, days_back),
        get_cached_etf_bars(&state, "SPY", days_back, 15),
        get_cached_etf_bars(&state, "QQQ", days_back, 15),
        get_cached_etf_bars(&state, "DIA", days_back, 15),
        get_cached_etf_bars(&state, "IWM", days_back, 15),
    );

    let stock_bars = match stock_bars {
        Ok(bars) => bars,
        Err(_) => {
            return Ok(Json(ApiResponse {
                success: true,
                data: Some(CorrelationData {
                    symbol,
                    correlations: vec![],
                    beta_spy: None,
                    beta_qqq: None,
                    diversification_score: None,
                    highest_correlation: None,
                    lowest_correlation: None,
                    rolling_correlation_spy: vec![],
                }),
                error: None,
            }));
        }
    };
    let stock_returns = calculate_returns(&stock_bars);

    let mut correlations = Vec::new();
    let mut beta_spy = None;
    let mut beta_qqq = None;
    let mut rolling_correlation_spy = Vec::new();

    let benchmarks: Vec<(&str, &[analysis_core::Bar])> = vec![
        ("SPY", &spy_bars),
        ("QQQ", &qqq_bars),
        ("DIA", &dia_bars),
        ("IWM", &iwm_bars),
    ];

    for (name, etf_bars) in &benchmarks {
        if etf_bars.is_empty() { continue; }
        let etf_returns = calculate_returns(etf_bars);
        let corr = pearson_correlation(&stock_returns, &etf_returns);
        correlations.push(CorrelationPair {
            benchmark: name.to_string(),
            correlation: (corr * 1000.0).round() / 1000.0,
            period_days: days_back,
        });

        if *name == "SPY" {
            beta_spy = Some((calculate_beta(&stock_returns, &etf_returns) * 100.0).round() / 100.0);
            rolling_correlation_spy = compute_rolling_correlation(&stock_returns, &etf_returns, &stock_bars, 30);
        }
        if *name == "QQQ" {
            beta_qqq = Some((calculate_beta(&stock_returns, &etf_returns) * 100.0).round() / 100.0);
        }
    }

    // Diversification score: lower correlation = better diversification (0-100)
    let avg_corr = if !correlations.is_empty() {
        correlations.iter().map(|c| c.correlation.abs()).sum::<f64>() / correlations.len() as f64
    } else {
        0.5
    };
    let diversification_score = Some(((1.0 - avg_corr) * 100.0).clamp(0.0, 100.0));

    let highest_correlation = correlations.iter()
        .max_by(|a, b| a.correlation.abs().partial_cmp(&b.correlation.abs()).unwrap_or(std::cmp::Ordering::Equal))
        .cloned();
    let lowest_correlation = correlations.iter()
        .min_by(|a, b| a.correlation.abs().partial_cmp(&b.correlation.abs()).unwrap_or(std::cmp::Ordering::Equal))
        .cloned();

    Ok(Json(ApiResponse {
        success: true,
        data: Some(CorrelationData {
            symbol,
            correlations,
            beta_spy,
            beta_qqq,
            diversification_score,
            highest_correlation,
            lowest_correlation,
            rolling_correlation_spy,
        }),
        error: None,
    }))
}
