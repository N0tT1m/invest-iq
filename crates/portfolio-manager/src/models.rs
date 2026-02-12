use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Position {
    pub id: Option<i64>,
    pub symbol: String,
    #[sqlx(try_from = "f64")]
    pub shares: Decimal,
    #[sqlx(try_from = "f64")]
    pub entry_price: Decimal,
    pub entry_date: String,
    pub notes: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionWithPnL {
    #[serde(flatten)]
    pub position: Position,
    pub current_price: Decimal,
    pub market_value: Decimal,
    pub cost_basis: Decimal,
    pub unrealized_pnl: Decimal,
    pub unrealized_pnl_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Trade {
    pub id: Option<i64>,
    pub symbol: String,
    pub action: String, // "buy" or "sell"
    #[sqlx(try_from = "f64")]
    pub shares: Decimal,
    #[sqlx(try_from = "f64")]
    pub price: Decimal,
    pub trade_date: String,
    #[sqlx(try_from = "f64")]
    pub commission: Decimal,
    pub notes: Option<String>,
    pub profit_loss: Option<f64>,
    pub profit_loss_percent: Option<f64>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeInput {
    pub symbol: String,
    pub action: String,
    pub shares: Decimal,
    pub price: Decimal,
    pub trade_date: String,
    pub commission: Option<Decimal>,
    pub notes: Option<String>,
    #[serde(default)]
    pub alert_id: Option<i64>,
    #[serde(default)]
    pub analysis_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Alert {
    pub id: Option<i64>,
    pub symbol: String,
    pub alert_type: String,
    pub signal: String,
    pub confidence: f64,
    pub current_price: Option<f64>,
    pub target_price: Option<f64>,
    pub stop_loss_price: Option<f64>,
    pub reason: Option<String>,
    pub status: String,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertInput {
    pub symbol: String,
    pub alert_type: String,
    pub signal: String,
    pub confidence: f64,
    pub current_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub stop_loss_price: Option<Decimal>,
    pub reason: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WatchlistItem {
    pub id: Option<i64>,
    pub symbol: String,
    pub notes: Option<String>,
    pub added_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PortfolioSnapshot {
    pub id: Option<i64>,
    #[sqlx(try_from = "f64")]
    pub total_value: Decimal,
    #[sqlx(try_from = "f64")]
    pub total_cost: Decimal,
    #[sqlx(try_from = "f64")]
    pub total_pnl: Decimal,
    pub total_pnl_percent: f64,
    pub snapshot_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub total_positions: usize,
    pub total_value: Decimal,
    pub total_cost: Decimal,
    pub total_pnl: Decimal,
    pub total_pnl_percent: f64,
    pub positions: Vec<PositionWithPnL>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub total_realized_pnl: Decimal,
    pub average_win: Decimal,
    pub average_loss: Decimal,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
    pub recent_trades: Vec<Trade>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub priority: u8, // 1 = highest
    pub action_type: String,
    pub symbol: String,
    pub title: String,
    pub description: String,
    pub signal: String,
    pub confidence: f64,
    pub current_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub stop_loss_price: Option<Decimal>,
    pub in_portfolio: bool,
    pub position_pnl: Option<Decimal>,
    pub alert_id: Option<i64>,
}

// ============================================================
// New analytics models
// ============================================================

/// Cost basis calculation method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CostBasisMethod {
    #[default]
    Fifo,
    Lifo,
    AverageCost,
}

/// Portfolio-level risk metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRiskMetrics {
    pub sharpe_ratio: Option<f64>,
    pub sortino_ratio: Option<f64>,
    pub max_drawdown_percent: f64,
    pub current_drawdown_percent: f64,
    pub rolling_volatility_20d: Option<f64>,
    pub rolling_volatility_63d: Option<f64>,
    pub var_95: Option<f64>,
    pub cvar_95: Option<f64>,
    pub herfindahl_index: f64,
    pub top_holdings: Vec<HoldingWeight>,
    pub sector_weights: HashMap<String, f64>,
    pub data_points: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldingWeight {
    pub symbol: String,
    pub weight_percent: f64,
    pub market_value: f64,
}

/// Performance analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalytics {
    pub total_return_percent: f64,
    pub twr_percent: Option<f64>,
    pub rolling_30d_return: Option<f64>,
    pub rolling_90d_return: Option<f64>,
    pub ytd_return: Option<f64>,
    pub rolling_1y_return: Option<f64>,
    pub monthly_returns: Vec<MonthlyReturn>,
    pub symbol_attribution: Vec<SymbolAttribution>,
    pub data_points: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyReturn {
    pub year: i32,
    pub month: u32,
    pub return_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolAttribution {
    pub symbol: String,
    pub weight_percent: f64,
    pub return_percent: f64,
    pub contribution_percent: f64,
}

/// Enhanced trade performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedPerformanceMetrics {
    #[serde(flatten)]
    pub base: PerformanceMetrics,
    pub cost_basis_method: String,
    pub expectancy: f64,
    pub profit_factor: Option<f64>,
    pub payoff_ratio: Option<f64>,
    pub avg_holding_days: Option<f64>,
    pub median_holding_days: Option<f64>,
    pub holding_distribution: HoldingDistribution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldingDistribution {
    pub under_7d: usize,
    pub d7_to_30: usize,
    pub d30_to_90: usize,
    pub over_90d: usize,
}

/// Alert execution tracking
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AlertExecution {
    pub id: Option<i64>,
    pub alert_id: i64,
    pub trade_id: Option<i64>,
    pub symbol: String,
    pub alert_signal: String,
    pub alert_confidence: f64,
    pub alert_price: Option<f64>,
    pub execution_price: Option<f64>,
    pub outcome: String,
    pub outcome_pnl: Option<f64>,
    pub outcome_pnl_percent: Option<f64>,
    pub executed_at: Option<String>,
    pub closed_at: Option<String>,
    pub created_at: Option<String>,
}

/// Alert accuracy report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertAccuracyReport {
    pub total_executions: usize,
    pub profitable: usize,
    pub unprofitable: usize,
    pub still_open: usize,
    pub accuracy_percent: f64,
    pub avg_pnl: f64,
    pub avg_pnl_percent: f64,
    pub by_signal: HashMap<String, SignalAccuracy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalAccuracy {
    pub total: usize,
    pub profitable: usize,
    pub accuracy_percent: f64,
    pub avg_pnl: f64,
}

/// Broker position (neutral, no alpaca-broker dependency)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerPosition {
    pub symbol: String,
    pub shares: Decimal,
    pub avg_entry_price: Decimal,
    pub market_value: Decimal,
    pub current_price: Decimal,
    pub unrealized_pnl: Decimal,
}

/// Reconciliation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationResult {
    pub reconciliation_date: String,
    pub total_positions: usize,
    pub matches: usize,
    pub discrepancies: Vec<Discrepancy>,
    pub auto_resolved: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discrepancy {
    pub symbol: String,
    pub discrepancy_type: String,
    pub local_shares: Option<f64>,
    pub broker_shares: Option<f64>,
    pub local_price: Option<f64>,
    pub broker_price: Option<f64>,
    pub resolved: bool,
    pub resolution: Option<String>,
}

/// CSV trade import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvTradeRow {
    pub symbol: String,
    pub action: String,
    pub shares: f64,
    pub price: f64,
    pub date: String,
    pub commission: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

/// Target allocation
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TargetAllocation {
    pub id: Option<i64>,
    pub symbol: Option<String>,
    pub sector: Option<String>,
    pub target_weight_percent: f64,
    pub drift_tolerance_percent: f64,
    pub updated_at: Option<String>,
}

/// Rebalance proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceProposal {
    pub total_portfolio_value: f64,
    pub trades: Vec<RebalanceTrade>,
    pub estimated_turnover_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceTrade {
    pub symbol: String,
    pub action: String,
    pub shares: f64,
    pub current_weight_percent: f64,
    pub target_weight_percent: f64,
    pub estimated_value: f64,
}

/// Drift entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEntry {
    pub symbol: Option<String>,
    pub sector: Option<String>,
    pub target_weight_percent: f64,
    pub current_weight_percent: f64,
    pub drift_percent: f64,
    pub tolerance_percent: f64,
    pub needs_rebalance: bool,
}

/// Tax integration types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxSummary {
    pub jurisdiction: String,
    pub short_term_gains: f64,
    pub short_term_losses: f64,
    pub long_term_gains: f64,
    pub long_term_losses: f64,
    pub net_short_term: f64,
    pub net_long_term: f64,
    pub estimated_tax: f64,
    pub lots: Vec<TaxLotSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxLotSummary {
    pub symbol: String,
    pub shares: f64,
    pub cost_basis: f64,
    pub current_value: f64,
    pub gain_loss: f64,
    pub holding_period: String,
    pub days_held: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxImpactEstimate {
    pub symbol: String,
    pub shares: f64,
    pub estimated_gain_loss: f64,
    pub gain_type: String,
    pub estimated_tax: f64,
    pub effective_rate: f64,
    pub wash_sale_risk: bool,
}

/// Benchmark comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkAnalysis {
    pub benchmark_symbol: String,
    pub alpha: f64,
    pub beta: f64,
    pub r_squared: f64,
    pub tracking_error: f64,
    pub information_ratio: Option<f64>,
    pub portfolio_return_percent: f64,
    pub benchmark_return_percent: f64,
    pub excess_return_percent: f64,
    pub portfolio_indexed: Vec<IndexedPoint>,
    pub benchmark_indexed: Vec<IndexedPoint>,
    pub rolling_alpha: Vec<RollingAlpha>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedPoint {
    pub date: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingAlpha {
    pub date: String,
    pub alpha: f64,
}

/// Reconciliation log entry
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReconciliationLogEntry {
    pub id: Option<i64>,
    pub reconciliation_date: String,
    pub total_positions: i32,
    pub matches: i32,
    pub discrepancies: i32,
    pub auto_resolved: i32,
    pub details_json: Option<String>,
    pub created_at: Option<String>,
}
