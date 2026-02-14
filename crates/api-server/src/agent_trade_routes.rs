//! Agent Trade Approval Routes
//!
//! The trading agent queues proposed trades here for human approval.
//! Trades stay "pending" until approved or rejected from the dashboard.

use axum::{
    extract::{Path, Query, State},
    routing::get,
    routing::post,
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{audit, ApiResponse, AppError, AppState};
use notification_service::{Alert, AlertType};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct PendingTrade {
    pub id: i64,
    pub symbol: String,
    pub action: String,
    pub shares: f64,
    pub confidence: f64,
    pub reason: String,
    pub signal_type: String,
    pub proposed_at: String,
    pub status: String, // "pending", "approved", "rejected", "executed", "expired"
    pub reviewed_at: Option<String>,
    pub price: Option<f64>,
    pub order_id: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ProposeTrade {
    pub symbol: String,
    pub action: String,
    pub shares: f64,
    pub confidence: Option<f64>,
    pub reason: Option<String>,
    pub signal_type: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ReviewTrade {
    pub action: String, // "approve" or "reject"
}

/// Initialize the pending_trades table
pub async fn init_pending_trades_table(_pool: &sqlx::AnyPool) -> Result<(), sqlx::Error> {
    // Table is now created by sqlx migrations.
    Ok(())
}

pub fn agent_trade_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agent/trades", get(list_pending_trades))
        .route("/api/agent/trades", post(propose_trade))
        .route("/api/agent/trades/:id/review", post(review_trade))
        .route("/api/agent/trades/:id", get(get_pending_trade))
        .route("/api/agent/trades/:id/context", get(get_trade_context))
        .route("/api/agent/analytics/summary", get(get_analytics_summary))
        .route(
            "/api/agent/analytics/win-rate-by-regime",
            get(get_win_rate_by_regime),
        )
        .route(
            "/api/agent/analytics/win-rate-by-conviction",
            get(get_win_rate_by_conviction),
        )
        .route("/api/agent/analytics/pnl-by-symbol", get(get_pnl_by_symbol))
        .route(
            "/api/agent/analytics/confidence-calibration",
            get(get_confidence_calibration),
        )
        .route(
            "/api/agent/analytics/supplementary-outcomes",
            get(get_supplementary_outcomes),
        )
        .route(
            "/api/agent/analytics/daily-snapshots",
            get(get_daily_snapshots),
        )
        // Analysis-based analytics (populated by every stock analysis)
        .route(
            "/api/agent/analytics/analysis-summary",
            get(get_analysis_summary),
        )
        .route(
            "/api/agent/analytics/analysis-history",
            get(get_analysis_history),
        )
        .route(
            "/api/agent/analytics/signal-distribution",
            get(get_signal_distribution),
        )
        .route(
            "/api/agent/analytics/regime-distribution",
            get(get_regime_distribution),
        )
        .route(
            "/api/agent/analytics/conviction-distribution",
            get(get_conviction_distribution),
        )
        .route(
            "/api/agent/analytics/top-symbols",
            get(get_top_analyzed_symbols),
        )
}

#[utoipa::path(
    get,
    path = "/api/agent/trades",
    tag = "Agent",
    responses(
        (status = 200, description = "List of pending trades", body = [PendingTrade]),
    ),
)]
/// List pending trades (optionally filter by status)
async fn list_pending_trades(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PendingTrade>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let trades: Vec<PendingTrade> = sqlx::query_as(
        "SELECT id, symbol, action, shares, confidence, reason, signal_type,
                proposed_at, status, reviewed_at, price, order_id
         FROM pending_trades
         ORDER BY CASE status WHEN 'pending' THEN 0 ELSE 1 END, proposed_at DESC
         LIMIT 100",
    )
    .fetch_all(pool)
    .await?;

    Ok(Json(ApiResponse::success(trades)))
}

