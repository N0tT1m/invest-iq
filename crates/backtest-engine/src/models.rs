use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub strategy_name: String,
    pub symbols: Vec<String>,
    pub start_date: String,
    pub end_date: String,
    pub initial_capital: Decimal,
    pub position_size_percent: f64,     // 0-100
    pub stop_loss_percent: Option<f64>, // as decimal, e.g. 0.05 = 5%
    pub take_profit_percent: Option<f64>,
    pub confidence_threshold: f64,
    pub commission_rate: Option<f64>, // as decimal, e.g. 0.001 = 0.1%
    pub slippage_rate: Option<f64>,
    /// Maximum fraction of bar volume to fill (e.g. 0.05 = 5%). Default: 5%.
    #[serde(default)]
    pub max_volume_participation: Option<f64>,
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
    /// Allow short selling (sell signals with no position open a short).
    #[serde(default)]
    pub allow_short_selling: Option<bool>,
    /// Margin multiplier for buying power (e.g. 2.0 = 2x leverage). Default: 1.0 (no margin).
    #[serde(default)]
    pub margin_multiplier: Option<f64>,
    /// Signal timeframe: "daily" (default) or "weekly". Weekly aggregates bars before signal generation.
    #[serde(default)]
    pub signal_timeframe: Option<String>,
    /// Trailing stop as a fraction (e.g. 0.05 = 5%). Replaces fixed stop-loss when set.
    #[serde(default)]
    pub trailing_stop_percent: Option<f64>,
    /// Halt trading if portfolio drawdown exceeds this percent (e.g. 20.0 = 20%).
    #[serde(default)]
    pub max_drawdown_halt_percent: Option<f64>,
    /// Regime-based position sizing configuration.
    #[serde(default)]
    pub regime_config: Option<RegimeConfig>,
    /// Tiered commission model (overrides flat commission_rate when set).
    #[serde(default)]
    pub commission_model: Option<CommissionModel>,
    /// Allow fractional shares (default: false, whole shares only).
    #[serde(default)]
    pub allow_fractional_shares: Option<bool>,
    /// Annual cash sweep rate (e.g. 0.04 = 4% APY on idle cash).
    #[serde(default)]
    pub cash_sweep_rate: Option<f64>,
    /// Use incremental rebalancing (only adjust positions >5% off target).
    #[serde(default)]
    pub incremental_rebalance: Option<bool>,
    /// Parameter search space for walk-forward optimization.
    #[serde(default)]
    pub param_search_space: Option<ParamSearchSpace>,
    /// Market impact modeling configuration.
    #[serde(default)]
    pub market_impact: Option<MarketImpactConfig>,
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
    /// Order type for this signal (default: Market).
    #[serde(default)]
    pub order_type: Option<OrderType>,
    /// Limit price for limit orders.
    #[serde(default)]
    pub limit_price: Option<Decimal>,
    /// Number of bars before limit order expires.
    #[serde(default)]
    pub limit_expiry_bars: Option<i32>,
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
    /// Compound Annual Growth Rate (CAGR).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annualized_return_percent: Option<f64>,
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
    /// Number of short trades executed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_trades: Option<i32>,
    /// Peak margin utilization during backtest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_used_peak: Option<f64>,
    /// Data quality report (gaps, corporate actions, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_quality_report: Option<DataQualityReport>,
    /// Bootstrap confidence intervals on key metrics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_intervals: Option<ConfidenceIntervals>,
    /// Extended performance metrics (Treynor, Omega, drawdown events, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended_metrics: Option<ExtendedMetrics>,
    /// CAPM factor attribution (beta, alpha, R-squared).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor_attribution: Option<FactorAttribution>,
    /// Structured tear sheet combining all analytics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tear_sheet: Option<serde_json::Value>,
    /// Advanced analytics (expectancy, streaks, overfitting, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advanced_analytics: Option<AdvancedAnalytics>,
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
    pub confidence: f64,
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
    /// Trade direction: "long" or "short".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
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

// =============================================================================
// New types for the professional upgrade
// =============================================================================

/// Order type for signals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    #[default]
    Market,
    Limit,
    StopLimit,
}

/// Tiered commission model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommissionModel {
    /// Minimum commission per trade.
    pub min_per_trade: f64,
    /// Maximum commission per trade (0 = no max).
    #[serde(default)]
    pub max_per_trade: f64,
    /// Tiered rates by monthly volume.
    pub tiers: Vec<CommissionTier>,
}

/// A single commission tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommissionTier {
    /// Monthly volume threshold (shares). Orders above this use this rate.
    pub volume_threshold: f64,
    /// Per-share rate for this tier.
    pub per_share_rate: f64,
}

