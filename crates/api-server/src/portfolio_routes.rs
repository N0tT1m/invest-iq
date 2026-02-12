use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use portfolio_manager::*;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{ApiResponse, AppError, AppState};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddPositionRequest {
    pub symbol: String,
    #[schema(value_type = f64)]
    pub shares: Decimal,
    #[schema(value_type = f64)]
    pub entry_price: Decimal,
    pub entry_date: String,
    pub notes: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdatePositionRequest {
    #[schema(value_type = f64)]
    pub shares: Decimal,
    #[schema(value_type = f64)]
    pub entry_price: Decimal,
    pub entry_date: String,
    pub notes: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct LogTradeRequest {
    pub symbol: String,
    pub action: String,
    #[schema(value_type = f64)]
    pub shares: Decimal,
    #[schema(value_type = f64)]
    pub price: Decimal,
    pub trade_date: String,
    #[schema(value_type = Option<f64>)]
    pub commission: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateAlertRequest {
    pub symbol: String,
    pub alert_type: String,
    pub signal: String,
    pub confidence: f64, // Confidence is a ratio, not money - stays f64
    #[schema(value_type = Option<f64>)]
    pub current_price: Option<Decimal>,
    #[schema(value_type = Option<f64>)]
    pub target_price: Option<Decimal>,
    #[schema(value_type = Option<f64>)]
    pub stop_loss_price: Option<Decimal>,
    pub reason: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct WatchlistRequest {
    pub symbol: String,
    pub notes: Option<String>,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct SnapshotsQuery {
    pub days: Option<i64>,
}

#[derive(Deserialize, utoipa::IntoParams)]
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
#[utoipa::path(
    get,
    path = "/api/portfolio",
    tag = "Portfolio",
    responses(
        (status = 200, description = "Portfolio summary with total value and P&L"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_portfolio_summary(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<PortfolioSummary>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
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
            let bars = orch
                .get_bars(&symbol, analysis_core::Timeframe::Day1, 1)
                .await?;
            bars.last()
                .map(|bar| bar.close)
                .ok_or_else(|| anyhow::anyhow!("No price data available for {}", symbol))
        })
    };

    let summary = portfolio_manager
        .get_portfolio_summary(price_fetcher)
        .await?;

    Ok(Json(ApiResponse::success(summary)))
}

#[utoipa::path(
    get,
    path = "/api/portfolio/positions",
    tag = "Portfolio",
    responses(
        (status = 200, description = "List of all portfolio positions"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_positions(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Position>>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let positions = portfolio_manager.get_all_positions().await?;

    Ok(Json(ApiResponse::success(positions)))
}

#[utoipa::path(
    post,
    path = "/api/portfolio/positions",
    tag = "Portfolio",
    request_body = AddPositionRequest,
    responses(
        (status = 200, description = "Position added successfully"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn add_position(
    State(state): State<AppState>,
    Json(req): Json<AddPositionRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
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

#[utoipa::path(
    get,
    path = "/api/portfolio/positions/{symbol}",
    tag = "Portfolio",
    params(
        ("symbol" = String, Path, description = "Stock ticker symbol (e.g. AAPL)"),
    ),
    responses(
        (status = 200, description = "Position details for the given symbol"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_position(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<Option<Position>>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let position = portfolio_manager
        .get_position(&symbol.to_uppercase())
        .await?;

    Ok(Json(ApiResponse::success(position)))
}

#[utoipa::path(
    put,
    path = "/api/portfolio/positions/{symbol}",
    tag = "Portfolio",
    params(
        ("symbol" = String, Path, description = "Stock ticker symbol (e.g. AAPL)"),
    ),
    request_body = UpdatePositionRequest,
    responses(
        (status = 200, description = "Position updated successfully"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn update_position(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(req): Json<UpdatePositionRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let symbol = symbol.to_uppercase();

    // Get existing position to find ID
    let existing = portfolio_manager
        .get_position(&symbol)
        .await?
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

    let position_id = existing
        .id
        .ok_or_else(|| anyhow::anyhow!("Position has no ID"))?;
    portfolio_manager
        .update_position(position_id, position)
        .await?;

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "message": "Position updated" }),
    )))
}

#[utoipa::path(
    delete,
    path = "/api/portfolio/positions/{symbol}",
    tag = "Portfolio",
    params(
        ("symbol" = String, Path, description = "Stock ticker symbol (e.g. AAPL)"),
    ),
    responses(
        (status = 200, description = "Position deleted successfully"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn delete_position(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    portfolio_manager
        .delete_position(&symbol.to_uppercase())
        .await?;

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "message": "Position deleted" }),
    )))
}

#[utoipa::path(
    get,
    path = "/api/portfolio/snapshots",
    tag = "Portfolio",
    params(SnapshotsQuery),
    responses(
        (status = 200, description = "Portfolio snapshots over time"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_snapshots(
    State(state): State<AppState>,
    Query(query): Query<SnapshotsQuery>,
) -> Result<Json<ApiResponse<Vec<PortfolioSnapshot>>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let days = query.days.unwrap_or(30);
    let snapshots = portfolio_manager.get_snapshots(days).await?;

    Ok(Json(ApiResponse::success(snapshots)))
}

#[utoipa::path(
    post,
    path = "/api/portfolio/snapshots",
    tag = "Portfolio",
    responses(
        (status = 200, description = "Portfolio snapshot saved"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn save_snapshot(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    // Get current portfolio summary
    let orchestrator = state.orchestrator.clone();
    let price_fetcher = move |symbol: &str| -> anyhow::Result<f64> {
        let rt = tokio::runtime::Handle::current();
        let symbol = symbol.to_string();
        let orch = orchestrator.clone();

        rt.block_on(async move {
            let bars = orch
                .get_bars(&symbol, analysis_core::Timeframe::Day1, 1)
                .await?;
            bars.last()
                .map(|bar| bar.close)
                .ok_or_else(|| anyhow::anyhow!("No price data available for {}", symbol))
        })
    };

    let summary = portfolio_manager
        .get_portfolio_summary(price_fetcher)
        .await?;
    let id = portfolio_manager.save_snapshot(&summary).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "id": id }))))
}

// Trade handlers
#[utoipa::path(
    get,
    path = "/api/trades",
    tag = "Portfolio",
    responses(
        (status = 200, description = "List of all trades (up to 100)"),
        (status = 500, description = "Trade logger error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_all_trades(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Trade>>>, AppError> {
    let trade_logger = state
        .trade_logger
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    let trades = trade_logger.get_all_trades(Some(100)).await?;

    Ok(Json(ApiResponse::success(trades)))
}

#[utoipa::path(
    post,
    path = "/api/trades",
    tag = "Portfolio",
    request_body = LogTradeRequest,
    responses(
        (status = 200, description = "Trade logged and portfolio updated"),
        (status = 500, description = "Trade logger error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn log_trade(
    State(state): State<AppState>,
    Json(req): Json<LogTradeRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let trade_logger = state
        .trade_logger
        .as_ref()
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
        alert_id: None,
        analysis_id: None,
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

#[utoipa::path(
    get,
    path = "/api/trades/{id}",
    tag = "Portfolio",
    params(
        ("id" = i64, Path, description = "Trade ID"),
    ),
    responses(
        (status = 200, description = "Trade details"),
        (status = 500, description = "Trade logger error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Option<Trade>>>, AppError> {
    let trade_logger = state
        .trade_logger
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    let trade = trade_logger.get_trade(id).await?;

    Ok(Json(ApiResponse::success(trade)))
}

#[utoipa::path(
    put,
    path = "/api/trades/{id}",
    tag = "Portfolio",
    params(
        ("id" = i64, Path, description = "Trade ID"),
    ),
    request_body = LogTradeRequest,
    responses(
        (status = 200, description = "Trade updated successfully"),
        (status = 500, description = "Trade logger error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn update_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<LogTradeRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let trade_logger = state
        .trade_logger
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    let trade = TradeInput {
        symbol: req.symbol.to_uppercase(),
        action: req.action,
        shares: req.shares,
        price: req.price,
        trade_date: req.trade_date,
        commission: req.commission,
        notes: req.notes,
        alert_id: None,
        analysis_id: None,
    };

    trade_logger.update_trade(id, trade).await?;

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "message": "Trade updated" }),
    )))
}

#[utoipa::path(
    delete,
    path = "/api/trades/{id}",
    tag = "Portfolio",
    params(
        ("id" = i64, Path, description = "Trade ID"),
    ),
    responses(
        (status = 200, description = "Trade deleted successfully"),
        (status = 500, description = "Trade logger error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn delete_trade(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let trade_logger = state
        .trade_logger
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    trade_logger.delete_trade(id).await?;

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "message": "Trade deleted" }),
    )))
}

