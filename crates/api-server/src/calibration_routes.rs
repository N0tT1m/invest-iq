//! Calibration API Routes
//!
//! Endpoints for confidence calibration and uncertainty analysis.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use confidence_calibrator::{
    CalibratedPrediction, CalibrationMethod, CalibrationStats, ConfidenceCalibrator,
    PredictionContext, UncertaintyAnalysis, UncertaintyEstimator,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{get_default_analysis, ApiResponse, AppError, AppState};

/// Request for calibrating a single prediction
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CalibrateRequest {
    pub raw_confidence: f64,
    #[allow(dead_code)]
    pub source: Option<String>,
}

/// Request for uncertainty analysis
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UncertaintyRequest {
    pub confidence: f64,
    #[serde(default)]
    pub context: PredictionContextRequest,
}

/// Prediction context from request
#[derive(Default, Deserialize, utoipa::ToSchema)]
pub struct PredictionContextRequest {
    pub regime_change_probability: Option<f64>,
    pub model_disagreement: Option<f64>,
    pub market_volatility: Option<f64>,
    pub days_to_earnings: Option<i32>,
    pub news_sentiment_variance: Option<f64>,
}

impl From<PredictionContextRequest> for PredictionContext {
    fn from(req: PredictionContextRequest) -> Self {
        PredictionContext {
            regime_change_probability: req.regime_change_probability.unwrap_or(0.0),
            model_disagreement: req.model_disagreement.unwrap_or(0.0),
            market_volatility: req.market_volatility.unwrap_or(0.02),
            days_to_earnings: req.days_to_earnings,
            news_sentiment_variance: req.news_sentiment_variance.unwrap_or(0.2),
        }
    }
}

/// Query params for calibration stats
#[derive(Deserialize, utoipa::IntoParams)]
pub struct StatsQuery {
    #[allow(dead_code)]
    pub source: Option<String>,
}

/// Calibrated analysis response
#[derive(Serialize, utoipa::ToSchema)]
pub struct CalibratedAnalysisResponse {
    pub symbol: String,
    pub original_confidence: f64,
    pub calibrated: CalibratedPrediction,
    pub uncertainty: UncertaintyAnalysis,
}

/// Shared calibrator state
struct CalibratorState {
    calibrator: ConfidenceCalibrator,
    uncertainty_estimator: UncertaintyEstimator,
}

impl Default for CalibratorState {
    fn default() -> Self {
        Self {
            calibrator: ConfidenceCalibrator::new(),
            uncertainty_estimator: UncertaintyEstimator::new(),
        }
    }
}

/// Lazy-initialized global calibrator
static CALIBRATOR: std::sync::OnceLock<Arc<RwLock<CalibratorState>>> = std::sync::OnceLock::new();

fn get_calibrator() -> &'static Arc<RwLock<CalibratorState>> {
    CALIBRATOR.get_or_init(|| Arc::new(RwLock::new(CalibratorState::default())))
}

pub fn calibration_routes() -> Router<AppState> {
    Router::new()
        .route("/api/calibration/calibrate", post(calibrate_prediction))
        .route("/api/calibration/uncertainty", post(analyze_uncertainty))
        .route("/api/calibration/stats", get(get_calibration_stats))
        .route(
            "/api/analyze/:symbol/calibrated",
            get(get_calibrated_analysis),
        )
        .route("/api/calibration/fit", post(fit_calibrator))
}

/// Calibrate a single prediction
#[utoipa::path(
    post,
    path = "/api/calibration/calibrate",
    request_body = CalibrateRequest,
    responses((status = 200, description = "Calibrated prediction result")),
    tag = "Analysis"
)]
async fn calibrate_prediction(
    Json(req): Json<CalibrateRequest>,
) -> Result<Json<ApiResponse<CalibratedPrediction>>, AppError> {
    let calibrator = get_calibrator().read().await;
    let result = calibrator.calibrator.calibrate(req.raw_confidence);
    Ok(Json(ApiResponse::success(result)))
}

/// Analyze uncertainty for a prediction
#[utoipa::path(
    post,
    path = "/api/calibration/uncertainty",
    request_body = UncertaintyRequest,
    responses((status = 200, description = "Uncertainty analysis for a prediction")),
    tag = "Analysis"
)]
async fn analyze_uncertainty(
    Json(req): Json<UncertaintyRequest>,
) -> Result<Json<ApiResponse<UncertaintyAnalysis>>, AppError> {
    let calibrator = get_calibrator().read().await;
    let context: PredictionContext = req.context.into();
    let result = calibrator
        .uncertainty_estimator
        .estimate(req.confidence, &context);
    Ok(Json(ApiResponse::success(result)))
}

