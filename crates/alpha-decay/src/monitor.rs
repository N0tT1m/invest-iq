//! Alpha Decay Monitor
//!
//! Tracks strategy performance over time and detects degradation.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row, SqlitePool};

/// A performance snapshot at a point in time
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PerformanceSnapshot {
    pub id: Option<i64>,
    pub strategy_name: String,
    pub snapshot_date: NaiveDate,
    pub rolling_sharpe: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub trades_count: i32,
    #[sqlx(default)]
    pub cumulative_return: f64,
    #[sqlx(default)]
    pub max_drawdown: f64,
    pub created_at: Option<DateTime<Utc>>,
}

/// Current performance metrics for a strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    pub strategy_name: String,
    pub current_sharpe: f64,
    pub avg_historical_sharpe: f64,
    pub peak_sharpe: f64,
    pub current_win_rate: f64,
    pub avg_historical_win_rate: f64,
    pub current_profit_factor: f64,
    pub total_trades: i32,
    pub days_tracked: i32,
    pub last_updated: DateTime<Utc>,
}

/// Row for strategy name query
#[derive(Debug, FromRow)]
struct StrategyNameRow {
    strategy_name: String,
}

/// Alpha Decay Monitor for tracking strategy health
pub struct AlphaDecayMonitor {
    pool: SqlitePool,
}

impl AlphaDecayMonitor {
    /// Create a new monitor
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record a performance snapshot
    pub async fn record_snapshot(&self, snapshot: &PerformanceSnapshot) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO strategy_health_snapshots (
                strategy_name, snapshot_date, rolling_sharpe, win_rate,
                profit_factor, trades_count
            )
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&snapshot.strategy_name)
        .bind(snapshot.snapshot_date)
        .bind(snapshot.rolling_sharpe)
        .bind(snapshot.win_rate)
        .bind(snapshot.profit_factor)
        .bind(snapshot.trades_count)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get all snapshots for a strategy
    pub async fn get_snapshots(
        &self,
        strategy_name: &str,
        limit: i64,
    ) -> Result<Vec<PerformanceSnapshot>> {
        let snapshots: Vec<PerformanceSnapshot> = sqlx::query_as(
            r#"
            SELECT
                id, strategy_name, snapshot_date, rolling_sharpe, win_rate,
                profit_factor, trades_count,
                0.0 as cumulative_return, 0.0 as max_drawdown,
                created_at
            FROM strategy_health_snapshots
            WHERE strategy_name = ?
            ORDER BY snapshot_date DESC
            LIMIT ?
            "#,
        )
        .bind(strategy_name)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(snapshots)
    }

    /// Get current performance summary for a strategy
    pub async fn get_strategy_performance(
        &self,
        strategy_name: &str,
    ) -> Result<Option<StrategyPerformance>> {
        let snapshots = self.get_snapshots(strategy_name, 365).await?;

        if snapshots.is_empty() {
            return Ok(None);
        }

        let current = &snapshots[0];
        let n = snapshots.len() as f64;

        // Calculate averages
        let avg_sharpe: f64 = snapshots.iter().map(|s| s.rolling_sharpe).sum::<f64>() / n;
        let avg_win_rate: f64 = snapshots.iter().map(|s| s.win_rate).sum::<f64>() / n;
        let peak_sharpe = snapshots
            .iter()
            .map(|s| s.rolling_sharpe)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
        let total_trades: i32 = snapshots.iter().map(|s| s.trades_count).sum();
        let days = snapshots.len() as i32;

        Ok(Some(StrategyPerformance {
            strategy_name: strategy_name.to_string(),
            current_sharpe: current.rolling_sharpe,
            avg_historical_sharpe: avg_sharpe,
            peak_sharpe,
            current_win_rate: current.win_rate,
            avg_historical_win_rate: avg_win_rate,
            current_profit_factor: current.profit_factor,
            total_trades,
            days_tracked: days,
            last_updated: Utc::now(),
        }))
    }

    /// Get all tracked strategies
    pub async fn get_all_strategies(&self) -> Result<Vec<String>> {
        let rows: Vec<StrategyNameRow> = sqlx::query_as(
            r#"
            SELECT DISTINCT strategy_name
            FROM strategy_health_snapshots
            ORDER BY strategy_name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.strategy_name).collect())
    }

    /// Get Sharpe ratio time series for decay analysis
    pub async fn get_sharpe_series(
        &self,
        strategy_name: &str,
    ) -> Result<Vec<(NaiveDate, f64)>> {
        let snapshots = self.get_snapshots(strategy_name, 365).await?;

        Ok(snapshots
            .into_iter()
            .rev() // Oldest first for time series
            .map(|s| (s.snapshot_date, s.rolling_sharpe))
            .collect())
    }

    /// Calculate decay percentage from peak
    pub async fn calculate_decay(
        &self,
        strategy_name: &str,
    ) -> Result<Option<DecayMetrics>> {
        let performance = self.get_strategy_performance(strategy_name).await?;

        match performance {
            Some(perf) => {
                let decay_from_peak = if perf.peak_sharpe > 0.0 {
                    (perf.peak_sharpe - perf.current_sharpe) / perf.peak_sharpe * 100.0
                } else {
                    0.0
                };

                let decay_from_avg = if perf.avg_historical_sharpe > 0.0 {
                    (perf.avg_historical_sharpe - perf.current_sharpe)
                        / perf.avg_historical_sharpe
                        * 100.0
                } else {
                    0.0
                };

                // Calculate trend (regression slope)
                let series = self.get_sharpe_series(strategy_name).await?;
                let trend = calculate_trend(&series);

                // Estimate time to breakeven (Sharpe = 0) if decaying
                let time_to_breakeven = if trend < 0.0 && perf.current_sharpe > 0.0 {
                    Some((perf.current_sharpe / (-trend)).round() as i32)
                } else {
                    None
                };

                Ok(Some(DecayMetrics {
                    strategy_name: strategy_name.to_string(),
                    current_sharpe: perf.current_sharpe,
                    peak_sharpe: perf.peak_sharpe,
                    avg_sharpe: perf.avg_historical_sharpe,
                    decay_from_peak_pct: decay_from_peak,
                    decay_from_avg_pct: decay_from_avg,
                    trend_per_day: trend,
                    days_to_breakeven: time_to_breakeven,
                    is_decaying: trend < -0.001,
                }))
            }
            None => Ok(None),
        }
    }
}

