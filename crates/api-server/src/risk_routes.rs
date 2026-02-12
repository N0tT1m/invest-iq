use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put},
    Json, Router,
};
use risk_manager::{
    CircuitBreakerCheck, PositionSizeCalculation, RiskCheck, RiskParameters, RiskProfile,
    RiskRadar, RiskRadarCalculator, RiskTargetProfile, StopLossAlert,
};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{get_default_analysis, ApiResponse, AppError, AppState};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PositionSizeRequest {
    #[schema(value_type = f64)]
    pub entry_price: Decimal,
    pub account_balance: f64,
    pub current_positions_value: f64,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RiskCheckRequest {
    pub confidence: f64,
    pub account_balance: f64,
    pub current_positions_value: f64,
    pub active_positions_count: i32,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PriceUpdate {
    pub symbol: String,
    #[schema(value_type = f64)]
    pub price: Decimal,
}

/// Request for target risk profile update
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateTargetRequest {
    pub market_risk: Option<f64>,
    pub volatility_risk: Option<f64>,
    pub liquidity_risk: Option<f64>,
    pub event_risk: Option<f64>,
    pub concentration_risk: Option<f64>,
    pub sentiment_risk: Option<f64>,
}

/// Query params for radar calculation
#[derive(Deserialize, utoipa::ToSchema)]
pub struct RadarQuery {
    pub include_target: Option<bool>,
}

/// Request for manual trading halt
#[derive(Deserialize, utoipa::ToSchema)]
pub struct TradingHaltRequest {
    pub halted: bool,
    pub reason: Option<String>,
}

pub fn risk_routes() -> Router<AppState> {
    Router::new()
        // Existing routes
        .route("/api/risk/parameters", get(get_risk_parameters))
        .route("/api/risk/parameters", put(update_risk_parameters))
        .route("/api/risk/position-size", post(calculate_position_size))
        .route("/api/risk/check", post(check_trade_risk))
        .route("/api/risk/positions", get(get_active_risk_positions))
        .route("/api/risk/stop-loss/check", post(check_stop_losses))
        .route(
            "/api/risk/trailing-stop/:symbol",
            post(update_trailing_stop),
        )
        .route(
            "/api/risk/position/:symbol/close",
            post(close_risk_position),
        )
        // Circuit breaker routes
        .route("/api/risk/circuit-breakers", get(get_circuit_breakers))
        .route("/api/risk/trading-halt", post(set_trading_halt))
        // Risk Radar routes
        .route("/api/risk/radar", get(get_portfolio_risk_radar))
        .route("/api/risk/radar/:symbol", get(get_symbol_risk_radar))
        .route("/api/risk/target", get(get_target_profile))
        .route("/api/risk/target", put(update_target_profile))
}

#[utoipa::path(
    get,
    path = "/api/risk/parameters",
    responses((status = 200, description = "Current risk parameters")),
    tag = "Risk"
)]
async fn get_risk_parameters(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<RiskParameters>>, AppError> {
    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    let params = risk_manager.get_parameters().await?;

    Ok(Json(ApiResponse::success(params)))
}

#[utoipa::path(
    put,
    path = "/api/risk/parameters",
    request_body(content = String, description = "Risk parameters JSON"),
    responses((status = 200, description = "Updated risk parameters")),
    tag = "Risk"
)]
async fn update_risk_parameters(
    State(state): State<AppState>,
    key_ext: Option<axum::extract::Extension<crate::auth::ValidatedApiKey>>,
    Json(params): Json<RiskParameters>,
) -> Result<Json<ApiResponse<RiskParameters>>, AppError> {
    // Check admin role (skip in dev mode when no API_KEYS configured)
    if let Some(axum::extract::Extension(key)) = key_ext {
        if key.role < crate::auth::Role::Admin {
            return Err(crate::auth::AuthError::InsufficientRole(crate::auth::Role::Admin).into());
        }
    }

    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    risk_manager.update_parameters(&params).await?;

    Ok(Json(ApiResponse::success(params)))
}

#[utoipa::path(
    post,
    path = "/api/risk/position-size",
    request_body = PositionSizeRequest,
    responses((status = 200, description = "Calculated position size")),
    tag = "Risk"
)]
async fn calculate_position_size(
    State(state): State<AppState>,
    Json(req): Json<PositionSizeRequest>,
) -> Result<Json<ApiResponse<PositionSizeCalculation>>, AppError> {
    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    let calculation = risk_manager
        .calculate_position_size(
            req.entry_price,
            req.account_balance,
            req.current_positions_value,
        )
        .await?;

    Ok(Json(ApiResponse::success(calculation)))
}

