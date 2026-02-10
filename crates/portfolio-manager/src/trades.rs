use crate::models::*;
use crate::db::PortfolioDb;
use anyhow::Result;

pub struct TradeLogger {
    db: PortfolioDb,
}

impl TradeLogger {
    pub fn new(db: PortfolioDb) -> Self {
        Self { db }
    }

    /// Log a new trade
    pub async fn log_trade(&self, trade: TradeInput) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO trades (symbol, action, shares, price, trade_date, commission, notes)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&trade.symbol)
        .bind(&trade.action)
        .bind(trade.shares)
        .bind(trade.price)
        .bind(&trade.trade_date)
        .bind(trade.commission.unwrap_or(0.0))
        .bind(&trade.notes)
        .execute(self.db.pool())
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get all trades for a symbol
    pub async fn get_trades_for_symbol(&self, symbol: &str) -> Result<Vec<Trade>> {
        let trades = sqlx::query_as::<_, Trade>(
            "SELECT * FROM trades WHERE symbol = ? ORDER BY trade_date DESC"
        )
        .bind(symbol)
        .fetch_all(self.db.pool())
        .await?;

        Ok(trades)
    }

    /// Get all trades
    pub async fn get_all_trades(&self, limit: Option<i64>) -> Result<Vec<Trade>> {
        let query = if let Some(lim) = limit {
            format!("SELECT * FROM trades ORDER BY trade_date DESC, created_at DESC LIMIT {}", lim)
        } else {
            "SELECT * FROM trades ORDER BY trade_date DESC, created_at DESC".to_string()
        };

        let trades = sqlx::query_as::<_, Trade>(&query)
            .fetch_all(self.db.pool())
            .await?;

        Ok(trades)
    }

    /// Get trade by ID
    pub async fn get_trade(&self, id: i64) -> Result<Option<Trade>> {
        let trade = sqlx::query_as::<_, Trade>(
            "SELECT * FROM trades WHERE id = ?"
        )
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
        .bind(trade.shares)
        .bind(trade.price)
        .bind(&trade.trade_date)
        .bind(trade.commission.unwrap_or(0.0))
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

    /// Calculate performance metrics
    pub async fn get_performance_metrics(&self, days: Option<i64>) -> Result<PerformanceMetrics> {
        let trades = if let Some(d) = days {
            sqlx::query_as::<_, Trade>(
                "SELECT * FROM trades WHERE trade_date >= date('now', '-' || ? || ' days') ORDER BY trade_date DESC"
            )
            .bind(d)
            .fetch_all(self.db.pool())
            .await?
        } else {
            self.get_all_trades(None).await?
        };

        // Calculate P&L for each trade by matching buys with sells
        let mut trade_pnl: Vec<(Trade, f64)> = Vec::new();
        let mut position_map: std::collections::HashMap<String, Vec<(f64, f64)>> = std::collections::HashMap::new();

        for trade in trades.iter() {
            let entry = position_map.entry(trade.symbol.clone()).or_insert_with(Vec::new);

            if trade.action == "buy" {
                // Add to position
                entry.push((trade.shares, trade.price));
                trade_pnl.push((trade.clone(), 0.0));
            } else if trade.action == "sell" {
                // Calculate P&L using FIFO
                let mut remaining_shares = trade.shares;
                let mut total_cost = 0.0;

                while remaining_shares > 0.0001 && !entry.is_empty() {
                    let (buy_shares, buy_price) = entry[0];
                    let shares_to_sell = remaining_shares.min(buy_shares);

                    total_cost += shares_to_sell * buy_price;
                    remaining_shares -= shares_to_sell;

                    if shares_to_sell >= buy_shares - 0.0001 {
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
        let winning_trades = trade_pnl.iter().filter(|(_, pnl)| *pnl > 0.0).count();
        let losing_trades = trade_pnl.iter().filter(|(_, pnl)| *pnl < 0.0).count();
        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        let total_realized_pnl: f64 = trade_pnl.iter().map(|(_, pnl)| pnl).sum();

        let wins: Vec<f64> = trade_pnl.iter()
            .filter(|(_, pnl)| *pnl > 0.0)
            .map(|(_, pnl)| *pnl)
            .collect();

        let losses: Vec<f64> = trade_pnl.iter()
            .filter(|(_, pnl)| *pnl < 0.0)
            .map(|(_, pnl)| *pnl)
            .collect();

        let average_win = if !wins.is_empty() {
            wins.iter().sum::<f64>() / wins.len() as f64
        } else {
            0.0
        };

        let average_loss = if !losses.is_empty() {
            losses.iter().sum::<f64>() / losses.len() as f64
        } else {
            0.0
        };

        let largest_win = wins.iter().cloned().fold(0.0, f64::max);
        let largest_loss = losses.iter().cloned().fold(0.0, f64::min);

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
            shares: 10.0,
            price: 150.0,
            trade_date: "2025-01-01".to_string(),
            commission: Some(1.0),
            notes: Some("Test trade".to_string()),
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
        logger.log_trade(TradeInput {
            symbol: "AAPL".to_string(),
            action: "buy".to_string(),
            shares: 10.0,
            price: 100.0,
            trade_date: "2025-01-01".to_string(),
            commission: Some(1.0),
            notes: None,
        }).await.unwrap();

        // Sell trade (profit)
        logger.log_trade(TradeInput {
            symbol: "AAPL".to_string(),
            action: "sell".to_string(),
            shares: 10.0,
            price: 120.0,
            trade_date: "2025-01-15".to_string(),
            commission: Some(1.0),
            notes: None,
        }).await.unwrap();

        let metrics = logger.get_performance_metrics(None).await.unwrap();
        assert_eq!(metrics.total_trades, 2);
        assert!(metrics.total_realized_pnl > 0.0);
    }
}
