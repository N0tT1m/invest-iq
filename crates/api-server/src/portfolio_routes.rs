use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use portfolio_manager::*;
use serde::Deserialize;

use crate::{ApiResponse, AppError, AppState};

#[derive(Deserialize)]
pub struct AddPositionRequest {
    pub symbol: String,
    pub shares: f64,
    pub entry_price: f64,
    pub entry_date: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdatePositionRequest {
    pub shares: f64,
    pub entry_price: f64,
    pub entry_date: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct LogTradeRequest {
    pub symbol: String,
    pub action: String,
    pub shares: f64,
    pub price: f64,
    pub trade_date: String,
    pub commission: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateAlertRequest {
    pub symbol: String,
    pub alert_type: String,
    pub signal: String,
    pub confidence: f64,
    pub current_price: Option<f64>,
    pub target_price: Option<f64>,
    pub stop_loss_price: Option<f64>,
    pub reason: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Deserialize)]
pub struct WatchlistRequest {
    pub symbol: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct SnapshotsQuery {
    pub days: Option<i64>,
}

#[derive(Deserialize)]
pub struct PerformanceQuery {
    pub days: Option<i64>,
}

pub fn portfolio_routes() -> Router<AppState> {
    Router::new()
        // Portfolio endpoints
        .route("/api/portfolio", get(get_portfolio_summary))
        .route("/api/portfolio/positions", get(get_positions))
        .route("/api/portfolio/positions", post(add_position))
        .route("/api/portfolio/positions/:symbol", get(get_position))
        .route("/api/portfolio/positions/:symbol", put(update_position))
        .route("/api/portfolio/positions/:symbol", delete(delete_position))
        .route("/api/portfolio/snapshots", get(get_snapshots))
        .route("/api/portfolio/snapshots", post(save_snapshot))
        // Trades endpoints
        .route("/api/trades", get(get_all_trades))
        .route("/api/trades", post(log_trade))
        .route("/api/trades/:id", get(get_trade))
        .route("/api/trades/:id", put(update_trade))
        .route("/api/trades/:id", delete(delete_trade))
        .route("/api/trades/performance", get(get_performance))
        // Alerts endpoints
        .route("/api/alerts", get(get_active_alerts))
        .route("/api/alerts", post(create_alert))
        .route("/api/alerts/:id", get(get_alert))
        .route("/api/alerts/:id/complete", post(complete_alert))
        .route("/api/alerts/:id/ignore", post(ignore_alert))
        .route("/api/alerts/:id", delete(delete_alert))
        .route("/api/alerts/actions", get(get_action_items))
        // Watchlist endpoints
        .route("/api/watchlist", get(get_watchlist))
        .route("/api/watchlist", post(add_to_watchlist))
        .route("/api/watchlist/:symbol", delete(remove_from_watchlist))
}

// Portfolio handlers
async fn get_portfolio_summary(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<PortfolioSummary>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    // Create price fetcher closure
    let orchestrator = state.orchestrator.clone();
    let price_fetcher = move |symbol: &str| -> anyhow::Result<f64> {
        // We need to make this async work in sync context
        // For now, we'll use a blocking approach
        let rt = tokio::runtime::Handle::current();
        let symbol = symbol.to_string();
        let orch = orchestrator.clone();

        rt.block_on(async move {
            let bars = orch.get_bars(&symbol, analysis_core::Timeframe::Day1, 1).await?;
            bars.last()
                .map(|bar| bar.close)
                .ok_or_else(|| anyhow::anyhow!("No price data available for {}", symbol))
        })
    };

    let summary = portfolio_manager.get_portfolio_summary(price_fetcher).await?;

    Ok(Json(ApiResponse::success(summary)))
}

async fn get_positions(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Position>>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let positions = portfolio_manager.get_all_positions().await?;

    Ok(Json(ApiResponse::success(positions)))
}

async fn add_position(
    State(state): State<AppState>,
    Json(req): Json<AddPositionRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let position = Position {
        id: None,
        symbol: req.symbol.to_uppercase(),
        shares: req.shares,
        entry_price: req.entry_price,
        entry_date: req.entry_date,
        notes: req.notes,
        created_at: None,
    };

    let id = portfolio_manager.add_position(position).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "id": id }))))
}

async fn get_position(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<Option<Position>>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let position = portfolio_manager.get_position(&symbol.to_uppercase()).await?;

    Ok(Json(ApiResponse::success(position)))
}

