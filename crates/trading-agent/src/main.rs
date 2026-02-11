use std::sync::Arc;
use std::time::Duration;

use analysis_orchestrator::AnalysisOrchestrator;
use alpaca_broker::AlpacaClient;
use anyhow::Result;
use chrono::Timelike;
use risk_manager::RiskManager;
use tokio::time;

mod config;
mod types;
mod ml_gate;
mod market_scanner;
mod strategy_manager;
mod trade_executor;
mod position_manager;
mod portfolio_guard;
mod discord_notifier;
mod metrics;
mod state_manager;

use config::AgentConfig;
use discord_notifier::{DailyReport, DiscordNotifier};
use market_scanner::MarketScanner;
use metrics::AgentMetrics;
use ml_gate::MLTradeGate;
use position_manager::PositionManager;
use state_manager::StateManager;
use strategy_manager::StrategyManager;
use trade_executor::TradeExecutor;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load .env, init tracing
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting InvestIQ Autonomous Trading Agent");

    // 2. Load configuration (with validation — P10)
    let config = AgentConfig::from_env()?;
    tracing::info!("Configuration loaded and validated");
    tracing::info!("  Risk per trade: {}%", config.max_risk_per_trade_percent);
    tracing::info!("  Max position size: ${}", config.max_position_size);
    tracing::info!("  Scan interval: {} seconds", config.scan_interval_seconds);
    tracing::info!("  Min confidence: {:.0}%", config.min_confidence * 100.0);
    tracing::info!("  Position limit: dynamic (min ${}/position, hard cap {})", config.min_position_value, config.max_open_positions);
    tracing::info!("  Order timeout: {}s", config.order_timeout_seconds);

    // 3. Initialize Alpaca (paper trading)
    let alpaca = Arc::new(AlpacaClient::new(
        config.alpaca_api_key.clone(),
        config.alpaca_secret_key.clone(),
        config.alpaca_base_url.clone(),
    )?);

    // 4. Safety gate: paper by default, live requires LIVE_TRADING_APPROVED=yes
    if !alpaca.is_paper() {
        let approved = std::env::var("LIVE_TRADING_APPROVED")
            .map(|v| v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);
        if !approved {
            tracing::error!(
                "ALPACA_BASE_URL points to live trading ({}). \
                 Set LIVE_TRADING_APPROVED=yes to enable, or use \
                 https://paper-api.alpaca.markets for paper.",
                alpaca.base_url()
            );
            std::process::exit(1);
        }
        tracing::warn!("LIVE TRADING MODE — REAL MONEY AT RISK ({})", alpaca.base_url());
    } else {
        tracing::info!("Paper trading mode ({})", alpaca.base_url());
    }

    // 5. Initialize DB for risk manager + trade logging
    let db_pool = sqlx::SqlitePool::connect(&config.database_url).await?;
    let risk_manager = Arc::new(RiskManager::new(db_pool.clone()));
    risk_manager.init_circuit_breaker_tables().await?;
    tracing::info!("Risk manager initialized with circuit breakers");

    // 5b. Initialize state manager (P8)
    let state_manager = StateManager::new(db_pool.clone());
    state_manager.init_tables().await?;
    tracing::info!("State manager initialized");

    // 6. Initialize orchestrator
    let orchestrator = Arc::new(
        AnalysisOrchestrator::new(config.polygon_api_key.clone()).with_db_pool(db_pool.clone()),
    );
    tracing::info!("Analysis orchestrator initialized");

    // 7. Initialize ML gate (P1: now takes orchestrator for SPY/VIX features)
    let ml_gate = MLTradeGate::new(&config.ml_signal_models_url, Arc::clone(&orchestrator));
    tracing::info!("ML trade gate initialized ({})", config.ml_signal_models_url);

    // 8. Initialize remaining components
    let scanner = MarketScanner::new(config.clone()).await?;
    tracing::info!("Market scanner initialized");

    let strategy_manager = StrategyManager::new(config.clone(), Arc::clone(&orchestrator));
    tracing::info!(
        "Strategy manager initialized ({} engines)",
        strategy_manager.strategy_count()
    );

    let executor = TradeExecutor::new(
        config.clone(),
        Arc::clone(&alpaca),
        Arc::clone(&risk_manager),
        db_pool.clone(),
    );
    tracing::info!("Trade executor initialized");

    // 8b. Cancel stale open orders from previous runs (P3/P8)
    if let Err(e) = executor.cancel_stale_orders().await {
        tracing::warn!("Failed to check for stale orders: {}", e);
    }

    let position_manager = PositionManager::new(Arc::clone(&alpaca), Arc::clone(&risk_manager));
    tracing::info!("Position manager initialized");

    let notifier = DiscordNotifier::new(config.discord_webhook_url.clone())?;
    tracing::info!("Discord notifier ready");

    // 8c. Initialize metrics (P7) with optional restore from persisted state (P8)
    let mut agent_metrics = AgentMetrics::new(config.metrics_log_interval_cycles);
    if let Ok(Some(saved)) = state_manager.load_metrics().await {
        agent_metrics.restore_from_json(&saved);
    }

    // 9. Log account info
    let account = alpaca.get_account().await?;
    tracing::info!(
        "Paper account: ${} cash, ${} buying power, ${} portfolio value",
        account.cash,
        account.buying_power,
        account.portfolio_value
    );

    // 10. Send startup notification
    notifier
        .send_message(&format!(
            "**Trading Agent Started**\n\
             Cash: ${} | Buying power: ${} | Portfolio: ${}\n\
             Watchlist: {} symbols\n\
             Min confidence: {:.0}%\n\
             Scan interval: {}s | Max positions: {} | Order timeout: {}s",
            account.cash,
            account.buying_power,
            account.portfolio_value,
            config.watchlist.len(),
            config.min_confidence * 100.0,
            config.scan_interval_seconds,
            config.max_open_positions,
            config.order_timeout_seconds
        ))
        .await?;

    tracing::info!(
        "Agent is now running. Scanning every {}s. Press Ctrl+C to stop.",
        config.scan_interval_seconds
    );

    // Track last daily report date (P9)
    let mut last_report_date = state_manager
        .load_last_report_date()
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    // Main loop with graceful shutdown
    let mut interval = time::interval(Duration::from_secs(config.scan_interval_seconds));
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = run_trading_cycle(
                    &scanner,
                    &strategy_manager,
                    &ml_gate,
                    &executor,
                    &position_manager,
                    &notifier,
                    &config,
                    &mut agent_metrics,
                )
                .await
                {
                    tracing::error!("Error in trading cycle: {}", e);
                }

                // Persist metrics after each cycle (P8)
                if let Err(e) = state_manager.save_metrics(&agent_metrics.to_json()).await {
                    tracing::debug!("Failed to persist metrics: {}", e);
                }

                // Daily report check (P9): send after 4:05 PM ET
                let now_et = chrono::Utc::now().with_timezone(&chrono_tz::US::Eastern);
                let today = now_et.format("%Y-%m-%d").to_string();
                let hour = now_et.hour();
                let minute = now_et.minute();

                if today != last_report_date && hour == 16 && minute >= 5 {
                    tracing::info!("Generating daily report for {}", today);
                    if let Err(e) = send_daily_report(
                        &alpaca,
                        &notifier,
                        &agent_metrics,
                    ).await {
                        tracing::warn!("Failed to send daily report: {}", e);
                    }
                    last_report_date.clone_from(&today);
                    state_manager.save_last_report_date(&today).await.ok();
                }
            }
            _ = &mut shutdown => {
                tracing::info!("Shutdown signal received, exiting gracefully...");

                // Final metrics persist
                state_manager.save_metrics(&agent_metrics.to_json()).await.ok();
                agent_metrics.log_metrics();

                notifier
                    .send_message("**Trading Agent Stopped** — graceful shutdown")
                    .await
                    .ok();
                break;
            }
        }
    }

    tracing::info!("Trading agent shut down.");
    Ok(())
}