/// Get calibration statistics
#[utoipa::path(
    get,
    path = "/api/calibration/stats",
    params(StatsQuery),
    responses((status = 200, description = "Calibration statistics")),
    tag = "Analysis"
)]
async fn get_calibration_stats(
    Query(_query): Query<StatsQuery>,
) -> Result<Json<ApiResponse<Option<CalibrationStats>>>, AppError> {
    let calibrator = get_calibrator().read().await;
    let stats = calibrator.calibrator.stats().cloned();
    Ok(Json(ApiResponse::success(stats)))
}

/// Get calibrated analysis for a symbol
#[utoipa::path(
    get,
    path = "/api/analyze/{symbol}/calibrated",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    responses((status = 200, description = "Analysis with calibrated confidence and uncertainty")),
    tag = "Analysis"
)]
async fn get_calibrated_analysis(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<CalibratedAnalysisResponse>>, AppError> {
    let symbol = symbol.to_uppercase();

    // Get raw analysis (uses shared cache)
    let analysis = get_default_analysis(&state, &symbol).await?;
    let original_confidence = analysis.overall_confidence;

    // Build context from analysis
    let mut context = PredictionContext::default();

    // Extract model disagreement from analysis variance
    if let (Some(tech), Some(quant), Some(sentiment)) = (
        &analysis.technical,
        &analysis.quantitative,
        &analysis.sentiment,
    ) {
        let confidences = [tech.confidence, quant.confidence, sentiment.confidence];
        let mean: f64 = confidences.iter().sum::<f64>() / 3.0;
        let variance: f64 = confidences.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / 3.0;
        context.model_disagreement = variance.sqrt();
    }

    // Extract volatility if available
    if let Some(quant) = &analysis.quantitative {
        if let Some(vol) = quant.metrics.get("volatility").and_then(|v| v.as_f64()) {
            context.market_volatility = vol / 100.0; // Assuming percentage
        }
    }

    // Extract sentiment variance
    if let Some(sentiment) = &analysis.sentiment {
        let confidence = sentiment.confidence;
        context.news_sentiment_variance = 1.0 - confidence; // Lower confidence = higher variance
    }

    // Get calibrated prediction and uncertainty
    let calibrator = get_calibrator().read().await;
    let calibrated = calibrator.calibrator.calibrate(original_confidence);
    let uncertainty = calibrator
        .uncertainty_estimator
        .estimate(original_confidence, &context);

    Ok(Json(ApiResponse::success(CalibratedAnalysisResponse {
        symbol,
        original_confidence,
        calibrated,
        uncertainty,
    })))
}

/// Request to fit the calibrator with historical data
#[derive(Deserialize, utoipa::ToSchema)]
pub struct FitRequest {
    pub predictions: Vec<PredictionOutcomeInput>,
    pub method: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PredictionOutcomeInput {
    pub confidence: f64,
    pub outcome: bool,
}

/// Fit the calibrator with historical prediction data
#[utoipa::path(
    post,
    path = "/api/calibration/fit",
    request_body = FitRequest,
    responses((status = 200, description = "Calibrator fitted with updated statistics")),
    tag = "Analysis"
)]
async fn fit_calibrator(
    Json(req): Json<FitRequest>,
) -> Result<Json<ApiResponse<CalibrationStats>>, AppError> {
    if req.predictions.len() < 10 {
        return Err(anyhow::anyhow!("Need at least 10 predictions for calibration").into());
    }

    let method = match req.method.as_deref() {
        Some("platt") => CalibrationMethod::PlattScaling,
        Some("isotonic") => CalibrationMethod::IsotonicRegression,
        Some("temperature") => CalibrationMethod::TemperatureScaling,
        _ => CalibrationMethod::PlattScaling, // Default
    };

    let predictions: Vec<(f64, bool)> = req
        .predictions
        .into_iter()
        .map(|p| (p.confidence, p.outcome))
        .collect();

    let mut calibrator_state = get_calibrator().write().await;
    calibrator_state.calibrator.fit(&predictions, method)?;

    // Also update uncertainty estimator
    calibrator_state
        .uncertainty_estimator
        .update_from_history(&predictions);

    let stats = calibrator_state
        .calibrator
        .stats()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Failed to compute calibration stats"))?;

    Ok(Json(ApiResponse::success(stats)))
}