async fn update_position(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(req): Json<UpdatePositionRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let symbol = symbol.to_uppercase();

    // Get existing position to find ID
    let existing = portfolio_manager.get_position(&symbol).await?
        .ok_or_else(|| anyhow::anyhow!("Position not found: {}", symbol))?;

    let position = Position {
        id: existing.id,
        symbol: symbol.clone(),
        shares: req.shares,
        entry_price: req.entry_price,
        entry_date: req.entry_date,
        notes: req.notes,
        created_at: existing.created_at,
    };

    let position_id = existing.id
        .ok_or_else(|| anyhow::anyhow!("Position has no ID"))?;
    portfolio_manager.update_position(position_id, position).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "message": "Position updated" }))))
}

async fn delete_position(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    portfolio_manager.delete_position(&symbol.to_uppercase()).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "message": "Position deleted" }))))
}

async fn get_snapshots(
    State(state): State<AppState>,
    Query(query): Query<SnapshotsQuery>,
) -> Result<Json<ApiResponse<Vec<PortfolioSnapshot>>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let days = query.days.unwrap_or(30);
    let snapshots = portfolio_manager.get_snapshots(days).await?;

    Ok(Json(ApiResponse::success(snapshots)))
}

async fn save_snapshot(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    // Get current portfolio summary
    let orchestrator = state.orchestrator.clone();
    let price_fetcher = move |symbol: &str| -> anyhow::Result<f64> {
        let rt = tokio::runtime::Handle::current();
        let symbol = symbol.to_string();
        let orch = orchestrator.clone();

        rt.block_on(async move {
            let bars = orch.get_bars(&symbol, analysis_core::Timeframe::Day1, 1).await?;
            bars.last()
                .map(|bar| bar.close)
                .ok_or_else(|| anyhow::anyhow!("No price data available for {}", symbol))
        })
    };

    let summary = portfolio_manager.get_portfolio_summary(price_fetcher).await?;
    let id = portfolio_manager.save_snapshot(&summary).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "id": id }))))
}

// Trade handlers
async fn get_all_trades(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Trade>>>, AppError> {
    let trade_logger = state.trade_logger.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    let trades = trade_logger.get_all_trades(Some(100)).await?;

    Ok(Json(ApiResponse::success(trades)))
}

async fn log_trade(
    State(state): State<AppState>,
    Json(req): Json<LogTradeRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let trade_logger = state.trade_logger.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    let symbol = req.symbol.to_uppercase();
    let action = req.action.clone();
    let shares = req.shares;
    let price = req.price;
    let trade_date = req.trade_date.clone();

    let trade = TradeInput {
        symbol: symbol.clone(),
        action: action.clone(),
        shares,
        price,
        trade_date: trade_date.clone(),
        commission: req.commission,
        notes: req.notes,
    };

    let id = trade_logger.log_trade(trade).await?;

    // Also update portfolio if it's a position manager
    if let Some(portfolio_manager) = state.portfolio_manager.as_ref() {
        if action == "buy" {
            let position = Position {
                id: None,
                symbol: symbol.clone(),
                shares,
                entry_price: price,
                entry_date: trade_date,
                notes: Some("Auto-added from trade log".to_string()),
                created_at: None,
            };
            let _ = portfolio_manager.add_position(position).await;
        } else if action == "sell" {
            let _ = portfolio_manager.remove_shares(&symbol, shares).await;
        }
    }

    Ok(Json(ApiResponse::success(serde_json::json!({ "id": id }))))
}

async fn get_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Option<Trade>>>, AppError> {
    let trade_logger = state.trade_logger.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    let trade = trade_logger.get_trade(id).await?;

    Ok(Json(ApiResponse::success(trade)))
}

async fn update_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<LogTradeRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let trade_logger = state.trade_logger.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    let trade = TradeInput {
        symbol: req.symbol.to_uppercase(),
        action: req.action,
        shares: req.shares,
        price: req.price,
        trade_date: req.trade_date,
        commission: req.commission,
        notes: req.notes,
    };

    trade_logger.update_trade(id, trade).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "message": "Trade updated" }))))
}

async fn delete_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let trade_logger = state.trade_logger.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    trade_logger.delete_trade(id).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "message": "Trade deleted" }))))
}

async fn get_performance(
    State(state): State<AppState>,
    Query(query): Query<PerformanceQuery>,
) -> Result<Json<ApiResponse<PerformanceMetrics>>, AppError> {
    let trade_logger = state.trade_logger.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    let metrics = trade_logger.get_performance_metrics(query.days).await?;

    Ok(Json(ApiResponse::success(metrics)))
}