#[utoipa::path(
    post,
    path = "/api/risk/check",
    request_body = RiskCheckRequest,
    responses((status = 200, description = "Trade risk check result")),
    tag = "Risk"
)]
async fn check_trade_risk(
    State(state): State<AppState>,
    Json(req): Json<RiskCheckRequest>,
) -> Result<Json<ApiResponse<RiskCheck>>, AppError> {
    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    let check = risk_manager
        .check_trade_risk(
            req.confidence,
            req.account_balance,
            req.current_positions_value,
            req.active_positions_count,
        )
        .await?;

    Ok(Json(ApiResponse::success(check)))
}

#[utoipa::path(
    get,
    path = "/api/risk/positions",
    responses((status = 200, description = "List of active risk-tracked positions")),
    tag = "Risk"
)]
async fn get_active_risk_positions(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<risk_manager::ActiveRiskPosition>>>, AppError> {
    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    let positions = risk_manager.get_active_positions().await?;

    Ok(Json(ApiResponse::success(positions)))
}

#[utoipa::path(
    post,
    path = "/api/risk/stop-loss/check",
    request_body = Vec<PriceUpdate>,
    responses((status = 200, description = "Stop-loss alerts for given price updates")),
    tag = "Risk"
)]
async fn check_stop_losses(
    State(state): State<AppState>,
    Json(prices): Json<Vec<PriceUpdate>>,
) -> Result<Json<ApiResponse<Vec<StopLossAlert>>>, AppError> {
    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    let price_tuples: Vec<(String, Decimal)> =
        prices.iter().map(|p| (p.symbol.clone(), p.price)).collect();

    let alerts = risk_manager.check_stop_losses(price_tuples).await?;

    Ok(Json(ApiResponse::success(alerts)))
}

#[utoipa::path(
    post,
    path = "/api/risk/trailing-stop/{symbol}",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    request_body(content = f64, description = "New trailing stop price"),
    responses((status = 200, description = "Trailing stop updated")),
    tag = "Risk"
)]
async fn update_trailing_stop(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(price): Json<Decimal>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    risk_manager.update_trailing_stop(&symbol, price).await?;

    Ok(Json(ApiResponse::success(format!(
        "Trailing stop updated for {}",
        symbol
    ))))
}

#[utoipa::path(
    post,
    path = "/api/risk/position/{symbol}/close",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    responses((status = 200, description = "Position closed")),
    tag = "Risk"
)]
async fn close_risk_position(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    risk_manager.close_position(&symbol, "manual_close").await?;

    Ok(Json(ApiResponse::success(format!(
        "Position {} closed",
        symbol
    ))))
}

// =============================================================================
// Risk Radar Handlers
// =============================================================================

#[utoipa::path(
    get,
    path = "/api/risk/radar",
    params(("include_target" = Option<bool>, Query, description = "Whether to include target risk profile")),
    responses((status = 200, description = "Portfolio-wide risk radar profile")),
    tag = "Risk"
)]
/// Get portfolio-wide risk radar
async fn get_portfolio_risk_radar(
    State(state): State<AppState>,
    Query(query): Query<RadarQuery>,
) -> Result<Json<ApiResponse<RiskProfile>>, AppError> {
    // Calculate risk radar from portfolio positions
    let portfolio_manager = state.portfolio_manager.as_ref();

    // Build risk radar from available data
    let mut radar = RiskRadar::moderate();

    // If we have positions, calculate concentration risk
    if let Some(pm) = portfolio_manager {
        if let Ok(summary) = pm.get_portfolio_summary(|_| Ok(100.0)).await {
            let num_positions = summary.positions.len();

            // Calculate concentration based on number of positions
            radar.concentration_risk = RiskRadarCalculator::calculate_concentration_risk(
                0.2, // Default 20% per position estimate
                0.3, // Default sector weight estimate
                num_positions,
            );
        }
    }

    // Default target profile
    let target = if query.include_target.unwrap_or(true) {
        Some(RiskTargetProfile::default().target)
    } else {
        None
    };

    let profile = RiskProfile::new(radar, target);

    Ok(Json(ApiResponse::success(profile)))
}

