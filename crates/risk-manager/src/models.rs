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

fn default_daily_loss_limit() -> f64 { 5.0 }
fn default_max_consecutive_losses() -> i32 { 3 }
fn default_drawdown_limit() -> f64 { 10.0 }

impl Default for RiskParameters {
    fn default() -> Self {
        Self {
            id: None,
            max_risk_per_trade_percent: 2.0,
            max_portfolio_risk_percent: 10.0,
            max_position_size_percent: 20.0,
            default_stop_loss_percent: 5.0,
            default_take_profit_percent: 10.0,
            trailing_stop_enabled: false,
            trailing_stop_percent: 3.0,
            min_confidence_threshold: 0.70,
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
    pub recommended_shares: f64,
    pub position_value: f64,
    pub risk_amount: f64,
    pub stop_loss_price: f64,
    pub take_profit_price: f64,
    pub position_size_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ActiveRiskPosition {
    pub id: Option<i64>,
    pub symbol: String,
    pub shares: f64,
    pub entry_price: f64,
    pub entry_date: String,
    pub stop_loss_price: Option<f64>,
    pub take_profit_price: Option<f64>,
    pub trailing_stop_enabled: bool,
    pub trailing_stop_percent: Option<f64>,
    pub max_price_seen: Option<f64>,
    pub risk_amount: Option<f64>,
    pub position_size_percent: Option<f64>,
    pub status: String,
    pub created_at: Option<String>,
    pub closed_at: Option<String>,
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
    pub current_price: f64,
    pub stop_loss_price: f64,
    pub should_exit: bool,
    pub loss_amount: f64,
    pub loss_percent: f64,
}