#[utoipa::path(
    get,
    path = "/api/trades/performance",
    tag = "Portfolio",
    params(PerformanceQuery),
    responses(
        (status = 200, description = "Trade performance metrics"),
        (status = 500, description = "Trade logger error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_performance(
    State(state): State<AppState>,
    Query(query): Query<PerformanceQuery>,
) -> Result<Json<ApiResponse<PerformanceMetrics>>, AppError> {
    let trade_logger = state
        .trade_logger
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized"))?;

    let metrics = trade_logger.get_performance_metrics(query.days).await?;

    Ok(Json(ApiResponse::success(metrics)))
}

// Alert handlers
#[utoipa::path(
    get,
    path = "/api/alerts",
    tag = "Portfolio",
    responses(
        (status = 200, description = "List of active alerts"),
        (status = 500, description = "Alert manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_active_alerts(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Alert>>>, AppError> {
    let alert_manager = state
        .alert_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    let alerts = alert_manager.get_active_alerts().await?;

    Ok(Json(ApiResponse::success(alerts)))
}

#[utoipa::path(
    post,
    path = "/api/alerts",
    tag = "Portfolio",
    request_body = CreateAlertRequest,
    responses(
        (status = 200, description = "Alert created successfully"),
        (status = 500, description = "Alert manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn create_alert(
    State(state): State<AppState>,
    Json(req): Json<CreateAlertRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let alert_manager = state
        .alert_manager
        .as_ref()
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

#[utoipa::path(
    get,
    path = "/api/alerts/{id}",
    tag = "Portfolio",
    params(
        ("id" = i64, Path, description = "Alert ID"),
    ),
    responses(
        (status = 200, description = "Alert details"),
        (status = 500, description = "Alert manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_alert(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Option<Alert>>>, AppError> {
    let alert_manager = state
        .alert_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    let alert = alert_manager.get_alert(id).await?;

    Ok(Json(ApiResponse::success(alert)))
}

#[utoipa::path(
    post,
    path = "/api/alerts/{id}/complete",
    tag = "Portfolio",
    params(
        ("id" = i64, Path, description = "Alert ID"),
    ),
    responses(
        (status = 200, description = "Alert marked as completed"),
        (status = 500, description = "Alert manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn complete_alert(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let alert_manager = state
        .alert_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    alert_manager.complete_alert(id).await?;

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "message": "Alert completed" }),
    )))
}

#[utoipa::path(
    post,
    path = "/api/alerts/{id}/ignore",
    tag = "Portfolio",
    params(
        ("id" = i64, Path, description = "Alert ID"),
    ),
    responses(
        (status = 200, description = "Alert marked as ignored"),
        (status = 500, description = "Alert manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn ignore_alert(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let alert_manager = state
        .alert_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    alert_manager.ignore_alert(id).await?;

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "message": "Alert ignored" }),
    )))
}

