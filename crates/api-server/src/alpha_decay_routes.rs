//! Alpha Decay API Routes
//!
//! Endpoints for strategy health monitoring and decay detection.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use alpha_decay::{
    AlphaDecayMonitor, ChangeDetector, DecayMetrics, HealthReport, HealthReportBuilder,
    HealthStatus, PerformanceSnapshot, StrategyHealth,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{ApiResponse, AppError, AppState};

/// Request to record a performance snapshot
#[derive(Deserialize)]
pub struct RecordSnapshotRequest {
    pub strategy_name: String,
    pub snapshot_date: String, // YYYY-MM-DD
    pub rolling_sharpe: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub trades_count: i32,
}

/// Response with all strategy health summaries
#[derive(Serialize)]
pub struct StrategiesHealthResponse {
    pub strategies: Vec<StrategyHealthSummary>,
    pub overall_portfolio_health: f64,
}

/// Summary of a single strategy's health
#[derive(Serialize)]
pub struct StrategyHealthSummary {
    pub strategy_name: String,
    pub status: String,
    pub status_color: String,
    pub health_score: f64,
    pub current_sharpe: f64,
    pub decay_pct: f64,
    pub is_decaying: bool,
    pub days_to_breakeven: Option<i32>,
}

pub fn alpha_decay_routes() -> Router<AppState> {
    Router::new()
        .route("/api/strategies/health", get(get_all_strategies_health))
        .route("/api/strategies/:name/health", get(get_strategy_health))
        .route("/api/strategies/:name/history", get(get_strategy_history))
        .route("/api/strategies/:name/decay", get(get_decay_analysis))
        .route("/api/strategies/:name/report", get(get_health_report))
        .route("/api/strategies/snapshot", post(record_snapshot))
        .route("/api/strategies/:name/retire", post(retire_strategy))
}

