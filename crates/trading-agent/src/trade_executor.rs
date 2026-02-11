use std::sync::Arc;

use alpaca_broker::{AlpacaClient, MarketOrderRequest};
use anyhow::Result;
use chrono::Utc;
use risk_manager::RiskManager;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use sqlx::SqlitePool;

use crate::config::AgentConfig;
use crate::portfolio_guard::PortfolioGuard;
use crate::types::{PositionAction, TradeExecution, TradeProposal, TradingSignal};

pub struct TradeExecutor {
    config: AgentConfig,
    alpaca: Arc<AlpacaClient>,
    risk_manager: Arc<RiskManager>,
    portfolio_guard: PortfolioGuard,
    db_pool: SqlitePool,
}

impl TradeExecutor {
    pub fn new(
        config: AgentConfig,
        alpaca: Arc<AlpacaClient>,
        risk_manager: Arc<RiskManager>,
        db_pool: SqlitePool,
    ) -> Self {
        let portfolio_guard = PortfolioGuard::new(&config);
        Self {
            config,
            alpaca,
            risk_manager,
            portfolio_guard,
            db_pool,
        }
    }

    #[allow(dead_code)]
    pub async fn execute_signal(&self, signal: &TradingSignal) -> Result<TradeExecution> {
        if !self.config.trading_enabled {
            return Err(anyhow::anyhow!("Trading is disabled"));
        }

        // 1. Fetch real account data
        let account = self.alpaca.get_account().await?;
        let portfolio_value: f64 = account.portfolio_value.parse().unwrap_or(0.0);
        let cash: f64 = account.cash.parse().unwrap_or(0.0);

        // 2. Fetch positions for risk checks
        let positions = self.alpaca.get_positions().await?;
        let positions_value: f64 = positions
            .iter()
            .filter_map(|p| p.market_value.parse::<f64>().ok())
            .sum();
        let positions_count = positions.len() as i32;

        // 2b. SELL signals require holding the position (no naked shorts)
        if signal.action == "SELL" {
            let holds_position = positions.iter().any(|p| p.symbol == signal.symbol);
            if !holds_position {
                return Err(anyhow::anyhow!(
                    "Cannot sell {} — not in portfolio (no short selling)",
                    signal.symbol
                ));
            }
        }

        // 3. Calculate daily P/L from positions
        let daily_pl: f64 = positions
            .iter()
            .filter_map(|p| p.unrealized_pl.parse::<f64>().ok())
            .sum();

        // 4. Circuit breaker check
        let cb_check = self
            .risk_manager
            .check_circuit_breakers(portfolio_value, daily_pl)
            .await?;
        if !cb_check.can_trade {
            return Err(anyhow::anyhow!(
                "Circuit breaker triggered: {}",
                cb_check.reason
            ));
        }

        // 5. Trade risk check
        let risk_check = self
            .risk_manager
            .check_trade_risk(signal.confidence, cash, positions_value, positions_count)
            .await?;
        if !risk_check.can_trade {
            return Err(anyhow::anyhow!("Risk check failed: {}", risk_check.reason));
        }

        // 5b. Portfolio guard check (P4)
        let trade_amount = self.config.max_position_size.min(cash * 0.2); // rough estimate
        self.portfolio_guard
            .check_new_trade(
                &signal.symbol,
                &signal.action,
                trade_amount,
                &positions,
                portfolio_value,
                daily_pl,
            )?;

        // 6. Position sizing
        let sizing = self
            .risk_manager
            .calculate_position_size(
                Decimal::from_f64(signal.entry_price).unwrap_or_default(),
                cash,
                positions_value,
            )
            .await?;

        let shares = sizing.recommended_shares.floor().to_i32().unwrap_or(0);
        if shares < 1 {
            return Err(anyhow::anyhow!("Position size too small (0 shares)"));
        }

        // Cap at max_position_size from config
        let max_shares = (self.config.max_position_size / signal.entry_price).floor() as i32;
        let shares = shares.min(max_shares).max(1);

        tracing::info!(
            "Executing {} {} - {} shares @ ~${:.2} (sizing: ${:.2})",
            signal.action,
            signal.symbol,
            shares,
            signal.entry_price,
            sizing.position_value
        );

        // 7. Submit order via Alpaca
        let order_request = if signal.action == "BUY" {
            MarketOrderRequest::buy(&signal.symbol, Decimal::from(shares))
        } else {
            MarketOrderRequest::sell(&signal.symbol, Decimal::from(shares))
        };

        let order = self.alpaca.submit_market_order(order_request).await?;
        let order_id = order.id.clone();

        // 8. Wait for fill with retry (configurable timeout, cancel on timeout)
        let (fill_price, filled_qty) = self
            .wait_for_fill(&order_id, signal.entry_price, shares)
            .await?;

        // 9. Register stop-loss with risk manager (for BUY orders)
        if signal.action == "BUY" {
            let shares_dec = Decimal::from_f64(filled_qty as f64).unwrap_or(Decimal::ZERO);
            let fill_price_dec = Decimal::from_f64(fill_price).unwrap_or(Decimal::ZERO);
            let stop_loss_dec = Decimal::from_f64(signal.stop_loss).unwrap_or(Decimal::ZERO);
            let take_profit_dec = Decimal::from_f64(signal.take_profit).unwrap_or(Decimal::ZERO);
            let risk_amount_dec = (fill_price_dec - stop_loss_dec).abs() * shares_dec;

            let risk_position = risk_manager::ActiveRiskPosition {
                id: None,
                symbol: signal.symbol.clone(),
                shares: shares_dec,
                entry_price: fill_price_dec,
                entry_date: Utc::now().format("%Y-%m-%d").to_string(),
                stop_loss_price: Some(stop_loss_dec),
                take_profit_price: Some(take_profit_dec),
                trailing_stop_enabled: false,
                trailing_stop_percent: None,
                max_price_seen: Some(fill_price_dec),
                risk_amount: Some(risk_amount_dec),
                position_size_percent: Some(sizing.position_size_percent),
                status: "active".to_string(),
                created_at: None,
                closed_at: None,
            };
            if let Err(e) = self.risk_manager.add_active_position(&risk_position).await {
                tracing::warn!("Failed to register stop-loss position: {}", e);
            }
        }

        // 10. Log to pending_trades for dashboard visibility
        self.log_agent_trade(signal, &order_id, fill_price, filled_qty)
            .await;

        Ok(TradeExecution {
            symbol: signal.symbol.clone(),
            action: signal.action.clone(),
            quantity: filled_qty,
            price: fill_price,
            order_id,
        })
    }