/// Regime-based position sizing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeConfig {
    /// Lookback window for regime detection (in bars).
    #[serde(default = "default_regime_lookback")]
    pub lookback_bars: usize,
    /// Position size multiplier during high-volatility regimes.
    #[serde(default = "default_high_vol_multiplier")]
    pub high_vol_multiplier: f64,
    /// Position size multiplier during low-volatility regimes.
    #[serde(default = "default_low_vol_multiplier")]
    pub low_vol_multiplier: f64,
    /// Annualized volatility threshold for "high vol" classification.
    #[serde(default = "default_high_vol_threshold")]
    pub high_vol_threshold: f64,
    /// Annualized volatility threshold for "low vol" classification (below this).
    #[serde(default = "default_low_vol_threshold")]
    pub low_vol_threshold: f64,
}

fn default_regime_lookback() -> usize {
    60
}
fn default_high_vol_multiplier() -> f64 {
    0.5
}
fn default_low_vol_multiplier() -> f64 {
    1.5
}
fn default_high_vol_threshold() -> f64 {
    0.30
}
fn default_low_vol_threshold() -> f64 {
    0.15
}

impl Default for RegimeConfig {
    fn default() -> Self {
        Self {
            lookback_bars: 60,
            high_vol_multiplier: 0.5,
            low_vol_multiplier: 1.5,
            high_vol_threshold: 0.30,
            low_vol_threshold: 0.15,
        }
    }
}

/// Data quality report for the backtest input data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityReport {
    pub total_bars: usize,
    pub missing_dates: usize,
    pub zero_volume_bars: usize,
    pub price_spike_count: usize,
    pub warnings: Vec<DataWarning>,
    pub corporate_events: Vec<CorporateEvent>,
    pub market_events: Vec<MarketEvent>,
}

/// A data quality warning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataWarning {
    pub date: String,
    pub symbol: String,
    pub warning_type: String,
    pub message: String,
}

/// Detected corporate event (stock split, dividend, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorporateEvent {
    pub date: String,
    pub symbol: String,
    pub event_type: String,
    pub magnitude: f64,
}

/// Detected market event (circuit breaker, extreme volatility).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub date: String,
    pub event_type: String,
    pub magnitude: f64,
}

/// Bootstrap confidence intervals on key metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceIntervals {
    pub sharpe_ci_lower: f64,
    pub sharpe_ci_upper: f64,
    pub win_rate_ci_lower: f64,
    pub win_rate_ci_upper: f64,
    pub profit_factor_ci_lower: f64,
    pub profit_factor_ci_upper: f64,
    pub bootstrap_samples: i32,
}

/// Hypothesis correction results for multiple-testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypothesisCorrectionResult {
    pub raw_p_value: f64,
    pub bonferroni_p_value: f64,
    pub bh_p_value: f64,
    pub num_tests: i32,
    pub is_significant_bonferroni: bool,
    pub is_significant_bh: bool,
}

/// Extended performance metrics beyond standard Sharpe/Sortino.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedMetrics {
    pub treynor_ratio: Option<f64>,
    pub jensens_alpha: Option<f64>,
    pub omega_ratio: Option<f64>,
    pub tail_ratio: Option<f64>,
    pub skewness: Option<f64>,
    pub kurtosis: Option<f64>,
    pub top_drawdown_events: Vec<DrawdownEvent>,
    pub monthly_returns: Vec<MonthlyReturn>,
    pub rolling_sharpe: Vec<RollingSharpePoint>,
    pub max_drawdown_duration_days: Option<i64>,
    // Advanced risk metrics (new)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdar_95: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ulcer_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pain_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_to_pain_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burke_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sterling_ratio: Option<f64>,
}

/// A significant drawdown event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownEvent {
    pub start_date: String,
    pub trough_date: String,
    pub recovery_date: Option<String>,
    pub drawdown_percent: f64,
    pub duration_days: i64,
    pub recovery_days: Option<i64>,
}

/// Monthly return breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyReturn {
    pub year: i32,
    pub month: i32,
    pub return_percent: f64,
}

/// A point on the rolling Sharpe curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingSharpePoint {
    pub date: String,
    pub sharpe: f64,
}

/// CAPM factor attribution results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactorAttribution {
    pub beta: f64,
    pub alpha_annualized: f64,
    pub r_squared: f64,
    pub tracking_error: f64,
    pub residual_risk: f64,
}

/// Parameter search space for walk-forward optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSearchSpace {
    #[serde(default)]
    pub confidence_thresholds: Vec<f64>,
    #[serde(default)]
    pub position_size_percents: Vec<f64>,
    #[serde(default)]
    pub stop_loss_percents: Vec<f64>,
    #[serde(default)]
    pub take_profit_percents: Vec<f64>,
}

/// Walk-forward optimization result (with parameter tuning).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedWalkForwardResult {
    pub walk_forward: WalkForwardResult,
    pub optimized_params: Vec<OptimizedParams>,
    pub best_params: OptimizedParams,
}

