use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::{get_default_analysis, ApiResponse, AppError, AppState};

#[derive(Serialize)]
pub struct SqueezeComponent {
    pub name: String,
    pub score: f64,
    pub max_score: f64,
    pub description: String,
}

#[derive(Serialize)]
pub struct ShortInterestData {
    pub symbol: String,
    pub available: bool,
    pub squeeze_risk_score: Option<f64>,
    pub squeeze_risk_level: Option<String>,
    pub volume_spike: Option<f64>,
    pub volume_trend: Option<f64>,
    pub price_momentum: Option<f64>,
    pub volatility_level: Option<f64>,
    pub volatility_percentile: Option<f64>,
    pub bb_squeeze: Option<bool>,
    pub bb_width: Option<f64>,
    pub rsi: Option<f64>,
    pub components: Vec<SqueezeComponent>,
    pub interpretation: Option<String>,
    pub message: Option<String>,
}

pub fn short_interest_routes() -> Router<AppState> {
    Router::new()
        .route("/api/short-interest/:symbol", get(get_short_interest))
}

async fn get_short_interest(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<ShortInterestData>>, AppError> {
    let analysis = match get_default_analysis(&state, &symbol).await {
        Ok(a) => a,
        Err(_) => {
            return Ok(Json(ApiResponse {
                success: true,
                data: Some(ShortInterestData {
                    symbol,
                    available: false,
                    squeeze_risk_score: None,
                    squeeze_risk_level: None,
                    volume_spike: None,
                    volume_trend: None,
                    price_momentum: None,
                    volatility_level: None,
                    volatility_percentile: None,
                    bb_squeeze: None,
                    bb_width: None,
                    rsi: None,
                    components: vec![],
                    interpretation: None,
                    message: Some("Unable to compute squeeze risk at this time.".to_string()),
                }),
                error: None,
            }));
        }
    };

    let quant_metrics = analysis.quantitative
        .as_ref()
        .map(|q| q.metrics.clone())
        .unwrap_or_default();

    let tech_metrics = analysis.technical
        .as_ref()
        .map(|t| t.metrics.clone())
        .unwrap_or_default();

    let volatility = quant_metrics.get("volatility").and_then(|v| v.as_f64());
    let recent_return = quant_metrics.get("recent_return").and_then(|v| v.as_f64());
    let rsi = tech_metrics.get("rsi").and_then(|v| v.as_f64());
    let bb_width = tech_metrics.get("bb_width").and_then(|v| v.as_f64());

    // Get 252 days for proper 1-year volatility percentile
    let bars = state.orchestrator.get_bars(
        &symbol,
        analysis_core::Timeframe::Day1,
        252,
    ).await.ok();

    // Volume spike: latest volume / 20d average
    let volume_spike = bars.as_ref().and_then(|b| {
        if b.len() < 20 { return None; }
        let recent_vol = b.last()?.volume;
        let avg_vol: f64 = b[b.len()-20..].iter().map(|bar| bar.volume).sum::<f64>() / 20.0;
        if avg_vol > 0.0 { Some(recent_vol / avg_vol) } else { None }
    });

    // Volume trend: 5d avg / 20d avg
    let volume_trend = bars.as_ref().and_then(|b| {
        if b.len() < 20 { return None; }
        let avg_5d: f64 = b[b.len()-5..].iter().map(|bar| bar.volume).sum::<f64>() / 5.0;
        let avg_20d: f64 = b[b.len()-20..].iter().map(|bar| bar.volume).sum::<f64>() / 20.0;
        if avg_20d > 0.0 { Some(avg_5d / avg_20d) } else { None }
    });

    // Volatility percentile: current 20d vol vs 1-year range
    let volatility_percentile = bars.as_ref().and_then(|b| {
        if b.len() < 60 { return None; }
        // Calculate rolling 20d volatility at several points
        let mut vols = Vec::new();
        for i in 20..b.len() {
            let slice = &b[i-20..i];
            let returns: Vec<f64> = slice.windows(2)
                .map(|w| if w[0].close > 0.0 { (w[1].close / w[0].close).ln() } else { 0.0 })
                .collect();
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
            vols.push(var.sqrt() * (252.0_f64).sqrt() * 100.0);
        }
        if vols.len() < 2 { return None; }
        let current = vols.last()?;
        let below = vols.iter().filter(|v| *v < current).count();
        Some((below as f64 / (vols.len() - 1).max(1) as f64) * 100.0)
    });

    // BB squeeze: bb_width < 0.04 is considered a squeeze
    let bb_squeeze = bb_width.map(|w| w < 0.04);

    // === 6-component scoring ===
    let mut components = Vec::new();

    // 1. Volume Spike (15%)
    let vol_spike_score = volume_spike.map(|v| ((v - 1.0) * 15.0).clamp(0.0, 15.0)).unwrap_or(0.0);
    components.push(SqueezeComponent {
        name: "Volume Spike".to_string(),
        score: (vol_spike_score * 10.0).round() / 10.0,
        max_score: 15.0,
        description: format!("Latest volume vs 20d avg: {:.1}x", volume_spike.unwrap_or(0.0)),
    });

    // 2. Volume Trend (15%)
    let vol_trend_score = volume_trend.map(|v| ((v - 0.8) * 25.0).clamp(0.0, 15.0)).unwrap_or(0.0);
    components.push(SqueezeComponent {
        name: "Volume Trend".to_string(),
        score: (vol_trend_score * 10.0).round() / 10.0,
        max_score: 15.0,
        description: format!("5d avg / 20d avg: {:.2}x", volume_trend.unwrap_or(0.0)),
    });

    // 3. Momentum (20%)
    let momentum_score = recent_return.map(|r| if r > 0.0 { (r * 2.0).min(20.0) } else { 0.0 }).unwrap_or(0.0);
    components.push(SqueezeComponent {
        name: "Price Momentum".to_string(),
        score: (momentum_score * 10.0).round() / 10.0,
        max_score: 20.0,
        description: format!("Recent return: {:+.1}%", recent_return.unwrap_or(0.0)),
    });

    // 4. Volatility Compression (15%) — LOW vol percentile = squeeze setup = high score
    let vol_comp_score = volatility_percentile.map(|p| ((100.0 - p) / 100.0 * 15.0).clamp(0.0, 15.0)).unwrap_or(
        volatility.map(|v| ((40.0 - v.max(0.0)) / 40.0 * 15.0).clamp(0.0, 15.0)).unwrap_or(0.0)
    );
    components.push(SqueezeComponent {
        name: "Vol Compression".to_string(),
        score: (vol_comp_score * 10.0).round() / 10.0,
        max_score: 15.0,
        description: format!(
            "Vol percentile: {:.0}% — {}",
            volatility_percentile.unwrap_or(0.0),
            if volatility_percentile.unwrap_or(50.0) < 25.0 { "Compressed" }
            else if volatility_percentile.unwrap_or(50.0) < 50.0 { "Below avg" }
            else { "Normal/Elevated" }
        ),
    });

    // 5. BB Squeeze (20%)
    let bb_score = match (bb_squeeze, bb_width) {
        (Some(true), Some(w)) => (20.0 - (w * 250.0)).clamp(10.0, 20.0),
        (Some(false), Some(w)) => ((0.06 - w) * 200.0).clamp(0.0, 10.0),
        _ => 0.0,
    };
    components.push(SqueezeComponent {
        name: "BB Squeeze".to_string(),
        score: (bb_score * 10.0).round() / 10.0,
        max_score: 20.0,
        description: format!(
            "BB Width: {:.3} — {}",
            bb_width.unwrap_or(0.0),
            if bb_squeeze.unwrap_or(false) { "SQUEEZE ACTIVE" } else { "No squeeze" }
        ),
    });

    // 6. RSI Extreme (15%)
    let rsi_score = rsi.map(|r| {
        if r < 30.0 { ((30.0 - r) / 30.0 * 15.0).min(15.0) }
        else if r > 70.0 { ((r - 70.0) / 30.0 * 15.0).min(15.0) }
        else { 0.0 }
    }).unwrap_or(0.0);
    components.push(SqueezeComponent {
        name: "RSI Extreme".to_string(),
        score: (rsi_score * 10.0).round() / 10.0,
        max_score: 15.0,
        description: format!("RSI: {:.1}", rsi.unwrap_or(50.0)),
    });

    let squeeze_score = vol_spike_score + vol_trend_score + momentum_score + vol_comp_score + bb_score + rsi_score;
    let squeeze_score = (squeeze_score * 10.0).round() / 10.0;

    let squeeze_level = if squeeze_score >= 70.0 {
        "High"
    } else if squeeze_score >= 40.0 {
        "Moderate"
    } else {
        "Low"
    };

    // Build interpretation
    let mut interp_parts = Vec::new();
    if bb_squeeze.unwrap_or(false) {
        interp_parts.push("Bollinger Bands are in squeeze mode, indicating compressed volatility that often precedes a breakout.".to_string());
    }
    if volume_spike.unwrap_or(0.0) > 1.5 {
        interp_parts.push(format!("Volume is elevated at {:.1}x average, signaling increased trader interest.", volume_spike.unwrap_or(0.0)));
    }
    if let Some(r) = rsi {
        if r < 30.0 { interp_parts.push(format!("RSI at {:.1} shows oversold conditions, potential for short squeeze.", r)); }
        else if r > 70.0 { interp_parts.push(format!("RSI at {:.1} shows overbought conditions.", r)); }
    }
    let interpretation = if interp_parts.is_empty() {
        format!("Squeeze risk is {} with a score of {:.0}/100. No extreme conditions detected.", squeeze_level.to_lowercase(), squeeze_score)
    } else {
        interp_parts.join(" ")
    };

    Ok(Json(ApiResponse {
        success: true,
        data: Some(ShortInterestData {
            symbol,
            available: true,
            squeeze_risk_score: Some(squeeze_score),
            squeeze_risk_level: Some(squeeze_level.to_string()),
            volume_spike,
            volume_trend: volume_trend.map(|v| (v * 100.0).round() / 100.0),
            price_momentum: recent_return,
            volatility_level: volatility,
            volatility_percentile: volatility_percentile.map(|v| v.round()),
            bb_squeeze,
            bb_width: bb_width.map(|v| (v * 1000.0).round() / 1000.0),
            rsi: rsi.map(|v| (v * 10.0).round() / 10.0),
            components,
            interpretation: Some(interpretation),
            message: Some("Heuristic squeeze risk based on volume, momentum, volatility, Bollinger Bands, and RSI.".to_string()),
        }),
        error: None,
    }))
}