    /// Run risk checks + position sizing and return a proposal for manual review.
    /// Does NOT submit an order — the trade stays pending until the user approves.
    pub async fn propose_signal(&self, signal: &TradingSignal, ml_reasoning: &str) -> Result<TradeProposal> {
        // 1. Fetch real account data
        let account = self.alpaca.get_account().await?;
        let portfolio_value: f64 = account.portfolio_value.parse().unwrap_or(0.0);
        let cash: f64 = account.cash.parse().unwrap_or(0.0);

        // 2. Fetch positions for risk checks
        let positions = self.alpaca.get_positions().await?;
        let positions_value: f64 = positions
            .iter()
            .filter_map(|p| p.market_value.parse::<f64>().ok())
            .sum();
        let positions_count = positions.len() as i32;

        // 2b. SELL signals require holding the position (no naked shorts)
        if signal.action == "SELL" {
            let holds_position = positions.iter().any(|p| p.symbol == signal.symbol);
            if !holds_position {
                return Err(anyhow::anyhow!(
                    "Cannot sell {} — not in portfolio (no short selling)",
                    signal.symbol
                ));
            }
        }

        // 3. Calculate daily P/L from positions
        let daily_pl: f64 = positions
            .iter()
            .filter_map(|p| p.unrealized_pl.parse::<f64>().ok())
            .sum();

        // 4. Circuit breaker check
        let cb_check = self
            .risk_manager
            .check_circuit_breakers(portfolio_value, daily_pl)
            .await?;
        if !cb_check.can_trade {
            return Err(anyhow::anyhow!(
                "Circuit breaker triggered: {}",
                cb_check.reason
            ));
        }

        // 5. Trade risk check
        let risk_check = self
            .risk_manager
            .check_trade_risk(signal.confidence, cash, positions_value, positions_count)
            .await?;
        if !risk_check.can_trade {
            return Err(anyhow::anyhow!("Risk check failed: {}", risk_check.reason));
        }

        // 5b. Portfolio guard check
        let trade_amount = self.config.max_position_size.min(cash * 0.2);
        self.portfolio_guard.check_new_trade(
            &signal.symbol,
            &signal.action,
            trade_amount,
            &positions,
            portfolio_value,
            daily_pl,
        )?;

        // 6. Position sizing
        let sizing = self
            .risk_manager
            .calculate_position_size(
                Decimal::from_f64(signal.entry_price).unwrap_or_default(),
                cash,
                positions_value,
            )
            .await?;

        let shares = sizing.recommended_shares.floor().to_i32().unwrap_or(0);
        if shares < 1 {
            return Err(anyhow::anyhow!("Position size too small (0 shares)"));
        }

        let max_shares = (self.config.max_position_size / signal.entry_price).floor() as i32;
        let shares = shares.min(max_shares).max(1);

        // Build reason string
        let mut reason = signal.technical_reason.clone();
        if !ml_reasoning.is_empty() {
            reason = format!("{} | ML: {}", reason, ml_reasoning);
        }
        if !signal.signal_adjustments.is_empty() {
            reason = format!("{} | Adj: {}", reason, signal.signal_adjustments.join(", "));
        }

        Ok(TradeProposal {
            symbol: signal.symbol.clone(),
            action: signal.action.to_lowercase(),
            shares,
            entry_price: signal.entry_price,
            confidence: signal.confidence,
            reason,
            strategy_name: signal.strategy_name.clone(),
        })
    }

