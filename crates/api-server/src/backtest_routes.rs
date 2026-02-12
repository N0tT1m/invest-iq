use analysis_core::{SignalStrength, Timeframe};
use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use backtest_engine::{
    run_monte_carlo, BacktestConfig, BacktestEngine, BacktestResult, BacktestTrade, HistoricalBar,
    MonteCarloResult, Signal, WalkForwardFoldData, WalkForwardResult, WalkForwardRunner,
};
use chrono::NaiveDate;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use alpha_decay::{AlphaDecayMonitor, PerformanceSnapshot};

use crate::{combine_pit_signals, get_cached_etf_bars, ApiResponse, AppError, AppState};

#[derive(Deserialize, utoipa::ToSchema)]
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
    pub commission_rate: Option<f64>,
    pub slippage_rate: Option<f64>,
    pub max_volume_participation: Option<f64>,
    pub allocation_strategy: Option<String>,
    pub symbol_weights: Option<HashMap<String, f64>>,
    pub rebalance_interval_days: Option<i32>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct BacktestSummary {
    #[schema(value_type = Object)]
    pub backtest: BacktestResult,
    pub trades_count: usize,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct MonteCarloQuery {
    #[serde(default = "default_simulations")]
    pub simulations: i32,
}
fn default_simulations() -> i32 {
    1000
}

pub fn backtest_routes() -> Router<AppState> {
    Router::new()
        .route("/api/backtest/run", post(run_backtest))
        .route("/api/backtest/results", get(get_all_backtests))
        .route("/api/backtest/results/:id", get(get_backtest))
        .route("/api/backtest/results/:id", delete(delete_backtest))
        .route("/api/backtest/results/:id/trades", get(get_backtest_trades))
        .route(
            "/api/backtest/results/:id/monte-carlo",
            get(get_monte_carlo),
        )
        .route(
            "/api/backtest/strategy/:name",
            get(get_backtests_by_strategy),
        )
        .route("/api/backtest/walk-forward", post(run_walk_forward))
}

/// Convert analysis signal to "buy" / "sell" / "hold" action string.
fn signal_to_action(signal: &SignalStrength) -> &'static str {
    match signal {
        SignalStrength::StrongBuy | SignalStrength::Buy | SignalStrength::WeakBuy => "buy",
        SignalStrength::StrongSell | SignalStrength::Sell | SignalStrength::WeakSell => "sell",
        SignalStrength::Neutral => "hold",
    }
}

/// Convert analysis signal to a display name for trade records.
fn signal_to_display(signal: &SignalStrength) -> &'static str {
    match signal {
        SignalStrength::StrongBuy => "StrongBuy",
        SignalStrength::Buy => "Buy",
        SignalStrength::WeakBuy => "WeakBuy",
        SignalStrength::StrongSell => "StrongSell",
        SignalStrength::Sell => "Sell",
        SignalStrength::WeakSell => "WeakSell",
        SignalStrength::Neutral => "Neutral",
    }
}