#[utoipa::path(
    delete,
    path = "/api/alerts/{id}",
    tag = "Portfolio",
    params(
        ("id" = i64, Path, description = "Alert ID"),
    ),
    responses(
        (status = 200, description = "Alert deleted successfully"),
        (status = 500, description = "Alert manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn delete_alert(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let alert_manager = state
        .alert_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized"))?;

    alert_manager.delete_alert(id).await?;

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "message": "Alert deleted" }),
    )))
}

#[utoipa::path(
    get,
    path = "/api/alerts/actions",
    tag = "Portfolio",
    responses(
        (status = 200, description = "Prioritized action items derived from alerts"),
        (status = 500, description = "Alert manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_action_items(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ActionItem>>>, AppError> {
    let alert_manager = state
        .alert_manager
        .as_ref()
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

    let position_map: std::collections::HashMap<String, &Position> =
        positions.iter().map(|p| (p.symbol.clone(), p)).collect();

    // Convert alerts to action items
    let mut action_items: Vec<ActionItem> = alerts
        .into_iter()
        .map(|alert| {
            let in_portfolio = position_map.contains_key(&alert.symbol);
            let position_pnl = if let Some(pos) = position_map.get(&alert.symbol) {
                alert.current_price.and_then(|cp| {
                    let current = Decimal::from_f64_retain(cp)?;
                    Some((current - pos.entry_price) * pos.shares)
                })
            } else {
                None
            };

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
                    format!(
                        "Price approaching stop loss at ${:.2}",
                        alert.stop_loss_price.unwrap_or(0.0)
                    ),
                ),
                "take_profit" => (
                    1,
                    format!("🎯 Take Profit: {}", alert.symbol),
                    format!(
                        "Target price ${:.2} reached!",
                        alert.target_price.unwrap_or(0.0)
                    ),
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
                current_price: alert.current_price.and_then(Decimal::from_f64_retain),
                target_price: alert.target_price.and_then(Decimal::from_f64_retain),
                stop_loss_price: alert.stop_loss_price.and_then(Decimal::from_f64_retain),
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
#[utoipa::path(
    get,
    path = "/api/watchlist",
    tag = "Portfolio",
    responses(
        (status = 200, description = "List of watchlist items"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn get_watchlist(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<WatchlistItem>>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let watchlist = portfolio_manager.get_watchlist().await?;

    Ok(Json(ApiResponse::success(watchlist)))
}

#[utoipa::path(
    post,
    path = "/api/watchlist",
    tag = "Portfolio",
    request_body = WatchlistRequest,
    responses(
        (status = 200, description = "Symbol added to watchlist"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn add_to_watchlist(
    State(state): State<AppState>,
    Json(req): Json<WatchlistRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    let id = portfolio_manager
        .add_to_watchlist(&req.symbol.to_uppercase(), req.notes)
        .await?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "id": id }))))
}

#[utoipa::path(
    delete,
    path = "/api/watchlist/{symbol}",
    tag = "Portfolio",
    params(
        ("symbol" = String, Path, description = "Stock ticker symbol to remove"),
    ),
    responses(
        (status = 200, description = "Symbol removed from watchlist"),
        (status = 500, description = "Portfolio manager error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
async fn remove_from_watchlist(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let portfolio_manager = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized"))?;

    portfolio_manager
        .remove_from_watchlist(&symbol.to_uppercase())
        .await?;

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "message": "Removed from watchlist" }),
    )))
}
