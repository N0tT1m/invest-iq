//! Data Retention Policy Routes
//!
//! Archives old records and provides retention management endpoints.
//! Covers: trades, audit log, backtest results, sentiment history, analysis features.

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::{ApiResponse, AppError, AppState};

#[derive(Deserialize)]
pub struct RetentionQuery {
    pub days: Option<i64>,  // Records older than this many days (default: 365)
    pub dry_run: Option<bool>,  // If true, just count without archiving
}

#[derive(Serialize)]
pub struct RetentionResult {
    pub trades_archived: i64,
    pub audit_entries_archived: i64,
    pub backtest_results_deleted: i64,
    pub sentiment_history_deleted: i64,
    pub analysis_features_deleted: i64,
    pub dry_run: bool,
    pub cutoff_date: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RetentionRun {
    pub id: i64,
    pub run_date: String,
    pub trades_archived: i64,
    pub audit_entries_archived: i64,
    pub backtest_results_archived: i64,
    pub completed_at: Option<String>,
}

pub fn retention_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/retention", get(get_retention_history))
        .route("/api/admin/retention/run", post(run_retention))
        .route("/api/admin/retention/policy", get(get_retention_policy))
        .route("/api/admin/audit/verify", get(verify_audit_chain))
}

/// Verify the tamper-evident audit hash chain.
async fn verify_audit_chain(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::audit::AuditChainVerification>>, AppError> {
    let pool = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db().pool();

    let verification = crate::audit::verify_audit_chain(pool).await?;
    Ok(Json(ApiResponse::success(verification)))
}

/// Get configured retention policy
async fn get_retention_policy() -> Result<Json<ApiResponse<RetentionPolicy>>, AppError> {
    let policy = RetentionPolicy::from_env();
    Ok(Json(ApiResponse::success(policy)))
}

#[derive(Serialize)]
pub struct RetentionPolicy {
    pub trades_retention_days: i64,
    pub audit_retention_days: i64,
    pub backtest_retention_days: i64,
    pub sentiment_retention_days: i64,
    pub analysis_features_retention_days: i64,
}

impl RetentionPolicy {
    fn from_env() -> Self {
        let default_days: i64 = std::env::var("RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(365);

        Self {
            trades_retention_days: std::env::var("RETENTION_TRADES_DAYS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(default_days),
            audit_retention_days: std::env::var("RETENTION_AUDIT_DAYS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(default_days),
            backtest_retention_days: std::env::var("RETENTION_BACKTEST_DAYS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(default_days),
            sentiment_retention_days: std::env::var("RETENTION_SENTIMENT_DAYS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(90),
            analysis_features_retention_days: std::env::var("RETENTION_FEATURES_DAYS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(90),
        }
    }
}

async fn get_retention_history(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<RetentionRun>>>, AppError> {
    let pool = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db().pool();

    let runs: Vec<RetentionRun> = sqlx::query_as(
        "SELECT id, run_date, trades_archived, audit_entries_archived,
                backtest_results_archived, completed_at
         FROM retention_runs ORDER BY run_date DESC LIMIT 20"
    )
    .fetch_all(pool)
    .await?;

    Ok(Json(ApiResponse::success(runs)))
}

async fn run_retention(
    State(state): State<AppState>,
    Query(query): Query<RetentionQuery>,
) -> Result<Json<ApiResponse<RetentionResult>>, AppError> {
    let pool = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db().pool();

    let days = query.days.unwrap_or(365);
    let dry_run = query.dry_run.unwrap_or(false);
    let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
    let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();

    // Shorter retention for high-volume tables
    let sentiment_cutoff = chrono::Utc::now() - chrono::Duration::days(days.min(90));
    let sentiment_cutoff_str = sentiment_cutoff.format("%Y-%m-%d %H:%M:%S").to_string();
    let features_cutoff = chrono::Utc::now() - chrono::Duration::days(days.min(90));
    let features_cutoff_str = features_cutoff.format("%Y-%m-%d %H:%M:%S").to_string();

    if dry_run {
        let trade_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM trades WHERE trade_date < ?"
        )
        .bind(&cutoff_str)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let audit_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM audit_log WHERE created_at < ?"
        )
        .bind(&cutoff_str)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let backtest_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM backtest_results WHERE created_at < ?"
        )
        .bind(&cutoff_str)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let sentiment_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sentiment_history WHERE recorded_at < ?"
        )
        .bind(&sentiment_cutoff_str)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let features_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM analysis_features WHERE created_at < ?"
        )
        .bind(&features_cutoff_str)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        return Ok(Json(ApiResponse::success(RetentionResult {
            trades_archived: trade_count.0,
            audit_entries_archived: audit_count.0,
            backtest_results_deleted: backtest_count.0,
            sentiment_history_deleted: sentiment_count.0,
            analysis_features_deleted: features_count.0,
            dry_run: true,
            cutoff_date: cutoff_str,
        })));
    }

    // Archive trades (copy to archive, then delete)
    let trades_result = sqlx::query(
        "INSERT INTO trades_archive (id, symbol, action, shares, price, total_value, order_id, status, trade_date, notes)
         SELECT id, symbol, action, shares, price, total_value, order_id, status, trade_date, notes
         FROM trades WHERE trade_date < ? ON CONFLICT DO NOTHING"
    )
    .bind(&cutoff_str)
    .execute(pool)
    .await?;
    let trades_archived = trades_result.rows_affected() as i64;

    sqlx::query("DELETE FROM trades WHERE trade_date < ?")
        .bind(&cutoff_str)
        .execute(pool)
        .await?;

    // Archive audit log
    let audit_result = sqlx::query(
        "INSERT INTO audit_log_archive (id, event_type, symbol, action, details, user_id, order_id, created_at)
         SELECT id, event_type, symbol, action, details, user_id, order_id, created_at
         FROM audit_log WHERE created_at < ? ON CONFLICT DO NOTHING"
    )
    .bind(&cutoff_str)
    .execute(pool)
    .await?;
    let audit_archived = audit_result.rows_affected() as i64;

    sqlx::query("DELETE FROM audit_log WHERE created_at < ?")
        .bind(&cutoff_str)
        .execute(pool)
        .await?;

    // Delete old backtest results (no archive — can be re-run)
    let backtest_result = sqlx::query("DELETE FROM backtest_results WHERE created_at < ?")
        .bind(&cutoff_str)
        .execute(pool)
        .await?;
    let backtest_deleted = backtest_result.rows_affected() as i64;

    // Delete old sentiment history (high volume, short retention)
    let sentiment_result = sqlx::query("DELETE FROM sentiment_history WHERE recorded_at < ?")
        .bind(&sentiment_cutoff_str)
        .execute(pool)
        .await?;
    let sentiment_deleted = sentiment_result.rows_affected() as i64;

    // Delete old analysis features (high volume, short retention)
    let features_result = sqlx::query("DELETE FROM analysis_features WHERE created_at < ?")
        .bind(&features_cutoff_str)
        .execute(pool)
        .await?;
    let features_deleted = features_result.rows_affected() as i64;

    // Record the retention run
    sqlx::query(
        "INSERT INTO retention_runs (trades_archived, audit_entries_archived, backtest_results_archived, completed_at)
         VALUES (?, ?, ?, ?)"
    )
    .bind(trades_archived)
    .bind(audit_archived)
    .bind(backtest_deleted)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;

    tracing::info!(
        "Retention run complete: {} trades archived, {} audit archived, {} backtests deleted, {} sentiment deleted, {} features deleted (cutoff: {})",
        trades_archived, audit_archived, backtest_deleted, sentiment_deleted, features_deleted, cutoff_str
    );

    Ok(Json(ApiResponse::success(RetentionResult {
        trades_archived,
        audit_entries_archived: audit_archived,
        backtest_results_deleted: backtest_deleted,
        sentiment_history_deleted: sentiment_deleted,
        analysis_features_deleted: features_deleted,
        dry_run: false,
        cutoff_date: cutoff_str,
    })))
}