/// Optimized parameters for a walk-forward fold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedParams {
    pub fold_number: i32,
    pub confidence_threshold: f64,
    pub position_size_percent: f64,
    pub stop_loss_percent: Option<f64>,
    pub take_profit_percent: Option<f64>,
    pub in_sample_sharpe: Option<f64>,
}

/// Enhanced Monte Carlo configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloConfig {
    /// Number of simulations to run.
    #[serde(default = "default_mc_simulations")]
    pub num_simulations: i32,
    /// Block size for block bootstrap (1 = standard shuffle).
    #[serde(default = "default_block_size")]
    pub block_size: usize,
    /// Whether to add parameter uncertainty (vary commission/slippage +/-20%).
    #[serde(default)]
    pub parameter_uncertainty: bool,
}

fn default_mc_simulations() -> i32 {
    1000
}
fn default_block_size() -> usize {
    1
}

/// Combinatorially Purged Cross-Validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpcvResult {
    pub num_combinations: i32,
    pub mean_oos_sharpe: f64,
    pub std_oos_sharpe: f64,
    pub probability_of_loss: f64,
    pub deflated_sharpe: Option<f64>,
}

/// A pending limit order being tracked in the engine.
#[derive(Debug, Clone)]
pub struct PendingLimitOrder {
    pub signal: Signal,
    pub bars_remaining: i32,
    pub direction: String,
}

// =============================================================================
// New types for institutional-grade backtesting (2026-02-11)
// =============================================================================

/// Advanced backtest analytics combining all new metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedAnalytics {
    /// Trade expectancy analysis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expectancy: Option<ExpectancyAnalysis>,
    /// Win/loss streak distribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaks: Option<StreakDistribution>,
    /// Payoff ratios by holding period bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regime_payoffs: Option<Vec<RegimePayoff>>,
    /// Time-in-market metrics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_in_market: Option<TimeInMarketAnalysis>,
    /// Drawdown recovery analysis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawdown_recovery: Option<DrawdownRecoveryStats>,
    /// Overfitting detection (PBO, deflated Sharpe).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overfitting_analysis: Option<OverfittingAnalysis>,
}

/// Trade expectancy metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectancyAnalysis {
    pub expectancy: f64,
    pub expectancy_percent: f64,
    pub kelly_fraction: f64,
    pub payoff_ratio: f64,
    pub sqn: f64,
}

/// Win/loss streak distribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreakDistribution {
    pub max_win_streak: i32,
    pub max_loss_streak: i32,
    pub avg_win_streak: f64,
    pub avg_loss_streak: f64,
    pub prob_win_after_win: f64,
    pub prob_win_after_loss: f64,
}

/// Payoff analysis by regime (holding period buckets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimePayoff {
    pub regime_name: String,
    pub num_trades: i32,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub payoff_ratio: f64,
    pub expectancy: f64,
}

/// Time-in-market analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeInMarketAnalysis {
    pub time_in_market_percent: f64,
    pub avg_concurrent_positions: f64,
    pub max_concurrent_positions: i32,
    pub active_trading_days: i32,
    pub total_calendar_days: i32,
}

/// Drawdown recovery statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownRecoveryStats {
    pub avg_recovery_days: f64,
    pub max_recovery_days: i64,
    pub num_recovered: usize,
    pub num_ongoing: usize,
    pub time_in_drawdown_percent: f64,
}

/// Overfitting detection analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverfittingAnalysis {
    /// Deflated Sharpe ratio (adjusted for multiple testing).
    pub deflated_sharpe: f64,
    /// p-value for observed Sharpe.
    pub sharpe_p_value: f64,
    /// Expected max Sharpe under null hypothesis.
    pub expected_max_sharpe_null: f64,
    /// Recommended minimum backtest length.
    pub min_backtest_length: i32,
}

/// Configuration for market impact modeling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketImpactConfig {
    /// Enable market impact modeling.
    #[serde(default)]
    pub enabled: bool,
    /// Market impact coefficient (gamma). Default: 0.2.
    #[serde(default = "default_gamma")]
    pub gamma: f64,
    /// Lookback period for ADV calculation (trading days). Default: 20.
    #[serde(default = "default_adv_lookback")]
    pub adv_lookback_days: usize,
    /// Lookback period for volatility calculation (trading days). Default: 20.
    #[serde(default = "default_vol_lookback")]
    pub vol_lookback_days: usize,
    /// Permanent vs temporary impact split (0.0-1.0). Default: 0.6.
    #[serde(default = "default_permanent_fraction")]
    pub permanent_impact_fraction: f64,
}

fn default_gamma() -> f64 {
    0.2
}
fn default_adv_lookback() -> usize {
    20
}
fn default_vol_lookback() -> usize {
    20
}
fn default_permanent_fraction() -> f64 {
    0.6
}

impl Default for MarketImpactConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gamma: 0.2,
            adv_lookback_days: 20,
            vol_lookback_days: 20,
            permanent_impact_fraction: 0.6,
        }
    }
}
