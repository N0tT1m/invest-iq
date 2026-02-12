use std::sync::Arc;
use std::time::Duration;

use alpaca_broker::AlpacaClient;
use analysis_orchestrator::AnalysisOrchestrator;
use anyhow::Result;
use chrono::Timelike;
use risk_manager::RiskManager;
use tokio::signal::unix::SignalKind;
use tokio::time;

mod config;
mod discord_notifier;
mod market_scanner;
mod metrics;
mod ml_gate;
mod portfolio_guard;
mod position_manager;
mod state_manager;
mod strategy_manager;
mod trade_executor;
mod types;

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

    let json_logging = std::env::var("RUST_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    if json_logging {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .init();
    }

    // Panic hook: log panic info before crashing
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC: {info}");
        tracing::error!("PANIC: {info}");
    }));

    tracing::info!("Starting InvestIQ Autonomous Trading Agent");

    // 2. Load configuration (with validation — P10)
    let config = AgentConfig::from_env()?;
    tracing::info!("Configuration loaded and validated");
    tracing::info!("  Risk per trade: {}%", config.max_risk_per_trade_percent);
    tracing::info!("  Max position size: ${}", config.max_position_size);
    tracing::info!("  Scan interval: {} seconds", config.scan_interval_seconds);
    tracing::info!("  Min confidence: {:.0}%", config.min_confidence * 100.0);
    tracing::info!(
        "  Position limit: dynamic (min ${}/position, hard cap {})",
        config.min_position_value,
        config.max_open_positions
    );
    tracing::info!("  Order timeout: {}s", config.order_timeout_seconds);
    tracing::info!(
        "  Auto-execute: {} (max {} concurrent)",
        config.auto_execute,
        config.max_concurrent_executions
    );

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
        tracing::warn!(
            "LIVE TRADING MODE — REAL MONEY AT RISK ({})",
            alpaca.base_url()
        );
    } else {
        tracing::info!("Paper trading mode ({})", alpaca.base_url());
    }

    // 5. Initialize DB for risk manager + trade logging
    sqlx::any::install_default_drivers();
    let db_pool = sqlx::AnyPool::connect(&config.database_url).await?;
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
    tracing::info!(
        "ML trade gate initialized ({})",
        config.ml_signal_models_url
    );

    // 8. Initialize remaining components
    let scanner = MarketScanner::new(config.clone()).await?;
    tracing::info!("Market scanner initialized");

    let strategy_manager = StrategyManager::new(config.clone(), Arc::clone(&orchestrator));
    tracing::info!(
        "Strategy manager initialized ({} engines)",
        strategy_manager.strategy_count()
    );

    let executor = Arc::new(TradeExecutor::new(
        config.clone(),
        Arc::clone(&alpaca),
        Arc::clone(&risk_manager),
        db_pool.clone(),
    ));
    tracing::info!("Trade executor initialized");

    // 8b. Cancel stale open orders from previous runs (P3/P8)
    if let Err(e) = executor.cancel_stale_orders().await {
        tracing::warn!("Failed to check for stale orders: {}", e);
    }

    let position_manager = PositionManager::new(Arc::clone(&alpaca), Arc::clone(&risk_manager));
    tracing::info!("Position manager initialized");

    let notifier = DiscordNotifier::new(config.discord_webhook_url.clone())?;
    let notifier_webhook_url = config.discord_webhook_url.clone();
    tracing::info!("Discord notifier ready");

    // 8c. Initialize metrics (P7) with optional restore from persisted state (P8)
    let mut agent_metrics = AgentMetrics::new(config.metrics_log_interval_cycles);
    if let Ok(Some(saved)) = state_manager.load_metrics().await {
        agent_metrics.restore_from_json(&saved);
    }

    // 9. Startup connectivity checks
    // DB check
    sqlx::query("SELECT 1")
        .execute(&db_pool)
        .await
        .map_err(|e| anyhow::anyhow!("Database connectivity check failed: {}", e))?;
    tracing::info!("Startup check: database OK");

    // Alpaca check (also provides account info)
    let account = alpaca
        .get_account()
        .await
        .map_err(|e| anyhow::anyhow!("Alpaca connectivity check failed: {}", e))?;
    tracing::info!("Startup check: Alpaca OK");

    // ML signal models check (warn-only, not fatal)
    match reqwest::Client::new()
        .get(format!("{}/health", config.ml_signal_models_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("Startup check: ML signal models OK");
        }
        Ok(resp) => {
            tracing::warn!(
                "Startup check: ML signal models returned {} — trades will use fallback scoring",
                resp.status()
            );
        }
        Err(e) => {
            tracing::warn!(
                "Startup check: ML signal models unreachable ({}) — trades will use fallback scoring",
                e
            );
        }
    }

    // Log account info
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

    // Main loop with graceful shutdown (SIGINT + SIGTERM)
    let mut interval = time::interval(Duration::from_secs(config.scan_interval_seconds));
    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate())?;
    let shutdown = async {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received SIGINT");
            }
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM");
            }
        }
    };
    tokio::pin!(shutdown);

    // Heartbeat: send a Discord status every N cycles so the user knows the bot is alive
    let heartbeat_interval_cycles: u64 = std::env::var("HEARTBEAT_INTERVAL_CYCLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6); // Every 6 cycles = every 30 min at 5-min intervals

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
                    &notifier_webhook_url,
                    &config,
                    &mut agent_metrics,
                    &state_manager,
                )
                .await
                {
                    tracing::error!("Error in trading cycle: {}", e);
                    // Notify Discord about cycle errors so the user doesn't think it stopped
                    notifier
                        .send_message(&format!(
                            "**Cycle Error** (cycle #{}): {}\n_Agent is still running._",
                            agent_metrics.cycles_run + 1,
                            e
                        ))
                        .await
                        .ok();
                }

                // Persist metrics after each cycle (P8)
                if let Err(e) = state_manager.save_metrics(&agent_metrics.to_json()).await {
                    tracing::debug!("Failed to persist metrics: {}", e);
                }

                // Heartbeat: periodic Discord status so the user knows the bot is alive
                if heartbeat_interval_cycles > 0
                    && agent_metrics.cycles_run > 0
                    && agent_metrics.cycles_run.is_multiple_of(heartbeat_interval_cycles)
                {
                    let positions = alpaca.get_positions().await.map(|p| p.len()).unwrap_or(0);
                    notifier
                        .send_message(&format!(
                            "**Heartbeat** | Cycle #{} | {} signals scanned, {} proposed, {} positions | Last cycle: {:.1}s",
                            agent_metrics.cycles_run,
                            agent_metrics.signals_generated,
                            agent_metrics.trades_executed,
                            positions,
                            agent_metrics.last_total_duration_ms as f64 / 1000.0,
                        ))
                        .await
                        .ok();
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
                        &state_manager,
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

#[allow(clippy::too_many_arguments)]
async fn run_trading_cycle(
    scanner: &MarketScanner,
    strategy_manager: &StrategyManager,
    ml_gate: &MLTradeGate,
    executor: &Arc<TradeExecutor>,
    position_manager: &PositionManager,
    notifier: &DiscordNotifier,
    notifier_webhook_url: &str,
    config: &AgentConfig,
    metrics: &mut AgentMetrics,
    state_manager: &StateManager,
) -> Result<()> {
    let cycle_start = AgentMetrics::start_timer();
    tracing::info!("Starting trading cycle...");

    // 1. Manage existing positions (check stop losses, take profits)
    let position_actions = position_manager.check_positions().await?;
    for action in &position_actions {
        tracing::info!(
            "Position action: {} {} @ ${:.2}",
            action.action_type,
            action.symbol,
            action.price
        );

        if let Err(e) = executor.execute_position_action(action).await {
            tracing::error!(
                "Failed to execute position action for {}: {}",
                action.symbol,
                e
            );
        } else {
            metrics.record_trade_result(action.pnl);

            // Record exit in trade context
            let pending_id: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM pending_trades WHERE symbol = ? AND status = 'executed'
                 ORDER BY proposed_at DESC LIMIT 1",
            )
            .bind(&action.symbol)
            .fetch_optional(&state_manager.db_pool)
            .await
            .ok()
            .flatten();

            if let Some((trade_id,)) = pending_id {
                let regime = state_manager
                    .load_state("current_regime")
                    .await
                    .ok()
                    .flatten();
                if let Err(e) = state_manager
                    .record_trade_exit(
                        trade_id,
                        &action.action_type,
                        action.price,
                        action.pnl,
                        0.0,
                        regime.as_deref(),
                    )
                    .await
                {
                    tracing::debug!("Failed to record trade exit context: {}", e);
                }
            }

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
        tracing::info!(
            "Cycle #{} complete (no opportunities found)",
            metrics.cycles_run
        );
        return Ok(());
    }

    // 3. Run orchestrator analysis on opportunities → generate signals
    let analysis_start = AgentMetrics::start_timer();
    let signal_results = strategy_manager.generate_signals(&opportunities).await?;
    metrics.record_analysis_duration(analysis_start);
    metrics.signals_generated += signal_results.len() as u64;
    tracing::info!(
        "Generated {} signals from orchestrator",
        signal_results.len()
    );

    // Save current regime from first signal
    if let Some(first) = signal_results.first() {
        if let Some(regime) = &first.signal.regime {
            state_manager
                .save_state("current_regime", regime)
                .await
                .ok();
        }
    }

    // 4. Filter by config thresholds
    if !signal_results.is_empty() {
        // Log confidence distribution so we can tune thresholds
        let mut confidences: Vec<f64> = signal_results
            .iter()
            .map(|sr| sr.signal.confidence)
            .collect();
        confidences.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let max_conf = confidences.first().copied().unwrap_or(0.0);
        let min_conf = confidences.last().copied().unwrap_or(0.0);
        let median_conf = confidences[confidences.len() / 2];
        let above_threshold = confidences
            .iter()
            .filter(|&&c| c >= config.min_confidence)
            .count();
        tracing::info!(
            "Signal confidence distribution: max={:.1}%, median={:.1}%, min={:.1}% ({} of {} >= {:.0}% threshold)",
            max_conf * 100.0, median_conf * 100.0, min_conf * 100.0,
            above_threshold, confidences.len(), config.min_confidence * 100.0
        );
        // Log top 5 signals for visibility
        for sr in signal_results.iter().take(5) {
            let wr = sr
                .signal
                .historical_win_rate
                .map(|w| format!("{:.1}%", w * 100.0))
                .unwrap_or_else(|| "N/A".to_string());
            tracing::info!(
                "  Top signal: {} {} conf={:.1}% win_rate={}",
                sr.signal.action,
                sr.signal.symbol,
                sr.signal.confidence * 100.0,
                wr
            );
        }
    }

    let filtered: Vec<_> = signal_results
        .into_iter()
        .filter(|sr| {
            sr.signal.confidence >= config.min_confidence
                && sr.signal.historical_win_rate.unwrap_or(config.min_win_rate)
                    >= config.min_win_rate
        })
        .collect();

    let filtered_count = filtered.len();
    metrics.signals_filtered += filtered_count as u64;
    tracing::info!(
        "{} signals passed confidence/win-rate filter",
        filtered_count
    );

    // 5. Check if market is open (for logging only — signals still go to pending review)
    let market_open = scanner.is_market_open();
    if !market_open && !filtered.is_empty() {
        tracing::info!(
            "Market closed: {} signals will be queued for approval",
            filtered.len()
        );
    }

    // 6. For each signal, run through ML gate then propose for review
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

    // 7. Execute or propose approved signals
    // Skip symbols that already have a pending (unreviewed) trade
    let pending_symbols = executor.get_pending_symbols().await.unwrap_or_default();
    let approved_signals: Vec<_> = approved_signals
        .into_iter()
        .filter(|(sr, _)| {
            if pending_symbols.contains(&sr.signal.symbol) {
                tracing::info!(
                    "Skipping {} {} — already has a pending trade awaiting review",
                    sr.signal.action,
                    sr.signal.symbol
                );
                false
            } else {
                true
            }
        })
        .collect();

    let approved_signals_count = approved_signals.len();
    let exec_start = AgentMetrics::start_timer();

    let auto_execute = config.auto_execute && market_open;
    if auto_execute && !approved_signals.is_empty() {
        tracing::info!(
            "Auto-executing {} trades concurrently (max {})",
            approved_signals.len(),
            config.max_concurrent_executions
        );
    }

    // Concurrent execution via semaphore
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(
        config.max_concurrent_executions,
    ));
    let mut handles = Vec::new();

    for (sr, decision) in &approved_signals {
        let signal = sr.signal.clone();
        let decision_reasoning = decision.reasoning.clone();
        let decision_probability = decision.probability;
        let sem = std::sync::Arc::clone(&sem);

        if auto_execute {
            // Auto-execute: submit order to Alpaca immediately
            handles.push(tokio::spawn({
                let executor = std::sync::Arc::clone(executor);
                let _url = notifier_webhook_url.to_string();
                async move {
                    let _permit = sem.acquire().await.unwrap();
                    match executor.execute_signal(&signal).await {
                        Ok(exec) => {
                            tracing::info!(
                                "Executed {} {} x{} @ ${:.2} (order {})",
                                exec.action,
                                exec.symbol,
                                exec.quantity,
                                exec.price,
                                exec.order_id
                            );
                            let msg = format!(
                                "**Trade Executed**\n\
                                 **{} {}** — {} shares @ ${:.2}\n\
                                 Confidence: {:.1}% | ML P(win): {:.1}%\n\
                                 Order: {}",
                                exec.action,
                                exec.symbol,
                                exec.quantity,
                                exec.price,
                                signal.confidence * 100.0,
                                decision_probability * 100.0,
                                exec.order_id,
                            );
                            (true, signal.symbol.clone(), Some(msg))
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to execute {} {}: {}",
                                signal.action,
                                signal.symbol,
                                e
                            );
                            (false, signal.symbol.clone(), None)
                        }
                    }
                }
            }));
        } else {
            // Manual review: save as pending trade
            handles.push(tokio::spawn({
                let executor = std::sync::Arc::clone(executor);
                async move {
                    let _permit = sem.acquire().await.unwrap();
                    match executor.propose_signal(&signal, &decision_reasoning).await {
                        Ok(proposal) => match executor.save_pending_trade(&proposal).await {
                            Ok(trade_id) => {
                                let msg = format!(
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
                                    ml = decision_probability * 100.0,
                                    reason = proposal.reason,
                                );
                                (true, signal.symbol.clone(), Some(msg))
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to save pending trade for {}: {}",
                                    signal.symbol,
                                    e
                                );
                                (false, signal.symbol.clone(), None)
                            }
                        },
                        Err(e) => {
                            tracing::warn!("Skipped {} {}: {}", signal.action, signal.symbol, e);
                            (false, signal.symbol.clone(), None)
                        }
                    }
                }
            }));
        }
    }

    // Collect results and batch Discord notifications
    let mut proposed_count = 0usize;
    let mut notification_lines: Vec<String> = Vec::new();
    for handle in handles {
        if let Ok((success, _symbol, msg)) = handle.await {
            if success {
                metrics.trades_executed += 1;
                proposed_count += 1;
            } else {
                metrics.trades_failed += 1;
            }
            if let Some(msg) = msg {
                notification_lines.push(msg);
            }
        }
    }

    // Send a single batched Discord message to avoid rate limiting
    if !notification_lines.is_empty() {
        if notification_lines.len() == 1 {
            notifier.send_message(&notification_lines[0]).await.ok();
        } else {
            // Batch into chunks of ~1900 chars (Discord 2000 char limit)
            let mut batch = String::new();
            for line in &notification_lines {
                if !batch.is_empty() && batch.len() + line.len() + 2 > 1900 {
                    notifier.send_message(&batch).await.ok();
                    batch.clear();
                }
                if !batch.is_empty() {
                    batch.push_str("\n\n");
                }
                batch.push_str(line);
            }
            if !batch.is_empty() {
                notifier.send_message(&batch).await.ok();
            }
        }
    }

    // Save trade context for all approved signals (fire-and-forget)
    for (sr, decision) in &approved_signals {
        if let Ok(Some((trade_id,))) = sqlx::query_as::<_, (i64,)>(
            "SELECT id FROM pending_trades WHERE symbol = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(&sr.signal.symbol)
        .fetch_optional(&state_manager.db_pool)
        .await
        {
            state_manager
                .save_trade_context_v2(trade_id, &sr.signal, &sr.analysis, decision)
                .await
                .ok();
        }
    }

    metrics.record_execution_duration(exec_start);

    metrics.finish_cycle(cycle_start);
    tracing::info!(
        "Cycle #{} complete in {:.1}s — {} opportunities, {} filtered, {} ML-approved, {} proposed",
        metrics.cycles_run,
        metrics.last_total_duration_ms as f64 / 1000.0,
        opportunities.len(),
        filtered_count,
        approved_signals_count,
        proposed_count,
    );
    Ok(())
}

