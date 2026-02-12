use crate::db::PortfolioDb;
use crate::models::*;
use anyhow::Result;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

pub struct TradeLogger {
    db: PortfolioDb,
}

impl TradeLogger {
    pub fn new(db: PortfolioDb) -> Self {
        Self { db }
    }

    /// Log a new trade
    pub async fn log_trade(&self, trade: TradeInput) -> Result<i64> {
        let (id,): (i64,) = sqlx::query_as(
            r#"
            INSERT INTO trades (symbol, action, shares, price, trade_date, commission, notes)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(&trade.symbol)
        .bind(&trade.action)
        .bind(trade.shares.to_f64().unwrap_or(0.0))
        .bind(trade.price.to_f64().unwrap_or(0.0))
        .bind(&trade.trade_date)
        .bind(
            trade
                .commission
                .unwrap_or(Decimal::ZERO)
                .to_f64()
                .unwrap_or(0.0),
        )
        .bind(&trade.notes)
        .fetch_one(self.db.pool())
        .await?;

        Ok(id)
    }

    /// Get all trades for a symbol
    pub async fn get_trades_for_symbol(&self, symbol: &str) -> Result<Vec<Trade>> {
        let trades = sqlx::query_as::<_, Trade>(
            "SELECT * FROM trades WHERE symbol = ? ORDER BY trade_date DESC",
        )
        .bind(symbol)
        .fetch_all(self.db.pool())
        .await?;

        Ok(trades)
    }

    /// Get all trades
    pub async fn get_all_trades(&self, limit: Option<i64>) -> Result<Vec<Trade>> {
        let trades = if let Some(lim) = limit {
            sqlx::query_as::<_, Trade>(
                "SELECT * FROM trades ORDER BY trade_date DESC, created_at DESC LIMIT ?",
            )
            .bind(lim)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, Trade>(
                "SELECT * FROM trades ORDER BY trade_date DESC, created_at DESC",
            )
            .fetch_all(self.db.pool())
            .await?
        };

        Ok(trades)
    }

    /// Get trade by ID
    pub async fn get_trade(&self, id: i64) -> Result<Option<Trade>> {
        let trade = sqlx::query_as::<_, Trade>("SELECT * FROM trades WHERE id = ?")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;

        Ok(trade)
    }

    /// Update trade
    pub async fn update_trade(&self, id: i64, trade: TradeInput) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE trades
            SET symbol = ?, action = ?, shares = ?, price = ?, trade_date = ?, commission = ?, notes = ?
            WHERE id = ?
            "#
        )
        .bind(&trade.symbol)
        .bind(&trade.action)
        .bind(trade.shares.to_f64().unwrap_or(0.0))
        .bind(trade.price.to_f64().unwrap_or(0.0))
        .bind(&trade.trade_date)
        .bind(trade.commission.unwrap_or(Decimal::ZERO).to_f64().unwrap_or(0.0))
        .bind(&trade.notes)
        .bind(id)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// Delete trade
    pub async fn delete_trade(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM trades WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }

