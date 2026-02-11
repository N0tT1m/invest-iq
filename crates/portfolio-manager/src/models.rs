use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;

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
