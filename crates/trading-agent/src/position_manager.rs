use std::sync::Arc;

use alpaca_broker::AlpacaClient;
use anyhow::Result;
use risk_manager::RiskManager;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::types::PositionAction;

pub struct PositionManager {
    alpaca: Arc<AlpacaClient>,
    risk_manager: Arc<RiskManager>,
}

impl PositionManager {
    pub fn new(alpaca: Arc<AlpacaClient>, risk_manager: Arc<RiskManager>) -> Self {
        Self {
            alpaca,
            risk_manager,
        }
    }

    /// Check all open positions against stop-loss / take-profit levels.
    /// Returns a list of actions to execute (closures).
    pub async fn check_positions(&self) -> Result<Vec<PositionAction>> {
        let mut actions = Vec::new();

        // 1. Get real positions from Alpaca
        let positions = self.alpaca.get_positions().await?;
        if positions.is_empty() {
            return Ok(actions);
        }

        // 2. Build price map for risk manager (Decimal)
        let mut price_map: Vec<(String, Decimal)> = Vec::new();
        for pos in &positions {
            if let Ok(current_price) = pos.current_price.parse::<f64>() {
                let current_price_dec = Decimal::from_f64(current_price).unwrap_or_default();
                price_map.push((pos.symbol.clone(), current_price_dec));

                // Update trailing stops with current price
                if let Err(e) = self
                    .risk_manager
                    .update_trailing_stop(&pos.symbol, current_price_dec)
                    .await
                {
                    tracing::debug!("Trailing stop update for {}: {}", pos.symbol, e);
                }
            }
        }

        // 3. Check stop-losses via risk manager
        let alerts = self.risk_manager.check_stop_losses(price_map).await?;

        for alert in alerts {
            if alert.should_exit {
                // Find the Alpaca position to calculate P/L
                let pnl = positions
                    .iter()
                    .find(|p| p.symbol == alert.symbol)
                    .and_then(|p| p.unrealized_pl.parse::<f64>().ok())
                    .unwrap_or(0.0);

                actions.push(PositionAction {
                    action_type: "STOP_LOSS".to_string(),
                    symbol: alert.symbol.clone(),
                    price: alert.current_price.to_f64().unwrap_or(0.0),
                    pnl,
                });
            }
        }

        // 4. Check take-profit levels from risk manager active positions
        let risk_positions = self.risk_manager.get_active_positions().await?;
        for rp in &risk_positions {
            if let Some(tp_price) = rp.take_profit_price {
                let tp_f64 = tp_price.to_f64().unwrap_or(0.0);
                // Find the Alpaca position for this symbol
                if let Some(ap) = positions.iter().find(|p| p.symbol == rp.symbol) {
                    if let Ok(current_price) = ap.current_price.parse::<f64>() {
                        // For long positions: current >= take_profit
                        if current_price >= tp_f64 {
                            let pnl = ap.unrealized_pl.parse::<f64>().unwrap_or(0.0);

                            // Don't add duplicate if already added from stop-loss alerts
                            if !actions.iter().any(|a| a.symbol == rp.symbol) {
                                tracing::info!(
                                    "Take profit hit for {}: ${:.2} >= ${:.2}",
                                    rp.symbol,
                                    current_price,
                                    tp_f64
                                );
                                actions.push(PositionAction {
                                    action_type: "TAKE_PROFIT".to_string(),
                                    symbol: rp.symbol.clone(),
                                    price: current_price,
                                    pnl,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(actions)
    }
}
