//! Time Machine API Routes
//!
//! Endpoints for interactive historical replay sessions.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use time_machine::{
    Difficulty, ReplayEngine, ReplayState, Scenario, ScenarioLibrary, ScoreCard, SessionConfig,
    SessionScorer, TimeMachineSession, TradeAction,
};
use tokio::sync::RwLock;

use crate::{ApiResponse, AppError, AppState};

/// In-memory session store (would use database in production)
type SessionStore = Arc<RwLock<HashMap<String, (TimeMachineSession, Vec<time_machine::BarData>)>>>;

lazy_static::lazy_static! {
    static ref SESSIONS: SessionStore = Arc::new(RwLock::new(HashMap::new()));
}

/// Request to start a new session
#[derive(Deserialize)]
pub struct StartSessionRequest {
    pub scenario_id: Option<String>,
    pub symbol: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub starting_capital: Option<f64>,
    pub user_id: Option<String>,
}

/// Request to make a decision
#[derive(Deserialize)]
pub struct DecisionRequest {
    pub action: String,
    pub shares: Option<i32>,
    pub reason: Option<String>,
}

/// Query for listing scenarios
#[derive(Deserialize)]
pub struct ScenarioQuery {
    pub difficulty: Option<String>,
    #[allow(dead_code)]
    pub category: Option<String>,
    pub featured_only: Option<bool>,
}

/// Response with session state
#[derive(Serialize)]
pub struct SessionResponse {
    pub session: TimeMachineSession,
    pub current_snapshot: Option<CurrentSnapshot>,
}

/// Current day snapshot for the frontend
#[derive(Serialize)]
pub struct CurrentSnapshot {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub ai_recommendation: String,
    pub ai_confidence: f64,
    pub indicators: IndicatorsResponse,
    pub visible_bars: Vec<BarResponse>,
    pub is_final_day: bool,
}

#[derive(Serialize)]
pub struct IndicatorsResponse {
    pub rsi_14: f64,
    pub sma_20: f64,
    pub sma_50: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub bollinger_upper: f64,
    pub bollinger_lower: f64,
    pub atr_14: f64,
}

#[derive(Serialize)]
pub struct BarResponse {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

/// Leaderboard query
#[derive(Deserialize)]
pub struct LeaderboardQuery {
    pub scenario_id: String,
    #[allow(dead_code)]
    pub limit: Option<usize>,
}

pub fn time_machine_routes() -> Router<AppState> {
    Router::new()
        .route("/api/time-machine/scenarios", get(list_scenarios))
        .route("/api/time-machine/scenarios/:id", get(get_scenario))
        .route("/api/time-machine/start", post(start_session))
        .route("/api/time-machine/session/:id", get(get_session))
        .route("/api/time-machine/session/:id/decide", post(make_decision))
        .route("/api/time-machine/session/:id/advance", post(advance_day))
        .route(
            "/api/time-machine/session/:id/abandon",
            post(abandon_session),
        )
        .route("/api/time-machine/session/:id/score", get(get_score))
        .route("/api/time-machine/leaderboard", get(get_leaderboard))
}

/// List available scenarios
async fn list_scenarios(
    Query(query): Query<ScenarioQuery>,
) -> Result<Json<ApiResponse<Vec<Scenario>>>, AppError> {
    let mut scenarios = if query.featured_only.unwrap_or(false) {
        ScenarioLibrary::featured_scenarios()
    } else {
        ScenarioLibrary::all_scenarios()
    };

    // Filter by difficulty
    if let Some(diff_str) = &query.difficulty {
        let difficulty = match diff_str.to_lowercase().as_str() {
            "beginner" => Some(Difficulty::Beginner),
            "intermediate" => Some(Difficulty::Intermediate),
            "advanced" => Some(Difficulty::Advanced),
            "expert" => Some(Difficulty::Expert),
            _ => None,
        };

        if let Some(diff) = difficulty {
            scenarios.retain(|s| s.difficulty == diff);
        }
    }

    Ok(Json(ApiResponse::success(scenarios)))
}

/// Get a specific scenario
async fn get_scenario(Path(id): Path<String>) -> Result<Json<ApiResponse<Scenario>>, AppError> {
    let scenario = ScenarioLibrary::get_scenario(&id)
        .ok_or_else(|| anyhow::anyhow!("Scenario not found: {}", id))?;

    Ok(Json(ApiResponse::success(scenario)))
}

/// Start a new time machine session
async fn start_session(
    State(state): State<AppState>,
    Json(request): Json<StartSessionRequest>,
) -> Result<Json<ApiResponse<SessionResponse>>, AppError> {
    let engine = ReplayEngine::new();

    // Build session config
    let config = if let Some(scenario_id) = &request.scenario_id {
        // Use scenario settings
        let scenario = ScenarioLibrary::get_scenario(scenario_id)
            .ok_or_else(|| anyhow::anyhow!("Scenario not found: {}", scenario_id))?;

        SessionConfig {
            scenario_id: Some(scenario_id.clone()),
            symbol: scenario.primary_symbol.clone(),
            start_date: scenario.start_date,
            end_date: scenario.end_date,
            starting_capital: request.starting_capital.unwrap_or(10000.0),
            user_id: request.user_id.clone(),
        }
    } else {
        // Custom session
        let start_date = request
            .start_date
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2020, 3, 2).unwrap());

