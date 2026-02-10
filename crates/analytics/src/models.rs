use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StrategyPerformance {
    pub id: Option<i64>,
    pub strategy_name: String,
    pub symbol: Option<String>,
    pub total_signals: i32,
    pub signals_taken: i32,
    pub signals_ignored: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub total_profit_loss: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SignalQuality {
    pub id: Option<i64>,
    pub signal_type: String,
    pub confidence_range: String,
    pub total_signals: i32,
    pub signals_taken: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub actual_win_rate: f64,
    pub avg_return: f64,
    pub calibration_error: f64,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceOverview {
    pub total_strategies: i32,
    pub total_trades: i32,
    pub overall_win_rate: f64,
    pub overall_profit_factor: f64,
    pub total_profit_loss: f64,
    pub best_strategy: Option<StrategyPerformance>,
    pub worst_strategy: Option<StrategyPerformance>,
    pub strategies: Vec<StrategyPerformance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalQualityReport {
    pub total_signal_types: i32,
    pub avg_calibration_error: f64,
    pub best_signals: Vec<SignalQuality>,
    pub worst_signals: Vec<SignalQuality>,
    pub all_signals: Vec<SignalQuality>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeOutcome {
    pub trade_id: i64,
    pub symbol: String,
    pub signal_type: String,
    pub predicted_confidence: f64,
    pub actual_outcome: bool, // true = win, false = loss
    pub profit_loss: f64,
    pub profit_loss_percent: f64,
}
