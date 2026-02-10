use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use backtest_engine::{BacktestConfig, BacktestEngine, BacktestResult, BacktestTrade, HistoricalBar, Signal};
use analysis_core::{SignalStrength, Timeframe};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use alpha_decay::{AlphaDecayMonitor, PerformanceSnapshot};

use crate::{combine_pit_signals, get_cached_etf_bars, ApiResponse, AppError, AppState};

#[derive(Deserialize)]
pub struct RunBacktestRequest {
    pub strategy_name: String,
    pub symbols: Vec<String>,
    pub start_date: String,
    pub end_date: String,
    pub initial_capital: f64,
    pub position_size_percent: f64,
    pub stop_loss_percent: Option<f64>,
    pub take_profit_percent: Option<f64>,
    pub confidence_threshold: f64,
}

#[derive(Serialize)]
pub struct BacktestSummary {
    pub backtest: BacktestResult,
    pub trades_count: usize,
}

pub fn backtest_routes() -> Router<AppState> {
    Router::new()
        .route("/api/backtest/run", post(run_backtest))
        .route("/api/backtest/results", get(get_all_backtests))
        .route("/api/backtest/results/:id", get(get_backtest))
        .route("/api/backtest/results/:id", delete(delete_backtest))
        .route("/api/backtest/results/:id/trades", get(get_backtest_trades))
        .route("/api/backtest/strategy/:name", get(get_backtests_by_strategy))
}

/// Convert analysis signal to "buy" / "sell" / "hold" string
fn signal_to_action(signal: &SignalStrength) -> &'static str {
    match signal {
        SignalStrength::StrongBuy | SignalStrength::Buy => "buy",
        SignalStrength::StrongSell | SignalStrength::Sell => "sell",
        _ => "hold",
    }
}