async fn run_trading_cycle(
    scanner: &MarketScanner,
    strategy_manager: &StrategyManager,
    ml_gate: &MLTradeGate,
    executor: &TradeExecutor,
    position_manager: &PositionManager,
    notifier: &DiscordNotifier,
    config: &AgentConfig,
    metrics: &mut AgentMetrics,
) -> Result<()> {
    let cycle_start = AgentMetrics::start_timer();
    tracing::info!("Starting trading cycle...");

    // 1. Manage existing positions (check stop losses, take profits)
    let position_actions = position_manager.check_positions().await?;
    for action in &position_actions {
        tracing::info!("Position action: {} {} @ ${:.2}", action.action_type, action.symbol, action.price);

        if let Err(e) = executor.execute_position_action(action).await {
            tracing::error!("Failed to execute position action for {}: {}", action.symbol, e);
        } else {
            metrics.record_trade_result(action.pnl);
            let message = format!(
                "**{}** {}\nPrice: ${:.2}\nP/L: ${:.2}",
                action.action_type, action.symbol, action.price, action.pnl
            );
            notifier.send_message(&message).await?;
        }
    }

    // 2. Scan market for opportunities
    let scan_start = AgentMetrics::start_timer();
    let opportunities = scanner.scan().await?;
    metrics.record_scan_duration(scan_start);
    tracing::info!("Found {} potential opportunities", opportunities.len());

    if opportunities.is_empty() {
        metrics.finish_cycle(cycle_start);
        return Ok(());
    }

    // 3. Run orchestrator analysis on opportunities → generate signals
    let analysis_start = AgentMetrics::start_timer();
    let signal_results = strategy_manager.generate_signals(&opportunities).await?;
    metrics.record_analysis_duration(analysis_start);
    metrics.signals_generated += signal_results.len() as u64;
    tracing::info!("Generated {} signals from orchestrator", signal_results.len());

    // 4. Filter by config thresholds
    if !signal_results.is_empty() {
        // Log confidence distribution so we can tune thresholds
        let mut confidences: Vec<f64> = signal_results.iter().map(|sr| sr.signal.confidence).collect();
        confidences.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let max_conf = confidences.first().copied().unwrap_or(0.0);
        let min_conf = confidences.last().copied().unwrap_or(0.0);
        let median_conf = confidences[confidences.len() / 2];
        let above_threshold = confidences.iter().filter(|&&c| c >= config.min_confidence).count();
        tracing::info!(
            "Signal confidence distribution: max={:.1}%, median={:.1}%, min={:.1}% ({} of {} >= {:.0}% threshold)",
            max_conf * 100.0, median_conf * 100.0, min_conf * 100.0,
            above_threshold, confidences.len(), config.min_confidence * 100.0
        );
        // Log top 5 signals for visibility
        for sr in signal_results.iter().take(5) {
            let wr = sr.signal.historical_win_rate.map(|w| format!("{:.1}%", w * 100.0)).unwrap_or_else(|| "N/A".to_string());
            tracing::info!(
                "  Top signal: {} {} conf={:.1}% win_rate={}",
                sr.signal.action, sr.signal.symbol, sr.signal.confidence * 100.0, wr
            );
        }
    }

    let filtered: Vec<_> = signal_results
        .into_iter()
        .filter(|sr| {
            sr.signal.confidence >= config.min_confidence
                && sr
                    .signal
                    .historical_win_rate
                    .unwrap_or(config.min_win_rate)
                    >= config.min_win_rate
        })
        .collect();

    metrics.signals_filtered += filtered.len() as u64;
    tracing::info!("{} signals passed confidence/win-rate filter", filtered.len());

    // 5. Check if market is open for trading
    let market_open = scanner.is_market_open();
    if !market_open {
        if !filtered.is_empty() {
            tracing::info!(
                "Market closed: {} signals identified but trade execution skipped",
                filtered.len()
            );
            for sr in &filtered {
                tracing::info!(
                    "  Signal (analysis-only): {} {} @ ${:.2} (confidence: {:.1}%, win rate: {:.1}%)",
                    sr.signal.action,
                    sr.signal.symbol,
                    sr.signal.entry_price,
                    sr.signal.confidence * 100.0,
                    sr.signal.historical_win_rate.unwrap_or(0.0) * 100.0
                );
            }
        }
        metrics.finish_cycle(cycle_start);
        return Ok(());
    }

    // 6. For each signal, run through ML gate then execute
    let gate_start = AgentMetrics::start_timer();
    let mut approved_signals = Vec::new();

    for sr in &filtered {
        tracing::info!(
            "Evaluating: {} {} (confidence: {:.1}%{})",
            sr.signal.action,
            sr.signal.symbol,
            sr.signal.confidence * 100.0,
            if let Some(atr) = sr.signal.atr {
                format!(", ATR={:.2}", atr)
            } else {
                String::new()
            }
        );

        // ML gate decision
        let decision = ml_gate.evaluate_trade(&sr.signal, &sr.analysis).await;

        if decision.approved {
            metrics.signals_ml_approved += 1;
            tracing::info!(
                "ML gate approved {} {}: {}",
                sr.signal.action,
                sr.signal.symbol,
                decision.reasoning
            );
            approved_signals.push((sr, decision));
        } else {
            metrics.signals_ml_rejected += 1;
            tracing::info!(
                "ML gate rejected {} {}: {}",
                sr.signal.action,
                sr.signal.symbol,
                decision.reasoning
            );
        }
    }
    metrics.record_gate_duration(gate_start);

    // 7. Propose approved signals for manual review (pending trades)
    let exec_start = AgentMetrics::start_timer();
    for (sr, decision) in approved_signals {
        match executor.propose_signal(&sr.signal, &decision.reasoning).await {
            Ok(proposal) => {
                // Save to pending_trades table for human review
                match executor.save_pending_trade(&proposal).await {
                    Ok(trade_id) => {
                        metrics.trades_executed += 1;
                        tracing::info!(
                            "Trade #{} proposed for review: {} {} x{} @ ~${:.2}",
                            trade_id,
                            proposal.action.to_uppercase(),
                            proposal.symbol,
                            proposal.shares,
                            proposal.entry_price
                        );

                        // Notify via Discord for manual review
                        let message = format!(
                            "**Trade Pending Review** (#{trade_id})\n\
                             **{action} {symbol}** — {shares} shares @ ~${price:.2}\n\
                             Confidence: {conf:.1}% | ML P(win): {ml:.1}%\n\
                             {reason}\n\
                             _Approve or reject in the Agent Trades tab._",
                            trade_id = trade_id,
                            action = proposal.action.to_uppercase(),
                            symbol = proposal.symbol,
                            shares = proposal.shares,
                            price = proposal.entry_price,
                            conf = proposal.confidence * 100.0,
                            ml = decision.probability * 100.0,
                            reason = proposal.reason,
                        );
                        notifier.send_message(&message).await?;
                    }
                    Err(e) => {
                        metrics.trades_failed += 1;
                        tracing::warn!("Failed to save pending trade for {}: {}", sr.signal.symbol, e);
                    }
                }
            }
            Err(e) => {
                metrics.trades_failed += 1;
                tracing::warn!("Skipped {} {}: {}", sr.signal.action, sr.signal.symbol, e);
            }
        }
    }
    metrics.record_execution_duration(exec_start);

    metrics.finish_cycle(cycle_start);
    Ok(())
}

