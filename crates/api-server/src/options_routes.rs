use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::{get_default_analysis, ApiResponse, AppError, AppState};

#[derive(Serialize)]
pub struct OptionsAnalysis {
    pub symbol: String,
    pub available: bool,
    pub data_source: String,
    pub put_call_ratio: Option<f64>,
    pub total_call_volume: Option<i64>,
    pub total_put_volume: Option<i64>,
    pub total_call_oi: Option<i64>,
    pub total_put_oi: Option<i64>,
    pub avg_implied_volatility: Option<f64>,
    pub iv_rank: Option<f64>,
    pub max_pain: Option<f64>,
    pub unusual_activity: Vec<UnusualActivity>,
    pub hv_proxy: Option<f64>,
    pub hv_rank: Option<f64>,
    pub max_drawdown: Option<f64>,
    pub beta: Option<f64>,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct UnusualActivity {
    pub contract: String,
    pub contract_type: String,
    pub strike: f64,
    pub expiration: String,
    pub volume: i64,
    pub open_interest: i64,
    pub vol_oi_ratio: f64,
    pub implied_volatility: Option<f64>,
}

pub fn options_routes() -> Router<AppState> {
    Router::new()
        .route("/api/options/:symbol", get(get_options))
}

async fn get_options(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<OptionsAnalysis>>, AppError> {
    let contracts = state.orchestrator.get_options_snapshot(&symbol).await.unwrap_or_default();

    if contracts.is_empty() {
        // Fallback: use quant analysis for volatility metrics
        let analysis = get_default_analysis(&state, &symbol).await.ok();
        let quant_metrics = analysis.as_ref()
            .and_then(|a| a.quantitative.as_ref())
            .map(|q| q.metrics.clone())
            .unwrap_or_default();

        let volatility = quant_metrics.get("volatility").and_then(|v| v.as_f64());
        let max_drawdown = quant_metrics.get("max_drawdown").and_then(|v| v.as_f64());
        let beta = quant_metrics.get("beta").and_then(|v| v.as_f64());

        // Compute HV rank from bars
        let bars = state.orchestrator.get_bars(&symbol, analysis_core::Timeframe::Day1, 252).await.ok();
        let (hv_proxy, hv_rank) = if let Some(bars) = &bars {
            if bars.len() >= 60 {
                let mut vols = Vec::new();
                for i in 20..bars.len() {
                    let slice = &bars[i - 20..i];
                    let returns: Vec<f64> = slice.windows(2)
                        .map(|w| if w[0].close > 0.0 { (w[1].close / w[0].close).ln() } else { 0.0 })
                        .collect();
                    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
                    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
                    vols.push(var.sqrt() * (252.0_f64).sqrt() * 100.0);
                }
                if let Some(current) = vols.last() {
                    let below = vols.iter().filter(|v| *v < current).count();
                    let rank = (below as f64 / (vols.len() - 1).max(1) as f64) * 100.0;
                    (Some(*current), Some(rank))
                } else {
                    (volatility, None)
                }
            } else {
                (volatility, None)
            }
        } else {
            (volatility, None)
        };

        return Ok(Json(ApiResponse {
            success: true,
            data: Some(OptionsAnalysis {
                symbol,
                available: true,
                data_source: "hv_proxy".to_string(),
                put_call_ratio: None,
                total_call_volume: None,
                total_put_volume: None,
                total_call_oi: None,
                total_put_oi: None,
                avg_implied_volatility: None,
                iv_rank: None,
                max_pain: None,
                unusual_activity: vec![],
                hv_proxy: hv_proxy.map(|v| (v * 10.0).round() / 10.0),
                hv_rank: hv_rank.map(|v| v.round()),
                max_drawdown: max_drawdown.map(|v| (v * 10.0).round() / 10.0),
                beta: beta.map(|v| (v * 100.0).round() / 100.0),
                message: Some("Options chain data requires a premium plan. Showing historical volatility analysis as proxy.".to_string()),
            }),
            error: None,
        }));
    }

    // Full options path (premium)
    let mut total_call_vol: i64 = 0;
    let mut total_put_vol: i64 = 0;
    let mut total_call_oi: i64 = 0;
    let mut total_put_oi: i64 = 0;
    let mut iv_sum = 0.0;
    let mut iv_count = 0;
    let mut unusual = Vec::new();
    let mut strike_pain: std::collections::HashMap<i64, (f64, f64)> = std::collections::HashMap::new();

    for contract in &contracts {
        let details = contract.details.as_ref();
        let contract_type = details.and_then(|d| d.contract_type.as_deref()).unwrap_or("unknown");
        let strike = details.and_then(|d| d.strike_price).unwrap_or(0.0);
        let volume = contract.day.as_ref().and_then(|d| d.volume).unwrap_or(0);
        let oi = contract.open_interest.unwrap_or(0);

        if contract_type == "call" {
            total_call_vol += volume;
            total_call_oi += oi;
        } else if contract_type == "put" {
            total_put_vol += volume;
            total_put_oi += oi;
        }

        if let Some(iv) = contract.implied_volatility {
            iv_sum += iv;
            iv_count += 1;
        }

        if oi > 0 && volume > 100 && volume as f64 > oi as f64 * 2.0 {
            let expiration = details.and_then(|d| d.expiration_date.clone()).unwrap_or_default();
            let ticker = details.and_then(|d| d.ticker.clone()).unwrap_or_default();
            unusual.push(UnusualActivity {
                contract: ticker,
                contract_type: contract_type.to_string(),
                strike,
                expiration,
                volume,
                open_interest: oi,
                vol_oi_ratio: volume as f64 / oi as f64,
                implied_volatility: contract.implied_volatility,
            });
        }

        let strike_key = (strike * 100.0) as i64;
        let entry = strike_pain.entry(strike_key).or_insert((0.0, 0.0));
        if contract_type == "call" {
            entry.0 += oi as f64;
        } else {
            entry.1 += oi as f64;
        }
    }

    unusual.sort_by(|a, b| b.vol_oi_ratio.partial_cmp(&a.vol_oi_ratio).unwrap_or(std::cmp::Ordering::Equal));
    unusual.truncate(10);

    let put_call_ratio = if total_call_vol > 0 {
        Some(total_put_vol as f64 / total_call_vol as f64)
    } else {
        None
    };

    let avg_iv = if iv_count > 0 {
        Some((iv_sum / iv_count as f64) * 100.0)
    } else {
        None
    };

    let iv_rank = avg_iv.map(|iv| {
        ((iv - 15.0) / (80.0 - 15.0) * 100.0).clamp(0.0, 100.0)
    });

    let max_pain = if !strike_pain.is_empty() {
        let strikes: Vec<i64> = strike_pain.keys().copied().collect();
        let mut min_pain = f64::MAX;
        let mut best_strike = 0i64;
        for &test_strike in &strikes {
            let test_price = test_strike as f64 / 100.0;
            let mut pain = 0.0;
            for (&sk, &(call_oi, put_oi)) in &strike_pain {
                let s = sk as f64 / 100.0;
                pain += call_oi * (test_price - s).max(0.0);
                pain += put_oi * (s - test_price).max(0.0);
            }
            if pain < min_pain {
                min_pain = pain;
                best_strike = test_strike;
            }
        }
        Some(best_strike as f64 / 100.0)
    } else {
        None
    };

    Ok(Json(ApiResponse {
        success: true,
        data: Some(OptionsAnalysis {
            symbol,
            available: true,
            data_source: "premium".to_string(),
            put_call_ratio,
            total_call_volume: Some(total_call_vol),
            total_put_volume: Some(total_put_vol),
            total_call_oi: Some(total_call_oi),
            total_put_oi: Some(total_put_oi),
            avg_implied_volatility: avg_iv,
            iv_rank,
            max_pain,
            unusual_activity: unusual,
            hv_proxy: None,
            hv_rank: None,
            max_drawdown: None,
            beta: None,
            message: None,
        }),
        error: None,
    }))
}