#[utoipa::path(
    get,
    path = "/api/agent/trades/{id}",
    tag = "Agent",
    params(
        ("id" = i64, Path, description = "Pending trade ID"),
    ),
    responses(
        (status = 200, description = "Pending trade details", body = PendingTrade),
    ),
)]
/// Get a single pending trade
async fn get_pending_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<PendingTrade>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let trade: PendingTrade = sqlx::query_as(
        "SELECT id, symbol, action, shares, confidence, reason, signal_type,
                proposed_at, status, reviewed_at, price, order_id
         FROM pending_trades WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(Json(ApiResponse::success(trade)))
}

#[utoipa::path(
    post,
    path = "/api/agent/trades",
    tag = "Agent",
    request_body = ProposeTrade,
    responses(
        (status = 200, description = "Trade proposed successfully", body = PendingTrade),
    ),
)]
/// Agent proposes a new trade (queued for human review)
async fn propose_trade(
    State(state): State<AppState>,
    Json(req): Json<ProposeTrade>,
) -> Result<Json<ApiResponse<PendingTrade>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let symbol = req.symbol.to_uppercase();
    let confidence = req.confidence.unwrap_or(0.5);
    let reason = req.reason.unwrap_or_default();
    let signal_type = req.signal_type.unwrap_or_else(|| "Neutral".to_string());

    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO pending_trades (symbol, action, shares, confidence, reason, signal_type)
         VALUES (?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(&symbol)
    .bind(&req.action)
    .bind(req.shares)
    .bind(confidence)
    .bind(&reason)
    .bind(&signal_type)
    .fetch_one(pool)
    .await?;

    let trade: PendingTrade = sqlx::query_as(
        "SELECT id, symbol, action, shares, confidence, reason, signal_type,
                proposed_at, status, reviewed_at, price, order_id
         FROM pending_trades WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    tracing::info!(
        "Agent proposed trade: {} {} shares of {} (confidence: {:.0}%)",
        req.action,
        req.shares,
        symbol,
        confidence * 100.0
    );

    let details = serde_json::json!({
        "shares": req.shares, "confidence": confidence, "reason": reason
    })
    .to_string();
    audit::log_audit(
        pool,
        "agent_trade_proposed",
        Some(&symbol),
        Some(&req.action),
        Some(&details),
        "agent",
        None,
    )
    .await;

    // Notify: agent trade proposal
    if let Some(ref notif) = state.notification {
        notif.send_alert(Alert::new(
            AlertType::AgentTradeProposal {
                symbol: symbol.clone(),
                action: req.action.clone(),
                confidence,
                reason: reason.clone(),
            },
            format!("Agent: {} {}", req.action.to_uppercase(), symbol),
            format!("{:.0}% confidence — {}", confidence * 100.0, reason),
        ));
    }

    Ok(Json(ApiResponse::success(trade)))
}

#[utoipa::path(
    post,
    path = "/api/agent/trades/{id}/review",
    tag = "Agent",
    params(
        ("id" = i64, Path, description = "Pending trade ID to review"),
    ),
    request_body = ReviewTrade,
    responses(
        (status = 200, description = "Trade reviewed successfully"),
    ),
)]
/// Human approves or rejects a pending trade
async fn review_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<ReviewTrade>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pm = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?;
    let pool = pm.db().pool();

    // Atomically claim the trade by updating status only if still pending.
    // This prevents double-spend if two approvals race.
    let claim_result = sqlx::query(
        "UPDATE pending_trades SET status = 'claimed', reviewed_at = ? WHERE id = ? AND status = 'pending'"
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .execute(pool)
    .await?;

    if claim_result.rows_affected() == 0 {
        let current_status: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM pending_trades WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        let status = current_status.map(|r| r.0).unwrap_or_else(|| "not found".to_string());
        return Err(anyhow::anyhow!("Trade {} is already {}", id, status).into());
    }

    // Now fetch the full trade details (we own it exclusively)
    let trade: PendingTrade = sqlx::query_as(
        "SELECT id, symbol, action, shares, confidence, reason, signal_type,
                proposed_at, status, reviewed_at, price, order_id
         FROM pending_trades WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    match req.action.as_str() {
        "approve" => {
            // Execute the trade through broker
            let broker = state
                .broker_client
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Broker not configured"))?;

            let shares_decimal = Decimal::from_f64_retain(trade.shares)
                .ok_or_else(|| anyhow::anyhow!("Invalid shares value: {}", trade.shares))?;

            let market_order = match trade.action.as_str() {
                "buy" => broker_trait::BrokerOrderRequest::buy(&trade.symbol, shares_decimal),
                "sell" => broker_trait::BrokerOrderRequest::sell(&trade.symbol, shares_decimal),
                _ => return Err(anyhow::anyhow!("Invalid trade action: {}", trade.action).into()),
            };

            let order = broker.submit_market_order(market_order).await?;

            sqlx::query(
                "UPDATE pending_trades SET status = 'executed', reviewed_at = ?, order_id = ? WHERE id = ?"
            )
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(&order.id)
            .bind(id)
            .execute(pool)
            .await?;

            tracing::info!(
                "Approved agent trade {}: {} {} shares of {} -> order {}",
                id,
                trade.action,
                trade.shares,
                trade.symbol,
                order.id
            );

            audit::log_audit(
                pool,
                "agent_trade_approved",
                Some(&trade.symbol),
                Some(&trade.action),
                Some(&format!("trade_id={}, shares={}", id, trade.shares)),
                "user",
                Some(&order.id),
            )
            .await;

            Ok(Json(ApiResponse::success(serde_json::json!({
                "trade_id": id,
                "status": "executed",
                "order_id": order.id,
                "order_status": order.status,
            }))))
        }
        "reject" => {
            sqlx::query(
                "UPDATE pending_trades SET status = 'rejected', reviewed_at = ? WHERE id = ?",
            )
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await?;

            // Record rejection in trade context
            sqlx::query(
                "UPDATE agent_trade_context_v2 SET exit_reason='REJECTED', outcome='rejected', exit_date=?
                 WHERE pending_trade_id = ?"
            )
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .ok();

            tracing::info!(
                "Rejected agent trade {}: {} {} shares of {}",
                id,
                trade.action,
                trade.shares,
                trade.symbol
            );

            audit::log_audit(
                pool,
                "agent_trade_rejected",
                Some(&trade.symbol),
                Some(&trade.action),
                Some(&format!("trade_id={}, shares={}", id, trade.shares)),
                "user",
                None,
            )
            .await;

            Ok(Json(ApiResponse::success(serde_json::json!({
                "trade_id": id,
                "status": "rejected",
            }))))
        }
        _ => Err(anyhow::anyhow!("Invalid review action: must be 'approve' or 'reject'").into()),
    }
}

// ============================================================================
// Analytics endpoints
// ============================================================================

#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
struct TradeContextRow {
    id: i64,
    pending_trade_id: Option<i64>,
    symbol: String,
    action: String,
    entry_price: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    entry_regime: Option<String>,
    conviction_tier: Option<String>,
    entry_confidence: Option<f64>,
    entry_atr: Option<f64>,
    ml_probability: Option<f64>,
    ml_reasoning: Option<String>,
    ml_features_json: Option<String>,
    technical_reason: Option<String>,
    fundamental_reason: Option<String>,
    sentiment_score: Option<f64>,
    signal_adjustments: Option<String>,
    supplementary_signals: Option<String>,
    engine_signals_json: Option<String>,
    time_horizon_signals: Option<String>,
    created_at: String,
    exit_regime: Option<String>,
    exit_reason: Option<String>,
    exit_price: Option<f64>,
    exit_date: Option<String>,
    pnl: Option<f64>,
    pnl_percent: Option<f64>,
    outcome: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/agent/trades/{id}/context",
    tag = "Agent",
    params(
        ("id" = i64, Path, description = "Pending trade ID"),
    ),
    responses(
        (status = 200, description = "Trade context with ML features and signals", body = TradeContextRow),
    ),
)]
/// GET /api/agent/trades/:id/context
async fn get_trade_context(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<TradeContextRow>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let ctx: TradeContextRow =
        sqlx::query_as("SELECT * FROM agent_trade_context_v2 WHERE pending_trade_id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;

    Ok(Json(ApiResponse::success(ctx)))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/summary",
    tag = "Agent",
    responses(
        (status = 200, description = "Agent trading analytics summary"),
    ),
)]
/// GET /api/agent/analytics/summary
async fn get_analytics_summary(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let row: Option<(i64, i64, i64, f64)> = sqlx::query_as(
        "SELECT
            COUNT(*) as total,
            COALESCE(SUM(CASE WHEN outcome = 'win' THEN 1 ELSE 0 END), 0) as wins,
            COALESCE(SUM(CASE WHEN outcome = 'loss' THEN 1 ELSE 0 END), 0) as losses,
            COALESCE(SUM(pnl), 0.0) as total_pnl
         FROM agent_trade_context_v2
         WHERE outcome IS NOT NULL AND outcome != 'rejected'",
    )
    .fetch_optional(pool)
    .await?;

    let (total, wins, losses, total_pnl) = row.unwrap_or((0, 0, 0, 0.0));
    let win_rate = if (wins + losses) > 0 {
        wins as f64 / (wins + losses) as f64
    } else {
        0.0
    };

    // Load metrics from agent_state
    let metrics_json: Option<(String,)> =
        sqlx::query_as("SELECT value FROM agent_state WHERE key = 'agent_metrics'")
            .fetch_optional(pool)
            .await?;

    let metrics: serde_json::Value = metrics_json
        .and_then(|(s,)| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    let ml_approved = metrics
        .get("signals_ml_approved")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let ml_rejected = metrics
        .get("signals_ml_rejected")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let ml_total = ml_approved + ml_rejected;
    let ml_gate_rate = if ml_total > 0 {
        ml_approved as f64 / ml_total as f64
    } else {
        0.0
    };

    Ok(Json(ApiResponse::success(serde_json::json!({
        "total_trades": total,
        "wins": wins,
        "losses": losses,
        "total_pnl": total_pnl,
        "win_rate": win_rate,
        "ml_gate_approval_rate": ml_gate_rate,
        "signals_ml_approved": ml_approved,
        "signals_ml_rejected": ml_rejected,
        "agent_metrics": metrics,
    }))))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/win-rate-by-regime",
    tag = "Agent",
    responses(
        (status = 200, description = "Win rate broken down by market regime"),
    ),
)]
/// GET /api/agent/analytics/win-rate-by-regime
async fn get_win_rate_by_regime(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT
            COALESCE(entry_regime, 'unknown') as regime,
            COUNT(*) as total,
            COALESCE(SUM(CASE WHEN outcome = 'win' THEN 1 ELSE 0 END), 0) as wins
         FROM agent_trade_context_v2
         WHERE outcome IS NOT NULL AND outcome != 'rejected'
         GROUP BY entry_regime
         ORDER BY total DESC",
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(regime, total, wins)| {
            serde_json::json!({
                "regime": regime,
                "total": total,
                "wins": wins,
                "win_rate": if total > 0 { wins as f64 / total as f64 } else { 0.0 },
            })
        })
        .collect();

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/win-rate-by-conviction",
    tag = "Agent",
    responses(
        (status = 200, description = "Win rate broken down by conviction tier"),
    ),
)]
/// GET /api/agent/analytics/win-rate-by-conviction
async fn get_win_rate_by_conviction(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT
            COALESCE(conviction_tier, 'UNKNOWN') as tier,
            COUNT(*) as total,
            COALESCE(SUM(CASE WHEN outcome = 'win' THEN 1 ELSE 0 END), 0) as wins
         FROM agent_trade_context_v2
         WHERE outcome IS NOT NULL AND outcome != 'rejected'
         GROUP BY conviction_tier
         ORDER BY total DESC",
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(tier, total, wins)| {
            serde_json::json!({
                "conviction_tier": tier,
                "total": total,
                "wins": wins,
                "win_rate": if total > 0 { wins as f64 / total as f64 } else { 0.0 },
            })
        })
        .collect();

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/pnl-by-symbol",
    tag = "Agent",
    responses(
        (status = 200, description = "P&L broken down by symbol"),
    ),
)]
/// GET /api/agent/analytics/pnl-by-symbol
async fn get_pnl_by_symbol(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let rows: Vec<(String, i64, i64, f64)> = sqlx::query_as(
        "SELECT
            symbol,
            COUNT(*) as total,
            COALESCE(SUM(CASE WHEN outcome = 'win' THEN 1 ELSE 0 END), 0) as wins,
            COALESCE(SUM(pnl), 0.0) as total_pnl
         FROM agent_trade_context_v2
         WHERE outcome IS NOT NULL AND outcome != 'rejected'
         GROUP BY symbol
         ORDER BY total_pnl DESC",
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(symbol, total, wins, total_pnl)| {
            serde_json::json!({
                "symbol": symbol,
                "total": total,
                "wins": wins,
                "total_pnl": total_pnl,
                "win_rate": if total > 0 { wins as f64 / total as f64 } else { 0.0 },
            })
        })
        .collect();

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/confidence-calibration",
    tag = "Agent",
    responses(
        (status = 200, description = "Confidence calibration buckets with win rates"),
    ),
)]
/// GET /api/agent/analytics/confidence-calibration
async fn get_confidence_calibration(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    // Bucket entry_confidence into ranges
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT
            CASE
                WHEN entry_confidence < 0.6 THEN '50-60%'
                WHEN entry_confidence < 0.7 THEN '60-70%'
                WHEN entry_confidence < 0.8 THEN '70-80%'
                ELSE '80%+'
            END as bucket,
            COUNT(*) as total,
            COALESCE(SUM(CASE WHEN outcome = 'win' THEN 1 ELSE 0 END), 0) as wins
         FROM agent_trade_context_v2
         WHERE outcome IS NOT NULL AND outcome != 'rejected' AND entry_confidence IS NOT NULL
         GROUP BY bucket
         ORDER BY bucket",
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(bucket, total, wins)| {
            serde_json::json!({
                "bucket": bucket,
                "total": total,
                "wins": wins,
                "win_rate": if total > 0 { wins as f64 / total as f64 } else { 0.0 },
            })
        })
        .collect();

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/supplementary-outcomes",
    tag = "Agent",
    responses(
        (status = 200, description = "Win rates by supplementary signal adjustment type"),
    ),
)]
/// GET /api/agent/analytics/supplementary-outcomes
async fn get_supplementary_outcomes(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    // Fetch all completed trades with signal adjustments
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT signal_adjustments, outcome FROM agent_trade_context_v2
         WHERE outcome IS NOT NULL AND outcome != 'rejected'
           AND signal_adjustments IS NOT NULL AND signal_adjustments != '[]'",
    )
    .fetch_all(pool)
    .await?;

    // Parse and aggregate by adjustment type
    let mut counts: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();
    for (adj_json, outcome) in &rows {
        if let Ok(adjustments) = serde_json::from_str::<Vec<String>>(adj_json) {
            for adj in adjustments {
                let adj_type = adj.split('(').next().unwrap_or(&adj).trim().to_string();
                let entry = counts.entry(adj_type).or_insert((0, 0));
                entry.0 += 1; // total
                if outcome == "win" {
                    entry.1 += 1; // wins
                }
            }
        }
    }

    let result: Vec<serde_json::Value> = counts
        .into_iter()
        .map(|(adj_type, (total, wins))| {
            serde_json::json!({
                "adjustment_type": adj_type,
                "total": total,
                "wins": wins,
                "win_rate": if total > 0 { wins as f64 / total as f64 } else { 0.0 },
            })
        })
        .collect();

    Ok(Json(ApiResponse::success(result)))
}