    /// Insert a trade proposal into `pending_trades` for human review.
    pub async fn save_pending_trade(&self, proposal: &TradeProposal) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "INSERT INTO pending_trades (symbol, action, shares, confidence, reason, signal_type, status, price)
             VALUES (?, ?, ?, ?, ?, ?, 'pending', ?)
             RETURNING id",
        )
        .bind(&proposal.symbol)
        .bind(&proposal.action)
        .bind(proposal.shares as f64)
        .bind(proposal.confidence)
        .bind(&proposal.reason)
        .bind(&proposal.strategy_name)
        .bind(proposal.entry_price)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to save pending trade: {}", e))?;

        Ok(row.0)
    }

    /// Poll order status with exponential backoff until filled, canceled, or timeout.
    /// On timeout, cancels the order and handles partial fills.
    async fn wait_for_fill(
        &self,
        order_id: &str,
        fallback_price: f64,
        requested_shares: i32,
    ) -> Result<(f64, i32)> {
        let timeout_secs = self.config.order_timeout_seconds;

        // Build delay schedule that roughly sums to timeout_secs
        let delays: Vec<u64> = if timeout_secs <= 10 {
            vec![2, 3, 5]
        } else if timeout_secs <= 30 {
            vec![1, 2, 4, 8, 15]
        } else {
            vec![1, 2, 4, 8, 15, 15, 15]
        };

        for (i, delay_secs) in delays.iter().enumerate() {
            tokio::time::sleep(std::time::Duration::from_secs(*delay_secs)).await;

            let order = self.alpaca.get_order(order_id).await?;

            match order.status.as_str() {
                "filled" => {
                    let price = order
                        .filled_avg_price
                        .as_ref()
                        .and_then(|p| p.parse::<f64>().ok())
                        .unwrap_or(fallback_price);
                    let qty = order
                        .filled_quantity
                        .as_ref()
                        .and_then(|q| q.parse::<f64>().ok())
                        .map(|q| q as i32)
                        .unwrap_or(requested_shares);
                    tracing::info!(
                        "Order {} filled: {} shares @ ${:.2}",
                        order_id,
                        qty,
                        price
                    );
                    return Ok((price, qty));
                }
                "partially_filled" => {
                    tracing::info!(
                        "Order {} partially filled (attempt {}/{}), waiting...",
                        order_id,
                        i + 1,
                        delays.len()
                    );
                    // Continue retrying — may fill fully
                }
                "canceled" | "expired" | "rejected" => {
                    return Err(anyhow::anyhow!(
                        "Order {} was {}: not filled",
                        order_id,
                        order.status
                    ));
                }
                _ => {
                    // "new", "accepted", "pending_new" — still in flight
                    tracing::debug!(
                        "Order {} status: {} (attempt {}/{})",
                        order_id,
                        order.status,
                        i + 1,
                        delays.len()
                    );
                }
            }
        }

        // Final check after all retries
        let order = self.alpaca.get_order(order_id).await?;
        match order.status.as_str() {
            "filled" => {
                let price = order
                    .filled_avg_price
                    .as_ref()
                    .and_then(|p| p.parse::<f64>().ok())
                    .unwrap_or(fallback_price);
                let qty = order
                    .filled_quantity
                    .as_ref()
                    .and_then(|q| q.parse::<f64>().ok())
                    .map(|q| q as i32)
                    .unwrap_or(requested_shares);
                return Ok((price, qty));
            }
            "partially_filled" => {
                // Accept partial fill and cancel remainder
                let price = order
                    .filled_avg_price
                    .as_ref()
                    .and_then(|p| p.parse::<f64>().ok())
                    .unwrap_or(fallback_price);
                let qty = order
                    .filled_quantity
                    .as_ref()
                    .and_then(|q| q.parse::<f64>().ok())
                    .map(|q| q as i32)
                    .unwrap_or(0);

                if qty > 0 {
                    tracing::warn!(
                        "Order {} partially filled ({}/{} shares @ ${:.2}), canceling remainder",
                        order_id,
                        qty,
                        requested_shares,
                        price
                    );
                    if let Err(e) = self.alpaca.cancel_order(order_id).await {
                        tracing::warn!("Failed to cancel remainder of order {}: {}", order_id, e);
                    }
                    return Ok((price, qty));
                }
            }
            _ => {}
        }

        // Timeout: cancel the order to prevent unexpected fills
        tracing::warn!(
            "Order {} not filled after {}s (status: {}), canceling",
            order_id,
            timeout_secs,
            order.status
        );
        if let Err(e) = self.alpaca.cancel_order(order_id).await {
            tracing::error!(
                "CRITICAL: Failed to cancel timed-out order {}: {} — may fill unexpectedly!",
                order_id,
                e
            );
        }

        Err(anyhow::anyhow!(
            "Order {} not filled after {}s, canceled (was: {})",
            order_id,
            timeout_secs,
            order.status
        ))
    }

    /// Cancel any stale open orders on startup recovery (P3/P8)
    pub async fn cancel_stale_orders(&self) -> Result<usize> {
        let orders = self.alpaca.get_orders(Some(50)).await?;
        let mut canceled = 0;
        for order in &orders {
            if matches!(
                order.status.as_str(),
                "new" | "accepted" | "pending_new" | "partially_filled"
            ) {
                tracing::warn!(
                    "Canceling stale order {} ({} {} {})",
                    order.id,
                    order.side,
                    order.symbol,
                    order.status
                );
                if let Err(e) = self.alpaca.cancel_order(&order.id).await {
                    tracing::error!("Failed to cancel stale order {}: {}", order.id, e);
                } else {
                    canceled += 1;
                }
            }
        }
        if canceled > 0 {
            tracing::info!("Canceled {} stale orders on startup", canceled);
        }
        Ok(canceled)
    }

    pub async fn execute_position_action(&self, action: &PositionAction) -> Result<()> {
        tracing::info!(
            "Executing position action: {} for {}",
            action.action_type,
            action.symbol
        );

        // Close position via Alpaca
        let order = self.alpaca.close_position(&action.symbol).await?;
        tracing::info!(
            "Close order submitted for {}: order_id={}",
            action.symbol,
            order.id
        );

        // Close in risk manager
        if let Err(e) = self
            .risk_manager
            .close_position(&action.symbol, &action.action_type)
            .await
        {
            tracing::warn!("Failed to close risk position for {}: {}", action.symbol, e);
        }

        // Record trade outcome for circuit breaker tracking
        if let Err(e) = self
            .risk_manager
            .record_trade_outcome(
                &action.symbol,
                Some(&order.id),
                "sell",
                action.pnl,
            )
            .await
        {
            tracing::warn!("Failed to record trade outcome for {}: {}", action.symbol, e);
        }

        // Log the closure to pending_trades
        self.log_agent_close(action, &order.id).await;

        Ok(())
    }

    /// Log an executed trade using the existing pending_trades schema columns.
    async fn log_agent_trade(
        &self,
        signal: &TradingSignal,
        order_id: &str,
        fill_price: f64,
        quantity: i32,
    ) {
        let result = sqlx::query(
            "INSERT INTO pending_trades (symbol, action, shares, confidence, reason, signal_type, status, price, order_id)
             VALUES (?, ?, ?, ?, ?, ?, 'executed', ?, ?)",
        )
        .bind(&signal.symbol)
        .bind(&signal.action)
        .bind(quantity as f64)
        .bind(signal.confidence)
        .bind(&signal.technical_reason)
        .bind(&signal.strategy_name)
        .bind(fill_price)
        .bind(order_id)
        .execute(&self.db_pool)
        .await;

        if let Err(e) = result {
            tracing::warn!("Failed to log agent trade to pending_trades: {}", e);
        }
    }

    /// Log a position closure (stop-loss / take-profit).
    async fn log_agent_close(&self, action: &PositionAction, order_id: &str) {
        let result = sqlx::query(
            "INSERT INTO pending_trades (symbol, action, shares, confidence, reason, signal_type, status, price, order_id)
             VALUES (?, 'SELL', 0, 1.0, ?, ?, 'executed', ?, ?)",
        )
        .bind(&action.symbol)
        .bind(&format!("{}: P/L ${:.2}", action.action_type, action.pnl))
        .bind(&action.action_type)
        .bind(action.price)
        .bind(order_id)
        .execute(&self.db_pool)
        .await;

        if let Err(e) = result {
            tracing::warn!("Failed to log agent close to pending_trades: {}", e);
        }
    }
}
