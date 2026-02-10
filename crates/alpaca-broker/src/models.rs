use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub account_number: String,
    pub status: String,
    pub currency: String,
    pub buying_power: String,
    pub cash: String,
    pub portfolio_value: String,
    pub pattern_day_trader: bool,
    pub trading_blocked: bool,
    pub transfers_blocked: bool,
    pub account_blocked: bool,
    pub daytrade_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    #[serde(rename = "stop_limit")]
    StopLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeInForce {
    Day,
    Gtc, // Good til canceled
    Opg, // Market on open
    Cls, // Market on close
    Ioc, // Immediate or cancel
    Fok, // Fill or kill
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    New,
    #[serde(rename = "partially_filled")]
    PartiallyFilled,
    Filled,
    #[serde(rename = "done_for_day")]
    DoneForDay,
    Canceled,
    Expired,
    Replaced,
    #[serde(rename = "pending_cancel")]
    PendingCancel,
    #[serde(rename = "pending_replace")]
    PendingReplace,
    Accepted,
    #[serde(rename = "pending_new")]
    PendingNew,
    #[serde(rename = "accepted_for_bidding")]
    AcceptedForBidding,
    Stopped,
    Rejected,
    Suspended,
    Calculated,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qty: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notional: Option<String>, // Dollar amount instead of quantity
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Order {
    pub id: String,
    pub client_order_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub expired_at: Option<DateTime<Utc>>,
    pub canceled_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub replaced_at: Option<DateTime<Utc>>,
    pub replaced_by: Option<String>,
    pub replaces: Option<String>,
    pub asset_id: String,
    pub symbol: String,
    pub asset_class: String,
    #[serde(rename = "qty")]
    pub quantity: Option<String>,
    pub notional: Option<String>,
    #[serde(rename = "filled_qty")]
    pub filled_quantity: Option<String>,
    pub filled_avg_price: Option<String>,
    pub order_type: String,
    pub side: String,
    pub time_in_force: String,
    pub limit_price: Option<String>,
    pub stop_price: Option<String>,
    pub status: String,
    pub extended_hours: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Position {
    pub asset_id: String,
    pub symbol: String,
    pub exchange: String,
    pub asset_class: String,
    pub avg_entry_price: String,
    pub qty: String,
    pub side: String,
    pub market_value: String,
    pub cost_basis: String,
    pub unrealized_pl: String,
    pub unrealized_plpc: String,
    pub unrealized_intraday_pl: String,
    pub unrealized_intraday_plpc: String,
    pub current_price: String,
    pub lastday_price: String,
    pub change_today: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarketOrderRequest {
    pub symbol: String,
    pub qty: f64,
    pub side: OrderSide,
}

impl MarketOrderRequest {
    pub fn buy(symbol: impl Into<String>, qty: f64) -> Self {
        Self {
            symbol: symbol.into(),
            qty,
            side: OrderSide::Buy,
        }
    }

    pub fn sell(symbol: impl Into<String>, qty: f64) -> Self {
        Self {
            symbol: symbol.into(),
            qty,
            side: OrderSide::Sell,
        }
    }

    pub fn to_order_request(self) -> OrderRequest {
        OrderRequest {
            symbol: self.symbol,
            qty: Some(self.qty.to_string()),
            notional: None,
            side: self.side,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Day,
            limit_price: None,
            stop_price: None,
            client_order_id: None,
        }
    }
}
