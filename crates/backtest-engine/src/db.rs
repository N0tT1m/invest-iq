use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

use crate::models::{
    BacktestResult, BacktestTrade, BenchmarkComparison, EquityPoint, SymbolResult,
};

/// Persists backtest results and trades to the database.
pub struct BacktestDb {
    pool: sqlx::AnyPool,
}

impl BacktestDb {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    /// Initialize backtest tables if they don't exist.
    pub async fn init_tables(&self) -> Result<(), sqlx::Error> {
        // Tables are created by sqlx migrations in portfolio-manager.
        Ok(())
    }

    /// Save a backtest result and its trades. Returns the backtest ID.
    pub async fn save_backtest(&self, result: &BacktestResult) -> Result<i64, anyhow::Error> {
        self.init_tables().await?;

        let symbols_json = serde_json::to_string(&result.symbols)?;
        let equity_json = serde_json::to_string(&result.equity_curve)?;
        let benchmark_json = result
            .benchmark
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let per_symbol_json = result
            .per_symbol_results
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        let (backtest_id,): (i64,) = sqlx::query_as(
            "INSERT INTO backtests (
                strategy_name, symbols, start_date, end_date,
                initial_capital, final_capital, total_return, total_return_percent,
                total_trades, winning_trades, losing_trades, win_rate,
                profit_factor, sharpe_ratio, sortino_ratio, max_drawdown,
                calmar_ratio, max_consecutive_wins, max_consecutive_losses,
                avg_holding_period_days, exposure_time_percent, recovery_factor,
                average_win, average_loss, largest_win, largest_loss,
                avg_trade_return_percent, total_commission_paid, total_slippage_cost,
                equity_curve_json, benchmark_json, per_symbol_results_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id",
        )
        .bind(&result.strategy_name)
        .bind(&symbols_json)
        .bind(&result.start_date)
        .bind(&result.end_date)
        .bind(result.initial_capital.to_f64().unwrap_or(0.0))
        .bind(result.final_capital.to_f64().unwrap_or(0.0))
        .bind(result.total_return.to_f64().unwrap_or(0.0))
        .bind(result.total_return_percent)
        .bind(result.total_trades)
        .bind(result.winning_trades)
        .bind(result.losing_trades)
        .bind(result.win_rate)
        .bind(result.profit_factor)
        .bind(result.sharpe_ratio)
        .bind(result.sortino_ratio)
        .bind(result.max_drawdown)
        .bind(result.calmar_ratio)
        .bind(result.max_consecutive_wins)
        .bind(result.max_consecutive_losses)
        .bind(result.avg_holding_period_days)
        .bind(result.exposure_time_percent)
        .bind(result.recovery_factor)
        .bind(result.average_win.map(|v| v.to_f64().unwrap_or(0.0)))
        .bind(result.average_loss.map(|v| v.to_f64().unwrap_or(0.0)))
        .bind(result.largest_win.map(|v| v.to_f64().unwrap_or(0.0)))
        .bind(result.largest_loss.map(|v| v.to_f64().unwrap_or(0.0)))
        .bind(result.avg_trade_return_percent)
        .bind(result.total_commission_paid.to_f64().unwrap_or(0.0))
        .bind(result.total_slippage_cost.to_f64().unwrap_or(0.0))
        .bind(&equity_json)
        .bind(&benchmark_json)
        .bind(&per_symbol_json)
        .fetch_one(&self.pool)
        .await?;

        // Save trades
        for trade in &result.trades {
            sqlx::query(
                "INSERT INTO backtest_trades (
                    backtest_id, symbol, signal, confidence, entry_date, exit_date,
                    entry_price, exit_price, shares,
                    profit_loss, profit_loss_percent, holding_period_days,
                    commission_cost, slippage_cost, exit_reason
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(backtest_id)
            .bind(&trade.symbol)
            .bind(&trade.signal)
            .bind(trade.confidence)
            .bind(&trade.entry_date)
            .bind(&trade.exit_date)
            .bind(trade.entry_price.to_f64().unwrap_or(0.0))
            .bind(trade.exit_price.to_f64().unwrap_or(0.0))
            .bind(trade.shares.to_f64().unwrap_or(0.0))
            .bind(trade.profit_loss.to_f64().unwrap_or(0.0))
            .bind(trade.profit_loss_percent)
            .bind(trade.holding_period_days)
            .bind(trade.commission_cost.to_f64().unwrap_or(0.0))
            .bind(trade.slippage_cost.to_f64().unwrap_or(0.0))
            .bind(&trade.exit_reason)
            .execute(&self.pool)
            .await?;
        }

        Ok(backtest_id)
    }

    /// Get all backtest results (without trades or equity curves for performance).
    pub async fn get_all_backtests(&self) -> Result<Vec<BacktestResult>, anyhow::Error> {
        self.init_tables().await?;

        let rows = sqlx::query_as::<_, BacktestRow>(
            "SELECT id, strategy_name, symbols, start_date, end_date,
                    initial_capital, final_capital, total_return, total_return_percent,
                    total_trades, winning_trades, losing_trades, win_rate,
                    profit_factor, sharpe_ratio, sortino_ratio, max_drawdown,
                    calmar_ratio, max_consecutive_wins, max_consecutive_losses,
                    avg_holding_period_days, exposure_time_percent, recovery_factor,
                    average_win, average_loss, largest_win, largest_loss,
                    avg_trade_return_percent, total_commission_paid, total_slippage_cost,
                    equity_curve_json, benchmark_json, per_symbol_results_json, created_at
             FROM backtests ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_result()).collect())
    }

