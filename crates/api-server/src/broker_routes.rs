use axum::{
    extract::{Path, State},
    middleware,
    routing::{delete, get, post},
    Json, Router,
};
use alpaca_broker::MarketOrderRequest;
use portfolio_manager::TradeInput;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::Deserialize;

use crate::{audit, auth, ApiResponse, AppError, AppState};

#[derive(Deserialize)]
pub struct ExecuteTradeRequest {
    pub symbol: String,
    pub action: String, // "buy" or "sell"
    pub shares: Decimal,
    pub confidence: Option<f64>,
    pub notes: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Deserialize)]
pub struct ClosePositionRequest {
    pub notes: Option<String>,
}

/// Read-only broker routes (account info, positions, orders) — standard API key auth only
pub fn broker_read_routes() -> Router<AppState> {
    Router::new()
        .route("/api/broker/account", get(get_account_info))
        .route("/api/broker/positions", get(get_broker_positions))
        .route("/api/broker/orders", get(get_orders))
        .route("/api/broker/orders/:id", get(get_order))
}

/// Write broker routes (execute, close, cancel) — requires additional live trading key
pub fn broker_write_routes() -> Router<AppState> {
    Router::new()
        .route("/api/broker/execute", post(execute_trade))
        .route("/api/broker/positions/:symbol", delete(close_broker_position))
        .route("/api/broker/orders/:id/cancel", post(cancel_order))
        .layer(middleware::from_fn(auth::live_trading_auth_middleware))
}

