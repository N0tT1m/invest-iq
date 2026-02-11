//! Agent Trade Approval Routes
//!
//! The trading agent queues proposed trades here for human approval.
//! Trades stay "pending" until approved or rejected from the dashboard.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{audit, ApiResponse, AppError, AppState};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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

#[derive(Deserialize)]
pub struct ProposeTrade {
    pub symbol: String,
    pub action: String,
    pub shares: f64,
    pub confidence: Option<f64>,
    pub reason: Option<String>,
    pub signal_type: Option<String>,
}

#[derive(Deserialize)]
pub struct ReviewTrade {
    pub action: String, // "approve" or "reject"
}

/// Initialize the pending_trades table
pub async fn init_pending_trades_table(_pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    // Table is now created by sqlx migrations.
    Ok(())
}

pub fn agent_trade_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agent/trades", get(list_pending_trades))
        .route("/api/agent/trades", post(propose_trade))
        .route("/api/agent/trades/:id/review", post(review_trade))
        .route("/api/agent/trades/:id", get(get_pending_trade))
}

/// List pending trades (optionally filter by status)
async fn list_pending_trades(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PendingTrade>>>, AppError> {
    let pool = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db().pool();

    let trades: Vec<PendingTrade> = sqlx::query_as(
        "SELECT id, symbol, action, shares, confidence, reason, signal_type,
                proposed_at, status, reviewed_at, price, order_id
         FROM pending_trades
         ORDER BY CASE status WHEN 'pending' THEN 0 ELSE 1 END, proposed_at DESC
         LIMIT 100"
    )
    .fetch_all(pool)
    .await?;

    Ok(Json(ApiResponse::success(trades)))
}

/// Get a single pending trade
async fn get_pending_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<PendingTrade>>, AppError> {
    let pool = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db().pool();

    let trade: PendingTrade = sqlx::query_as(
        "SELECT id, symbol, action, shares, confidence, reason, signal_type,
                proposed_at, status, reviewed_at, price, order_id
         FROM pending_trades WHERE id = ?"
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(Json(ApiResponse::success(trade)))
}

/// Agent proposes a new trade (queued for human review)
async fn propose_trade(
    State(state): State<AppState>,
    Json(req): Json<ProposeTrade>,
) -> Result<Json<ApiResponse<PendingTrade>>, AppError> {
    let pool = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db().pool();

    let symbol = req.symbol.to_uppercase();
    let confidence = req.confidence.unwrap_or(0.5);
    let reason = req.reason.unwrap_or_default();
    let signal_type = req.signal_type.unwrap_or_else(|| "Neutral".to_string());

    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO pending_trades (symbol, action, shares, confidence, reason, signal_type)
         VALUES (?, ?, ?, ?, ?, ?)
         RETURNING id"
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
         FROM pending_trades WHERE id = ?"
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    tracing::info!("Agent proposed trade: {} {} shares of {} (confidence: {:.0}%)",
        req.action, req.shares, symbol, confidence * 100.0);

    let details = serde_json::json!({
        "shares": req.shares, "confidence": confidence, "reason": reason
    }).to_string();
    audit::log_audit(pool, "agent_trade_proposed", Some(&symbol), Some(&req.action),
        Some(&details), "agent", None).await;

    Ok(Json(ApiResponse::success(trade)))
}

/// Human approves or rejects a pending trade
async fn review_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<ReviewTrade>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pm = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?;
    let pool = pm.db().pool();

    // Verify trade is still pending
    let trade: PendingTrade = sqlx::query_as(
        "SELECT id, symbol, action, shares, confidence, reason, signal_type,
                proposed_at, status, reviewed_at, price, order_id
         FROM pending_trades WHERE id = ?"
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    if trade.status != "pending" {
        return Err(anyhow::anyhow!("Trade {} is already {}", id, trade.status).into());
    }

    match req.action.as_str() {
        "approve" => {
            // Execute the trade through Alpaca
            let alpaca_client = state.alpaca_client.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Alpaca broker not configured"))?;

            let shares_decimal = Decimal::from_f64_retain(trade.shares)
                .ok_or_else(|| anyhow::anyhow!("Invalid shares value: {}", trade.shares))?;

            let market_order = match trade.action.as_str() {
                "buy" => alpaca_broker::MarketOrderRequest::buy(&trade.symbol, shares_decimal),
                "sell" => alpaca_broker::MarketOrderRequest::sell(&trade.symbol, shares_decimal),
                _ => return Err(anyhow::anyhow!("Invalid trade action: {}", trade.action).into()),
            };

            let order = alpaca_client.submit_market_order(market_order).await?;

            sqlx::query(
                "UPDATE pending_trades SET status = 'executed', reviewed_at = datetime('now'), order_id = ? WHERE id = ?"
            )
            .bind(&order.id)
            .bind(id)
            .execute(pool)
            .await?;

            tracing::info!("Approved agent trade {}: {} {} shares of {} -> order {}",
                id, trade.action, trade.shares, trade.symbol, order.id);

            audit::log_audit(pool, "agent_trade_approved", Some(&trade.symbol), Some(&trade.action),
                Some(&format!("trade_id={}, shares={}", id, trade.shares)), "user", Some(&order.id)).await;

            Ok(Json(ApiResponse::success(serde_json::json!({
                "trade_id": id,
                "status": "executed",
                "order_id": order.id,
                "order_status": order.status,
            }))))
        }
        "reject" => {
            sqlx::query(
                "UPDATE pending_trades SET status = 'rejected', reviewed_at = datetime('now') WHERE id = ?"
            )
            .bind(id)
            .execute(pool)
            .await?;

            tracing::info!("Rejected agent trade {}: {} {} shares of {}", id, trade.action, trade.shares, trade.symbol);

            audit::log_audit(pool, "agent_trade_rejected", Some(&trade.symbol), Some(&trade.action),
                Some(&format!("trade_id={}, shares={}", id, trade.shares)), "user", None).await;

            Ok(Json(ApiResponse::success(serde_json::json!({
                "trade_id": id,
                "status": "rejected",
            }))))
        }
        _ => Err(anyhow::anyhow!("Invalid review action: must be 'approve' or 'reject'").into()),
    }
}