#[utoipa::path(
    get,
    path = "/api/risk/radar/{symbol}",
    params(
        ("symbol" = String, Path, description = "Stock ticker symbol"),
        ("include_target" = Option<bool>, Query, description = "Whether to include target risk profile"),
    ),
    responses((status = 200, description = "Symbol-specific risk radar profile")),
    tag = "Risk"
)]
/// Get risk radar for a specific symbol
async fn get_symbol_risk_radar(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<RadarQuery>,
) -> Result<Json<ApiResponse<RiskProfile>>, AppError> {
    let symbol = symbol.to_uppercase();

    // Get analysis for this symbol
    let analysis = get_default_analysis(&state, &symbol).await?;

    // Calculate risk dimensions from analysis
    let mut radar = RiskRadar::moderate();

    // Market risk from quant analysis (beta)
    if let Some(quant) = &analysis.quantitative {
        if let Some(beta) = quant.metrics.get("beta").and_then(|v| v.as_f64()) {
            radar.market_risk = RiskRadarCalculator::calculate_market_risk(beta, 0.7);
        }

        if let Some(volatility) = quant.metrics.get("volatility").and_then(|v| v.as_f64()) {
            let atr_pct = volatility / 16.0; // Rough daily from annualized
            radar.volatility_risk =
                RiskRadarCalculator::calculate_volatility_risk(volatility, atr_pct);
        }
    }

    // Sentiment risk from sentiment analysis
    if let Some(sentiment) = &analysis.sentiment {
        let confidence = sentiment.confidence;
        let article_count = sentiment
            .metrics
            .get("total_articles")
            .and_then(|v| v.as_i64())
            .unwrap_or(5) as i32;
        radar.sentiment_risk = RiskRadarCalculator::calculate_sentiment_risk(
            0.3, // Default sentiment volatility
            confidence,
            article_count,
        );
    }

    // Technical confidence affects event risk perception
    if let Some(technical) = &analysis.technical {
        radar.event_risk = RiskRadarCalculator::calculate_event_risk(
            None, // Would need earnings calendar integration
            None, false,
        ) * (1.0 + (1.0 - technical.confidence) * 0.5);
    }

    // Default target
    let target = if query.include_target.unwrap_or(true) {
        Some(RiskTargetProfile::default().target)
    } else {
        None
    };

    let profile = RiskProfile::new(radar, target);

    Ok(Json(ApiResponse::success(profile)))
}

#[utoipa::path(
    get,
    path = "/api/risk/target",
    responses((status = 200, description = "Current target risk profile")),
    tag = "Risk"
)]
/// Get target risk profile
async fn get_target_profile(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<RiskTargetProfile>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not configured"))?
        .db()
        .pool();

    // Query the database for the target profile
    #[allow(clippy::type_complexity)]
    let row: Option<(String, f64, f64, f64, f64, f64, f64, String)> = sqlx::query_as(
        "SELECT user_id, market_risk_target, volatility_risk_target,
                liquidity_risk_target, event_risk_target,
                concentration_risk_target, sentiment_risk_target, updated_at
         FROM risk_target_profile
         WHERE user_id = 'default'",
    )
    .fetch_optional(pool)
    .await?;

    let profile = if let Some((
        user_id,
        market,
        volatility,
        liquidity,
        event,
        concentration,
        sentiment,
        updated_at_str,
    )) = row
    {
        RiskTargetProfile {
            user_id,
            target: RiskRadar {
                market_risk: market,
                volatility_risk: volatility,
                liquidity_risk: liquidity,
                event_risk: event,
                concentration_risk: concentration,
                sentiment_risk: sentiment,
            },
            updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
        }
    } else {
        RiskTargetProfile::default()
    };

    Ok(Json(ApiResponse::success(profile)))
}

