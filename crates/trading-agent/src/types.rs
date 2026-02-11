use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub symbol: String,
    pub action: String, // "BUY" or "SELL"
    pub confidence: f64,
    pub strategy_name: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub historical_win_rate: Option<f64>,
    pub technical_reason: String,
    pub fundamental_reason: Option<String>,
    pub sentiment_score: Option<f64>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TradeExecution {
    pub symbol: String,
    pub action: String,
    pub quantity: i32,
    pub price: f64,
    pub order_id: String,
}

#[derive(Debug, Clone)]
pub struct PositionAction {
    pub action_type: String, // "STOP_LOSS", "TAKE_PROFIT", "TRAILING_STOP"
    pub symbol: String,
    pub price: f64,
    pub pnl: f64,
}

#[derive(Debug, Clone)]
pub struct GateDecision {
    pub approved: bool,
    pub probability: f64,
    pub reasoning: String,
}
