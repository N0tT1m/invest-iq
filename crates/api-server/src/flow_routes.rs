//! Flow Map API Routes
//!
//! Endpoints for sector flow and rotation analysis.

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use flow_map::{
    FlowMapData, RotationDetector, RotationPattern, SectorETF, SectorETFTracker, SectorNode,
};
use serde::{Deserialize, Serialize};

use crate::{get_cached_etf_bars, ApiResponse, AppError, AppState};

/// Query params for flow data
#[derive(Deserialize)]
pub struct FlowQuery {
    pub timeframe: Option<String>,
}

/// Response with sector performance
#[derive(Serialize)]
pub struct SectorPerformanceResponse {
    pub sectors: Vec<SectorPerformanceData>,
    pub flow_map: FlowMapData,
    pub rotations: Vec<RotationPattern>,
    pub market_summary: MarketSummary,
}

#[derive(Serialize)]
pub struct SectorPerformanceData {
    pub name: String,
    pub etf_symbol: String,
    pub performance_1d: f64,
    pub performance_1w: f64,
    pub performance_1m: f64,
    pub relative_strength: f64,
    pub net_flow: f64,
    pub color: String,
}

#[derive(Serialize)]
pub struct MarketSummary {
    pub trend: String,
    pub dominant_rotation: Option<String>,
    pub strongest_sector: String,
    pub weakest_sector: String,
    pub risk_sentiment: String,
}

pub fn flow_routes() -> Router<AppState> {
    Router::new()
        .route("/api/flows/sectors", get(get_sector_flows))
        .route("/api/flows/rotations", get(get_rotations))
        .route("/api/flows/etfs", get(get_sector_etfs))
}

/// Calculate percentage return from bars over a given lookback period
fn calc_return(bars: &[analysis_core::Bar], lookback_days: usize) -> f64 {
    if bars.len() < 2 {
        return 0.0;
    }
    let end_price = match bars.last() {
        Some(b) => b.close,
        None => return 0.0,
    };
    let start_idx = if bars.len() > lookback_days { bars.len() - lookback_days - 1 } else { 0 };
    let start_price = bars[start_idx].close;
    if start_price > 0.0 {
        (end_price - start_price) / start_price * 100.0
    } else {
        0.0
    }
}

/// Get sector flow data
async fn get_sector_flows(
    State(state): State<AppState>,
    Query(query): Query<FlowQuery>,
) -> Result<Json<ApiResponse<SectorPerformanceResponse>>, AppError> {
    let _timeframe = query.timeframe.unwrap_or_else(|| "1W".to_string());

    // Get sector ETFs to analyze
    let etfs = SectorETF::standard_sectors();

    // Fetch bars for all sector ETFs using the cache (fast, no full analysis needed)
    let mut sector_performances = Vec::new();

    for etf in &etfs {
        let bars = get_cached_etf_bars(&state, &etf.symbol, 90, 15).await;

        let (perf_1d, perf_1w, perf_1m) = if bars.len() >= 2 {
            (calc_return(&bars, 1), calc_return(&bars, 5), calc_return(&bars, 21))
        } else {
            (0.0, 0.0, 0.0)
        };

        let momentum = perf_1d * 0.1 + perf_1w * 0.3 + perf_1m * 0.6;

        sector_performances.push((
            SectorNode {
                name: etf.sector.clone(),
                etf_symbol: etf.symbol.clone(),
                net_flow: 0.0,
                performance_1d: perf_1d,
                performance_1w: perf_1w,
                performance_1m: perf_1m,
                relative_strength: (50.0 + momentum * 5.0).clamp(0.0, 100.0),
                momentum,
                color: SectorETF::sector_color(&etf.sector).to_string(),
            },
            perf_1w,
        ));
    }

    // Create flow map
    let flow_map = FlowMapData::from_performance(&sector_performances);

    // Detect rotations
    let detector = RotationDetector::new();
    let perf_for_rotation: Vec<_> = sector_performances
        .iter()
        .map(|(s, p)| (s.name.clone(), *p))
        .collect();
    let rotations = detector.detect(&perf_for_rotation);

    // Create response
    let sectors: Vec<SectorPerformanceData> = flow_map
        .sectors
        .iter()
        .map(|s| SectorPerformanceData {
            name: s.name.clone(),
            etf_symbol: s.etf_symbol.clone(),
            performance_1d: s.performance_1d,
            performance_1w: s.performance_1w,
            performance_1m: s.performance_1m,
            relative_strength: s.relative_strength,
            net_flow: s.net_flow,
            color: s.color.clone(),
        })
        .collect();

    // Find strongest and weakest
    let strongest = flow_map
        .sectors
        .iter()
        .max_by(|a, b| a.performance_1w.partial_cmp(&b.performance_1w).unwrap_or(std::cmp::Ordering::Equal))
        .map(|s| s.name.clone())
        .unwrap_or_default();

    let weakest = flow_map
        .sectors
        .iter()
        .min_by(|a, b| a.performance_1w.partial_cmp(&b.performance_1w).unwrap_or(std::cmp::Ordering::Equal))
        .map(|s| s.name.clone())
        .unwrap_or_default();

    // Determine risk sentiment
    let risk_sentiment = if rotations.iter().any(|r| r.rotation_type.is_risk_on()) {
        "Risk-On".to_string()
    } else if rotations.iter().any(|r| r.rotation_type.is_risk_off()) {
        "Risk-Off".to_string()
    } else {
        "Neutral".to_string()
    };

    let market_summary = MarketSummary {
        trend: format!("{:?}", flow_map.market_trend),
        dominant_rotation: flow_map.dominant_rotation.clone(),
        strongest_sector: strongest,
        weakest_sector: weakest,
        risk_sentiment,
    };

    Ok(Json(ApiResponse::success(SectorPerformanceResponse {
        sectors,
        flow_map,
        rotations,
        market_summary,
    })))
}

/// Get detected rotation patterns
async fn get_rotations(
    State(_state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<RotationPattern>>>, AppError> {
    // This would normally analyze recent data
    // For now, return empty as rotation detection requires historical data
    let rotations: Vec<RotationPattern> = Vec::new();
    Ok(Json(ApiResponse::success(rotations)))
}

/// Get list of sector ETFs being tracked
async fn get_sector_etfs(
) -> Result<Json<ApiResponse<Vec<SectorETF>>>, AppError> {
    let etfs = SectorETF::standard_sectors();
    Ok(Json(ApiResponse::success(etfs)))
}
