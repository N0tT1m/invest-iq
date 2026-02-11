use std::sync::Arc;
use std::time::Duration;

use analysis_orchestrator::AnalysisOrchestrator;
use alpaca_broker::AlpacaClient;
use anyhow::Result;
use risk_manager::RiskManager;
use tokio::time;

mod config;
mod types;
mod ml_gate;
mod market_scanner;
mod strategy_manager;
mod trade_executor;
mod position_manager;
mod discord_notifier;

use config::AgentConfig;
use discord_notifier::DiscordNotifier;
use market_scanner::MarketScanner;
use ml_gate::MLTradeGate;
use position_manager::PositionManager;
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

    // 2. Load configuration
    let config = AgentConfig::from_env()?;
    tracing::info!("Configuration loaded");
    tracing::info!("  Risk per trade: {}%", config.max_risk_per_trade_percent);
    tracing::info!("  Max position size: ${}", config.max_position_size);
    tracing::info!("  Scan interval: {} seconds", config.scan_interval_seconds);
    tracing::info!("  Min confidence: {:.0}%", config.min_confidence * 100.0);

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

    // 6. Initialize orchestrator
    let orchestrator = Arc::new(
        AnalysisOrchestrator::new(config.polygon_api_key.clone()).with_db_pool(db_pool.clone()),
    );
    tracing::info!("Analysis orchestrator initialized");

    // 7. Initialize ML gate
    let ml_gate = MLTradeGate::new(&config.ml_signal_models_url);
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

    let position_manager = PositionManager::new(Arc::clone(&alpaca), Arc::clone(&risk_manager));
    tracing::info!("Position manager initialized");

    let notifier = DiscordNotifier::new(config.discord_webhook_url.clone())?;
    tracing::info!("Discord notifier ready");

    // 9. Log account info
    let account = alpaca.get_account().await?;
    tracing::info!(
        "Paper account: ${} buying power, ${} portfolio value",
        account.buying_power,
        account.portfolio_value
    );

    // 10. Send startup notification
    notifier
        .send_message(&format!(
            "**Trading Agent Started**\n\
             Paper account: ${} buying power\n\
             Watchlist: {} symbols\n\
             Min confidence: {:.0}%\n\
             Scan interval: {}s",
            account.buying_power,
            config.watchlist.len(),
            config.min_confidence * 100.0,
            config.scan_interval_seconds
        ))
        .await?;

    tracing::info!(
        "Agent is now running. Scanning every {}s. Press Ctrl+C to stop.",
        config.scan_interval_seconds
    );

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
                )
                .await
                {
                    tracing::error!("Error in trading cycle: {}", e);
                    notifier
                        .send_message(&format!("**Error**: {}", e))
                        .await
                        .ok();
                }
            }
            _ = &mut shutdown => {
                tracing::info!("Shutdown signal received, exiting gracefully...");
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
) -> Result<()> {
    tracing::info!("Starting trading cycle...");

    // 1. Manage existing positions (check stop losses, take profits)
    let position_actions = position_manager.check_positions().await?;
    for action in &position_actions {
        tracing::info!("Position action: {} {} @ ${:.2}", action.action_type, action.symbol, action.price);

        if let Err(e) = executor.execute_position_action(action).await {
            tracing::error!("Failed to execute position action for {}: {}", action.symbol, e);
        } else {
            let message = format!(
                "**{}** {}\nPrice: ${:.2}\nP/L: ${:.2}",
                action.action_type, action.symbol, action.price, action.pnl
            );
            notifier.send_message(&message).await?;
        }
    }

    // 2. Scan market for opportunities
    let opportunities = scanner.scan().await?;
    tracing::info!("Found {} potential opportunities", opportunities.len());

    if opportunities.is_empty() {
        return Ok(());
    }

    // 3. Run orchestrator analysis on opportunities → generate signals
    let signal_results = strategy_manager.generate_signals(&opportunities).await?;
    tracing::info!("Generated {} signals from orchestrator", signal_results.len());

    // 4. Filter by config thresholds
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

    tracing::info!("{} signals passed confidence/win-rate filter", filtered.len());

    // 5. Check if market is open for trading
    let market_open = scanner.is_market_open();
    if !market_open {
        if !filtered.is_empty() {
            tracing::info!(
                "Market closed: {} signals identified but trade execution skipped",
                filtered.len()
            );
            // Log signals for analysis
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
        return Ok(());
    }

    // 6. For each signal, run through ML gate then execute
    for sr in filtered {
        tracing::info!(
            "Evaluating: {} {} (confidence: {:.1}%)",
            sr.signal.action,
            sr.signal.symbol,
            sr.signal.confidence * 100.0
        );

        // ML gate decision
        let decision = ml_gate.evaluate_trade(&sr.signal, &sr.analysis).await;

        if decision.approved {
            tracing::info!(
                "ML gate approved {} {}: {}",
                sr.signal.action,
                sr.signal.symbol,
                decision.reasoning
            );

            match executor.execute_signal(&sr.signal).await {
                Ok(execution) => {
                    let message = format!(
                        "**{} {}**\n{} shares @ ${:.2}\nConfidence: {:.1}%\nML P(win): {:.1}%\n{}",
                        sr.signal.action,
                        sr.signal.symbol,
                        execution.quantity,
                        execution.price,
                        sr.signal.confidence * 100.0,
                        decision.probability * 100.0,
                        decision.reasoning
                    );
                    notifier.send_message(&message).await?;
                }
                Err(e) => {
                    tracing::error!("Failed to execute trade for {}: {}", sr.signal.symbol, e);
                    notifier
                        .send_message(&format!(
                            "**Failed** {} {}: {}",
                            sr.signal.action, sr.signal.symbol, e
                        ))
                        .await?;
                }
            }
        } else {
            tracing::info!(
                "ML gate rejected {} {}: {}",
                sr.signal.action,
                sr.signal.symbol,
                decision.reasoning
            );
        }
    }

    Ok(())
}