/// Helper: fetch bars and generate PIT signals for a date range.
async fn fetch_bars_and_signals(
    state: &AppState,
    symbols: &[String],
    days: i64,
    sample_interval: usize,
) -> Result<(HashMap<String, Vec<HistoricalBar>>, Vec<Signal>), AppError> {
    let mut historical_data: HashMap<String, Vec<HistoricalBar>> = HashMap::new();
    let mut signals: Vec<Signal> = Vec::new();

    let spy_bars = get_cached_etf_bars(state, "SPY", 365, 15).await;
    let tech_engine = state.orchestrator.technical_engine();
    let quant_engine = state.orchestrator.quant_engine();

    for symbol in symbols {
        let symbol = symbol.to_uppercase();

        let bars = state
            .orchestrator
            .get_bars(&symbol, Timeframe::Day1, days)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch bars for {}: {}", symbol, e))?;

        if bars.len() < 50 {
            tracing::warn!(
                "Insufficient bars for {}: {} (need >= 50)",
                symbol,
                bars.len()
            );
            continue;
        }

        let hist_bars: Vec<HistoricalBar> = bars
            .iter()
            .map(|bar| HistoricalBar {
                date: bar.timestamp.format("%Y-%m-%d").to_string(),
                open: Decimal::from_f64(bar.open).unwrap_or_default(),
                high: Decimal::from_f64(bar.high).unwrap_or_default(),
                low: Decimal::from_f64(bar.low).unwrap_or_default(),
                close: Decimal::from_f64(bar.close).unwrap_or_default(),
                volume: bar.volume,
            })
            .collect();

        for i in (50..bars.len()).step_by(sample_interval) {
            let bar_slice = &bars[..i];
            let bar = &bars[i];

            let tech_result = tech_engine.analyze_enhanced(&symbol, bar_slice, None).ok();
            let quant_result = quant_engine
                .analyze_with_benchmark_and_rate(
                    &symbol,
                    bar_slice,
                    if spy_bars.len() >= 30 {
                        Some(&spy_bars)
                    } else {
                        None
                    },
                    None,
                )
                .ok();

            let (signal, confidence) = combine_pit_signals(&tech_result, &quant_result);
            let action = signal_to_action(&signal);

            if action != "hold" {
                signals.push(Signal {
                    date: bar.timestamp.format("%Y-%m-%d").to_string(),
                    symbol: symbol.clone(),
                    signal_type: signal_to_display(&signal).to_string(),
                    confidence,
                    price: Decimal::from_f64(bar.close).unwrap_or_default(),
                    reason: format!(
                        "{:?} signal at {:.0}% confidence (point-in-time)",
                        signal,
                        confidence * 100.0
                    ),
                    order_type: None,
                    limit_price: None,
                    limit_expiry_bars: None,
                });
            }
        }

        historical_data.insert(symbol, hist_bars);
    }

    Ok((historical_data, signals))
}

/// Helper: convert cached ETF bars to HistoricalBar format for benchmark.
fn etf_bars_to_historical(bars: &[analysis_core::Bar]) -> Vec<HistoricalBar> {
    bars.iter()
        .map(|b| HistoricalBar {
            date: b.timestamp.format("%Y-%m-%d").to_string(),
            open: Decimal::from_f64(b.open).unwrap_or_default(),
            high: Decimal::from_f64(b.high).unwrap_or_default(),
            low: Decimal::from_f64(b.low).unwrap_or_default(),
            close: Decimal::from_f64(b.close).unwrap_or_default(),
            volume: b.volume,
        })
        .collect()
}

