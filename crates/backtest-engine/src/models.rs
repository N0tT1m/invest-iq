use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;

/// Configuration for a backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub strategy_name: String,
    pub symbols: Vec<String>,
    pub start_date: String,
    pub end_date: String,
    pub initial_capital: Decimal,
    pub position_size_percent: f64, // 0-100
    pub stop_loss_percent: Option<f64>, // as decimal, e.g. 0.05 = 5%
    pub take_profit_percent: Option<f64>,
    pub confidence_threshold: f64,
    pub commission_rate: Option<f64>, // as decimal, e.g. 0.001 = 0.1%
    pub slippage_rate: Option<f64>,
    /// SPY (or other benchmark) bars for benchmark comparison.
    #[serde(default)]
    pub benchmark_bars: Option<Vec<HistoricalBar>>,
    /// Portfolio allocation strategy: "equal_weight" or "custom".
    #[serde(default)]
    pub allocation_strategy: Option<String>,
    /// Custom weights per symbol (must sum to ~1.0). Used when allocation_strategy = "custom".
    #[serde(default)]
    pub symbol_weights: Option<HashMap<String, f64>>,
    /// Rebalance positions every N trading days. None = no rebalancing.
    #[serde(default)]
    pub rebalance_interval_days: Option<i32>,
}

/// A single OHLCV bar for backtesting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalBar {
    pub date: String,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: f64,
}

/// A trading signal generated at a specific point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub date: String,
    pub symbol: String,
    pub signal_type: String, // "buy", "sell", "Buy", "StrongBuy", etc.
    pub confidence: f64,
    pub price: Decimal,
    pub reason: String,
}

/// Result of a completed backtest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub id: Option<i64>,
    pub strategy_name: String,
    pub symbols: Vec<String>,
    pub start_date: String,
    pub end_date: String,
    pub initial_capital: Decimal,
    pub final_capital: Decimal,
    pub total_return: Decimal,
    pub total_return_percent: f64,
    pub total_trades: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub win_rate: f64, // 0-100 percentage
    pub profit_factor: Option<f64>,
    pub sharpe_ratio: Option<f64>,
    pub sortino_ratio: Option<f64>,
    pub max_drawdown: Option<f64>,
    pub calmar_ratio: Option<f64>,
    pub max_consecutive_wins: i32,
    pub max_consecutive_losses: i32,
    pub avg_holding_period_days: Option<f64>,
    pub exposure_time_percent: Option<f64>,
    pub recovery_factor: Option<f64>,
    pub average_win: Option<Decimal>,
    pub average_loss: Option<Decimal>,
    pub largest_win: Option<Decimal>,
    pub largest_loss: Option<Decimal>,
    pub avg_trade_return_percent: Option<f64>,
    pub total_commission_paid: Decimal,
    pub total_slippage_cost: Decimal,
    pub equity_curve: Vec<EquityPoint>,
    pub trades: Vec<BacktestTrade>,
    pub created_at: Option<String>,
    /// Benchmark comparison (buy-and-hold, SPY, alpha).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<BenchmarkComparison>,
    /// Per-symbol breakdown (only populated for multi-symbol backtests).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_symbol_results: Option<Vec<SymbolResult>>,
}

/// A point on the equity curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: String,
    pub equity: Decimal,
    pub drawdown_percent: f64,
}

/// A round-trip trade (entry + exit) from the backtest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    pub id: Option<i64>,
    pub backtest_id: Option<i64>,
    pub symbol: String,
    pub signal: String,
    pub entry_date: String,
    pub exit_date: String,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub shares: Decimal,
    pub profit_loss: Decimal,
    pub profit_loss_percent: f64,
    pub holding_period_days: i64,
    pub commission_cost: Decimal,
    pub slippage_cost: Decimal,
    pub exit_reason: String,
}

// --- Benchmark Comparison ---

/// Benchmark comparison: strategy vs buy-and-hold vs SPY.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub buy_hold_return_percent: f64,
    pub spy_return_percent: Option<f64>,
    /// Strategy return - buy-and-hold return.
    pub alpha: f64,
    /// Strategy return - SPY return.
    pub spy_alpha: Option<f64>,
    /// (Strategy return - benchmark return) / tracking error.
    pub information_ratio: Option<f64>,
    pub buy_hold_equity_curve: Vec<EquityPoint>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spy_equity_curve: Vec<EquityPoint>,
}

// --- Walk-Forward Validation ---

/// Walk-forward validation results across all folds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardResult {
    pub folds: Vec<WalkForwardFold>,
    pub avg_in_sample_return: f64,
    pub avg_out_of_sample_return: f64,
    /// in_sample / out_of_sample. Values near 1.0 = low overfitting.
    pub overfitting_ratio: f64,
    pub out_of_sample_win_rate: f64,
    pub out_of_sample_sharpe: Option<f64>,
    /// Combined out-of-sample equity curve.
    pub combined_equity_curve: Vec<EquityPoint>,
    pub total_oos_trades: i32,
}

/// A single walk-forward fold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardFold {
    pub fold_number: i32,
    pub train_start: String,
    pub train_end: String,
    pub test_start: String,
    pub test_end: String,
    pub in_sample_return: f64,
    pub out_of_sample_return: f64,
    pub in_sample_sharpe: Option<f64>,
    pub out_of_sample_sharpe: Option<f64>,
    pub in_sample_trades: i32,
    pub out_of_sample_trades: i32,
}

/// Pre-prepared data for one walk-forward fold.
pub struct WalkForwardFoldData {
    pub train_data: HashMap<String, Vec<HistoricalBar>>,
    pub train_signals: Vec<Signal>,
    pub test_data: HashMap<String, Vec<HistoricalBar>>,
    pub test_signals: Vec<Signal>,
}

// --- Multi-Symbol Portfolio ---

/// Per-symbol breakdown in a multi-symbol backtest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolResult {
    pub symbol: String,
    pub total_trades: i32,
    pub winning_trades: i32,
    pub win_rate: f64,
    pub total_return: Decimal,
    pub total_return_percent: f64,
    pub weight: f64,
}

// --- Monte Carlo Simulation ---

/// Monte Carlo simulation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResult {
    pub simulations: i32,
    pub median_return: f64,
    pub mean_return: f64,
    pub std_dev_return: f64,
    pub percentile_5: f64,
    pub percentile_25: f64,
    pub percentile_75: f64,
    pub percentile_95: f64,
    pub probability_of_profit: f64,
    /// Probability of losing >50% of capital.
    pub probability_of_ruin: f64,
    pub expected_max_drawdown_95: f64,
    pub median_max_drawdown: f64,
    pub median_sharpe: f64,
    /// Sampled return values for histogram (up to 200 points).
    pub return_distribution: Vec<f64>,
    /// Sampled max drawdown values for histogram (up to 200 points).
    pub drawdown_distribution: Vec<f64>,
}
