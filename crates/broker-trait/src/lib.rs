use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Unified broker types (broker-agnostic)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerAccount {
    pub id: String,
    pub account_number: String,
    pub status: String,
    pub currency: String,
    pub buying_power: String,
    pub cash: String,
    pub portfolio_value: String,
    pub pattern_day_trader: bool,
    pub trading_blocked: bool,
    pub daytrade_count: i32,
}

impl BrokerAccount {
    pub fn buying_power_decimal(&self) -> Decimal {
        Decimal::from_str(&self.buying_power).unwrap_or_default()
    }
    pub fn cash_decimal(&self) -> Decimal {
        Decimal::from_str(&self.cash).unwrap_or_default()
    }
    pub fn portfolio_value_decimal(&self) -> Decimal {
        Decimal::from_str(&self.portfolio_value).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerPosition {
    pub symbol: String,
    pub qty: String,
    pub side: String,
    pub avg_entry_price: String,
    pub market_value: String,
    pub cost_basis: String,
    pub unrealized_pl: String,
    pub unrealized_plpc: String,
    pub unrealized_intraday_pl: String,
    pub current_price: String,
    pub lastday_price: String,
    pub change_today: String,
}

impl BrokerPosition {
    pub fn avg_entry_price_decimal(&self) -> Decimal {
        Decimal::from_str(&self.avg_entry_price).unwrap_or_default()
    }
    pub fn qty_decimal(&self) -> Decimal {
        Decimal::from_str(&self.qty).unwrap_or_default()
    }
    pub fn market_value_decimal(&self) -> Decimal {
        Decimal::from_str(&self.market_value).unwrap_or_default()
    }
    pub fn cost_basis_decimal(&self) -> Decimal {
        Decimal::from_str(&self.cost_basis).unwrap_or_default()
    }
    pub fn unrealized_pl_decimal(&self) -> Decimal {
        Decimal::from_str(&self.unrealized_pl).unwrap_or_default()
    }
    pub fn current_price_decimal(&self) -> Decimal {
        Decimal::from_str(&self.current_price).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrokerOrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerOrderRequest {
    pub symbol: String,
    pub qty: Decimal,
    pub side: BrokerOrderSide,
}

impl BrokerOrderRequest {
    pub fn buy(symbol: impl Into<String>, qty: Decimal) -> Self {
        Self {
            symbol: symbol.into(),
            qty,
            side: BrokerOrderSide::Buy,
        }
    }
    pub fn sell(symbol: impl Into<String>, qty: Decimal) -> Self {
        Self {
            symbol: symbol.into(),
            qty,
            side: BrokerOrderSide::Sell,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerOrder {
    pub id: String,
    pub client_order_id: String,
    pub created_at: DateTime<Utc>,
    pub filled_at: Option<DateTime<Utc>>,
    pub symbol: String,
    pub qty: Option<String>,
    pub filled_qty: Option<String>,
    pub filled_avg_price: Option<String>,
    pub order_type: String,
    pub side: String,
    pub status: String,
}

impl BrokerOrder {
    pub fn filled_quantity_decimal(&self) -> Option<Decimal> {
        self.filled_qty
            .as_ref()
            .and_then(|s| Decimal::from_str(s).ok())
    }
    pub fn filled_avg_price_decimal(&self) -> Option<Decimal> {
        self.filled_avg_price
            .as_ref()
            .and_then(|s| Decimal::from_str(s).ok())
    }
}

// ---------------------------------------------------------------------------
// Broker trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait BrokerClient: Send + Sync {
    /// Get account information (balance, buying power, etc.)
    async fn get_account(&self) -> Result<BrokerAccount>;

    /// Get all open positions
    async fn get_positions(&self) -> Result<Vec<BrokerPosition>>;

    /// Get a specific position by symbol (None if no position)
    async fn get_position(&self, symbol: &str) -> Result<Option<BrokerPosition>>;

    /// Submit a market order
    async fn submit_market_order(&self, order: BrokerOrderRequest) -> Result<BrokerOrder>;

    /// Get an order by ID
    async fn get_order(&self, order_id: &str) -> Result<BrokerOrder>;

    /// Get recent orders
    async fn get_orders(&self, limit: Option<usize>) -> Result<Vec<BrokerOrder>>;

    /// Cancel an order by ID
    async fn cancel_order(&self, order_id: &str) -> Result<()>;

    /// Close an entire position
    async fn close_position(&self, symbol: &str) -> Result<BrokerOrder>;

    /// Whether this is a paper/simulated account
    fn is_paper(&self) -> bool;

    /// Broker name for logging
    fn broker_name(&self) -> &str;
}
