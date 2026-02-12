use anyhow::Result;

use crate::models::{StrategyPerformance, PerformanceOverview};

pub struct PerformanceTracker {
    pool: sqlx::AnyPool,
}

impl PerformanceTracker {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    /// Update strategy performance after a trade
    pub async fn update_strategy_performance(
        &self,
        strategy_name: &str,
        symbol: &str,
        is_win: bool,
        profit_loss: f64,
    ) -> Result<()> {
        // Get existing performance or create new
        let existing: Option<StrategyPerformance> = sqlx::query_as(
            "SELECT * FROM strategy_performance WHERE strategy_name = ? AND symbol = ?"
        )
        .bind(strategy_name)
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(mut perf) = existing {
            // Update existing
            perf.signals_taken += 1;
            if is_win {
                perf.winning_trades += 1;
                perf.avg_win = ((perf.avg_win * (perf.winning_trades - 1) as f64) + profit_loss)
                    / perf.winning_trades as f64;
            } else {
                perf.losing_trades += 1;
                perf.avg_loss = ((perf.avg_loss * (perf.losing_trades - 1) as f64) + profit_loss.abs())
                    / perf.losing_trades as f64;
            }

            perf.total_profit_loss += profit_loss;
            perf.win_rate = perf.winning_trades as f64 / perf.signals_taken as f64;

            let total_wins = perf.winning_trades as f64 * perf.avg_win;
            let total_losses = perf.losing_trades as f64 * perf.avg_loss;
            perf.profit_factor = if total_losses > 0.0 {
                total_wins / total_losses
            } else {
                0.0
            };

            sqlx::query(
                r#"
                UPDATE strategy_performance
                SET signals_taken = ?, winning_trades = ?, losing_trades = ?,
                    total_profit_loss = ?, win_rate = ?, avg_win = ?, avg_loss = ?,
                    profit_factor = ?, last_updated = CURRENT_TIMESTAMP
                WHERE strategy_name = ? AND symbol = ?
                "#
            )
            .bind(perf.signals_taken)
            .bind(perf.winning_trades)
            .bind(perf.losing_trades)
            .bind(perf.total_profit_loss)
            .bind(perf.win_rate)
            .bind(perf.avg_win)
            .bind(perf.avg_loss)
            .bind(perf.profit_factor)
            .bind(strategy_name)
            .bind(symbol)
            .execute(&self.pool)
            .await?;
        } else {
            // Create new
            let win_rate = if is_win { 1.0 } else { 0.0 };
            let winning_trades = if is_win { 1 } else { 0 };
            let losing_trades = if is_win { 0 } else { 1 };
            let avg_win = if is_win { profit_loss } else { 0.0 };
            let avg_loss = if is_win { 0.0 } else { profit_loss.abs() };
            let profit_factor = if avg_loss > 0.0 { avg_win / avg_loss } else { 0.0 };

            sqlx::query(
                r#"
                INSERT INTO strategy_performance
                (strategy_name, symbol, total_signals, signals_taken, signals_ignored,
                 winning_trades, losing_trades, total_profit_loss, win_rate, avg_win, avg_loss, profit_factor)
                VALUES (?, ?, 1, 1, 0, ?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind(strategy_name)
            .bind(symbol)
            .bind(winning_trades)
            .bind(losing_trades)
            .bind(profit_loss)
            .bind(win_rate)
            .bind(avg_win)
            .bind(avg_loss)
            .bind(profit_factor)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get performance for a specific strategy
    pub async fn get_strategy_performance(&self, strategy_name: &str) -> Result<Vec<StrategyPerformance>> {
        let performances: Vec<StrategyPerformance> = sqlx::query_as(
            "SELECT * FROM strategy_performance WHERE strategy_name = ? ORDER BY win_rate DESC"
        )
        .bind(strategy_name)
        .fetch_all(&self.pool)
        .await?;

        Ok(performances)
    }

    /// Get all strategy performances
    pub async fn get_all_performances(&self) -> Result<Vec<StrategyPerformance>> {
        let performances: Vec<StrategyPerformance> = sqlx::query_as(
            "SELECT * FROM strategy_performance ORDER BY profit_factor DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(performances)
    }

    /// Get performance overview
    pub async fn get_overview(&self) -> Result<PerformanceOverview> {
        let strategies = self.get_all_performances().await?;

        let total_strategies = strategies.len() as i32;
        let total_trades: i32 = strategies.iter().map(|s| s.signals_taken).sum();
        let total_winning: i32 = strategies.iter().map(|s| s.winning_trades).sum();
        let total_pnl: f64 = strategies.iter().map(|s| s.total_profit_loss).sum();

        let overall_win_rate = if total_trades > 0 {
            total_winning as f64 / total_trades as f64
        } else {
            0.0
        };

        let total_wins: f64 = strategies.iter()
            .map(|s| s.winning_trades as f64 * s.avg_win)
            .sum();
        let total_losses: f64 = strategies.iter()
            .map(|s| s.losing_trades as f64 * s.avg_loss)
            .sum();

        let overall_profit_factor = if total_losses > 0.0 {
            total_wins / total_losses
        } else {
            0.0
        };

        let best_strategy = strategies.iter()
            .max_by(|a, b| a.profit_factor.partial_cmp(&b.profit_factor).unwrap())
            .cloned();

        let worst_strategy = strategies.iter()
            .min_by(|a, b| a.profit_factor.partial_cmp(&b.profit_factor).unwrap())
            .cloned();

        Ok(PerformanceOverview {
            total_strategies,
            total_trades,
            overall_win_rate,
            overall_profit_factor,
            total_profit_loss: total_pnl,
            best_strategy,
            worst_strategy,
            strategies,
        })
    }

    /// Get top performing strategies
    pub async fn get_top_strategies(&self, limit: i32) -> Result<Vec<StrategyPerformance>> {
        let strategies: Vec<StrategyPerformance> = sqlx::query_as(
            "SELECT * FROM strategy_performance ORDER BY profit_factor DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(strategies)
    }
}
