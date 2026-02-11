use crate::models::*;
use crate::db::PortfolioDb;
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

pub struct PortfolioManager {
    db: PortfolioDb,
}

impl PortfolioManager {
    pub fn new(db: PortfolioDb) -> Self {
        Self { db }
    }

    /// Get a reference to the database
    pub fn db(&self) -> &PortfolioDb {
        &self.db
    }

    /// Get all positions (simple version without prices)
    pub async fn get_portfolio(&self) -> Result<Vec<Position>> {
        self.get_all_positions().await
    }

    /// Add a new position to the portfolio
    pub async fn add_position(&self, position: Position) -> Result<i64> {
        let (id,): (i64,) = sqlx::query_as(
            r#"
            INSERT INTO positions (symbol, shares, entry_price, entry_date, notes)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(symbol) DO UPDATE SET
                shares = shares + excluded.shares,
                entry_price = ((positions.shares * positions.entry_price) + (excluded.shares * excluded.entry_price)) / (positions.shares + excluded.shares),
                notes = excluded.notes
            RETURNING id
            "#
        )
        .bind(&position.symbol)
        .bind(position.shares.to_f64().unwrap_or(0.0))
        .bind(position.entry_price.to_f64().unwrap_or(0.0))
        .bind(&position.entry_date)
        .bind(&position.notes)
        .fetch_one(self.db.pool())
        .await?;

        Ok(id)
    }

    /// Get a position by symbol
    pub async fn get_position(&self, symbol: &str) -> Result<Option<Position>> {
        let position = sqlx::query_as::<_, Position>(
            "SELECT * FROM positions WHERE symbol = ?"
        )
        .bind(symbol)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(position)
    }

    /// Get all positions
    pub async fn get_all_positions(&self) -> Result<Vec<Position>> {
        let positions = sqlx::query_as::<_, Position>(
            "SELECT * FROM positions ORDER BY symbol"
        )
        .fetch_all(self.db.pool())
        .await?;

        Ok(positions)
    }