    /// Get a single backtest by ID (includes equity curve).
    pub async fn get_backtest(&self, id: i64) -> Result<Option<BacktestResult>, anyhow::Error> {
        self.init_tables().await?;

        let row = sqlx::query_as::<_, BacktestRow>(
            "SELECT id, strategy_name, symbols, start_date, end_date,
                    initial_capital, final_capital, total_return, total_return_percent,
                    total_trades, winning_trades, losing_trades, win_rate,
                    profit_factor, sharpe_ratio, sortino_ratio, max_drawdown,
                    calmar_ratio, max_consecutive_wins, max_consecutive_losses,
                    avg_holding_period_days, exposure_time_percent, recovery_factor,
                    average_win, average_loss, largest_win, largest_loss,
                    avg_trade_return_percent, total_commission_paid, total_slippage_cost,
                    equity_curve_json, benchmark_json, per_symbol_results_json, created_at
             FROM backtests WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into_result()))
    }

    /// Delete a backtest and its trades.
    pub async fn delete_backtest(&self, id: i64) -> Result<(), anyhow::Error> {
        sqlx::query("DELETE FROM backtest_trades WHERE backtest_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM backtests WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get trades for a specific backtest.
    pub async fn get_backtest_trades(
        &self,
        backtest_id: i64,
    ) -> Result<Vec<BacktestTrade>, anyhow::Error> {
        let rows = sqlx::query_as::<_, TradeRow>(
            "SELECT id, backtest_id, symbol, signal, COALESCE(confidence, 0.0) as confidence,
                    entry_date, exit_date,
                    entry_price, exit_price, shares,
                    profit_loss, profit_loss_percent, holding_period_days,
                    commission_cost, slippage_cost, exit_reason
             FROM backtest_trades WHERE backtest_id = ? ORDER BY entry_date",
        )
        .bind(backtest_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| BacktestTrade {
                id: Some(r.id),
                backtest_id: Some(r.backtest_id),
                symbol: r.symbol,
                signal: r.signal,
                confidence: r.confidence,
                entry_date: r.entry_date,
                exit_date: r.exit_date,
                entry_price: Decimal::from_f64(r.entry_price).unwrap_or_default(),
                exit_price: Decimal::from_f64(r.exit_price).unwrap_or_default(),
                shares: Decimal::from_f64(r.shares).unwrap_or_default(),
                profit_loss: Decimal::from_f64(r.profit_loss).unwrap_or_default(),
                profit_loss_percent: r.profit_loss_percent,
                holding_period_days: r.holding_period_days,
                commission_cost: Decimal::from_f64(r.commission_cost).unwrap_or_default(),
                slippage_cost: Decimal::from_f64(r.slippage_cost).unwrap_or_default(),
                exit_reason: r.exit_reason,
                direction: None,
            })
            .collect())
    }

    /// Get backtests by strategy name.
    pub async fn get_backtests_by_strategy(
        &self,
        strategy_name: &str,
    ) -> Result<Vec<BacktestResult>, anyhow::Error> {
        self.init_tables().await?;

        let rows = sqlx::query_as::<_, BacktestRow>(
            "SELECT id, strategy_name, symbols, start_date, end_date,
                    initial_capital, final_capital, total_return, total_return_percent,
                    total_trades, winning_trades, losing_trades, win_rate,
                    profit_factor, sharpe_ratio, sortino_ratio, max_drawdown,
                    calmar_ratio, max_consecutive_wins, max_consecutive_losses,
                    avg_holding_period_days, exposure_time_percent, recovery_factor,
                    average_win, average_loss, largest_win, largest_loss,
                    avg_trade_return_percent, total_commission_paid, total_slippage_cost,
                    equity_curve_json, benchmark_json, per_symbol_results_json, created_at
             FROM backtests WHERE strategy_name = ? ORDER BY created_at DESC",
        )
        .bind(strategy_name)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_result()).collect())
    }
}