#[derive(Deserialize, utoipa::IntoParams)]
struct SnapshotQuery {
    days: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/daily-snapshots",
    tag = "Agent",
    params(SnapshotQuery),
    responses(
        (status = 200, description = "Daily agent performance snapshots"),
    ),
)]
/// GET /api/agent/analytics/daily-snapshots?days=30
async fn get_daily_snapshots(
    State(state): State<AppState>,
    Query(query): Query<SnapshotQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let days = query.days.unwrap_or(30).min(365);

    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        String,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        f64,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT snapshot_date, cycles_run, signals_generated, signals_filtered,
                signals_ml_approved, signals_ml_rejected, trades_proposed,
                winning_trades, losing_trades, total_pnl, regime
         FROM agent_daily_snapshots
         ORDER BY snapshot_date DESC
         LIMIT ?",
    )
    .bind(days)
    .fetch_all(pool)
    .await?;

    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(
            |(date, cycles, gen, filt, approved, rejected, proposed, wins, losses, pnl, regime)| {
                serde_json::json!({
                    "date": date,
                    "cycles_run": cycles,
                    "signals_generated": gen,
                    "signals_filtered": filt,
                    "signals_ml_approved": approved,
                    "signals_ml_rejected": rejected,
                    "trades_proposed": proposed,
                    "winning_trades": wins,
                    "losing_trades": losses,
                    "total_pnl": pnl,
                    "regime": regime,
                })
            },
        )
        .collect();

    Ok(Json(ApiResponse::success(result)))
}