/// Build and send the daily report after market close (P9).
async fn send_daily_report(
    alpaca: &Arc<AlpacaClient>,
    notifier: &DiscordNotifier,
    metrics: &AgentMetrics,
    state_manager: &StateManager,
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

    // Query best/worst trades today
    let (best, worst) = state_manager
        .get_best_worst_trades_today()
        .await
        .unwrap_or((None, None));

    let best_trade_symbol = best
        .as_ref()
        .map(|(s, _)| s.clone())
        .unwrap_or_else(|| "N/A".to_string());
    let best_trade_pnl = best.map(|(_, p)| p).unwrap_or(0.0);
    let worst_trade_symbol = worst
        .as_ref()
        .map(|(s, _)| s.clone())
        .unwrap_or_else(|| "N/A".to_string());
    let worst_trade_pnl = worst.map(|(_, p)| p).unwrap_or(0.0);

    // Load regime
    let regime = state_manager
        .load_state("current_regime")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "N/A".to_string());

    // Conviction breakdown
    let conviction = state_manager
        .get_conviction_breakdown_today()
        .await
        .unwrap_or_default();
    let conviction_high = *conviction.get("HIGH").unwrap_or(&0) as usize;
    let conviction_moderate = *conviction.get("MODERATE").unwrap_or(&0) as usize;
    let conviction_low = *conviction.get("LOW").unwrap_or(&0) as usize;

    // Adjustment summary
    let adjustments = state_manager
        .get_adjustment_summary_today()
        .await
        .unwrap_or_default();
    let insider_signals = *adjustments.get("insider_buying").unwrap_or(&0);
    let smart_money_boosts = *adjustments.get("smart_money_boost").unwrap_or(&0);
    let iv_penalties = *adjustments.get("high_iv_penalty").unwrap_or(&0);
    let gap_boosts = *adjustments.get("gap_up_boost").unwrap_or(&0)
        + *adjustments.get("gap_down_boost").unwrap_or(&0);

    // Save daily snapshot
    let today = chrono::Utc::now()
        .with_timezone(&chrono_tz::US::Eastern)
        .format("%Y-%m-%d")
        .to_string();
    if let Err(e) = state_manager
        .save_daily_snapshot(&today, metrics, Some(&regime))
        .await
    {
        tracing::debug!("Failed to save daily snapshot: {}", e);
    }

    let report = DailyReport {
        pnl: daily_pl,
        pnl_percent,
        trade_count: metrics.trades_executed as usize,
        win_rate,
        best_trade_symbol,
        best_trade_pnl,
        worst_trade_symbol,
        worst_trade_pnl,
        account_balance,
        positions_held: positions.len(),
        largest_position,
        exposure_percent: exposure_pct,
        regime,
        signals_generated: metrics.signals_generated,
        signals_filtered: metrics.signals_filtered,
        signals_ml_approved: metrics.signals_ml_approved,
        signals_ml_rejected: metrics.signals_ml_rejected,
        conviction_high,
        conviction_moderate,
        conviction_low,
        insider_signals,
        smart_money_boosts,
        iv_penalties,
        gap_boosts,
    };

    notifier.send_daily_report(&report).await?;
    tracing::info!("Daily report sent successfully");
    Ok(())
}
