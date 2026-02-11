//! Watchlist API Routes
//!
//! Endpoints for personalized opportunity feed and watchlist management.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post, delete},
    Json, Router,
};
use smart_watchlist::{
    InteractionType, Opportunity, OpportunityRanker, OpportunitySignal,
    PreferenceLearner, SymbolInteraction, UserPreference, WatchlistItem,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{get_cached_etf_bars, ApiResponse, AppError, AppState};

/// Query params for personalized feed
#[derive(Deserialize)]
pub struct FeedQuery {
    pub user_id: Option<String>,
    pub limit: Option<usize>,
    pub min_confidence: Option<f64>,
}

/// Request to record user interaction
#[derive(Deserialize)]
pub struct InteractionRequest {
    pub user_id: String,
    pub symbol: String,
    pub action: String,
    pub context: Option<String>,
}

/// Request to update preferences
#[derive(Deserialize)]
pub struct UpdatePreferencesRequest {
    pub sectors: Option<Vec<String>>,
    pub risk_tolerance: Option<f64>,
    pub excluded_symbols: Option<Vec<String>>,
    #[allow(dead_code)]
    pub min_confidence: Option<f64>,
}

/// Response for personalized feed
#[derive(Serialize)]
pub struct PersonalizedFeedResponse {
    pub opportunities: Vec<Opportunity>,
    pub total_scanned: usize,
    pub user_preferences_applied: bool,
}

pub fn watchlist_routes() -> Router<AppState> {
    Router::new()
        .route("/api/watchlist/personalized", get(get_personalized_feed))
        .route("/api/watchlist/interaction", post(record_interaction))
        .route("/api/watchlist/preferences", get(get_preferences))
        .route("/api/watchlist/preferences", post(update_preferences))
        .route("/api/watchlist/items", get(get_watchlist_items))
        .route("/api/watchlist/items", post(add_to_watchlist))
        .route("/api/watchlist/items/:symbol", delete(remove_from_watchlist))
        .route("/api/watchlist/scan", get(scan_opportunities))
}

/// Lightweight stock universe for fast scanning (covers major sectors)
const QUICK_UNIVERSE: &[(&str, &str)] = &[
    ("AAPL", "Technology"), ("MSFT", "Technology"), ("NVDA", "Technology"),
    ("GOOGL", "Communication Services"), ("AMZN", "Consumer Discretionary"),
    ("META", "Communication Services"), ("TSLA", "Consumer Discretionary"),
    ("JPM", "Financials"), ("V", "Financials"),
    ("UNH", "Healthcare"), ("JNJ", "Healthcare"),
    ("XOM", "Energy"), ("PG", "Consumer Staples"),
    ("HD", "Industrials"), ("LLY", "Healthcare"),
];

/// Compute a quick technical signal from bars (SMA cross + momentum)
fn quick_signal_from_bars(bars: &[analysis_core::Bar]) -> (f64, f64, String) {
    if bars.len() < 21 {
        return (0.5, 0.3, "Insufficient data".to_string());
    }

    let current = match bars.last() {
        Some(b) => b.close,
        None => return (0.5, 0.3, "No bar data".to_string()),
    };
    let len = bars.len();

    // SMA-20
    let sma_20: f64 = bars[len-20..].iter().map(|b| b.close).sum::<f64>() / 20.0;
    // SMA-50 (if available)
    let sma_50 = if len >= 50 {
        bars[len-50..].iter().map(|b| b.close).sum::<f64>() / 50.0
    } else {
        sma_20
    };

    // Price vs SMA trend
    let trend_20 = (current - sma_20) / sma_20 * 100.0;
    let trend_50 = (current - sma_50) / sma_50 * 100.0;

    // Simple RSI approximation (14 periods)
    let rsi_period = 14.min(len - 1);
    let mut gains = 0.0_f64;
    let mut losses = 0.0_f64;
    for i in (len - rsi_period)..len {
        let change = bars[i].close - bars[i-1].close;
        if change > 0.0 { gains += change; } else { losses += change.abs(); }
    }
    let avg_gain = gains / rsi_period as f64;
    let avg_loss = losses / rsi_period as f64;
    let rsi = if avg_loss == 0.0 { 100.0 } else {
        100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
    };

    // Composite signal score (0.0=StrongSell, 1.0=StrongBuy)
    let trend_score = ((trend_20 + trend_50) / 2.0 / 10.0 + 0.5).clamp(0.0, 1.0);
    let rsi_score = if rsi > 70.0 { 0.3 } else if rsi > 50.0 { 0.7 } else if rsi > 30.0 { 0.4 } else { 0.2 };
    let signal_score = trend_score * 0.6 + rsi_score * 0.4;

    let confidence = (trend_20.abs() / 8.0).clamp(0.3, 0.85);

    let summary = format!(
        "Price {:.1}% vs SMA20, RSI {:.0}",
        trend_20, rsi
    );

    (signal_score, confidence, summary)
}

/// Get personalized opportunity feed
async fn get_personalized_feed(
    State(state): State<AppState>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<ApiResponse<PersonalizedFeedResponse>>, AppError> {
    let user_id = query.user_id.clone().unwrap_or_else(|| "default".to_string());
    let limit = query.limit.unwrap_or(20);
    let min_confidence = query.min_confidence.unwrap_or(0.3);

    // Get user preferences if available
    let preferences = if let Some(pm) = &state.portfolio_manager {
        let learner = PreferenceLearner::new(pm.db().pool().clone());
        learner.get_preferences(&user_id).await.unwrap_or_default()
    } else {
        UserPreference::default()
    };

    // Use lightweight bars-based scanning instead of full orchestrator analysis
    let mut opportunities = Vec::new();

    for &(symbol, sector) in QUICK_UNIVERSE {
        let bars = get_cached_etf_bars(&state, symbol, 90, 15).await;
        if bars.is_empty() {
            continue;
        }

        let (signal_score, confidence, summary) = quick_signal_from_bars(&bars);

        if confidence < min_confidence {
            continue;
        }

        let signal = OpportunitySignal::from_score(signal_score);

        // Skip neutral signals for the feed
        if signal == OpportunitySignal::Neutral {
            continue;
        }

        let current_price = bars.last().map(|b| b.close);

        opportunities.push(Opportunity {
            symbol: symbol.to_string(),
            name: None,
            signal,
            confidence,
            reason: summary.clone(),
            summary,
            event_type: None,
            event_date: None,
            relevance_score: 50.0, // Will be personalized by ranker
            current_price,
            price_target: None,
            potential_return: None,
            sector: Some(sector.to_string()),
            tags: vec![],
            detected_at: Utc::now(),
            expires_at: None,
        });
    }

    let total_scanned = QUICK_UNIVERSE.len();

    // Rank by personal relevance
    let ranker = OpportunityRanker::new();
    ranker.rank(&mut opportunities, &preferences);

    // Take top results
    opportunities.truncate(limit);

    Ok(Json(ApiResponse::success(PersonalizedFeedResponse {
        opportunities,
        total_scanned,
        user_preferences_applied: query.user_id.is_some(),
    })))
}

/// Record a user interaction
async fn record_interaction(
    State(state): State<AppState>,
    Json(req): Json<InteractionRequest>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    let pool = state
        .portfolio_manager
        .as_ref()
        .map(|pm| pm.db().pool().clone())
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?;

    let learner = PreferenceLearner::new(pool);

    let interaction = SymbolInteraction {
        id: None,
        user_id: req.user_id,
        symbol: req.symbol.to_uppercase(),
        action: InteractionType::from_str(&req.action),
        context: req.context,
        created_at: Utc::now(),
    };

    let id = learner.record_interaction(&interaction).await?;

    Ok(Json(ApiResponse::success(id)))
}

/// Get user preferences
async fn get_preferences(
    State(state): State<AppState>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<ApiResponse<UserPreference>>, AppError> {
    let user_id = query.user_id.clone().unwrap_or_else(|| "default".to_string());

    let pool = state
        .portfolio_manager
        .as_ref()
        .map(|pm| pm.db().pool().clone())
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?;

    let learner = PreferenceLearner::new(pool);
    let prefs = learner.get_preferences(&user_id).await?;

    Ok(Json(ApiResponse::success(prefs)))
}

/// Update user preferences
async fn update_preferences(
    State(state): State<AppState>,
    Query(query): Query<FeedQuery>,
    Json(req): Json<UpdatePreferencesRequest>,
) -> Result<Json<ApiResponse<UserPreference>>, AppError> {
    let user_id = query.user_id.clone().unwrap_or_else(|| "default".to_string());

    let pool = state
        .portfolio_manager
        .as_ref()
        .map(|pm| pm.db().pool().clone())
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?;

    let learner = PreferenceLearner::new(pool);
    let prefs = learner
        .update_explicit_preferences(
            &user_id,
            req.sectors,
            req.risk_tolerance,
            req.excluded_symbols,
        )
        .await?;

    Ok(Json(ApiResponse::success(prefs)))
}

/// Get user's watchlist items
async fn get_watchlist_items(
    State(state): State<AppState>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<ApiResponse<Vec<WatchlistItem>>>, AppError> {
    let user_id = query.user_id.clone().unwrap_or_else(|| "default".to_string());

    let pool = state
        .portfolio_manager
        .as_ref()
        .map(|pm| pm.db().pool().clone())
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?;

    let rows: Vec<(i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, symbol, notes, added_at FROM watchlist ORDER BY added_at DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let items: Vec<WatchlistItem> = rows.into_iter().map(|(id, symbol, notes, added_at)| {
        WatchlistItem {
            id: Some(id),
            user_id: user_id.clone(),
            symbol,
            added_at: chrono::DateTime::parse_from_str(&added_at, "%Y-%m-%d %H:%M:%S")
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            notes,
            target_price: None,
            stop_loss: None,
            alert_enabled: false,
        }
    }).collect();

    Ok(Json(ApiResponse::success(items)))
}

/// Add symbol to watchlist
#[derive(Deserialize)]
pub struct AddWatchlistRequest {
    pub user_id: String,
    pub symbol: String,
    pub notes: Option<String>,
    #[allow(dead_code)]
    pub target_price: Option<f64>,
    #[allow(dead_code)]
    pub stop_loss: Option<f64>,
}

async fn add_to_watchlist(
    State(state): State<AppState>,
    Json(req): Json<AddWatchlistRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let pool = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db().pool();

    let symbol = req.symbol.to_uppercase();

    // Insert into watchlist table (ignore if already exists)
    sqlx::query(
        "INSERT OR IGNORE INTO watchlist (symbol, notes) VALUES (?, ?)"
    )
    .bind(&symbol)
    .bind(&req.notes)
    .execute(pool)
    .await?;

    // Record as interaction for learning
    let learner = PreferenceLearner::new(pool.clone());
    let interaction = SymbolInteraction {
        id: None,
        user_id: req.user_id.clone(),
        symbol: symbol.clone(),
        action: InteractionType::WatchlistAdd,
        context: None,
        created_at: Utc::now(),
    };
    let _ = learner.record_interaction(&interaction).await;

    Ok(Json(ApiResponse::success(format!(
        "{} added to watchlist",
        symbol
    ))))
}

/// Remove symbol from watchlist
async fn remove_from_watchlist(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let pool = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?
        .db().pool();

    let user_id = query.user_id.clone().unwrap_or_else(|| "default".to_string());
    let symbol = symbol.to_uppercase();

    // Delete from watchlist table
    sqlx::query("DELETE FROM watchlist WHERE symbol = ?")
        .bind(&symbol)
        .execute(pool)
        .await?;

    // Record as interaction
    let learner = PreferenceLearner::new(pool.clone());
    let interaction = SymbolInteraction {
        id: None,
        user_id,
        symbol: symbol.clone(),
        action: InteractionType::WatchlistRemove,
        context: None,
        created_at: Utc::now(),
    };
    let _ = learner.record_interaction(&interaction).await;

    Ok(Json(ApiResponse::success(format!(
        "{} removed from watchlist",
        symbol
    ))))
}

/// Scan opportunities without personalization
async fn scan_opportunities(
    State(state): State<AppState>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<ApiResponse<Vec<Opportunity>>>, AppError> {
    let limit = query.limit.unwrap_or(20);
    let min_confidence = query.min_confidence.unwrap_or(0.3);

    let mut opportunities = Vec::new();

    for &(symbol, sector) in QUICK_UNIVERSE {
        let bars = get_cached_etf_bars(&state, symbol, 90, 15).await;
        if bars.is_empty() {
            continue;
        }

        let (signal_score, confidence, summary) = quick_signal_from_bars(&bars);

        if confidence < min_confidence {
            continue;
        }

        let signal = OpportunitySignal::from_score(signal_score);

        let current_price = bars.last().map(|b| b.close);

        opportunities.push(Opportunity {
            symbol: symbol.to_string(),
            name: None,
            signal,
            confidence,
            reason: summary.clone(),
            summary,
            event_type: None,
            event_date: None,
            relevance_score: 50.0,
            current_price,
            price_target: None,
            potential_return: None,
            sector: Some(sector.to_string()),
            tags: vec![],
            detected_at: Utc::now(),
            expires_at: None,
        });
    }

    // Sort by confidence descending
    opportunities.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    opportunities.truncate(limit);

    Ok(Json(ApiResponse::success(opportunities)))
}