/// Metrics describing strategy decay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayMetrics {
    pub strategy_name: String,
    pub current_sharpe: f64,
    pub peak_sharpe: f64,
    pub avg_sharpe: f64,
    pub decay_from_peak_pct: f64,
    pub decay_from_avg_pct: f64,
    pub trend_per_day: f64,
    pub days_to_breakeven: Option<i32>,
    pub is_decaying: bool,
}

/// Calculate linear trend (slope) from time series
fn calculate_trend(series: &[(NaiveDate, f64)]) -> f64 {
    if series.len() < 2 {
        return 0.0;
    }

    let n = series.len() as f64;
    let x: Vec<f64> = (0..series.len()).map(|i| i as f64).collect();
    let y: Vec<f64> = series.iter().map(|(_, v)| *v).collect();

    let x_mean = x.iter().sum::<f64>() / n;
    let y_mean = y.iter().sum::<f64>() / n;

    let numerator: f64 = x.iter().zip(y.iter()).map(|(xi, yi)| (xi - x_mean) * (yi - y_mean)).sum();
    let denominator: f64 = x.iter().map(|xi| (xi - x_mean).powi(2)).sum();

    if denominator.abs() < 1e-10 {
        0.0
    } else {
        numerator / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trend_calculation() {
        // Upward trend
        let series: Vec<(NaiveDate, f64)> = (0..10)
            .map(|i| (NaiveDate::from_ymd_opt(2024, 1, i + 1).unwrap(), i as f64 * 0.1))
            .collect();
        let trend = calculate_trend(&series);
        assert!(trend > 0.0);

        // Downward trend
        let series: Vec<(NaiveDate, f64)> = (0..10)
            .map(|i| (NaiveDate::from_ymd_opt(2024, 1, i + 1).unwrap(), 1.0 - i as f64 * 0.1))
            .collect();
        let trend = calculate_trend(&series);
        assert!(trend < 0.0);

        // Flat
        let series: Vec<(NaiveDate, f64)> = (0..10)
            .map(|i| (NaiveDate::from_ymd_opt(2024, 1, i + 1).unwrap(), 0.5))
            .collect();
        let trend = calculate_trend(&series);
        assert!(trend.abs() < 0.001);
    }
}