#[utoipa::path(
    post,
    path = "/api/backtest/run",
    tag = "Backtest",
    request_body = RunBacktestRequest,
    responses(
        (status = 200, description = "Backtest completed successfully", body = BacktestSummary),
        (status = 500, description = "Backtest engine error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
/// Run a backtest
pub(crate) async fn run_backtest(
    State(state): State<AppState>,
    Json(req): Json<RunBacktestRequest>,
) -> Result<Json<ApiResponse<BacktestSummary>>, AppError> {
    let backtest_db = state
        .backtest_db
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    tracing::info!(
        "Running backtest: {} for {:?}",
        req.strategy_name,
        req.symbols
    );

    let start = NaiveDate::parse_from_str(&req.start_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid start_date format (use YYYY-MM-DD): {}", e))?;
    let end = NaiveDate::parse_from_str(&req.end_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid end_date format (use YYYY-MM-DD): {}", e))?;
    let days = (end - start).num_days();

    if days < 30 {
        return Err(anyhow::anyhow!("Backtest period must be at least 30 days").into());
    }

    let sample_interval: usize = if days > 180 {
        5
    } else if days > 90 {
        3
    } else {
        1
    };

    let (historical_data, signals) =
        fetch_bars_and_signals(&state, &req.symbols, days, sample_interval).await?;

    if historical_data.is_empty() {
        return Err(anyhow::anyhow!(
            "No historical data available for any of the requested symbols"
        )
        .into());
    }

    // Fetch SPY bars for benchmark comparison
    let spy_bars_raw = get_cached_etf_bars(&state, "SPY", days, 15).await;
    let benchmark_bars = if spy_bars_raw.len() >= 30 {
        Some(etf_bars_to_historical(&spy_bars_raw))
    } else {
        None
    };

    let config = BacktestConfig {
        strategy_name: req.strategy_name.clone(),
        symbols: req.symbols.clone(),
        start_date: req.start_date.clone(),
        end_date: req.end_date.clone(),
        initial_capital: Decimal::from_f64(req.initial_capital).unwrap_or(Decimal::new(100000, 0)),
        position_size_percent: req.position_size_percent,
        stop_loss_percent: req.stop_loss_percent,
        take_profit_percent: req.take_profit_percent,
        confidence_threshold: req.confidence_threshold,
        commission_rate: req.commission_rate,
        slippage_rate: req.slippage_rate,
        max_volume_participation: req.max_volume_participation,
        benchmark_bars,
        allocation_strategy: req.allocation_strategy,
        symbol_weights: req.symbol_weights,
        rebalance_interval_days: req.rebalance_interval_days,
        allow_short_selling: None,
        margin_multiplier: None,
        signal_timeframe: None,
        trailing_stop_percent: None,
        max_drawdown_halt_percent: None,
        regime_config: None,
        commission_model: None,
        allow_fractional_shares: None,
        cash_sweep_rate: None,
        incremental_rebalance: None,
        param_search_space: None,
        market_impact: None,
    };

    let mut engine = BacktestEngine::new(config);
    let result = engine
        .run(historical_data, signals)
        .map_err(|e| anyhow::anyhow!("Backtest engine error: {}", e))?;

    let backtest_id = backtest_db.save_backtest(&result).await?;

    let mut saved_result = result;
    saved_result.id = Some(backtest_id);

    let trades_count = saved_result.total_trades as usize;

    let summary = BacktestSummary {
        backtest: saved_result,
        trades_count,
    };

    tracing::info!(
        "Backtest complete. ID: {}, trades: {}",
        backtest_id,
        trades_count
    );

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
            max_drawdown: summary.backtest.max_drawdown.unwrap_or(0.0),
            created_at: None,
        };
        match monitor.record_snapshot(&snapshot).await {
            Ok(id) => tracing::info!("Strategy snapshot recorded (id: {}) for alpha decay", id),
            Err(e) => tracing::warn!("Failed to record strategy snapshot: {}", e),
        }
    }

    Ok(Json(ApiResponse::success(summary)))
}

#[utoipa::path(
    post,
    path = "/api/backtest/walk-forward",
    tag = "Backtest",
    request_body = RunBacktestRequest,
    responses(
        (status = 200, description = "Walk-forward validation completed"),
        (status = 500, description = "Walk-forward validation error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
/// Run walk-forward validation
pub(crate) async fn run_walk_forward(
    State(state): State<AppState>,
    Json(req): Json<RunBacktestRequest>,
) -> Result<Json<ApiResponse<WalkForwardResult>>, AppError> {
    let num_folds = 5i32; // default 5 folds

    tracing::info!(
        "Running walk-forward validation: {} for {:?} ({} folds)",
        req.strategy_name,
        req.symbols,
        num_folds
    );

    let start = NaiveDate::parse_from_str(&req.start_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid start_date: {}", e))?;
    let end = NaiveDate::parse_from_str(&req.end_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid end_date: {}", e))?;
    let total_days = (end - start).num_days();

    if total_days < 90 {
        return Err(anyhow::anyhow!("Walk-forward requires at least 90 days of data").into());
    }

    // Fetch all bars and signals for the full period
    let sample_interval: usize = if total_days > 180 { 5 } else { 3 };
    let (all_data, all_signals) =
        fetch_bars_and_signals(&state, &req.symbols, total_days, sample_interval).await?;

    if all_data.is_empty() {
        return Err(anyhow::anyhow!("No data available for walk-forward validation").into());
    }

    // Split into folds: 70% train / 30% test for each rolling window
    let fold_size = total_days / num_folds as i64;
    let train_ratio = 0.7;
    let train_days = (fold_size as f64 * train_ratio) as i64;

    let mut folds: Vec<WalkForwardFoldData> = Vec::new();

    for fold_idx in 0..num_folds {
        let fold_start = start + chrono::Duration::days(fold_idx as i64 * fold_size);
        let fold_train_end = fold_start + chrono::Duration::days(train_days);
        let fold_test_end = fold_start + chrono::Duration::days(fold_size);

        let train_start_str = fold_start.format("%Y-%m-%d").to_string();
        let train_end_str = fold_train_end.format("%Y-%m-%d").to_string();
        let test_start_str = fold_train_end.format("%Y-%m-%d").to_string();
        let test_end_str = fold_test_end.format("%Y-%m-%d").to_string();

        let mut train_data: HashMap<String, Vec<HistoricalBar>> = HashMap::new();
        let mut test_data: HashMap<String, Vec<HistoricalBar>> = HashMap::new();

        for (sym, bars) in &all_data {
            let train: Vec<HistoricalBar> = bars
                .iter()
                .filter(|b| b.date >= train_start_str && b.date < train_end_str)
                .cloned()
                .collect();
            let test: Vec<HistoricalBar> = bars
                .iter()
                .filter(|b| b.date >= test_start_str && b.date < test_end_str)
                .cloned()
                .collect();
            if !train.is_empty() {
                train_data.insert(sym.clone(), train);
            }
            if !test.is_empty() {
                test_data.insert(sym.clone(), test);
            }
        }

        let train_signals: Vec<Signal> = all_signals
            .iter()
            .filter(|s| s.date >= train_start_str && s.date < train_end_str)
            .cloned()
            .collect();
        let test_signals: Vec<Signal> = all_signals
            .iter()
            .filter(|s| s.date >= test_start_str && s.date < test_end_str)
            .cloned()
            .collect();

        if !train_data.is_empty() && !test_data.is_empty() {
            folds.push(WalkForwardFoldData {
                train_data,
                train_signals,
                test_data,
                test_signals,
            });
        }
    }

    if folds.is_empty() {
        return Err(anyhow::anyhow!("Could not create any valid walk-forward folds").into());
    }

    let config = BacktestConfig {
        strategy_name: req.strategy_name.clone(),
        symbols: req.symbols.clone(),
        start_date: req.start_date.clone(),
        end_date: req.end_date.clone(),
        initial_capital: Decimal::from_f64(req.initial_capital).unwrap_or(Decimal::new(100000, 0)),
        position_size_percent: req.position_size_percent,
        stop_loss_percent: req.stop_loss_percent,
        take_profit_percent: req.take_profit_percent,
        confidence_threshold: req.confidence_threshold,
        commission_rate: req.commission_rate,
        slippage_rate: req.slippage_rate,
        max_volume_participation: req.max_volume_participation,
        benchmark_bars: None,
        allocation_strategy: req.allocation_strategy,
        symbol_weights: req.symbol_weights,
        rebalance_interval_days: req.rebalance_interval_days,
        allow_short_selling: None,
        margin_multiplier: None,
        signal_timeframe: None,
        trailing_stop_percent: None,
        max_drawdown_halt_percent: None,
        regime_config: None,
        commission_model: None,
        allow_fractional_shares: None,
        cash_sweep_rate: None,
        incremental_rebalance: None,
        param_search_space: None,
        market_impact: None,
    };

    let result = WalkForwardRunner::run(&config, folds)
        .map_err(|e| anyhow::anyhow!("Walk-forward error: {}", e))?;

    tracing::info!(
        "Walk-forward complete: {} folds, overfitting ratio: {:.2}",
        result.folds.len(),
        result.overfitting_ratio
    );

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/backtest/results/{id}/monte-carlo",
    tag = "Backtest",
    params(
        ("id" = i64, Path, description = "Backtest result ID"),
        ("simulations" = Option<i32>, Query, description = "Number of Monte Carlo simulations (100-10000, default 1000)"),
    ),
    responses(
        (status = 200, description = "Monte Carlo simulation results"),
        (status = 500, description = "Simulation error or backtest not found"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
/// Run Monte Carlo simulation on a completed backtest
pub(crate) async fn get_monte_carlo(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<MonteCarloQuery>,
) -> Result<Json<ApiResponse<MonteCarloResult>>, AppError> {
    let backtest_db = state
        .backtest_db
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    let result = backtest_db
        .get_backtest(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Backtest not found"))?;

    let trades = backtest_db.get_backtest_trades(id).await?;

    if trades.is_empty() {
        return Err(anyhow::anyhow!("No trades in backtest — cannot run Monte Carlo").into());
    }

    let simulations = query.simulations.clamp(100, 10000);

    tracing::info!(
        "Running Monte Carlo ({} sims) on backtest {} with {} trades. Trade P&L%: {:?}",
        simulations,
        id,
        trades.len(),
        trades
            .iter()
            .map(|t| t.profit_loss_percent)
            .collect::<Vec<_>>()
    );

    let mc_result = run_monte_carlo(&trades, result.initial_capital, simulations);

    tracing::info!(
        "Monte Carlo result: P(profit)={:.1}%, P(ruin)={:.1}%, median_return={:.2}%, median_sharpe={:.3}",
        mc_result.probability_of_profit, mc_result.probability_of_ruin,
        mc_result.median_return, mc_result.median_sharpe
    );

    Ok(Json(ApiResponse::success(mc_result)))
}

#[utoipa::path(
    get,
    path = "/api/backtest/results",
    tag = "Backtest",
    responses(
        (status = 200, description = "List of all backtest results"),
        (status = 500, description = "Database error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
/// Get all backtest results
pub(crate) async fn get_all_backtests(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<BacktestResult>>>, AppError> {
    let backtest_db = state
        .backtest_db
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    let results = backtest_db.get_all_backtests().await?;

    Ok(Json(ApiResponse::success(results)))
}

#[utoipa::path(
    get,
    path = "/api/backtest/results/{id}",
    tag = "Backtest",
    params(
        ("id" = i64, Path, description = "Backtest result ID"),
    ),
    responses(
        (status = 200, description = "Backtest result details"),
        (status = 404, description = "Backtest not found"),
        (status = 500, description = "Database error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
/// Get specific backtest result
pub(crate) async fn get_backtest(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<BacktestResult>>, AppError> {
    let backtest_db = state
        .backtest_db
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    let result = backtest_db
        .get_backtest(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Backtest not found"))?;

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    delete,
    path = "/api/backtest/results/{id}",
    tag = "Backtest",
    params(
        ("id" = i64, Path, description = "Backtest result ID"),
    ),
    responses(
        (status = 200, description = "Backtest deleted successfully"),
        (status = 500, description = "Database error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
/// Delete backtest result
pub(crate) async fn delete_backtest(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let backtest_db = state
        .backtest_db
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    backtest_db.delete_backtest(id).await?;

    Ok(Json(ApiResponse::success(format!(
        "Backtest {} deleted",
        id
    ))))
}

#[utoipa::path(
    get,
    path = "/api/backtest/results/{id}/trades",
    tag = "Backtest",
    params(
        ("id" = i64, Path, description = "Backtest result ID"),
    ),
    responses(
        (status = 200, description = "List of trades from the backtest"),
        (status = 500, description = "Database error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
/// Get trades for a specific backtest
pub(crate) async fn get_backtest_trades(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<BacktestTrade>>>, AppError> {
    let backtest_db = state
        .backtest_db
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    let trades = backtest_db.get_backtest_trades(id).await?;

    Ok(Json(ApiResponse::success(trades)))
}

#[utoipa::path(
    get,
    path = "/api/backtest/strategy/{name}",
    tag = "Backtest",
    params(
        ("name" = String, Path, description = "Strategy name to filter by"),
    ),
    responses(
        (status = 200, description = "Backtest results for the given strategy"),
        (status = 500, description = "Database error"),
    ),
    security(("api_key" = []), ("bearer" = [])),
)]
/// Get backtests by strategy name
pub(crate) async fn get_backtests_by_strategy(
    State(state): State<AppState>,
    Path(strategy_name): Path<String>,
) -> Result<Json<ApiResponse<Vec<BacktestResult>>>, AppError> {
    let backtest_db = state
        .backtest_db
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Backtest database not configured"))?;

    let results = backtest_db
        .get_backtests_by_strategy(&strategy_name)
        .await?;

    Ok(Json(ApiResponse::success(results)))
}