// =====================================================
// Analysis-based analytics (from analysis_features table)
// =====================================================

#[derive(Deserialize, utoipa::IntoParams)]
struct AnalysisHistoryQuery {
    days: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/analysis-summary",
    tag = "Agent",
    responses(
        (status = 200, description = "Summary of all stock analyses"),
    ),
)]
/// GET /api/agent/analytics/analysis-summary
async fn get_analysis_summary(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let row: Option<(i64, f64)> = sqlx::query_as(
        "SELECT COUNT(*) as total, COALESCE(AVG(overall_confidence), 0.0) as avg_conf
         FROM analysis_features",
    )
    .fetch_optional(pool)
    .await?;

    let (total, avg_confidence) = row.unwrap_or((0, 0.0));

    // Count unique symbols
    let symbols_row: Option<(i64,)> =
        sqlx::query_as("SELECT COUNT(DISTINCT symbol) FROM analysis_features")
            .fetch_optional(pool)
            .await?;
    let unique_symbols = symbols_row.map(|r| r.0).unwrap_or(0);

    // Signal breakdown
    let signal_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT overall_signal, COUNT(*) as cnt
         FROM analysis_features
         GROUP BY overall_signal
         ORDER BY cnt DESC",
    )
    .fetch_all(pool)
    .await?;

    let signals: serde_json::Value = signal_rows
        .iter()
        .map(|(s, c)| serde_json::json!({"signal": s, "count": c}))
        .collect();

    Ok(Json(ApiResponse::success(serde_json::json!({
        "total_analyses": total,
        "avg_confidence": avg_confidence,
        "unique_symbols": unique_symbols,
        "signal_breakdown": signals,
    }))))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/analysis-history",
    tag = "Agent",
    params(AnalysisHistoryQuery),
    responses(
        (status = 200, description = "Historical analysis records"),
    ),
)]
/// GET /api/agent/analytics/analysis-history?days=30
async fn get_analysis_history(
    State(state): State<AppState>,
    Query(query): Query<AnalysisHistoryQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let days = query.days.unwrap_or(30).min(365);

    let rows: Vec<(i64, String, String, String, f64, String)> = sqlx::query_as(
        "SELECT id, symbol, analysis_date, overall_signal, overall_confidence, features_json
         FROM analysis_features
         ORDER BY id DESC
         LIMIT ?",
    )
    .bind(days * 10) // ~10 analyses per day max
    .fetch_all(pool)
    .await?;

    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, symbol, date, signal, confidence, features_json)| {
            let features: serde_json::Value =
                serde_json::from_str(&features_json).unwrap_or_default();
            let regime = features
                .get("market_regime")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let conviction = features
                .get("conviction_tier")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN");
            serde_json::json!({
                "id": id,
                "symbol": symbol,
                "date": date,
                "signal": signal,
                "confidence": confidence,
                "regime": regime,
                "conviction": conviction,
            })
        })
        .collect();

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/signal-distribution",
    tag = "Agent",
    responses(
        (status = 200, description = "Distribution of signal types with avg confidence"),
    ),
)]
/// GET /api/agent/analytics/signal-distribution
async fn get_signal_distribution(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let rows: Vec<(String, i64, f64)> = sqlx::query_as(
        "SELECT overall_signal, COUNT(*) as cnt, AVG(overall_confidence) as avg_conf
         FROM analysis_features
         GROUP BY overall_signal
         ORDER BY cnt DESC",
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(signal, count, avg_conf)| {
            serde_json::json!({
                "signal": signal,
                "count": count,
                "avg_confidence": avg_conf,
            })
        })
        .collect();

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/regime-distribution",
    tag = "Agent",
    responses(
        (status = 200, description = "Distribution of market regimes across analyses"),
    ),
)]
/// GET /api/agent/analytics/regime-distribution
async fn get_regime_distribution(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let rows: Vec<(String, f64)> =
        sqlx::query_as("SELECT features_json, overall_confidence FROM analysis_features")
            .fetch_all(pool)
            .await?;

    // Parse regime from features_json
    let mut regime_counts: std::collections::HashMap<String, (i64, f64)> =
        std::collections::HashMap::new();
    for (features_json, confidence) in &rows {
        let regime = serde_json::from_str::<serde_json::Value>(features_json)
            .ok()
            .and_then(|v| {
                v.get("market_regime")
                    .and_then(|r| r.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());
        let entry = regime_counts.entry(regime).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += confidence;
    }

    let mut result: Vec<serde_json::Value> = regime_counts
        .into_iter()
        .map(|(regime, (count, total_conf))| {
            serde_json::json!({
                "regime": regime,
                "count": count,
                "avg_confidence": if count > 0 { total_conf / count as f64 } else { 0.0 },
            })
        })
        .collect();
    result.sort_by(|a, b| b["count"].as_i64().cmp(&a["count"].as_i64()));

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/conviction-distribution",
    tag = "Agent",
    responses(
        (status = 200, description = "Distribution of conviction tiers across analyses"),
    ),
)]
/// GET /api/agent/analytics/conviction-distribution
async fn get_conviction_distribution(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let rows: Vec<(String, f64)> =
        sqlx::query_as("SELECT features_json, overall_confidence FROM analysis_features")
            .fetch_all(pool)
            .await?;

    let mut conviction_counts: std::collections::HashMap<String, (i64, f64)> =
        std::collections::HashMap::new();
    for (features_json, confidence) in &rows {
        let conviction = serde_json::from_str::<serde_json::Value>(features_json)
            .ok()
            .and_then(|v| {
                v.get("conviction_tier")
                    .and_then(|r| r.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let entry = conviction_counts.entry(conviction).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += confidence;
    }

    let mut result: Vec<serde_json::Value> = conviction_counts
        .into_iter()
        .map(|(tier, (count, total_conf))| {
            serde_json::json!({
                "conviction_tier": tier,
                "count": count,
                "avg_confidence": if count > 0 { total_conf / count as f64 } else { 0.0 },
            })
        })
        .collect();
    result.sort_by(|a, b| b["count"].as_i64().cmp(&a["count"].as_i64()));

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/agent/analytics/top-symbols",
    tag = "Agent",
    responses(
        (status = 200, description = "Top analyzed symbols by frequency"),
    ),
)]
/// GET /api/agent/analytics/top-symbols
async fn get_top_analyzed_symbols(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db()
        .pool();

    let rows: Vec<(String, i64, f64)> = sqlx::query_as(
        "SELECT symbol, COUNT(*) as cnt, AVG(overall_confidence) as avg_conf
         FROM analysis_features
         GROUP BY symbol
         ORDER BY cnt DESC
         LIMIT 20",
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(symbol, count, avg_conf)| {
            serde_json::json!({
                "symbol": symbol,
                "count": count,
                "avg_confidence": avg_conf,
            })
        })
        .collect();

    Ok(Json(ApiResponse::success(result)))
}