/// Get health status for all strategies
async fn get_all_strategies_health(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<StrategiesHealthResponse>>, AppError> {
    let pool = state.get_db_pool()?;
    let monitor = AlphaDecayMonitor::new(pool);

    let strategy_names = monitor.get_all_strategies().await?;
    let mut summaries = Vec::new();
    let mut total_health = 0.0;

    for name in &strategy_names {
        if let Some(metrics) = monitor.calculate_decay(name).await? {
            let status = HealthStatus::from_decay_metrics(&metrics);
            let health_score = calculate_quick_health_score(&metrics, &status);

            summaries.push(StrategyHealthSummary {
                strategy_name: name.clone(),
                status: status.as_str().to_string(),
                status_color: status.color().to_string(),
                health_score,
                current_sharpe: metrics.current_sharpe,
                decay_pct: metrics.decay_from_peak_pct,
                is_decaying: metrics.is_decaying,
                days_to_breakeven: metrics.days_to_breakeven,
            });

            total_health += health_score;
        }
    }

    let overall_health = if summaries.is_empty() {
        100.0
    } else {
        total_health / summaries.len() as f64
    };

    Ok(Json(ApiResponse::success(StrategiesHealthResponse {
        strategies: summaries,
        overall_portfolio_health: overall_health,
    })))
}

/// Get health status for a specific strategy
async fn get_strategy_health(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<StrategyHealth>>, AppError> {
    let pool = state.get_db_pool()?;
    let monitor = AlphaDecayMonitor::new(pool);

    let metrics = monitor
        .calculate_decay(&name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Strategy not found: {}", name))?;

    let status = HealthStatus::from_decay_metrics(&metrics);

    let recommendation = if status == HealthStatus::Healthy {
        "Strategy is performing well. Continue monitoring.".to_string()
    } else if status == HealthStatus::Degrading {
        "Performance is declining. Consider reducing position sizes.".to_string()
    } else if status == HealthStatus::Critical {
        "Strategy requires immediate attention. Consider pausing.".to_string()
    } else {
        "Strategy should be retired.".to_string()
    };

    let time_to_breakeven = metrics.days_to_breakeven.map(|d| chrono::Duration::days(d as i64));

    Ok(Json(ApiResponse::success(StrategyHealth {
        strategy_name: name,
        current_sharpe: metrics.current_sharpe,
        historical_sharpe: metrics.avg_sharpe,
        decay_percentage: metrics.decay_from_peak_pct,
        time_to_breakeven,
        status,
        recommendation,
    })))
}

/// Get performance history for a strategy
async fn get_strategy_history(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<Vec<PerformanceSnapshot>>>, AppError> {
    let pool = state.get_db_pool()?;
    let monitor = AlphaDecayMonitor::new(pool);

    let snapshots = monitor.get_snapshots(&name, 365).await?;

    Ok(Json(ApiResponse::success(snapshots)))
}

/// Get decay analysis with change detection
async fn get_decay_analysis(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<DecayAnalysisResponse>>, AppError> {
    let pool = state.get_db_pool()?;
    let monitor = AlphaDecayMonitor::new(pool);

    let metrics = monitor
        .calculate_decay(&name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Strategy not found: {}", name))?;

    // Get time series for CUSUM analysis
    let series = monitor.get_sharpe_series(&name).await?;

    let cusum_result = if series.len() >= 20 {
        let values: Vec<f64> = series.iter().map(|(_, v)| *v).collect();
        let dates: Vec<NaiveDate> = series.iter().map(|(d, _)| *d).collect();
        let detector = ChangeDetector::default();
        Some(detector.cusum_analysis(&values, Some(&dates)))
    } else {
        None
    };

    Ok(Json(ApiResponse::success(DecayAnalysisResponse {
        metrics,
        cusum_result,
        series_length: series.len(),
    })))
}

#[derive(Serialize)]
pub struct DecayAnalysisResponse {
    pub metrics: DecayMetrics,
    pub cusum_result: Option<alpha_decay::CusumResult>,
    pub series_length: usize,
}

/// Get full health report for a strategy
async fn get_health_report(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<HealthReport>>, AppError> {
    let pool = state.get_db_pool()?;
    let monitor = AlphaDecayMonitor::new(pool);

    let metrics = monitor
        .calculate_decay(&name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Strategy not found: {}", name))?;

    // Get time series for CUSUM
    let series = monitor.get_sharpe_series(&name).await?;

    let mut builder = HealthReportBuilder::new(&name).with_decay_metrics(metrics);

    if series.len() >= 20 {
        let values: Vec<f64> = series.iter().map(|(_, v)| *v).collect();
        let dates: Vec<NaiveDate> = series.iter().map(|(d, _)| *d).collect();
        let detector = ChangeDetector::default();
        let cusum = detector.cusum_analysis(&values, Some(&dates));
        builder = builder.with_cusum_result(cusum);
    }

    let report = builder.build();

    Ok(Json(ApiResponse::success(report)))
}

/// Record a performance snapshot
async fn record_snapshot(
    State(state): State<AppState>,
    Json(req): Json<RecordSnapshotRequest>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    let pool = state.get_db_pool()?;
    let monitor = AlphaDecayMonitor::new(pool);

    let date = NaiveDate::parse_from_str(&req.snapshot_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid date format: {}", e))?;

    let snapshot = PerformanceSnapshot {
        id: None,
        strategy_name: req.strategy_name,
        snapshot_date: date,
        rolling_sharpe: req.rolling_sharpe,
        win_rate: req.win_rate,
        profit_factor: req.profit_factor,
        trades_count: req.trades_count,
        cumulative_return: 0.0,
        max_drawdown: 0.0,
        created_at: None,
    };

    let id = monitor.record_snapshot(&snapshot).await?;

    Ok(Json(ApiResponse::success(id)))
}

/// Mark a strategy as retired
async fn retire_strategy(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let pool = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db().pool();

    sqlx::query(
        "UPDATE strategy_health_snapshots SET status = 'retired' WHERE strategy_name = ?"
    )
    .bind(&name)
    .execute(pool)
    .await?;

    Ok(Json(ApiResponse::success(format!("Strategy '{}' retired", name))))
}

fn calculate_quick_health_score(metrics: &DecayMetrics, status: &HealthStatus) -> f64 {
    let base = match status {
        HealthStatus::Healthy => 80.0,
        HealthStatus::Degrading => 50.0,
        HealthStatus::Critical => 25.0,
        HealthStatus::Retired => 0.0,
    };

    let sharpe_bonus = (metrics.current_sharpe * 10.0).clamp(-20.0, 20.0);
    let decay_penalty = metrics.decay_from_peak_pct.min(30.0) * 0.5;

    (base + sharpe_bonus - decay_penalty).clamp(0.0, 100.0)
}

// Extension trait to get DB pool from AppState
trait AppStateExt {
    fn get_db_pool(&self) -> anyhow::Result<sqlx::AnyPool>;
}

impl AppStateExt for AppState {
    fn get_db_pool(&self) -> anyhow::Result<sqlx::AnyPool> {
        // Get pool from portfolio manager if available
        self.portfolio_manager
            .as_ref()
            .map(|pm| pm.db().pool().clone())
            .ok_or_else(|| anyhow::anyhow!("Database not configured"))
    }
}