/// Internal row type for sqlx deserialization.
#[derive(sqlx::FromRow)]
struct BacktestRow {
    id: i64,
    strategy_name: String,
    symbols: String,
    start_date: String,
    end_date: String,
    initial_capital: f64,
    final_capital: f64,
    total_return: f64,
    total_return_percent: f64,
    total_trades: i32,
    winning_trades: i32,
    losing_trades: i32,
    win_rate: f64,
    profit_factor: Option<f64>,
    sharpe_ratio: Option<f64>,
    sortino_ratio: Option<f64>,
    max_drawdown: Option<f64>,
    calmar_ratio: Option<f64>,
    max_consecutive_wins: i32,
    max_consecutive_losses: i32,
    avg_holding_period_days: Option<f64>,
    exposure_time_percent: Option<f64>,
    recovery_factor: Option<f64>,
    average_win: Option<f64>,
    average_loss: Option<f64>,
    largest_win: Option<f64>,
    largest_loss: Option<f64>,
    avg_trade_return_percent: Option<f64>,
    total_commission_paid: f64,
    total_slippage_cost: f64,
    equity_curve_json: Option<String>,
    benchmark_json: Option<String>,
    per_symbol_results_json: Option<String>,
    created_at: Option<String>,
}

impl BacktestRow {
    fn into_result(self) -> BacktestResult {
        let symbols: Vec<String> = serde_json::from_str(&self.symbols).unwrap_or_default();
        let equity_curve: Vec<EquityPoint> = self
            .equity_curve_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();

        BacktestResult {
            id: Some(self.id),
            strategy_name: self.strategy_name,
            symbols,
            start_date: self.start_date,
            end_date: self.end_date,
            initial_capital: Decimal::from_f64(self.initial_capital).unwrap_or_default(),
            final_capital: Decimal::from_f64(self.final_capital).unwrap_or_default(),
            total_return: Decimal::from_f64(self.total_return).unwrap_or_default(),
            total_return_percent: self.total_return_percent,
            annualized_return_percent: None, // Recomputed at runtime, not persisted
            total_trades: self.total_trades,
            winning_trades: self.winning_trades,
            losing_trades: self.losing_trades,
            win_rate: self.win_rate,
            profit_factor: self.profit_factor,
            sharpe_ratio: self.sharpe_ratio,
            sortino_ratio: self.sortino_ratio,
            max_drawdown: self.max_drawdown,
            calmar_ratio: self.calmar_ratio,
            max_consecutive_wins: self.max_consecutive_wins,
            max_consecutive_losses: self.max_consecutive_losses,
            avg_holding_period_days: self.avg_holding_period_days,
            exposure_time_percent: self.exposure_time_percent,
            recovery_factor: self.recovery_factor,
            average_win: self.average_win.and_then(Decimal::from_f64),
            average_loss: self.average_loss.and_then(Decimal::from_f64),
            largest_win: self.largest_win.and_then(Decimal::from_f64),
            largest_loss: self.largest_loss.and_then(Decimal::from_f64),
            avg_trade_return_percent: self.avg_trade_return_percent,
            total_commission_paid: Decimal::from_f64(self.total_commission_paid)
                .unwrap_or_default(),
            total_slippage_cost: Decimal::from_f64(self.total_slippage_cost).unwrap_or_default(),
            equity_curve,
            trades: Vec::new(),
            created_at: self.created_at,
            benchmark: self
                .benchmark_json
                .as_deref()
                .and_then(|j| serde_json::from_str::<BenchmarkComparison>(j).ok()),
            per_symbol_results: self
                .per_symbol_results_json
                .as_deref()
                .and_then(|j| serde_json::from_str::<Vec<SymbolResult>>(j).ok()),
            short_trades: None,
            margin_used_peak: None,
            data_quality_report: None,
            confidence_intervals: None,
            extended_metrics: None,
            factor_attribution: None,
            tear_sheet: None,
            advanced_analytics: None, // Not persisted to DB, computed on-the-fly
        }
    }
}

#[derive(sqlx::FromRow)]
struct TradeRow {
    id: i64,
    backtest_id: i64,
    symbol: String,
    signal: String,
    confidence: f64,
    entry_date: String,
    exit_date: String,
    entry_price: f64,
    exit_price: f64,
    shares: f64,
    profit_loss: f64,
    profit_loss_percent: f64,
    holding_period_days: i64,
    commission_cost: f64,
    slippage_cost: f64,
    exit_reason: String,
}