#[utoipa::path(
    put,
    path = "/api/risk/target",
    request_body = UpdateTargetRequest,
    responses((status = 200, description = "Updated target risk profile")),
    tag = "Risk"
)]
/// Update target risk profile
async fn update_target_profile(
    State(state): State<AppState>,
    Json(req): Json<UpdateTargetRequest>,
) -> Result<Json<ApiResponse<RiskTargetProfile>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not configured"))?
        .db()
        .pool();

    // First, get the current profile (or default)
    let current: Option<(f64, f64, f64, f64, f64, f64)> = sqlx::query_as(
        "SELECT market_risk_target, volatility_risk_target,
                liquidity_risk_target, event_risk_target,
                concentration_risk_target, sentiment_risk_target
         FROM risk_target_profile
         WHERE user_id = 'default'",
    )
    .fetch_optional(pool)
    .await?;

    // Build updated values (use current or default, then override with request values)
    let default = RiskTargetProfile::default();
    let (mut market, mut volatility, mut liquidity, mut event, mut concentration, mut sentiment) =
        if let Some((m, v, l, e, c, s)) = current {
            (m, v, l, e, c, s)
        } else {
            (
                default.target.market_risk,
                default.target.volatility_risk,
                default.target.liquidity_risk,
                default.target.event_risk,
                default.target.concentration_risk,
                default.target.sentiment_risk,
            )
        };

    // Apply updates from request
    if let Some(v) = req.market_risk {
        market = v.clamp(0.0, 100.0);
    }
    if let Some(v) = req.volatility_risk {
        volatility = v.clamp(0.0, 100.0);
    }
    if let Some(v) = req.liquidity_risk {
        liquidity = v.clamp(0.0, 100.0);
    }
    if let Some(v) = req.event_risk {
        event = v.clamp(0.0, 100.0);
    }
    if let Some(v) = req.concentration_risk {
        concentration = v.clamp(0.0, 100.0);
    }
    if let Some(v) = req.sentiment_risk {
        sentiment = v.clamp(0.0, 100.0);
    }

    let updated_at = chrono::Utc::now();
    let updated_at_str = updated_at.to_rfc3339();

    // Insert or replace the profile in the database
    sqlx::query(
        "INSERT OR REPLACE INTO risk_target_profile
            (user_id, market_risk_target, volatility_risk_target,
             liquidity_risk_target, event_risk_target,
             concentration_risk_target, sentiment_risk_target, updated_at)
         VALUES ('default', ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(market)
    .bind(volatility)
    .bind(liquidity)
    .bind(event)
    .bind(concentration)
    .bind(sentiment)
    .bind(&updated_at_str)
    .execute(pool)
    .await?;

    // Build the response profile
    let profile = RiskTargetProfile {
        user_id: "default".to_string(),
        target: RiskRadar {
            market_risk: market,
            volatility_risk: volatility,
            liquidity_risk: liquidity,
            event_risk: event,
            concentration_risk: concentration,
            sentiment_risk: sentiment,
        },
        updated_at,
    };

    Ok(Json(ApiResponse::success(profile)))
}

// =============================================================================
// Circuit Breaker Handlers
// =============================================================================

#[utoipa::path(
    get,
    path = "/api/risk/circuit-breakers",
    responses((status = 200, description = "Current circuit breaker status")),
    tag = "Risk"
)]
/// Get current circuit breaker status
async fn get_circuit_breakers(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<CircuitBreakerCheck>>, AppError> {
    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    // Get portfolio value from broker if available, else use 0
    let (portfolio_value, daily_pl) = if let Some(broker) = state.broker_client.as_ref() {
        let account = broker.get_account().await.ok();
        let pv = account
            .as_ref()
            .and_then(|a| a.portfolio_value.parse::<f64>().ok())
            .unwrap_or(0.0);

        let positions = broker.get_positions().await.unwrap_or_default();
        let dpl: f64 = positions
            .iter()
            .filter_map(|p| p.unrealized_intraday_pl.parse::<f64>().ok())
            .sum();
        (pv, dpl)
    } else {
        (0.0, 0.0)
    };

    let check = risk_manager
        .check_circuit_breakers(portfolio_value, daily_pl)
        .await?;

    Ok(Json(ApiResponse::success(check)))
}

#[utoipa::path(
    post,
    path = "/api/risk/trading-halt",
    request_body = TradingHaltRequest,
    responses((status = 200, description = "Trading halt status updated")),
    tag = "Risk"
)]
/// Manually halt or resume trading
async fn set_trading_halt(
    State(state): State<AppState>,
    key_ext: Option<axum::extract::Extension<crate::auth::ValidatedApiKey>>,
    Json(req): Json<TradingHaltRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    // Check admin role (skip in dev mode when no API_KEYS configured)
    if let Some(axum::extract::Extension(key)) = key_ext {
        if key.role < crate::auth::Role::Admin {
            return Err(crate::auth::AuthError::InsufficientRole(crate::auth::Role::Admin).into());
        }
    }

    let risk_manager = state
        .risk_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Risk manager not configured"))?;

    risk_manager
        .set_trading_halt(req.halted, req.reason.as_deref())
        .await?;

    let status = if req.halted { "halted" } else { "resumed" };
    tracing::info!("Trading {}: {:?}", status, req.reason);

    // Audit log the halt/resume
    if let Some(pm) = state.portfolio_manager.as_ref() {
        let event = if req.halted {
            "trading_halted"
        } else {
            "trading_resumed"
        };
        crate::audit::log_audit(
            pm.db().pool(),
            event,
            None,
            None,
            req.reason.as_deref(),
            "user",
            None,
        )
        .await;
    }

    Ok(Json(ApiResponse::success(serde_json::json!({
        "trading_halted": req.halted,
        "message": format!("Trading {}", status),
    }))))
}
