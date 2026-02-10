use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use analytics::{PerformanceOverview, SignalQualityReport, StrategyPerformance, SignalQuality};
use serde::Deserialize;

use crate::{ApiResponse, AppError, AppState};

#[derive(Deserialize)]
pub struct UpdatePerformanceRequest {
    pub strategy_name: String,
    pub symbol: String,
    pub is_win: bool,
    pub profit_loss: f64,
}

#[derive(Deserialize)]
pub struct RecordSignalRequest {
    pub signal_type: String,
    pub confidence: f64,
    pub is_win: bool,
    pub return_pct: f64,
}

#[derive(Deserialize)]
pub struct FilterSignalQuery {
    pub signal_type: String,
    pub confidence: f64,
    pub min_win_rate: Option<f64>,
}

#[derive(Deserialize)]
pub struct CalibrateQuery {
    pub signal_type: String,
    pub confidence: f64,
}

pub fn analytics_routes() -> Router<AppState> {
    Router::new()
        // Performance endpoints
        .route("/api/analytics/overview", get(get_overview))
        .route("/api/analytics/strategy/:name", get(get_strategy_performance))
        .route("/api/analytics/top/:limit", get(get_top_strategies))
        .route("/api/analytics/performance/update", post(update_performance))

        // Signal quality endpoints
        .route("/api/analytics/signals/quality", get(get_signal_quality_report))
        .route("/api/analytics/signals/:type", get(get_signal_type_quality))
        .route("/api/analytics/signals/record", post(record_signal_outcome))
        .route("/api/analytics/signals/filter", get(check_signal_filter))
        .route("/api/analytics/signals/calibrate", get(calibrate_confidence))
}

/// Get performance overview
async fn get_overview(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<PerformanceOverview>>, AppError> {
    let tracker = state.performance_tracker.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Performance tracker not configured"))?;

    let overview = tracker.get_overview().await?;

    Ok(Json(ApiResponse::success(overview)))
}

/// Get strategy performance
async fn get_strategy_performance(
    State(state): State<AppState>,
    Path(strategy_name): Path<String>,
) -> Result<Json<ApiResponse<Vec<StrategyPerformance>>>, AppError> {
    let tracker = state.performance_tracker.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Performance tracker not configured"))?;

    let performance = tracker.get_strategy_performance(&strategy_name).await?;

    Ok(Json(ApiResponse::success(performance)))
}

/// Get top strategies
async fn get_top_strategies(
    State(state): State<AppState>,
    Path(limit): Path<i32>,
) -> Result<Json<ApiResponse<Vec<StrategyPerformance>>>, AppError> {
    let tracker = state.performance_tracker.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Performance tracker not configured"))?;

    let strategies = tracker.get_top_strategies(limit).await?;

    Ok(Json(ApiResponse::success(strategies)))
}

/// Update strategy performance
async fn update_performance(
    State(state): State<AppState>,
    Json(req): Json<UpdatePerformanceRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let tracker = state.performance_tracker.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Performance tracker not configured"))?;

    tracker.update_strategy_performance(
        &req.strategy_name,
        &req.symbol,
        req.is_win,
        req.profit_loss,
    ).await?;

    Ok(Json(ApiResponse::success("Performance updated".to_string())))
}

/// Get signal quality report
async fn get_signal_quality_report(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<SignalQualityReport>>, AppError> {
    let analyzer = state.signal_analyzer.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Signal analyzer not configured"))?;

    let report = analyzer.get_quality_report().await?;

    Ok(Json(ApiResponse::success(report)))
}

/// Get signal quality for specific type
async fn get_signal_type_quality(
    State(state): State<AppState>,
    Path(signal_type): Path<String>,
) -> Result<Json<ApiResponse<Vec<SignalQuality>>>, AppError> {
    let analyzer = state.signal_analyzer.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Signal analyzer not configured"))?;

    let quality = analyzer.get_signal_quality(&signal_type).await?;

    Ok(Json(ApiResponse::success(quality)))
}

/// Record signal outcome
async fn record_signal_outcome(
    State(state): State<AppState>,
    Json(req): Json<RecordSignalRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let analyzer = state.signal_analyzer.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Signal analyzer not configured"))?;

    analyzer.record_trade_outcome(
        &req.signal_type,
        req.confidence,
        req.is_win,
        req.return_pct,
    ).await?;

    Ok(Json(ApiResponse::success("Signal outcome recorded".to_string())))
}

/// Check if signal should be filtered
async fn check_signal_filter(
    State(state): State<AppState>,
    Query(query): Query<FilterSignalQuery>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    let analyzer = state.signal_analyzer.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Signal analyzer not configured"))?;

    let min_win_rate = query.min_win_rate.unwrap_or(0.55);

    let should_filter = analyzer.should_filter_signal(
        &query.signal_type,
        query.confidence,
        min_win_rate,
    ).await?;

    Ok(Json(ApiResponse::success(should_filter)))
}

/// Get calibrated confidence
async fn calibrate_confidence(
    State(state): State<AppState>,
    Query(query): Query<CalibrateQuery>,
) -> Result<Json<ApiResponse<f64>>, AppError> {
    let analyzer = state.signal_analyzer.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Signal analyzer not configured"))?;

    let calibrated = analyzer.get_calibrated_confidence(
        &query.signal_type,
        query.confidence,
    ).await?;

    Ok(Json(ApiResponse::success(calibrated)))
}