/// Run a backtest
async fn run_backtest(
    State(state): State<AppState>,
    Json(req): Json<RunBacktestRequest>,
) -> Result<Json<ApiResponse<BacktestSummary>>, AppError> {
    let backtest_db = state.backtest_db.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    tracing::info!("Running backtest: {} for {:?}", req.strategy_name, req.symbols);

    // Create backtest config
    let config = BacktestConfig {
        strategy_name: req.strategy_name.clone(),
        symbols: req.symbols.clone(),
        start_date: req.start_date.clone(),
        end_date: req.end_date.clone(),
        initial_capital: req.initial_capital,
        position_size_percent: req.position_size_percent,
        stop_loss_percent: req.stop_loss_percent,
        take_profit_percent: req.take_profit_percent,
        confidence_threshold: req.confidence_threshold,
        commission_rate: None,
        slippage_rate: None,
    };

    // Calculate how many days of history we need
    let start = NaiveDate::parse_from_str(&req.start_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid start_date format (use YYYY-MM-DD): {}", e))?;
    let end = NaiveDate::parse_from_str(&req.end_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid end_date format (use YYYY-MM-DD): {}", e))?;
    let days = (end - start).num_days();

    if days < 30 {
        return Err(anyhow::anyhow!("Backtest period must be at least 30 days").into());
    }

    // Fetch historical bars for each symbol and generate point-in-time signals
    let mut historical_data: HashMap<String, Vec<HistoricalBar>> = HashMap::new();
    let mut signals: Vec<Signal> = Vec::new();

    // Sample interval to keep computation reasonable
    let sample_interval: usize = if days > 180 { 5 } else if days > 90 { 3 } else { 1 };

    // Fetch SPY bars for benchmark (quant engine)
    let spy_bars = get_cached_etf_bars(&state, "SPY", 365, 15).await;
    let tech_engine = state.orchestrator.technical_engine();
    let quant_engine = state.orchestrator.quant_engine();

    for symbol in &req.symbols {
        let symbol = symbol.to_uppercase();

        // Fetch bars from orchestrator
        let bars = state.orchestrator
            .get_bars(&symbol, Timeframe::Day1, days)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch bars for {}: {}", symbol, e))?;

        if bars.len() < 50 {
            tracing::warn!("Insufficient bars for {}: {} (need >= 50)", symbol, bars.len());
            continue;
        }

        // Convert to HistoricalBar format for the engine
        let hist_bars: Vec<HistoricalBar> = bars.iter().map(|bar| HistoricalBar {
            date: bar.timestamp.format("%Y-%m-%d").to_string(),
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
        }).collect();

        // Point-in-time signal generation: run engines on bars[..i] only
        for i in (50..bars.len()).step_by(sample_interval) {
            let bar_slice = &bars[..i];
            let bar = &bars[i];

            let tech_result = tech_engine.analyze_enhanced(&symbol, bar_slice).ok();
            let quant_result = quant_engine.analyze_with_benchmark_and_rate(
                &symbol,
                bar_slice,
                if spy_bars.len() >= 30 { Some(&spy_bars) } else { None },
                None,
            ).ok();

            let (signal, confidence) = combine_pit_signals(&tech_result, &quant_result);
            let action = signal_to_action(&signal);

            if action != "hold" {
                signals.push(Signal {
                    date: bar.timestamp.format("%Y-%m-%d").to_string(),
                    symbol: symbol.clone(),
                    signal_type: action.to_string(),
                    confidence,
                    price: bar.close,
                    reason: format!("{:?} signal at {:.0}% confidence (point-in-time)",
                        signal, confidence * 100.0),
                });
            }
        }

        historical_data.insert(symbol, hist_bars);
    }

    if historical_data.is_empty() {
        return Err(anyhow::anyhow!("No historical data available for any of the requested symbols").into());
    }

    // Run the backtest engine
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(historical_data, signals)
        .map_err(|e| anyhow::anyhow!("Backtest engine error: {}", e))?;

    // Save to database
    let backtest_id = backtest_db.save_backtest(&result).await?;

    let mut saved_result = result;
    saved_result.id = Some(backtest_id);

    let trades_count = saved_result.total_trades as usize;

    let summary = BacktestSummary {
        backtest: saved_result,
        trades_count,
    };

    tracing::info!("Backtest complete. ID: {}, trades: {}", backtest_id, trades_count);

    // Auto-record a strategy health snapshot for alpha decay monitoring
    if let Some(pm) = state.portfolio_manager.as_ref() {
        let monitor = AlphaDecayMonitor::new(pm.db().pool().clone());
        let snapshot = PerformanceSnapshot {
            id: None,
            strategy_name: summary.backtest.strategy_name.clone(),
            snapshot_date: chrono::Utc::now().date_naive(),
            rolling_sharpe: summary.backtest.sharpe_ratio.unwrap_or(0.0),
            win_rate: summary.backtest.win_rate,
            profit_factor: summary.backtest.profit_factor.unwrap_or(0.0),
            trades_count: summary.backtest.total_trades,
            cumulative_return: summary.backtest.total_return_percent,
            max_drawdown: summary.backtest.max_drawdown_percent.unwrap_or(0.0),
            created_at: None,
        };
        match monitor.record_snapshot(&snapshot).await {
            Ok(id) => tracing::info!("Strategy snapshot recorded (id: {}) for alpha decay", id),
            Err(e) => tracing::warn!("Failed to record strategy snapshot: {}", e),
        }
    }

    Ok(Json(ApiResponse::success(summary)))
}

/// Get all backtest results
async fn get_all_backtests(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<BacktestResult>>>, AppError> {
    let backtest_db = state.backtest_db.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    let results = backtest_db.get_all_backtests().await?;

    Ok(Json(ApiResponse::success(results)))
}

/// Get specific backtest result
async fn get_backtest(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<BacktestResult>>, AppError> {
    let backtest_db = state.backtest_db.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    let result = backtest_db.get_backtest(id).await?
        .ok_or_else(|| anyhow::anyhow!("Backtest not found"))?;

    Ok(Json(ApiResponse::success(result)))
}

/// Delete backtest result
async fn delete_backtest(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let backtest_db = state.backtest_db.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    backtest_db.delete_backtest(id).await?;

    Ok(Json(ApiResponse::success(format!("Backtest {} deleted", id))))
}

/// Get trades for a specific backtest
async fn get_backtest_trades(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<BacktestTrade>>>, AppError> {
    let backtest_db = state.backtest_db.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    let trades = backtest_db.get_backtest_trades(id).await?;

    Ok(Json(ApiResponse::success(trades)))
}

/// Get backtests by strategy name
async fn get_backtests_by_strategy(
    State(state): State<AppState>,
    Path(strategy_name): Path<String>,
) -> Result<Json<ApiResponse<Vec<BacktestResult>>>, AppError> {
    let backtest_db = state.backtest_db.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    let results = backtest_db.get_backtests_by_strategy(&strategy_name).await?;

    Ok(Json(ApiResponse::success(results)))
}