/// Build and send the daily report after market close (P9).
async fn send_daily_report(
    alpaca: &Arc<AlpacaClient>,
    notifier: &DiscordNotifier,
    metrics: &AgentMetrics,
) -> Result<()> {
    let account = alpaca.get_account().await?;
    let positions = alpaca.get_positions().await?;

    let portfolio_value: f64 = account.portfolio_value.parse().unwrap_or(0.0);
    let account_balance: f64 = account.cash.parse().unwrap_or(0.0);

    let daily_pl: f64 = positions
        .iter()
        .filter_map(|p| p.unrealized_pl.parse::<f64>().ok())
        .sum();
    let pnl_percent = if portfolio_value > 0.0 {
        (daily_pl / portfolio_value) * 100.0
    } else {
        0.0
    };

    let largest_position = positions
        .iter()
        .max_by(|a, b| {
            let av = a.market_value.parse::<f64>().unwrap_or(0.0).abs();
            let bv = b.market_value.parse::<f64>().unwrap_or(0.0).abs();
            av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|p| p.symbol.clone())
        .unwrap_or_else(|| "None".to_string());

    let gross_exposure: f64 = positions
        .iter()
        .filter_map(|p| p.market_value.parse::<f64>().ok())
        .map(|v| v.abs())
        .sum();
    let exposure_pct = if portfolio_value > 0.0 {
        (gross_exposure / portfolio_value) * 100.0
    } else {
        0.0
    };

    let total_trades = metrics.winning_trades + metrics.losing_trades;
    let win_rate = if total_trades > 0 {
        metrics.winning_trades as f64 / total_trades as f64
    } else {
        0.0
    };

    let report = DailyReport {
        pnl: daily_pl,
        pnl_percent,
        trade_count: metrics.trades_executed as usize,
        win_rate,
        best_trade_symbol: "N/A".to_string(),
        best_trade_pnl: 0.0,
        worst_trade_symbol: "N/A".to_string(),
        worst_trade_pnl: 0.0,
        account_balance,
        positions_held: positions.len(),
        largest_position,
        exposure_percent: exposure_pct,
        regime: "N/A".to_string(),
    };

    notifier.send_daily_report(&report).await?;
    tracing::info!("Daily report sent successfully");
    Ok(())
}
