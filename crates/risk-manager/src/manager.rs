use anyhow::Result;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::models::*;

pub struct RiskManager {
    pool: sqlx::AnyPool,
}

impl RiskManager {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the underlying database pool
    pub fn pool(&self) -> &sqlx::AnyPool {
        &self.pool
    }

    /// Get current risk parameters
    pub async fn get_parameters(&self) -> Result<RiskParameters> {
        let params: Option<RiskParameters> = sqlx::query_as(
            r#"
            SELECT
                id, max_risk_per_trade_percent, max_portfolio_risk_percent,
                max_position_size_percent, default_stop_loss_percent,
                default_take_profit_percent, trailing_stop_enabled,
                trailing_stop_percent, min_confidence_threshold,
                min_win_rate_threshold,
                daily_loss_limit_percent, max_consecutive_losses,
                account_drawdown_limit_percent, trading_halted,
                halt_reason, halted_at, updated_at
            FROM risk_parameters
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(params.unwrap_or_default())
    }

    /// Update risk parameters
    pub async fn update_parameters(&self, params: &RiskParameters) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO risk_parameters (
                max_risk_per_trade_percent, max_portfolio_risk_percent,
                max_position_size_percent, default_stop_loss_percent,
                default_take_profit_percent, trailing_stop_enabled,
                trailing_stop_percent, min_confidence_threshold,
                min_win_rate_threshold
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(params.max_risk_per_trade_percent)
        .bind(params.max_portfolio_risk_percent)
        .bind(params.max_position_size_percent)
        .bind(params.default_stop_loss_percent)
        .bind(params.default_take_profit_percent)
        .bind(params.trailing_stop_enabled)
        .bind(params.trailing_stop_percent)
        .bind(params.min_confidence_threshold)
        .bind(params.min_win_rate_threshold)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Calculate position size based on risk parameters
    pub async fn calculate_position_size(
        &self,
        entry_price: Decimal,
        account_balance: f64,
        current_positions_value: f64,
    ) -> Result<PositionSizeCalculation> {
        let params = self.get_parameters().await?;

        // Calculate risk amount per trade
        let risk_amount_f64 = account_balance * (params.max_risk_per_trade_percent / 100.0);
        let risk_amount = Decimal::from_f64(risk_amount_f64).unwrap_or_default();

        // Calculate stop loss and take profit prices
        let stop_loss_multiplier =
            Decimal::from_f64(1.0 - params.default_stop_loss_percent / 100.0)
                .unwrap_or(Decimal::ONE);
        let take_profit_multiplier =
            Decimal::from_f64(1.0 + params.default_take_profit_percent / 100.0)
                .unwrap_or(Decimal::ONE);

        let stop_loss_price = entry_price * stop_loss_multiplier;
        let take_profit_price = entry_price * take_profit_multiplier;

        // Calculate position size based on stop loss distance
        let risk_per_share = entry_price - stop_loss_price;
        let shares = if risk_per_share > Decimal::ZERO {
            (risk_amount / risk_per_share).floor()
        } else {
            Decimal::ZERO
        };

        let position_value = shares * entry_price;

        // Check if position exceeds max position size
        let total_portfolio_value = account_balance + current_positions_value;
        let position_value_f64 = position_value.to_f64().unwrap_or(0.0);
        let position_size_percent = (position_value_f64 / total_portfolio_value) * 100.0;

        let adjusted_shares = if position_size_percent > params.max_position_size_percent {
            let max_position_value =
                total_portfolio_value * (params.max_position_size_percent / 100.0);
            let max_position_value_dec = Decimal::from_f64(max_position_value).unwrap_or_default();
            (max_position_value_dec / entry_price).floor()
        } else {
            shares
        };

        let final_position_value = adjusted_shares * entry_price;
        let final_position_value_f64 = final_position_value.to_f64().unwrap_or(0.0);
        let final_position_size_percent =
            (final_position_value_f64 / total_portfolio_value) * 100.0;

        Ok(PositionSizeCalculation {
            recommended_shares: adjusted_shares,
            position_value: final_position_value,
            risk_amount,
            stop_loss_price,
            take_profit_price,
            position_size_percent: final_position_size_percent,
        })
    }

    /// Check if a new trade meets risk criteria
    pub async fn check_trade_risk(
        &self,
        confidence: f64,
        account_balance: f64,
        current_positions_value: f64,
        active_positions_count: i32,
    ) -> Result<RiskCheck> {
        let params = self.get_parameters().await?;

        // Check confidence threshold
        if confidence < params.min_confidence_threshold {
            return Ok(RiskCheck {
                can_trade: false,
                reason: format!(
                    "Confidence {:.1}% below minimum threshold {:.1}%",
                    confidence * 100.0,
                    params.min_confidence_threshold * 100.0
                ),
                current_portfolio_risk: 0.0,
                position_count: active_positions_count,
                suggested_action: Some("Wait for higher confidence signal".to_string()),
            });
        }

        // Calculate actual portfolio exposure (positions value as % of total portfolio)
        let total_value = account_balance + current_positions_value;
        let current_portfolio_risk_percent = if total_value > 0.0 {
            (current_positions_value / total_value) * 100.0
        } else {
            0.0
        };

        // Check if current exposure exceeds portfolio risk limit
        if current_portfolio_risk_percent >= params.max_portfolio_risk_percent {
            return Ok(RiskCheck {
                can_trade: false,
                reason: format!(
                    "Portfolio risk {:.1}% at or above maximum {:.1}%",
                    current_portfolio_risk_percent, params.max_portfolio_risk_percent
                ),
                current_portfolio_risk: current_portfolio_risk_percent,
                position_count: active_positions_count,
                suggested_action: Some(
                    "Close existing positions before opening new ones".to_string(),
                ),
            });
        }

        // Position count is enforced by the caller's PortfolioGuard (configurable via MAX_OPEN_POSITIONS).
        // No hardcoded limit here.

        // All checks passed
        Ok(RiskCheck {
            can_trade: true,
            reason: "Trade meets all risk criteria".to_string(),
            current_portfolio_risk: current_portfolio_risk_percent,
            position_count: active_positions_count,
            suggested_action: None,
        })
    }

    /// Add active risk position
    pub async fn add_active_position(&self, position: &ActiveRiskPosition) -> Result<i64> {
        let trailing_stop_enabled = if position.trailing_stop_enabled { 1 } else { 0 };

        let (id,): (i64,) = sqlx::query_as(
            r#"
            INSERT INTO active_risk_positions (
                symbol, shares, entry_price, entry_date,
                stop_loss_price, take_profit_price, trailing_stop_enabled,
                trailing_stop_percent, max_price_seen, risk_amount,
                position_size_percent, status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(&position.symbol)
        .bind(position.shares.to_f64().unwrap_or(0.0))
        .bind(position.entry_price.to_f64().unwrap_or(0.0))
        .bind(&position.entry_date)
        .bind(position.stop_loss_price.map(|d| d.to_f64().unwrap_or(0.0)))
        .bind(
            position
                .take_profit_price
                .map(|d| d.to_f64().unwrap_or(0.0)),
        )
        .bind(trailing_stop_enabled)
        .bind(position.trailing_stop_percent)
        .bind(position.max_price_seen.map(|d| d.to_f64().unwrap_or(0.0)))
        .bind(position.risk_amount.map(|d| d.to_f64().unwrap_or(0.0)))
        .bind(position.position_size_percent)
        .bind(&position.status)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Get active risk positions
    pub async fn get_active_positions(&self) -> Result<Vec<ActiveRiskPosition>> {
        use crate::models::ActiveRiskPositionRow;

        let rows: Vec<ActiveRiskPositionRow> = sqlx::query_as(
            r#"
            SELECT
                id, symbol, shares, entry_price, entry_date,
                stop_loss_price, take_profit_price,
                trailing_stop_enabled,
                trailing_stop_percent, max_price_seen, risk_amount,
                position_size_percent, status, created_at, closed_at
            FROM active_risk_positions
            WHERE status = 'active'
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Update trailing stop
    pub async fn update_trailing_stop(&self, symbol: &str, current_price: Decimal) -> Result<()> {
        use crate::models::ActiveRiskPositionRow;

        let row: Option<ActiveRiskPositionRow> = sqlx::query_as(
            r#"
            SELECT
                id, symbol, shares, entry_price, entry_date,
                stop_loss_price, take_profit_price,
                trailing_stop_enabled,
                trailing_stop_percent, max_price_seen, risk_amount,
                position_size_percent, status, created_at, closed_at
            FROM active_risk_positions
            WHERE symbol = ? AND status = 'active'
            "#,
        )
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let pos: ActiveRiskPosition = row.into();

            if pos.trailing_stop_enabled {
                let max_price = pos
                    .max_price_seen
                    .unwrap_or(pos.entry_price)
                    .max(current_price);

                if let Some(trailing_pct) = pos.trailing_stop_percent {
                    let trailing_multiplier =
                        Decimal::from_f64(1.0 - trailing_pct / 100.0).unwrap_or(Decimal::ONE);
                    let new_stop = max_price * trailing_multiplier;

                    // Only update if new stop is higher than current stop
                    let current_stop = pos.stop_loss_price.unwrap_or(Decimal::ZERO);
                    if new_stop > current_stop {
                        sqlx::query(
                            "UPDATE active_risk_positions SET stop_loss_price = ?, max_price_seen = ? WHERE id = ?"
                        )
                        .bind(new_stop.to_f64().unwrap_or(0.0))
                        .bind(max_price.to_f64().unwrap_or(0.0))
                        .bind(pos.id)
                        .execute(&self.pool)
                        .await?;

                        tracing::info!(
                            "Updated trailing stop for {}: ${:.2} -> ${:.2}",
                            symbol,
                            current_stop,
                            new_stop
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Check stop losses for all active positions
    pub async fn check_stop_losses(
        &self,
        current_prices: Vec<(String, Decimal)>,
    ) -> Result<Vec<StopLossAlert>> {
        let mut alerts = Vec::new();

        for (symbol, current_price) in current_prices {
            if let Some(alert) = self
                .check_position_stop_loss(&symbol, current_price)
                .await?
            {
                alerts.push(alert);
            }
        }

        Ok(alerts)
    }

    async fn check_position_stop_loss(
        &self,
        symbol: &str,
        current_price: Decimal,
    ) -> Result<Option<StopLossAlert>> {
        use crate::models::ActiveRiskPositionRow;

        let row: Option<ActiveRiskPositionRow> = sqlx::query_as(
            r#"
            SELECT
                id, symbol, shares, entry_price, entry_date,
                stop_loss_price, take_profit_price,
                trailing_stop_enabled,
                trailing_stop_percent, max_price_seen, risk_amount,
                position_size_percent, status, created_at, closed_at
            FROM active_risk_positions
            WHERE symbol = ? AND status = 'active'
            "#,
        )
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let pos: ActiveRiskPosition = row.into();

            if let Some(stop_loss_price) = pos.stop_loss_price {
                if current_price <= stop_loss_price {
                    let loss_amount = (pos.entry_price - current_price) * pos.shares;
                    let loss_percent_dec =
                        ((current_price - pos.entry_price) / pos.entry_price) * Decimal::from(100);
                    let loss_percent = loss_percent_dec.to_f64().unwrap_or(0.0);

                    return Ok(Some(StopLossAlert {
                        symbol: symbol.to_string(),
                        current_price,
                        stop_loss_price,
                        should_exit: true,
                        loss_amount,
                        loss_percent,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Initialize circuit breaker tables (called on startup)
    pub async fn init_circuit_breaker_tables(&self) -> Result<()> {
        // Tables and columns are now created by sqlx migrations.
        Ok(())
    }

    /// Check all circuit breakers before allowing a trade
    pub async fn check_circuit_breakers(
        &self,
        portfolio_value: f64,
        daily_pl: f64,
    ) -> Result<CircuitBreakerCheck> {
        let params = self.get_parameters().await?;
        let mut breakers_triggered = Vec::new();

        // 1. Manual halt check
        if params.trading_halted {
            let reason = params
                .halt_reason
                .unwrap_or_else(|| "Manual trading halt active".to_string());
            return Ok(CircuitBreakerCheck {
                can_trade: false,
                reason,
                daily_pl_percent: 0.0,
                consecutive_losses: 0,
                drawdown_percent: 0.0,
                breakers_triggered: vec!["manual_halt".to_string()],
            });
        }

        // 2. Daily loss limit
        let daily_pl_percent = if portfolio_value > 0.0 {
            (daily_pl / portfolio_value) * 100.0
        } else {
            0.0
        };

        if daily_pl_percent < -params.daily_loss_limit_percent {
            breakers_triggered.push(format!(
                "daily_loss: {:.1}% exceeds limit of {:.1}%",
                daily_pl_percent.abs(),
                params.daily_loss_limit_percent
            ));
        }

        // 3. Consecutive losses
        let consecutive_losses = self.get_consecutive_losses().await.unwrap_or(0);
        if consecutive_losses >= params.max_consecutive_losses {
            breakers_triggered.push(format!(
                "consecutive_losses: {} >= limit of {}",
                consecutive_losses, params.max_consecutive_losses
            ));
        }

        // 4. Drawdown from peak
        let drawdown_percent = self
            .check_drawdown_from_peak(portfolio_value)
            .await
            .unwrap_or(0.0);
        if drawdown_percent > params.account_drawdown_limit_percent {
            breakers_triggered.push(format!(
                "drawdown: {:.1}% exceeds limit of {:.1}%",
                drawdown_percent, params.account_drawdown_limit_percent
            ));
        }

        let can_trade = breakers_triggered.is_empty();
        let reason = if can_trade {
            "All circuit breakers clear".to_string()
        } else {
            format!(
                "Circuit breakers triggered: {}",
                breakers_triggered.join("; ")
            )
        };

        Ok(CircuitBreakerCheck {
            can_trade,
            reason,
            daily_pl_percent,
            consecutive_losses,
            drawdown_percent,
            breakers_triggered,
        })
    }

    /// Count recent consecutive losing trades from trade_outcomes table
    pub async fn get_consecutive_losses(&self) -> Result<i32> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT outcome FROM trade_outcomes ORDER BY id DESC LIMIT 20")
                .fetch_all(&self.pool)
                .await
                .unwrap_or_default();

        let mut consecutive = 0i32;
        for (outcome,) in rows {
            if outcome == "loss" {
                consecutive += 1;
            } else {
                break;
            }
        }

        Ok(consecutive)
    }

    /// Record the outcome of a completed trade for circuit breaker tracking
    pub async fn record_trade_outcome(
        &self,
        symbol: &str,
        order_id: Option<&str>,
        action: &str,
        pnl: f64,
    ) -> Result<()> {
        let outcome = if pnl > 0.01 {
            "win"
        } else if pnl < -0.01 {
            "loss"
        } else {
            "breakeven"
        };

        sqlx::query(
            "INSERT INTO trade_outcomes (symbol, order_id, action, outcome, pnl)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(symbol)
        .bind(order_id)
        .bind(action)
        .bind(outcome)
        .bind(pnl)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check drawdown from portfolio peak, updating peak if new high
    pub async fn check_drawdown_from_peak(&self, current_value: f64) -> Result<f64> {
        let peak: Option<(f64,)> =
            sqlx::query_as("SELECT peak_value FROM portfolio_peak ORDER BY id DESC LIMIT 1")
                .fetch_optional(&self.pool)
                .await?;

        match peak {
            None => {
                // No peak recorded yet — record current value as initial peak
                sqlx::query("INSERT INTO portfolio_peak (peak_value) VALUES (?)")
                    .bind(current_value)
                    .execute(&self.pool)
                    .await?;
                Ok(0.0)
            }
            Some((peak_value,)) if current_value > peak_value => {
                // New high — update peak
                sqlx::query("INSERT INTO portfolio_peak (peak_value) VALUES (?)")
                    .bind(current_value)
                    .execute(&self.pool)
                    .await?;
                Ok(0.0)
            }
            Some((peak_value,)) if peak_value > 0.0 => {
                let drawdown = ((peak_value - current_value) / peak_value) * 100.0;
                Ok(drawdown)
            }
            _ => Ok(0.0),
        }
    }

    /// Manually halt or resume trading
    pub async fn set_trading_halt(&self, halted: bool, reason: Option<&str>) -> Result<()> {
        let params = self.get_parameters().await?;

        // We store the halt state. For simplicity, update the most recent row
        // or insert a new parameters row.
        if halted {
            sqlx::query(
                r#"
                INSERT INTO risk_parameters (
                    max_risk_per_trade_percent, max_portfolio_risk_percent,
                    max_position_size_percent, default_stop_loss_percent,
                    default_take_profit_percent, trailing_stop_enabled,
                    trailing_stop_percent, min_confidence_threshold,
                    min_win_rate_threshold, daily_loss_limit_percent,
                    max_consecutive_losses, account_drawdown_limit_percent,
                    trading_halted, halt_reason, halted_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)
                "#,
            )
            .bind(params.max_risk_per_trade_percent)
            .bind(params.max_portfolio_risk_percent)
            .bind(params.max_position_size_percent)
            .bind(params.default_stop_loss_percent)
            .bind(params.default_take_profit_percent)
            .bind(params.trailing_stop_enabled)
            .bind(params.trailing_stop_percent)
            .bind(params.min_confidence_threshold)
            .bind(params.min_win_rate_threshold)
            .bind(params.daily_loss_limit_percent)
            .bind(params.max_consecutive_losses)
            .bind(params.account_drawdown_limit_percent)
            .bind(reason.unwrap_or("Manual halt"))
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO risk_parameters (
                    max_risk_per_trade_percent, max_portfolio_risk_percent,
                    max_position_size_percent, default_stop_loss_percent,
                    default_take_profit_percent, trailing_stop_enabled,
                    trailing_stop_percent, min_confidence_threshold,
                    min_win_rate_threshold, daily_loss_limit_percent,
                    max_consecutive_losses, account_drawdown_limit_percent,
                    trading_halted, halt_reason, halted_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, NULL)
                "#,
            )
            .bind(params.max_risk_per_trade_percent)
            .bind(params.max_portfolio_risk_percent)
            .bind(params.max_position_size_percent)
            .bind(params.default_stop_loss_percent)
            .bind(params.default_take_profit_percent)
            .bind(params.trailing_stop_enabled)
            .bind(params.trailing_stop_percent)
            .bind(params.min_confidence_threshold)
            .bind(params.min_win_rate_threshold)
            .bind(params.daily_loss_limit_percent)
            .bind(params.max_consecutive_losses)
            .bind(params.account_drawdown_limit_percent)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Close active position
    pub async fn close_position(&self, symbol: &str, reason: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE active_risk_positions
            SET status = ?, closed_at = CURRENT_TIMESTAMP
            WHERE symbol = ? AND status = 'active'
            "#,
        )
        .bind(reason)
        .bind(symbol)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
