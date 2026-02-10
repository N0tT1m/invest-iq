use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::{get_default_analysis, get_cached_etf_bars, ApiResponse, AppError, AppState};

#[derive(Serialize)]
pub struct MacroIndicators {
    pub available: bool,
    pub market_regime: String,
    pub indicators: Vec<MacroIndicator>,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct MacroIndicator {
    pub name: String,
    pub value: Option<f64>,
    pub unit: String,
    pub description: String,
    pub trend: Option<String>,
    pub interpretation: Option<String>,
}

#[derive(Serialize)]
pub struct MacroSensitivity {
    pub symbol: String,
    pub available: bool,
    pub interest_rate_sensitivity: Option<f64>,
    pub market_beta: Option<f64>,
    pub sector: Option<String>,
    pub cycle_phase: Option<String>,
    pub message: Option<String>,
}

pub fn macro_routes() -> Router<AppState> {
    Router::new()
        .route("/api/macro/indicators", get(get_macro_indicators))
        .route("/api/macro/sensitivity/:symbol", get(get_macro_sensitivity))
}

fn compute_return(bars: &[analysis_core::Bar], days: usize) -> Option<f64> {
    if bars.len() < days + 1 { return None; }
    let recent = bars.last()?.close;
    let past = bars[bars.len() - 1 - days].close;
    if past == 0.0 { return None; }
    Some(((recent - past) / past) * 100.0)
}

fn compute_sma(bars: &[analysis_core::Bar], period: usize) -> Option<f64> {
    if bars.len() < period { return None; }
    let sum: f64 = bars[bars.len() - period..].iter().map(|b| b.close).sum();
    Some(sum / period as f64)
}

fn compute_realized_vol(bars: &[analysis_core::Bar], days: usize) -> Option<f64> {
    if bars.len() < days + 1 { return None; }
    let returns: Vec<f64> = bars[bars.len() - days - 1..]
        .windows(2)
        .map(|w| (w[1].close / w[0].close).ln())
        .collect();
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
    Some(var.sqrt() * (252.0_f64).sqrt() * 100.0)
}

async fn get_macro_indicators(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<MacroIndicators>>, AppError> {
    // Fetch ETF bars in parallel, 90 days, 15-min cache
    let (spy_bars, tlt_bars, gld_bars) = tokio::join!(
        get_cached_etf_bars(&state, "SPY", 90, 15),
        get_cached_etf_bars(&state, "TLT", 90, 15),
        get_cached_etf_bars(&state, "GLD", 90, 15),
    );

    if spy_bars.is_empty() {
        return Ok(Json(ApiResponse {
            success: true,
            data: Some(MacroIndicators {
                available: false,
                market_regime: "Unknown".to_string(),
                indicators: vec![],
                message: Some("Unable to fetch market ETF data.".to_string()),
            }),
            error: None,
        }));
    }

    let mut indicators = Vec::new();

    // Market Trend: SPY 20d return + SMA20 vs SMA50
    let spy_return = compute_return(&spy_bars, 20);
    let spy_sma20 = compute_sma(&spy_bars, 20);
    let spy_sma50 = compute_sma(&spy_bars, 50);
    let market_trend = match (spy_return, spy_sma20, spy_sma50) {
        (Some(ret), Some(sma20), Some(sma50)) => {
            if ret > 2.0 && sma20 > sma50 { "Bullish" }
            else if ret < -2.0 && sma20 < sma50 { "Bearish" }
            else { "Neutral" }
        }
        _ => "Unknown",
    };
    indicators.push(MacroIndicator {
        name: "Market Trend (SPY)".to_string(),
        value: spy_return.map(|v| (v * 10.0).round() / 10.0),
        unit: "%".to_string(),
        description: "S&P 500 20-day return".to_string(),
        trend: Some(if spy_return.unwrap_or(0.0) > 0.0 { "up" } else { "down" }.to_string()),
        interpretation: Some(format!("{} — SMA20 {} SMA50",
            market_trend,
            if spy_sma20.unwrap_or(0.0) > spy_sma50.unwrap_or(0.0) { "above" } else { "below" }
        )),
    });

    // Rate Environment: TLT 20d return
    let tlt_return = compute_return(&tlt_bars, 20);
    let rate_env = match tlt_return {
        Some(r) if r > 1.5 => "Easing signal",
        Some(r) if r < -1.5 => "Tightening signal",
        _ => "Stable",
    };
    indicators.push(MacroIndicator {
        name: "Rate Environment (TLT)".to_string(),
        value: tlt_return.map(|v| (v * 10.0).round() / 10.0),
        unit: "%".to_string(),
        description: "Long-term Treasury 20-day return".to_string(),
        trend: Some(if tlt_return.unwrap_or(0.0) > 0.0 { "up" } else { "down" }.to_string()),
        interpretation: Some(rate_env.to_string()),
    });

    // Inflation Signal: GLD 20d return
    let gld_return = compute_return(&gld_bars, 20);
    let inflation_sig = match gld_return {
        Some(r) if r > 3.0 => "Rising inflation concerns",
        Some(r) if r < -2.0 => "Inflation expectations cooling",
        _ => "Stable expectations",
    };
    indicators.push(MacroIndicator {
        name: "Inflation Signal (GLD)".to_string(),
        value: gld_return.map(|v| (v * 10.0).round() / 10.0),
        unit: "%".to_string(),
        description: "Gold 20-day return as inflation proxy".to_string(),
        trend: Some(if gld_return.unwrap_or(0.0) > 0.0 { "up" } else { "down" }.to_string()),
        interpretation: Some(inflation_sig.to_string()),
    });

    // Market Volatility: SPY 20d realized vol
    let vol = compute_realized_vol(&spy_bars, 20);
    let vol_label = match vol {
        Some(v) if v > 25.0 => "High volatility",
        Some(v) if v < 12.0 => "Low volatility",
        _ => "Normal volatility",
    };
    indicators.push(MacroIndicator {
        name: "Market Volatility".to_string(),
        value: vol.map(|v| (v * 10.0).round() / 10.0),
        unit: "%".to_string(),
        description: "SPY 20-day annualized realized volatility".to_string(),
        trend: None,
        interpretation: Some(vol_label.to_string()),
    });

    // Risk Appetite: SPY return minus TLT return
    let risk_appetite = match (spy_return, tlt_return) {
        (Some(s), Some(t)) => Some(s - t),
        _ => None,
    };
    let risk_label = match risk_appetite {
        Some(r) if r > 3.0 => "Strong risk-on",
        Some(r) if r > 0.0 => "Mild risk-on",
        Some(r) if r > -3.0 => "Mild risk-off",
        Some(_) => "Strong risk-off",
        None => "Unknown",
    };
    indicators.push(MacroIndicator {
        name: "Risk Appetite".to_string(),
        value: risk_appetite.map(|v| (v * 10.0).round() / 10.0),
        unit: "%".to_string(),
        description: "SPY vs TLT relative performance (positive = risk-on)".to_string(),
        trend: Some(if risk_appetite.unwrap_or(0.0) > 0.0 { "up" } else { "down" }.to_string()),
        interpretation: Some(risk_label.to_string()),
    });

    // Determine market regime
    let market_regime = match risk_appetite {
        Some(r) if r > 2.0 && spy_return.unwrap_or(0.0) > 0.0 => "Risk-On",
        Some(r) if r < -2.0 && spy_return.unwrap_or(0.0) < 0.0 => "Risk-Off",
        _ => "Transitioning",
    };

    Ok(Json(ApiResponse {
        success: true,
        data: Some(MacroIndicators {
            available: true,
            market_regime: market_regime.to_string(),
            indicators,
            message: Some("Derived from ETF market data (SPY, TLT, GLD)".to_string()),
        }),
        error: None,
    }))
}

async fn get_macro_sensitivity(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<MacroSensitivity>>, AppError> {
    // Get beta from quant analysis as a proxy for market sensitivity
    let analysis = get_default_analysis(&state, &symbol).await.ok();
    let beta = analysis
        .as_ref()
        .and_then(|a| a.quantitative.as_ref())
        .and_then(|q| q.metrics.get("beta"))
        .and_then(|v| v.as_f64());

    // Get ticker details for sector
    let details = state.orchestrator.get_ticker_details(&symbol).await.ok();
    let sector = details.map(|d| d.ticker_type.clone());

    // Heuristic cycle phase from market momentum
    let cycle_phase = analysis
        .as_ref()
        .and_then(|a| a.technical.as_ref())
        .and_then(|t| t.metrics.get("trend"))
        .and_then(|v| v.as_str())
        .map(|trend| match trend {
            "Uptrend" => "Expansion",
            "Downtrend" => "Contraction",
            _ => "Transition",
        })
        .map(String::from);

    // Interest rate sensitivity heuristic based on beta
    let interest_rate_sensitivity = beta.map(|b| (b * 50.0).clamp(0.0, 100.0));

    Ok(Json(ApiResponse {
        success: true,
        data: Some(MacroSensitivity {
            symbol,
            available: true,
            interest_rate_sensitivity,
            market_beta: beta,
            sector,
            cycle_phase,
            message: None,
        }),
        error: None,
    }))
}
