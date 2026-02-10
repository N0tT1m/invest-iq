use anyhow::Result;
use sqlx::SqlitePool;

use crate::models::*;

pub struct RiskManager {
    pool: SqlitePool,
}

impl RiskManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get a reference to the underlying database pool
    pub fn pool(&self) -> &SqlitePool {
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
            "#
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
            "#
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
        entry_price: f64,
        account_balance: f64,
        current_positions_value: f64,
    ) -> Result<PositionSizeCalculation> {
        let params = self.get_parameters().await?;

        // Calculate risk amount per trade
        let risk_amount = account_balance * (params.max_risk_per_trade_percent / 100.0);

        // Calculate stop loss and take profit prices
        let stop_loss_price = entry_price * (1.0 - params.default_stop_loss_percent / 100.0);
        let take_profit_price = entry_price * (1.0 + params.default_take_profit_percent / 100.0);

        // Calculate position size based on stop loss distance
        let risk_per_share = entry_price - stop_loss_price;
        let shares = if risk_per_share > 0.0 {
            (risk_amount / risk_per_share).floor()
        } else {
            0.0
        };

        let position_value = shares * entry_price;

        // Check if position exceeds max position size
        let total_portfolio_value = account_balance + current_positions_value;
        let position_size_percent = (position_value / total_portfolio_value) * 100.0;

        let adjusted_shares = if position_size_percent > params.max_position_size_percent {
            let max_position_value = total_portfolio_value * (params.max_position_size_percent / 100.0);
            (max_position_value / entry_price).floor()
        } else {
            shares
        };

        let final_position_value = adjusted_shares * entry_price;
        let final_position_size_percent = (final_position_value / total_portfolio_value) * 100.0;

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

        // Calculate current portfolio risk
        let total_value = account_balance + current_positions_value;
        let max_portfolio_risk = total_value * (params.max_portfolio_risk_percent / 100.0);

        // Estimate current risk (simplified - assumes each position risks 2%)
        let estimated_current_risk = active_positions_count as f64 *
            (total_value * (params.max_risk_per_trade_percent / 100.0));

        let current_portfolio_risk_percent = (estimated_current_risk / total_value) * 100.0;

        // Check if adding another position exceeds portfolio risk
        if estimated_current_risk >= max_portfolio_risk {
            return Ok(RiskCheck {
                can_trade: false,
                reason: format!(
                    "Portfolio risk {:.1}% at or above maximum {:.1}%",
                    current_portfolio_risk_percent,
                    params.max_portfolio_risk_percent
                ),
                current_portfolio_risk: current_portfolio_risk_percent,
                position_count: active_positions_count,
                suggested_action: Some("Close existing positions before opening new ones".to_string()),
            });
        }

        // Check position concentration
        if active_positions_count >= 10 {
            return Ok(RiskCheck {
                can_trade: false,
                reason: "Too many open positions (max 10)".to_string(),
                current_portfolio_risk: current_portfolio_risk_percent,
                position_count: active_positions_count,
                suggested_action: Some("Reduce position count before adding new trades".to_string()),
            });
        }

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

        let result = sqlx::query(
            r#"
            INSERT INTO active_risk_positions (
                symbol, shares, entry_price, entry_date,
                stop_loss_price, take_profit_price, trailing_stop_enabled,
                trailing_stop_percent, max_price_seen, risk_amount,
                position_size_percent, status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&position.symbol)
        .bind(position.shares)
        .bind(position.entry_price)
        .bind(&position.entry_date)
        .bind(position.stop_loss_price)
        .bind(position.take_profit_price)
        .bind(trailing_stop_enabled)
        .bind(position.trailing_stop_percent)
        .bind(position.max_price_seen)
        .bind(position.risk_amount)
        .bind(position.position_size_percent)
        .bind(&position.status)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get active risk positions
    pub async fn get_active_positions(&self) -> Result<Vec<ActiveRiskPosition>> {
        let positions: Vec<ActiveRiskPosition> = sqlx::query_as(
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
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(positions)
    }

    /// Update trailing stop
    pub async fn update_trailing_stop(&self, symbol: &str, current_price: f64) -> Result<()> {
        let position: Option<ActiveRiskPosition> = sqlx::query_as(
            r#"
            SELECT
                id, symbol, shares, entry_price, entry_date,
                stop_loss_price, take_profit_price,
                trailing_stop_enabled,
                trailing_stop_percent, max_price_seen, risk_amount,
                position_size_percent, status, created_at, closed_at
            FROM active_risk_positions
            WHERE symbol = ? AND status = 'active'
            "#
        )
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(mut pos) = position {
            if pos.trailing_stop_enabled {
                let max_price = pos.max_price_seen.unwrap_or(pos.entry_price).max(current_price);

                if let Some(trailing_pct) = pos.trailing_stop_percent {
                    let new_stop = max_price * (1.0 - trailing_pct / 100.0);

                    // Only update if new stop is higher than current stop
                    let current_stop = pos.stop_loss_price.unwrap_or(0.0);
                    if new_stop > current_stop {
                        sqlx::query(
                            "UPDATE active_risk_positions SET stop_loss_price = ?, max_price_seen = ? WHERE id = ?"
                        )
                        .bind(new_stop)
                        .bind(max_price)
                        .bind(pos.id)
                        .execute(&self.pool)
                        .await?;

                        tracing::info!(
                            "Updated trailing stop for {}: ${:.2} -> ${:.2}",
                            symbol, current_stop, new_stop
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Check stop losses for all active positions
    pub async fn check_stop_losses(&self, current_prices: Vec<(String, f64)>) -> Result<Vec<StopLossAlert>> {
        let mut alerts = Vec::new();

        for (symbol, current_price) in current_prices {
            if let Some(alert) = self.check_position_stop_loss(&symbol, current_price).await? {
                alerts.push(alert);
            }
        }

        Ok(alerts)
    }

    async fn check_position_stop_loss(&self, symbol: &str, current_price: f64) -> Result<Option<StopLossAlert>> {
        let position: Option<ActiveRiskPosition> = sqlx::query_as(
            r#"
            SELECT
                id, symbol, shares, entry_price, entry_date,
                stop_loss_price, take_profit_price,
                trailing_stop_enabled,
                trailing_stop_percent, max_price_seen, risk_amount,
                position_size_percent, status, created_at, closed_at
            FROM active_risk_positions
            WHERE symbol = ? AND status = 'active'
            "#
        )
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(pos) = position {
            if let Some(stop_loss_price) = pos.stop_loss_price {
                if current_price <= stop_loss_price {
                    let loss_amount = (pos.entry_price - current_price) * pos.shares;
                    let loss_percent = ((current_price - pos.entry_price) / pos.entry_price) * 100.0;

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
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS portfolio_peak (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                peak_value REAL NOT NULL,
                peak_date TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Migrate: add circuit breaker columns to risk_parameters if missing
        for (col, col_type) in &[
            ("daily_loss_limit_percent", "REAL NOT NULL DEFAULT 5.0"),
            ("max_consecutive_losses", "INTEGER NOT NULL DEFAULT 3"),
            ("account_drawdown_limit_percent", "REAL NOT NULL DEFAULT 10.0"),
            ("trading_halted", "INTEGER NOT NULL DEFAULT 0"),
            ("halt_reason", "TEXT"),
            ("halted_at", "TEXT"),
        ] {
            let sql = format!(
                "ALTER TABLE risk_parameters ADD COLUMN {} {}",
                col, col_type
            );
            // Ignore "duplicate column" errors — column already exists
            let _ = sqlx::query(&sql).execute(&self.pool).await;
        }

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
            let reason = params.halt_reason.unwrap_or_else(|| "Manual trading halt active".to_string());
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
                daily_pl_percent.abs(), params.daily_loss_limit_percent
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
        let drawdown_percent = self.check_drawdown_from_peak(portfolio_value).await.unwrap_or(0.0);
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
            format!("Circuit breakers triggered: {}", breakers_triggered.join("; "))
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

    /// Count recent consecutive losing trades
    pub async fn get_consecutive_losses(&self) -> Result<i32> {
        // Look at recent trades in reverse chronological order, count consecutive losses
        let rows: Vec<(f64,)> = sqlx::query_as(
            r#"
            SELECT COALESCE(
                (shares * (
                    CASE WHEN action = 'sell' THEN price ELSE -price END
                )), 0.0) as pnl
            FROM trades
            ORDER BY id DESC
            LIMIT 20
            "#
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let mut consecutive = 0i32;
        for (pnl,) in rows {
            if pnl < 0.0 {
                consecutive += 1;
            } else {
                break;
            }
        }

        Ok(consecutive)
    }

    /// Check drawdown from portfolio peak, updating peak if new high
    pub async fn check_drawdown_from_peak(&self, current_value: f64) -> Result<f64> {
        let peak: Option<(f64,)> = sqlx::query_as(
            "SELECT peak_value FROM portfolio_peak ORDER BY id DESC LIMIT 1"
        )
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
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, datetime('now'))
                "#
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
                "#
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
            "#
        )
        .bind(reason)
        .bind(symbol)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
