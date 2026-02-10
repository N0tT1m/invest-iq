use std::sync::Arc;

use alpaca_broker::{AlpacaClient, MarketOrderRequest};
use anyhow::Result;
use chrono::Utc;
use risk_manager::RiskManager;
use sqlx::SqlitePool;

use crate::config::AgentConfig;
use crate::types::{PositionAction, TradeExecution, TradingSignal};

pub struct TradeExecutor {
    config: AgentConfig,
    alpaca: Arc<AlpacaClient>,
    risk_manager: Arc<RiskManager>,
    db_pool: SqlitePool,
}

impl TradeExecutor {
    pub fn new(
        config: AgentConfig,
        alpaca: Arc<AlpacaClient>,
        risk_manager: Arc<RiskManager>,
        db_pool: SqlitePool,
    ) -> Self {
        Self {
            config,
            alpaca,
            risk_manager,
            db_pool,
        }
    }

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

        // 6. Position sizing
        let sizing = self
            .risk_manager
            .calculate_position_size(signal.entry_price, cash, positions_value)
            .await?;

        let shares = sizing.recommended_shares.floor() as i32;
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
            MarketOrderRequest::buy(&signal.symbol, shares as f64)
        } else {
            MarketOrderRequest::sell(&signal.symbol, shares as f64)
        };

        let order = self.alpaca.submit_market_order(order_request).await?;
        let order_id = order.id.clone();

        // 8. Wait for fill with retry (up to 30s)
        let (fill_price, filled_qty) =
            self.wait_for_fill(&order_id, signal.entry_price, shares).await?;

        // 9. Register stop-loss with risk manager (for BUY orders)
        if signal.action == "BUY" {
            let risk_position = risk_manager::ActiveRiskPosition {
                id: None,
                symbol: signal.symbol.clone(),
                shares: filled_qty as f64,
                entry_price: fill_price,
                entry_date: Utc::now().format("%Y-%m-%d").to_string(),
                stop_loss_price: Some(signal.stop_loss),
                take_profit_price: Some(signal.take_profit),
                trailing_stop_enabled: false,
                trailing_stop_percent: None,
                max_price_seen: Some(fill_price),
                risk_amount: Some((fill_price - signal.stop_loss).abs() * filled_qty as f64),
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

    /// Poll order status with exponential backoff until filled, canceled, or timeout.
    async fn wait_for_fill(
        &self,
        order_id: &str,
        fallback_price: f64,
        requested_shares: i32,
    ) -> Result<(f64, i32)> {
        // Retry: 1s, 2s, 4s, 8s, 15s = 30s total
        let delays = [1, 2, 4, 8, 15];

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
        if order.status == "filled" || order.status == "partially_filled" {
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
                    "Order {} {} after retries: {} shares @ ${:.2}",
                    order_id,
                    order.status,
                    qty,
                    price
                );
                return Ok((price, qty));
            }
        }

        Err(anyhow::anyhow!(
            "Order {} not filled after 30s (status: {}). May need manual review.",
            order_id,
            order.status
        ))
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