/// Get Alpaca account information
async fn get_account_info(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<alpaca_broker::Account>>, AppError> {
    let alpaca_client = state.alpaca_client.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alpaca broker not configured"))?;

    let account = alpaca_client.get_account().await?;

    Ok(Json(ApiResponse::success(account)))
}

/// Helper: sum unrealized intraday P&L from Alpaca positions
async fn account_daily_pl(alpaca_client: &alpaca_broker::AlpacaClient) -> f64 {
    match alpaca_client.get_positions().await {
        Ok(positions) => {
            positions.iter()
                .filter_map(|p| p.unrealized_intraday_pl.parse::<f64>().ok())
                .sum()
        }
        Err(_) => 0.0,
    }
}

/// Execute a trade through Alpaca
async fn execute_trade(
    State(state): State<AppState>,
    Json(req): Json<ExecuteTradeRequest>,
) -> Result<Json<ApiResponse<alpaca_broker::Order>>, AppError> {
    let alpaca_client = state.alpaca_client.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alpaca broker not configured"))?;

    let symbol = req.symbol.to_uppercase();
    let is_buy = req.action.to_lowercase() == "buy";

    // Get DB pool for audit logging, idempotency, and transactions
    let pool = state.portfolio_manager.as_ref().map(|pm| pm.db().pool().clone());

    // --- Idempotency check ---
    if let (Some(ref idem_key), Some(ref pool)) = (&req.idempotency_key, &pool) {
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT response_json FROM trade_idempotency WHERE idempotency_key = ? AND expires_at > datetime('now')"
        )
        .bind(idem_key)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some((response_json,)) = existing {
            tracing::info!("Idempotency hit for key={}", idem_key);
            if let Ok(cached_order) = serde_json::from_str::<alpaca_broker::Order>(&response_json) {
                return Ok(Json(ApiResponse::success(cached_order)));
            }
        }
    }

    // Pre-trade risk checks for buy orders
    if is_buy {
        if let Some(risk_manager) = state.risk_manager.as_ref() {
            let account = alpaca_client.get_account().await?;
            let portfolio_value = account.portfolio_value.parse::<f64>().unwrap_or(0.0);
            let cash = account.cash.parse::<f64>().unwrap_or(0.0);
            let buying_power = account.buying_power.parse::<f64>().unwrap_or(0.0);

            let positions = alpaca_client.get_positions().await.unwrap_or_default();
            let positions_count = positions.len() as i32;
            let positions_value: f64 = positions.iter()
                .filter_map(|p| p.market_value.parse::<f64>().ok())
                .sum();

            let daily_pl = account_daily_pl(alpaca_client).await;
            let cb_check = risk_manager.check_circuit_breakers(portfolio_value, daily_pl).await?;
            if !cb_check.can_trade {
                if let Some(ref pool) = pool {
                    audit::log_audit(pool, "circuit_breaker_triggered", Some(&symbol), Some(&req.action),
                        Some(&cb_check.reason), "user", None).await;
                }
                return Err(anyhow::anyhow!("Circuit breaker: {}", cb_check.reason).into());
            }

            if !alpaca_client.is_paper() {
                let confidence = req.confidence.unwrap_or(0.5);
                let risk_check = risk_manager.check_trade_risk(
                    confidence, cash, positions_value, positions_count,
                ).await?;
                if !risk_check.can_trade {
                    if let Some(ref pool) = pool {
                        audit::log_audit(pool, "risk_check_failed", Some(&symbol), Some(&req.action),
                            Some(&risk_check.reason), "user", None).await;
                    }
                    return Err(anyhow::anyhow!("Risk check failed: {}", risk_check.reason).into());
                }
            }

            if buying_power <= 0.0 {
                return Err(anyhow::anyhow!("Insufficient buying power: ${:.2}", buying_power).into());
            }
        }
    }

    // Create market order
    let market_order = match req.action.to_lowercase().as_str() {
        "buy" => MarketOrderRequest::buy(&symbol, req.shares),
        "sell" => MarketOrderRequest::sell(&symbol, req.shares),
        _ => return Err(anyhow::anyhow!("Invalid action: must be 'buy' or 'sell'").into()),
    };

    tracing::info!("Executing {} order: {} shares of {}", req.action, req.shares, symbol);

    // Submit order to Alpaca
    let order = alpaca_client.submit_market_order(market_order).await?;

    tracing::info!("Order submitted successfully: {} ({})", order.id, order.status);

    // Audit: order submitted
    if let Some(ref pool) = pool {
        let details = serde_json::json!({
            "shares": req.shares.to_f64().unwrap_or(0.0),
            "status": order.status,
        }).to_string();
        audit::log_audit(pool, "order_submitted", Some(&symbol), Some(&req.action),
            Some(&details), "user", Some(&order.id)).await;
    }

    // --- Post-fill: log trade, register risk, update portfolio in a transaction ---
    if let Some(trade_logger) = state.trade_logger.as_ref() {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let filled_order = alpaca_client.get_order(&order.id).await.unwrap_or(order.clone());

        let fill_price_f64 = if let Some(avg_price_str) = &filled_order.filled_avg_price {
            avg_price_str.parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };

        if fill_price_f64 > 0.0 {
            let fill_price = Decimal::from_f64(fill_price_f64).unwrap_or(Decimal::ZERO);

            // Use a DB transaction for all post-fill writes
            if let Some(ref pool) = pool {
                let mut tx = pool.begin().await.map_err(|e| anyhow::anyhow!("Transaction start failed: {}", e))?;

                // 1. Log trade
                let trade_result = sqlx::query(
                    "INSERT INTO trades (symbol, action, shares, price, trade_date, commission, notes)
                     VALUES (?, ?, ?, ?, ?, 0.0, ?)"
                )
                .bind(&symbol)
                .bind(&req.action)
                .bind(req.shares.to_f64().unwrap_or(0.0))
                .bind(fill_price_f64)
                .bind(chrono::Utc::now().format("%Y-%m-%d").to_string())
                .bind(req.notes.as_deref().unwrap_or(&format!("Auto-logged from Alpaca order {}", filled_order.id)))
                .execute(&mut *tx)
                .await;

                if let Err(e) = trade_result {
                    tracing::warn!("Failed to log trade in tx: {}", e);
                    let _ = tx.rollback().await;
                } else {
                    // 2. Register risk position for buys
                    if is_buy {
                        if let Some(risk_manager) = state.risk_manager.as_ref() {
                            let params = risk_manager.get_parameters().await.unwrap_or_default();
                            let stop_loss_price = fill_price_f64 * (1.0 - params.default_stop_loss_percent / 100.0);
                            let take_profit_price = fill_price_f64 * (1.0 + params.default_take_profit_percent / 100.0);

                            let _ = sqlx::query(
                                "INSERT OR REPLACE INTO active_risk_positions
                                 (symbol, shares, entry_price, entry_date, stop_loss_price, take_profit_price,
                                  trailing_stop_enabled, trailing_stop_percent, max_price_seen, status)
                                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active')"
                            )
                            .bind(&symbol)
                            .bind(req.shares.to_f64().unwrap_or(0.0))
                            .bind(fill_price_f64)
                            .bind(chrono::Utc::now().format("%Y-%m-%d").to_string())
                            .bind(stop_loss_price)
                            .bind(take_profit_price)
                            .bind(params.trailing_stop_enabled)
                            .bind(if params.trailing_stop_enabled { Some(params.trailing_stop_percent) } else { None })
                            .bind(fill_price_f64)
                            .execute(&mut *tx)
                            .await;
                        }
                    }

                    // 3. Update portfolio
                    if is_buy {
                        let _ = sqlx::query(
                            "INSERT OR REPLACE INTO positions (symbol, shares, entry_price, entry_date, notes)
                             VALUES (?, ?, ?, ?, 'Auto-added from Alpaca trade')"
                        )
                        .bind(&symbol)
                        .bind(req.shares.to_f64().unwrap_or(0.0))
                        .bind(fill_price_f64)
                        .bind(chrono::Utc::now().format("%Y-%m-%d").to_string())
                        .execute(&mut *tx)
                        .await;
                    } else {
                        let _ = sqlx::query(
                            "UPDATE positions SET shares = shares - ? WHERE symbol = ?"
                        )
                        .bind(req.shares.to_f64().unwrap_or(0.0))
                        .bind(&symbol)
                        .execute(&mut *tx)
                        .await;

                        // Remove position if shares <= 0
                        let _ = sqlx::query(
                            "DELETE FROM positions WHERE symbol = ? AND shares <= 0"
                        )
                        .bind(&symbol)
                        .execute(&mut *tx)
                        .await;
                    }

                    // 4. Audit log within transaction
                    let details = serde_json::json!({
                        "fill_price": fill_price_f64,
                        "shares": req.shares.to_f64().unwrap_or(0.0),
                        "order_status": filled_order.status,
                    }).to_string();
                    let _ = sqlx::query(
                        "INSERT INTO audit_log (event_type, symbol, action, details, user_id, order_id)
                         VALUES ('trade_executed', ?, ?, ?, 'user', ?)"
                    )
                    .bind(&symbol)
                    .bind(&req.action)
                    .bind(&details)
                    .bind(&order.id)
                    .execute(&mut *tx)
                    .await;

                    // Commit all writes atomically
                    if let Err(e) = tx.commit().await {
                        tracing::error!("Transaction commit failed: {}", e);
                    } else {
                        tracing::info!("Trade logged, risk registered, portfolio updated (tx committed)");
                        // Increment trade counter metric
                        state.metrics.trade_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            } else {
                // Fallback: no pool, use trade_logger directly (no transaction)
                let trade = TradeInput {
                    symbol: symbol.clone(),
                    action: req.action.clone(),
                    shares: req.shares,
                    price: fill_price,
                    trade_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                    commission: Some(Decimal::ZERO),
                    notes: req.notes.clone().or(Some(format!("Auto-logged from Alpaca order {}", filled_order.id))),
                };
                if trade_logger.log_trade(trade).await.is_ok() {
                    // Increment trade counter metric
                    state.metrics.trade_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    }

    // --- Store idempotency result ---
    if let (Some(ref idem_key), Some(ref pool)) = (&req.idempotency_key, &pool) {
        if let Ok(response_json) = serde_json::to_string(&order) {
            let _ = sqlx::query(
                "INSERT OR REPLACE INTO trade_idempotency (idempotency_key, order_id, symbol, action, shares, status, response_json, expires_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now', '+24 hours'))"
            )
            .bind(idem_key)
            .bind(&order.id)
            .bind(&symbol)
            .bind(&req.action)
            .bind(req.shares.to_f64().unwrap_or(0.0))
            .bind(&order.status)
            .bind(&response_json)
            .execute(pool)
            .await;
        }
    }

    Ok(Json(ApiResponse::success(order)))
}

/// Get all positions from Alpaca
async fn get_broker_positions(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<alpaca_broker::Position>>>, AppError> {
    let alpaca_client = state.alpaca_client.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alpaca broker not configured"))?;

    let positions = alpaca_client.get_positions().await?;

    Ok(Json(ApiResponse::success(positions)))
}

/// Close a position (sell all shares)
async fn close_broker_position(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(req): Json<ClosePositionRequest>,
) -> Result<Json<ApiResponse<alpaca_broker::Order>>, AppError> {
    let alpaca_client = state.alpaca_client.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alpaca broker not configured"))?;

    let symbol = symbol.to_uppercase();

    tracing::info!("Closing position for {}", symbol);

    let order = alpaca_client.close_position(&symbol).await?;

    // Auto-log if trade logger available
    if let Some(trade_logger) = state.trade_logger.as_ref() {
        // Wait for fill
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let filled_order = alpaca_client.get_order(&order.id).await.unwrap_or(order.clone());

        if let (Some(qty_str), Some(price_str)) = (&filled_order.filled_quantity, &filled_order.filled_avg_price) {
            if let (Ok(qty_f64), Ok(price_f64)) = (qty_str.parse::<f64>(), price_str.parse::<f64>()) {
                let qty = Decimal::from_f64(qty_f64).unwrap_or(Decimal::ZERO);
                let price = Decimal::from_f64(price_f64).unwrap_or(Decimal::ZERO);
                let trade = TradeInput {
                    symbol: symbol.clone(),
                    action: "sell".to_string(),
                    shares: qty,
                    price,
                    trade_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                    commission: Some(Decimal::ZERO),
                    notes: req.notes.or(Some(format!("Closed position via Alpaca order {}", filled_order.id))),
                };

                if trade_logger.log_trade(trade).await.is_ok() {
                    // Increment trade counter metric
                    state.metrics.trade_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }

        // Remove from portfolio
        if let Some(portfolio_manager) = state.portfolio_manager.as_ref() {
            let _ = portfolio_manager.delete_position(&symbol).await;
        }
    }

    Ok(Json(ApiResponse::success(order)))
}

/// Get all orders
async fn get_orders(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<alpaca_broker::Order>>>, AppError> {
    let alpaca_client = state.alpaca_client.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alpaca broker not configured"))?;

    let orders = alpaca_client.get_orders(Some(50)).await?;

    Ok(Json(ApiResponse::success(orders)))
}

/// Get a specific order
async fn get_order(
    State(state): State<AppState>,
    Path(order_id): Path<String>,
) -> Result<Json<ApiResponse<alpaca_broker::Order>>, AppError> {
    let alpaca_client = state.alpaca_client.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alpaca broker not configured"))?;

    let order = alpaca_client.get_order(&order_id).await?;

    Ok(Json(ApiResponse::success(order)))
}

/// Cancel an order
async fn cancel_order(
    State(state): State<AppState>,
    Path(order_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let alpaca_client = state.alpaca_client.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alpaca broker not configured"))?;

    alpaca_client.cancel_order(&order_id).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({
        "message": "Order canceled successfully",
        "order_id": order_id
    }))))
}
