use axum::{
    extract::{Path, State},
    middleware,
    routing::{delete, get, post},
    Json, Router,
};
use alpaca_broker::MarketOrderRequest;
use portfolio_manager::{Position, TradeInput};
use risk_manager::ActiveRiskPosition;
use serde::Deserialize;

use crate::{auth, ApiResponse, AppError, AppState};

#[derive(Deserialize)]
pub struct ExecuteTradeRequest {
    pub symbol: String,
    pub action: String, // "buy" or "sell"
    pub shares: f64,
    pub confidence: Option<f64>,
    pub notes: Option<String>,
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

    // Pre-trade risk checks for buy orders
    if is_buy {
        if let Some(risk_manager) = state.risk_manager.as_ref() {
            // Fetch account data from Alpaca
            let account = alpaca_client.get_account().await?;
            let portfolio_value = account.portfolio_value.parse::<f64>().unwrap_or(0.0);
            let cash = account.cash.parse::<f64>().unwrap_or(0.0);
            let buying_power = account.buying_power.parse::<f64>().unwrap_or(0.0);

            // Get positions for count and total market value
            let positions = alpaca_client.get_positions().await.unwrap_or_default();
            let positions_count = positions.len() as i32;
            let positions_value: f64 = positions.iter()
                .filter_map(|p| p.market_value.parse::<f64>().ok())
                .sum();

            // Check circuit breakers
            let daily_pl = account_daily_pl(alpaca_client).await;
            let cb_check = risk_manager.check_circuit_breakers(portfolio_value, daily_pl).await?;
            if !cb_check.can_trade {
                return Err(anyhow::anyhow!("Circuit breaker: {}", cb_check.reason).into());
            }

            // Check per-trade risk (skip confidence gate for paper trading)
            if !alpaca_client.is_paper() {
                let confidence = req.confidence.unwrap_or(0.5);
                let risk_check = risk_manager.check_trade_risk(
                    confidence, cash, positions_value, positions_count,
                ).await?;
                if !risk_check.can_trade {
                    return Err(anyhow::anyhow!("Risk check failed: {}", risk_check.reason).into());
                }
            }

            // Verify we have sufficient buying power
            // (rough check — actual order might differ slightly)
            // We don't have a live price here, so just check buying_power > 0
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

    // Auto-log the trade if trade logger is available
    if let Some(trade_logger) = state.trade_logger.as_ref() {
        // Wait a moment for order to fill
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Get updated order status
        let filled_order = alpaca_client.get_order(&order.id).await.unwrap_or(order.clone());

        // Get fill price (use average fill price if available)
        let fill_price = if let Some(avg_price_str) = &filled_order.filled_avg_price {
            avg_price_str.parse::<f64>().unwrap_or(0.0)
        } else {
            0.0 // Will be updated later when order fills
        };

        if fill_price > 0.0 {
            let trade = TradeInput {
                symbol: symbol.clone(),
                action: req.action.clone(),
                shares: req.shares,
                price: fill_price,
                trade_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                commission: Some(0.0), // Alpaca is commission-free
                notes: req.notes.or(Some(format!("Auto-logged from Alpaca order {}", filled_order.id))),
            };

            match trade_logger.log_trade(trade).await {
                Ok(id) => tracing::info!("Trade auto-logged with ID: {}", id),
                Err(e) => tracing::warn!("Failed to auto-log trade: {}", e),
            }

            // Register with risk manager for stop-loss tracking
            if req.action == "buy" {
                if let Some(risk_manager) = state.risk_manager.as_ref() {
                    let params = risk_manager.get_parameters().await.unwrap_or_default();
                    let stop_loss_price = fill_price * (1.0 - params.default_stop_loss_percent / 100.0);
                    let take_profit_price = fill_price * (1.0 + params.default_take_profit_percent / 100.0);

                    let risk_position = ActiveRiskPosition {
                        id: None,
                        symbol: symbol.clone(),
                        shares: req.shares,
                        entry_price: fill_price,
                        entry_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                        stop_loss_price: Some(stop_loss_price),
                        take_profit_price: Some(take_profit_price),
                        trailing_stop_enabled: params.trailing_stop_enabled,
                        trailing_stop_percent: if params.trailing_stop_enabled { Some(params.trailing_stop_percent) } else { None },
                        max_price_seen: Some(fill_price),
                        risk_amount: None,
                        position_size_percent: None,
                        status: "active".to_string(),
                        created_at: None,
                        closed_at: None,
                    };

                    match risk_manager.add_active_position(&risk_position).await {
                        Ok(id) => tracing::info!("Risk position registered (id: {}, SL: ${:.2}, TP: ${:.2})", id, stop_loss_price, take_profit_price),
                        Err(e) => tracing::warn!("Failed to register risk position: {}", e),
                    }
                }
            }

            // Also update portfolio if it's a buy
            if req.action == "buy" {
                if let Some(portfolio_manager) = state.portfolio_manager.as_ref() {
                    let position = Position {
                        id: None,
                        symbol: symbol.clone(),
                        shares: req.shares,
                        entry_price: fill_price,
                        entry_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                        notes: Some("Auto-added from Alpaca trade".to_string()),
                        created_at: None,
                    };

                    match portfolio_manager.add_position(position).await {
                        Ok(_) => tracing::info!("Portfolio updated with new position"),
                        Err(e) => tracing::warn!("Failed to update portfolio: {}", e),
                    }
                }
            } else if req.action == "sell" {
                if let Some(portfolio_manager) = state.portfolio_manager.as_ref() {
                    match portfolio_manager.remove_shares(&symbol, req.shares).await {
                        Ok(_) => tracing::info!("Portfolio updated - shares removed"),
                        Err(e) => tracing::warn!("Failed to update portfolio: {}", e),
                    }
                }
            }
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
            if let (Ok(qty), Ok(price)) = (qty_str.parse::<f64>(), price_str.parse::<f64>()) {
                let trade = TradeInput {
                    symbol: symbol.clone(),
                    action: "sell".to_string(),
                    shares: qty,
                    price,
                    trade_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                    commission: Some(0.0),
                    notes: req.notes.or(Some(format!("Closed position via Alpaca order {}", filled_order.id))),
                };

                let _ = trade_logger.log_trade(trade).await;
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
