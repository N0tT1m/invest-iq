use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RiskParameters {
    pub id: Option<i64>,
    pub max_risk_per_trade_percent: f64,
    pub max_portfolio_risk_percent: f64,
    pub max_position_size_percent: f64,
    pub default_stop_loss_percent: f64,
    pub default_take_profit_percent: f64,
    pub trailing_stop_enabled: bool,
    pub trailing_stop_percent: f64,
    pub min_confidence_threshold: f64,
    pub min_win_rate_threshold: f64,
    /// Maximum daily loss as percentage of portfolio before halting (default 5%)
    #[serde(default = "default_daily_loss_limit")]
    #[sqlx(default)]
    pub daily_loss_limit_percent: f64,
    /// Maximum consecutive losing trades before halting (default 3)
    #[serde(default = "default_max_consecutive_losses")]
    #[sqlx(default)]
    pub max_consecutive_losses: i32,
    /// Maximum drawdown from peak as percentage before halting (default 10%)
    #[serde(default = "default_drawdown_limit")]
    #[sqlx(default)]
    pub account_drawdown_limit_percent: f64,
    /// Whether trading is manually halted
    #[serde(default)]
    #[sqlx(default)]
    pub trading_halted: bool,
    /// Reason for manual halt
    #[serde(default)]
    #[sqlx(default)]
    pub halt_reason: Option<String>,
    /// When trading was halted
    #[serde(default)]
    #[sqlx(default)]
    pub halted_at: Option<String>,
    pub updated_at: Option<String>,
}

fn default_daily_loss_limit() -> f64 {
    5.0
}
fn default_max_consecutive_losses() -> i32 {
    3
}
fn default_drawdown_limit() -> f64 {
    10.0
}

impl Default for RiskParameters {
    fn default() -> Self {
        Self {
            id: None,
            max_risk_per_trade_percent: 2.0,
            max_portfolio_risk_percent: 80.0,
            max_position_size_percent: 20.0,
            default_stop_loss_percent: 5.0,
            default_take_profit_percent: 10.0,
            trailing_stop_enabled: false,
            trailing_stop_percent: 3.0,
            min_confidence_threshold: 0.55,
            min_win_rate_threshold: 0.55,
            daily_loss_limit_percent: 5.0,
            max_consecutive_losses: 3,
            account_drawdown_limit_percent: 10.0,
            trading_halted: false,
            halt_reason: None,
            halted_at: None,
            updated_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSizeCalculation {
    pub recommended_shares: Decimal,
    pub position_value: Decimal,
    pub risk_amount: Decimal,
    pub stop_loss_price: Decimal,
    pub take_profit_price: Decimal,
    pub position_size_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveRiskPosition {
    pub id: Option<i64>,
    pub symbol: String,
    pub shares: Decimal,
    pub entry_price: Decimal,
    pub entry_date: String,
    pub stop_loss_price: Option<Decimal>,
    pub take_profit_price: Option<Decimal>,
    pub trailing_stop_enabled: bool,
    pub trailing_stop_percent: Option<f64>,
    pub max_price_seen: Option<Decimal>,
    pub risk_amount: Option<Decimal>,
    pub position_size_percent: Option<f64>,
    pub status: String,
    pub created_at: Option<String>,
    pub closed_at: Option<String>,
}

// Helper struct for reading from DB (f64 values)
#[derive(sqlx::FromRow)]
pub(crate) struct ActiveRiskPositionRow {
    id: Option<i64>,
    symbol: String,
    shares: f64,
    entry_price: f64,
    entry_date: String,
    stop_loss_price: Option<f64>,
    take_profit_price: Option<f64>,
    trailing_stop_enabled: bool,
    trailing_stop_percent: Option<f64>,
    max_price_seen: Option<f64>,
    risk_amount: Option<f64>,
    position_size_percent: Option<f64>,
    status: String,
    created_at: Option<String>,
    closed_at: Option<String>,
}

impl From<ActiveRiskPositionRow> for ActiveRiskPosition {
    fn from(row: ActiveRiskPositionRow) -> Self {
        Self {
            id: row.id,
            symbol: row.symbol,
            shares: Decimal::from_f64(row.shares).unwrap_or_default(),
            entry_price: Decimal::from_f64(row.entry_price).unwrap_or_default(),
            entry_date: row.entry_date,
            stop_loss_price: row.stop_loss_price.and_then(Decimal::from_f64),
            take_profit_price: row.take_profit_price.and_then(Decimal::from_f64),
            trailing_stop_enabled: row.trailing_stop_enabled,
            trailing_stop_percent: row.trailing_stop_percent,
            max_price_seen: row.max_price_seen.and_then(Decimal::from_f64),
            risk_amount: row.risk_amount.and_then(Decimal::from_f64),
            position_size_percent: row.position_size_percent,
            status: row.status,
            created_at: row.created_at,
            closed_at: row.closed_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskCheck {
    pub can_trade: bool,
    pub reason: String,
    pub current_portfolio_risk: f64,
    pub position_count: i32,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerCheck {
    pub can_trade: bool,
    pub reason: String,
    pub daily_pl_percent: f64,
    pub consecutive_losses: i32,
    pub drawdown_percent: f64,
    pub breakers_triggered: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopLossAlert {
    pub symbol: String,
    pub current_price: Decimal,
    pub stop_loss_price: Decimal,
    pub should_exit: bool,
    pub loss_amount: Decimal,
    pub loss_percent: f64,
}