// Alert handlers
async fn get_active_alerts(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Alert>>>, AppError> {
    let alert_manager = state.alert_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    let alerts = alert_manager.get_active_alerts().await?;

    Ok(Json(ApiResponse::success(alerts)))
}

async fn create_alert(
    State(state): State<AppState>,
    Json(req): Json<CreateAlertRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let alert_manager = state.alert_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    let alert = AlertInput {
        symbol: req.symbol.to_uppercase(),
        alert_type: req.alert_type,
        signal: req.signal,
        confidence: req.confidence,
        current_price: req.current_price,
        target_price: req.target_price,
        stop_loss_price: req.stop_loss_price,
        reason: req.reason,
        expires_at: req.expires_at,
    };

    let id = alert_manager.create_alert(alert).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "id": id }))))
}

async fn get_alert(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Option<Alert>>>, AppError> {
    let alert_manager = state.alert_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    let alert = alert_manager.get_alert(id).await?;

    Ok(Json(ApiResponse::success(alert)))
}

async fn complete_alert(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let alert_manager = state.alert_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    alert_manager.complete_alert(id).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "message": "Alert completed" }))))
}

async fn ignore_alert(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let alert_manager = state.alert_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    alert_manager.ignore_alert(id).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "message": "Alert ignored" }))))
}

async fn delete_alert(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let alert_manager = state.alert_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    alert_manager.delete_alert(id).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "message": "Alert deleted" }))))
}

async fn get_action_items(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ActionItem>>>, AppError> {
    let alert_manager = state.alert_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    let portfolio_manager = state.portfolio_manager.as_ref();

    // Get active alerts
    let alerts = alert_manager.get_active_alerts().await?;

    // Get positions to check which symbols are in portfolio
    let positions = if let Some(pm) = portfolio_manager {
        pm.get_all_positions().await.unwrap_or_default()
    } else {
        Vec::new()
    };

    let position_map: std::collections::HashMap<String, &Position> = positions
        .iter()
        .map(|p| (p.symbol.clone(), p))
        .collect();

    // Convert alerts to action items
    let mut action_items: Vec<ActionItem> = alerts
        .into_iter()
        .map(|alert| {
            let in_portfolio = position_map.contains_key(&alert.symbol);
            let position_pnl = None; // TODO: Calculate PnL if in portfolio

            let (priority, title, description) = match alert.alert_type.as_str() {
                "buy" => (
                    1,
                    format!("🚀 {} Signal: {}", alert.signal, alert.symbol),
                    format!(
                        "Buy {} at ${:.2} (Confidence: {:.0}%)",
                        alert.symbol,
                        alert.current_price.unwrap_or(0.0),
                        alert.confidence * 100.0
                    ),
                ),
                "sell" => (
                    if in_portfolio { 1 } else { 2 },
                    format!("📉 Sell Signal: {}", alert.symbol),
                    if in_portfolio {
                        format!("You own {}. Consider taking profits.", alert.symbol)
                    } else {
                        format!("Sell signal for {} (not in portfolio)", alert.symbol)
                    },
                ),
                "stop_loss" => (
                    1,
                    format!("⚠️ Stop Loss Warning: {}", alert.symbol),
                    format!("Price approaching stop loss at ${:.2}", alert.stop_loss_price.unwrap_or(0.0)),
                ),
                "take_profit" => (
                    1,
                    format!("🎯 Take Profit: {}", alert.symbol),
                    format!("Target price ${:.2} reached!", alert.target_price.unwrap_or(0.0)),
                ),
                _ => (
                    3,
                    format!("📌 Watch: {}", alert.symbol),
                    alert.reason.clone().unwrap_or_default(),
                ),
            };

            ActionItem {
                priority,
                action_type: alert.alert_type.clone(),
                symbol: alert.symbol.clone(),
                title,
                description,
                signal: alert.signal.clone(),
                confidence: alert.confidence,
                current_price: alert.current_price,
                target_price: alert.target_price,
                stop_loss_price: alert.stop_loss_price,
                in_portfolio,
                position_pnl,
                alert_id: alert.id,
            }
        })
        .collect();

    // Sort by priority
    action_items.sort_by_key(|item| item.priority);

    Ok(Json(ApiResponse::success(action_items)))
}

// Watchlist handlers
async fn get_watchlist(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<WatchlistItem>>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let watchlist = portfolio_manager.get_watchlist().await?;

    Ok(Json(ApiResponse::success(watchlist)))
}

async fn add_to_watchlist(
    State(state): State<AppState>,
    Json(req): Json<WatchlistRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let id = portfolio_manager.add_to_watchlist(&req.symbol.to_uppercase(), req.notes).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "id": id }))))
}

async fn remove_from_watchlist(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    portfolio_manager.remove_from_watchlist(&symbol.to_uppercase()).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "message": "Removed from watchlist" }))))
}