    /// Get trades within date range (ascending order for lot matching)
    async fn get_trades_ascending(&self, days: Option<i64>) -> Result<Vec<Trade>> {
        let trades = if let Some(d) = days {
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(d))
                .format("%Y-%m-%d")
                .to_string();
            sqlx::query_as::<_, Trade>(
                "SELECT * FROM trades WHERE trade_date >= ? ORDER BY trade_date ASC, created_at ASC"
            )
            .bind(&cutoff)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, Trade>(
                "SELECT * FROM trades ORDER BY trade_date ASC, created_at ASC",
            )
            .fetch_all(self.db.pool())
            .await?
        };
        Ok(trades)
    }

    /// Enhanced performance metrics with cost basis method selection.
    pub async fn get_enhanced_metrics(
        &self,
        days: Option<i64>,
        method: CostBasisMethod,
    ) -> Result<EnhancedPerformanceMetrics> {
        let base = self.get_performance_metrics(days).await?;
        let trades = self.get_trades_ascending(days).await?;

        // Calculate lot matching with selected method
        let mut trade_pnl: Vec<(f64, i64)> = Vec::new(); // (pnl, holding_days)
        let mut position_map: std::collections::HashMap<String, Vec<(Decimal, Decimal, String)>> =
            std::collections::HashMap::new(); // (shares, price, date)

        for trade in trades.iter() {
            let entry = position_map.entry(trade.symbol.clone()).or_default();

            if trade.action == "buy" {
                entry.push((trade.shares, trade.price, trade.trade_date.clone()));
            } else if trade.action == "sell" {
                let sell_date = chrono::NaiveDate::parse_from_str(&trade.trade_date, "%Y-%m-%d")
                    .unwrap_or_else(|_| chrono::Utc::now().date_naive());
                let mut remaining = trade.shares;
                let mut total_cost = Decimal::ZERO;

                match method {
                    CostBasisMethod::Fifo => {
                        while remaining > Decimal::from_f64(0.0001).unwrap_or_default()
                            && !entry.is_empty()
                        {
                            let (buy_shares, buy_price, buy_date_str) = entry[0].clone();
                            let shares_to_sell = remaining.min(buy_shares);
                            total_cost += shares_to_sell * buy_price;

                            let buy_date =
                                chrono::NaiveDate::parse_from_str(&buy_date_str, "%Y-%m-%d")
                                    .unwrap_or(sell_date);
                            let hold_days = (sell_date - buy_date).num_days();
                            trade_pnl.push((
                                (shares_to_sell * trade.price - shares_to_sell * buy_price)
                                    .to_f64()
                                    .unwrap_or(0.0),
                                hold_days,
                            ));

                            remaining -= shares_to_sell;
                            if shares_to_sell
                                >= buy_shares - Decimal::from_f64(0.0001).unwrap_or_default()
                            {
                                entry.remove(0);
                            } else {
                                entry[0].0 -= shares_to_sell;
                            }
                        }
                    }
                    CostBasisMethod::Lifo => {
                        while remaining > Decimal::from_f64(0.0001).unwrap_or_default()
                            && !entry.is_empty()
                        {
                            let last = entry.len() - 1;
                            let (buy_shares, buy_price, buy_date_str) = entry[last].clone();
                            let shares_to_sell = remaining.min(buy_shares);
                            total_cost += shares_to_sell * buy_price;

                            let buy_date =
                                chrono::NaiveDate::parse_from_str(&buy_date_str, "%Y-%m-%d")
                                    .unwrap_or(sell_date);
                            let hold_days = (sell_date - buy_date).num_days();
                            trade_pnl.push((
                                (shares_to_sell * trade.price - shares_to_sell * buy_price)
                                    .to_f64()
                                    .unwrap_or(0.0),
                                hold_days,
                            ));

                            remaining -= shares_to_sell;
                            if shares_to_sell
                                >= buy_shares - Decimal::from_f64(0.0001).unwrap_or_default()
                            {
                                entry.remove(last);
                            } else {
                                entry[last].0 -= shares_to_sell;
                            }
                        }
                    }
                    CostBasisMethod::AverageCost => {
                        let total_shares: Decimal = entry.iter().map(|(s, _, _)| s).sum();
                        let total_value: Decimal = entry.iter().map(|(s, p, _)| *s * *p).sum();
                        let avg_price = if total_shares > Decimal::ZERO {
                            total_value / total_shares
                        } else {
                            Decimal::ZERO
                        };

                        let shares_sold = remaining.min(total_shares);
                        total_cost = shares_sold * avg_price;
                        let pnl = (shares_sold * trade.price - total_cost)
                            .to_f64()
                            .unwrap_or(0.0);
                        trade_pnl.push((pnl, 0)); // avg cost doesn't track individual holding days

                        // Reduce all lots proportionally
                        let ratio = if total_shares > Decimal::ZERO {
                            (total_shares - shares_sold) / total_shares
                        } else {
                            Decimal::ZERO
                        };
                        for lot in entry.iter_mut() {
                            lot.0 *= ratio;
                        }
                        entry.retain(|l| l.0 > Decimal::from_f64(0.0001).unwrap_or_default());
                    }
                }
            }
        }

        // Holding period stats
        let holding_days: Vec<i64> = trade_pnl.iter().map(|(_, d)| *d).collect();
        let avg_holding = if !holding_days.is_empty() {
            Some(holding_days.iter().sum::<i64>() as f64 / holding_days.len() as f64)
        } else {
            None
        };
        let median_holding = if !holding_days.is_empty() {
            let mut sorted = holding_days.clone();
            sorted.sort();
            Some(sorted[sorted.len() / 2] as f64)
        } else {
            None
        };

        let distribution = HoldingDistribution {
            under_7d: holding_days.iter().filter(|&&d| d < 7).count(),
            d7_to_30: holding_days
                .iter()
                .filter(|&&d| (7..30).contains(&d))
                .count(),
            d30_to_90: holding_days
                .iter()
                .filter(|&&d| (30..90).contains(&d))
                .count(),
            over_90d: holding_days.iter().filter(|&&d| d >= 90).count(),
        };

        // Trade quality metrics
        let wins: Vec<f64> = trade_pnl
            .iter()
            .filter(|(p, _)| *p > 0.0)
            .map(|(p, _)| *p)
            .collect();
        let losses: Vec<f64> = trade_pnl
            .iter()
            .filter(|(p, _)| *p < 0.0)
            .map(|(p, _)| p.abs())
            .collect();

        let avg_win = if !wins.is_empty() {
            wins.iter().sum::<f64>() / wins.len() as f64
        } else {
            0.0
        };
        let avg_loss = if !losses.is_empty() {
            losses.iter().sum::<f64>() / losses.len() as f64
        } else {
            0.0
        };
        let total_win: f64 = wins.iter().sum();
        let total_loss: f64 = losses.iter().sum();

        let expectancy = if !trade_pnl.is_empty() {
            trade_pnl.iter().map(|(p, _)| *p).sum::<f64>() / trade_pnl.len() as f64
        } else {
            0.0
        };
        let profit_factor = if total_loss > 0.0 {
            Some(total_win / total_loss)
        } else {
            None
        };
        let payoff_ratio = if avg_loss > 0.0 {
            Some(avg_win / avg_loss)
        } else {
            None
        };

        Ok(EnhancedPerformanceMetrics {
            base,
            cost_basis_method: format!("{:?}", method),
            expectancy,
            profit_factor,
            payoff_ratio,
            avg_holding_days: avg_holding,
            median_holding_days: median_holding,
            holding_distribution: distribution,
        })
    }

    /// Calculate performance metrics
    pub async fn get_performance_metrics(&self, days: Option<i64>) -> Result<PerformanceMetrics> {
        let trades = if let Some(d) = days {
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(d))
                .format("%Y-%m-%d")
                .to_string();
            sqlx::query_as::<_, Trade>(
                "SELECT * FROM trades WHERE trade_date >= ? ORDER BY trade_date DESC",
            )
            .bind(&cutoff)
            .fetch_all(self.db.pool())
            .await?
        } else {
            self.get_all_trades(None).await?
        };

        // Calculate P&L for each trade by matching buys with sells
        let mut trade_pnl: Vec<(Trade, Decimal)> = Vec::new();
        let mut position_map: std::collections::HashMap<String, Vec<(Decimal, Decimal)>> =
            std::collections::HashMap::new();

        for trade in trades.iter() {
            let entry = position_map.entry(trade.symbol.clone()).or_default();

            if trade.action == "buy" {
                // Add to position
                entry.push((trade.shares, trade.price));
                trade_pnl.push((trade.clone(), Decimal::ZERO));
            } else if trade.action == "sell" {
                // Calculate P&L using FIFO
                let mut remaining_shares = trade.shares;
                let mut total_cost = Decimal::ZERO;

                while remaining_shares > Decimal::from_f64(0.0001).unwrap_or_default()
                    && !entry.is_empty()
                {
                    let (buy_shares, buy_price) = entry[0];
                    let shares_to_sell = remaining_shares.min(buy_shares);

                    total_cost += shares_to_sell * buy_price;
                    remaining_shares -= shares_to_sell;

                    if shares_to_sell >= buy_shares - Decimal::from_f64(0.0001).unwrap_or_default()
                    {
                        entry.remove(0);
                    } else {
                        entry[0].0 -= shares_to_sell;
                    }
                }

                let revenue = trade.shares * trade.price;
                let pnl = revenue - total_cost - trade.commission;
                trade_pnl.push((trade.clone(), pnl));
            }
        }

        // Calculate metrics
        let total_trades = trade_pnl.len();
        let winning_trades = trade_pnl
            .iter()
            .filter(|(_, pnl)| *pnl > Decimal::ZERO)
            .count();
        let losing_trades = trade_pnl
            .iter()
            .filter(|(_, pnl)| *pnl < Decimal::ZERO)
            .count();
        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        let total_realized_pnl: Decimal = trade_pnl.iter().map(|(_, pnl)| pnl).sum();

        let wins: Vec<Decimal> = trade_pnl
            .iter()
            .filter(|(_, pnl)| *pnl > Decimal::ZERO)
            .map(|(_, pnl)| *pnl)
            .collect();

        let losses: Vec<Decimal> = trade_pnl
            .iter()
            .filter(|(_, pnl)| *pnl < Decimal::ZERO)
            .map(|(_, pnl)| *pnl)
            .collect();

        let average_win = if !wins.is_empty() {
            wins.iter().sum::<Decimal>() / Decimal::from(wins.len())
        } else {
            Decimal::ZERO
        };

        let average_loss = if !losses.is_empty() {
            losses.iter().sum::<Decimal>() / Decimal::from(losses.len())
        } else {
            Decimal::ZERO
        };

        let largest_win = wins.iter().cloned().fold(Decimal::ZERO, Decimal::max);
        let largest_loss = losses.iter().cloned().fold(Decimal::ZERO, Decimal::min);

        let recent_trades = self.get_all_trades(Some(20)).await?;

        Ok(PerformanceMetrics {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            total_realized_pnl,
            average_win,
            average_loss,
            largest_win,
            largest_loss,
            recent_trades,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> PortfolioDb {
        PortfolioDb::new("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_log_and_get_trade() {
        let db = setup_test_db().await;
        let logger = TradeLogger::new(db);

        let trade = TradeInput {
            symbol: "AAPL".to_string(),
            action: "buy".to_string(),
            shares: Decimal::from(10),
            price: Decimal::from(150),
            trade_date: "2025-01-01".to_string(),
            commission: Some(Decimal::from(1)),
            notes: Some("Test trade".to_string()),
            alert_id: None,
            analysis_id: None,
        };

        let id = logger.log_trade(trade).await.unwrap();
        assert!(id > 0);

        let retrieved = logger.get_trade(id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().symbol, "AAPL");
    }

    #[tokio::test]
    async fn test_performance_metrics() {
        let db = setup_test_db().await;
        let logger = TradeLogger::new(db);

        // Buy trade
        logger
            .log_trade(TradeInput {
                symbol: "AAPL".to_string(),
                action: "buy".to_string(),
                shares: Decimal::from(10),
                price: Decimal::from(100),
                trade_date: "2025-01-01".to_string(),
                commission: Some(Decimal::from(1)),
                notes: None,
                alert_id: None,
                analysis_id: None,
            })
            .await
            .unwrap();

        // Sell trade (profit)
        logger
            .log_trade(TradeInput {
                symbol: "AAPL".to_string(),
                action: "sell".to_string(),
                shares: Decimal::from(10),
                price: Decimal::from(120),
                trade_date: "2025-01-15".to_string(),
                commission: Some(Decimal::from(1)),
                notes: None,
                alert_id: None,
                analysis_id: None,
            })
            .await
            .unwrap();

        let metrics = logger.get_performance_metrics(None).await.unwrap();
        assert_eq!(metrics.total_trades, 2);
        assert!(metrics.total_realized_pnl > Decimal::ZERO);
    }
}