        let end_date = request
            .end_date
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2020, 3, 23).unwrap());

        SessionConfig {
            scenario_id: None,
            symbol: request.symbol.clone().unwrap_or_else(|| "SPY".to_string()),
            start_date,
            end_date,
            starting_capital: request.starting_capital.unwrap_or(10000.0),
            user_id: request.user_id.clone(),
        }
    };

    // Create session
    let session = engine.start_session(config.clone());

    // Fetch historical price data for the session
    let bars =
        fetch_historical_bars(&state, &config.symbol, config.start_date, config.end_date).await?;

    if bars.is_empty() {
        return Err(anyhow::anyhow!(
            "No historical data available for {} in the specified date range",
            config.symbol
        )
        .into());
    }

    // Get initial snapshot
    let snapshot = engine.get_current_snapshot(&session, &bars);

    // Store session
    {
        let mut sessions = SESSIONS.write().await;
        sessions.insert(session.id.clone(), (session.clone(), bars));
    }

    let response = SessionResponse {
        session: session.clone(),
        current_snapshot: snapshot.map(convert_snapshot),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Get current session state
async fn get_session(
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<SessionResponse>>, AppError> {
    let sessions = SESSIONS.read().await;
    let (session, bars) = sessions
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id))?;

    let engine = ReplayEngine::new();
    let snapshot = engine.get_current_snapshot(session, bars);

    let response = SessionResponse {
        session: session.clone(),
        current_snapshot: snapshot.map(convert_snapshot),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Make a trading decision
async fn make_decision(
    Path(id): Path<String>,
    Json(request): Json<DecisionRequest>,
) -> Result<Json<ApiResponse<SessionResponse>>, AppError> {
    let action: TradeAction = request.action.parse()?;

    let mut sessions = SESSIONS.write().await;
    let (session, bars) = sessions
        .get_mut(&id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id))?;

    if session.status != ReplayState::Active {
        return Err(anyhow::anyhow!("Session is not active").into());
    }

    let engine = ReplayEngine::new();

    // Get current snapshot for AI recommendation and price
    let current_snapshot = engine
        .get_current_snapshot(session, bars)
        .ok_or_else(|| anyhow::anyhow!("Cannot get current day data"))?;

    // Calculate shares if not specified
    let shares = request.shares.unwrap_or_else(|| {
        match action {
            TradeAction::Buy => {
                // Calculate max shares we can buy
                (session.cash / current_snapshot.close).floor() as i32
            }
            TradeAction::Sell => session.shares_held,
            TradeAction::Hold => 0,
        }
    });

    // Find actual next day return
    let current_idx = bars.iter().position(|b| b.date == session.current_date);
    let actual_return = current_idx.and_then(|idx| {
        bars.get(idx + 1).map(|next_bar| {
            ((next_bar.close - current_snapshot.close) / current_snapshot.close) * 100.0
        })
    });

    // Create decision
    let decision = time_machine::UserDecision {
        decision_date: session.current_date,
        action,
        shares: Some(shares),
        price: current_snapshot.close,
        ai_recommendation: current_snapshot.ai_recommendation,
        actual_return,
        reason: request.reason,
    };

    // Advance session
    engine.advance_session(session, decision, bars)?;

    // Get new snapshot
    let new_snapshot = engine.get_current_snapshot(session, bars);

    let response = SessionResponse {
        session: session.clone(),
        current_snapshot: new_snapshot.map(convert_snapshot),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Advance to next day without making a trade (auto-hold)
async fn advance_day(
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<SessionResponse>>, AppError> {
    make_decision(
        Path(id),
        Json(DecisionRequest {
            action: "hold".to_string(),
            shares: None,
            reason: Some("Auto-hold".to_string()),
        }),
    )
    .await
}

/// Abandon a session
async fn abandon_session(
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<TimeMachineSession>>, AppError> {
    let mut sessions = SESSIONS.write().await;
    let (session, _) = sessions
        .get_mut(&id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id))?;

    session.status = ReplayState::Abandoned;

    Ok(Json(ApiResponse::success(session.clone())))
}

/// Get score for a completed session
async fn get_score(Path(id): Path<String>) -> Result<Json<ApiResponse<ScoreCard>>, AppError> {
    let sessions = SESSIONS.read().await;
    let (session, bars) = sessions
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id))?;

    if session.status != ReplayState::Completed && session.status != ReplayState::Abandoned {
        return Err(anyhow::anyhow!("Session must be completed or abandoned to get score").into());
    }

    // Get first and last prices
    let first_price = bars.first().map(|b| b.close).unwrap_or(0.0);
    let last_price = bars.last().map(|b| b.close).unwrap_or(0.0);

    let scorer = SessionScorer::new();
    let score_card = scorer.score_session(session, first_price, last_price);

    Ok(Json(ApiResponse::success(score_card)))
}

/// Get leaderboard for a scenario
async fn get_leaderboard(
    Query(query): Query<LeaderboardQuery>,
) -> Result<Json<ApiResponse<time_machine::Leaderboard>>, AppError> {
    // In production, this would query the database
    // For now, return empty leaderboard
    let scenario = ScenarioLibrary::get_scenario(&query.scenario_id)
        .ok_or_else(|| anyhow::anyhow!("Scenario not found"))?;

    let leaderboard = time_machine::Leaderboard {
        scenario_id: query.scenario_id.clone(),
        scenario_name: scenario.name,
        entries: Vec::new(),
        total_participants: 0,
        average_score: 0.0,
    };

    Ok(Json(ApiResponse::success(leaderboard)))
}

/// Fetch historical bars for a date range
async fn fetch_historical_bars(
    state: &AppState,
    symbol: &str,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
) -> Result<Vec<time_machine::BarData>, AppError> {
    // Calculate days needed
    let days = (end_date - start_date).num_days() + 60; // Extra buffer for history

    // Fetch bars from orchestrator
    let bars = state
        .orchestrator
        .get_bars(symbol, analysis_core::Timeframe::Day1, days)
        .await?;

    // Convert and filter to date range
    let filtered: Vec<time_machine::BarData> = bars
        .into_iter()
        .filter(|b| {
            let bar_date = b.timestamp.date_naive();
            bar_date >= start_date && bar_date <= end_date
        })
        .map(|b| time_machine::BarData {
            date: b.timestamp.date_naive(),
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: b.volume as u64,
        })
        .collect();

    Ok(filtered)
}

/// Convert internal snapshot to response format
fn convert_snapshot(snapshot: time_machine::DaySnapshot) -> CurrentSnapshot {
    CurrentSnapshot {
        date: snapshot.date.to_string(),
        open: snapshot.open,
        high: snapshot.high,
        low: snapshot.low,
        close: snapshot.close,
        volume: snapshot.volume,
        ai_recommendation: format!("{:?}", snapshot.ai_recommendation),
        ai_confidence: snapshot.ai_confidence,
        indicators: IndicatorsResponse {
            rsi_14: snapshot.indicators.rsi_14,
            sma_20: snapshot.indicators.sma_20,
            sma_50: snapshot.indicators.sma_50,
            macd: snapshot.indicators.macd,
            macd_signal: snapshot.indicators.macd_signal,
            bollinger_upper: snapshot.indicators.bollinger_upper,
            bollinger_lower: snapshot.indicators.bollinger_lower,
            atr_14: snapshot.indicators.atr_14,
        },
        visible_bars: snapshot
            .visible_history
            .into_iter()
            .map(|b| BarResponse {
                date: b.date.to_string(),
                open: b.open,
                high: b.high,
                low: b.low,
                close: b.close,
                volume: b.volume,
            })
            .collect(),
        is_final_day: snapshot.is_final_day,
    }
}