    /// Update a position
    pub async fn update_position(&self, id: i64, position: Position) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE positions
            SET shares = ?, entry_price = ?, entry_date = ?, notes = ?
            WHERE id = ?
            "#
        )
        .bind(position.shares.to_f64().unwrap_or(0.0))
        .bind(position.entry_price.to_f64().unwrap_or(0.0))
        .bind(&position.entry_date)
        .bind(&position.notes)
        .bind(id)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// Remove shares from a position (or delete if shares reach 0)
    pub async fn remove_shares(&self, symbol: &str, shares: Decimal) -> Result<()> {
        // Get current position
        let position = self.get_position(symbol).await?
            .ok_or_else(|| anyhow!("Position not found: {}", symbol))?;

        let new_shares = position.shares - shares;

        if new_shares <= Decimal::from_f64(0.0001).unwrap_or_default() {
            // Delete position if shares are 0 or negative
            sqlx::query("DELETE FROM positions WHERE symbol = ?")
                .bind(symbol)
                .execute(self.db.pool())
                .await?;
        } else {
            // Update shares
            sqlx::query("UPDATE positions SET shares = ? WHERE symbol = ?")
                .bind(new_shares.to_f64().unwrap_or(0.0))
                .bind(symbol)
                .execute(self.db.pool())
                .await?;
        }

        Ok(())
    }

    /// Delete a position
    pub async fn delete_position(&self, symbol: &str) -> Result<()> {
        sqlx::query("DELETE FROM positions WHERE symbol = ?")
            .bind(symbol)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }

    /// Get portfolio summary with current prices
    pub async fn get_portfolio_summary<F>(&self, price_fetcher: F) -> Result<PortfolioSummary>
    where
        F: Fn(&str) -> Result<f64>,
    {
        let positions = self.get_all_positions().await?;
        let mut positions_with_pnl = Vec::new();
        let mut total_value = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;

        for position in positions {
            let current_price_f64 = price_fetcher(&position.symbol)?;
            let current_price = Decimal::from_f64(current_price_f64).unwrap_or_default();
            let market_value = position.shares * current_price;
            let cost_basis = position.shares * position.entry_price;
            let unrealized_pnl = market_value - cost_basis;
            let unrealized_pnl_percent = if cost_basis > Decimal::ZERO {
                ((unrealized_pnl / cost_basis) * Decimal::from(100)).to_f64().unwrap_or(0.0)
            } else {
                0.0
            };

            total_value += market_value;
            total_cost += cost_basis;

            positions_with_pnl.push(PositionWithPnL {
                position,
                current_price,
                market_value,
                cost_basis,
                unrealized_pnl,
                unrealized_pnl_percent,
            });
        }

        let total_pnl = total_value - total_cost;
        let total_pnl_percent = if total_cost > Decimal::ZERO {
            ((total_pnl / total_cost) * Decimal::from(100)).to_f64().unwrap_or(0.0)
        } else {
            0.0
        };

        Ok(PortfolioSummary {
            total_positions: positions_with_pnl.len(),
            total_value,
            total_cost,
            total_pnl,
            total_pnl_percent,
            positions: positions_with_pnl,
        })
    }

    /// Save a portfolio snapshot for equity curve
    pub async fn save_snapshot(&self, summary: &PortfolioSummary) -> Result<i64> {
        let (id,): (i64,) = sqlx::query_as(
            r#"
            INSERT INTO portfolio_snapshots (total_value, total_cost, total_pnl, total_pnl_percent, snapshot_date)
            VALUES (?, ?, ?, ?, datetime('now'))
            RETURNING id
            "#
        )
        .bind(summary.total_value.to_f64().unwrap_or(0.0))
        .bind(summary.total_cost.to_f64().unwrap_or(0.0))
        .bind(summary.total_pnl.to_f64().unwrap_or(0.0))
        .bind(summary.total_pnl_percent)
        .fetch_one(self.db.pool())
        .await?;

        Ok(id)
    }

    /// Get portfolio snapshots for equity curve
    pub async fn get_snapshots(&self, days: i64) -> Result<Vec<PortfolioSnapshot>> {
        let snapshots = sqlx::query_as::<_, PortfolioSnapshot>(
            "SELECT * FROM portfolio_snapshots WHERE snapshot_date >= datetime('now', '-' || ? || ' days') ORDER BY snapshot_date"
        )
        .bind(days)
        .fetch_all(self.db.pool())
        .await?;

        Ok(snapshots)
    }

    /// Add to watchlist
    pub async fn add_to_watchlist(&self, symbol: &str, notes: Option<String>) -> Result<i64> {
        let (id,): (i64,) = sqlx::query_as(
            "INSERT OR IGNORE INTO watchlist (symbol, notes) VALUES (?, ?) RETURNING id"
        )
        .bind(symbol)
        .bind(&notes)
        .fetch_one(self.db.pool())
        .await?;

        Ok(id)
    }

    /// Get watchlist
    pub async fn get_watchlist(&self) -> Result<Vec<WatchlistItem>> {
        let items = sqlx::query_as::<_, WatchlistItem>(
            "SELECT * FROM watchlist ORDER BY added_at DESC"
        )
        .fetch_all(self.db.pool())
        .await?;

        Ok(items)
    }

    /// Remove from watchlist
    pub async fn remove_from_watchlist(&self, symbol: &str) -> Result<()> {
        sqlx::query("DELETE FROM watchlist WHERE symbol = ?")
            .bind(symbol)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> PortfolioDb {
        PortfolioDb::new("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_add_and_get_position() {
        let db = setup_test_db().await;
        let manager = PortfolioManager::new(db);

        let position = Position {
            id: None,
            symbol: "AAPL".to_string(),
            shares: Decimal::from(10),
            entry_price: Decimal::from(150),
            entry_date: "2025-01-01".to_string(),
            notes: Some("Test position".to_string()),
            created_at: None,
        };

        let id = manager.add_position(position).await.unwrap();
        assert!(id > 0);

        let retrieved = manager.get_position("AAPL").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().shares, Decimal::from(10));
    }

    #[tokio::test]
    async fn test_remove_shares() {
        let db = setup_test_db().await;
        let manager = PortfolioManager::new(db);

        let position = Position {
            id: None,
            symbol: "AAPL".to_string(),
            shares: Decimal::from(10),
            entry_price: Decimal::from(150),
            entry_date: "2025-01-01".to_string(),
            notes: None,
            created_at: None,
        };

        manager.add_position(position).await.unwrap();
        manager.remove_shares("AAPL", Decimal::from(5)).await.unwrap();

        let retrieved = manager.get_position("AAPL").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().shares, Decimal::from(5));

        // Remove all remaining shares
        manager.remove_shares("AAPL", Decimal::from(5)).await.unwrap();
        let retrieved = manager.get_position("AAPL").await.unwrap();
        assert!(retrieved.is_none());
    }
}
