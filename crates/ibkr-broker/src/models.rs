use serde::{Deserialize, Serialize};

/// IBKR Client Portal API account response
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct IbkrAccount {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "accountTitle")]
    pub account_title: Option<String>,
    #[serde(rename = "type")]
    pub account_type: Option<String>,
}

/// IBKR account ledger (balance info)
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct IbkrLedger {
    #[serde(rename = "cashbalance")]
    pub cash_balance: Option<f64>,
    #[serde(rename = "netliquidationvalue")]
    pub net_liquidation: Option<f64>,
    #[serde(rename = "buyingpower")]
    pub buying_power: Option<f64>,
    pub currency: Option<String>,
}

/// IBKR position from portfolio endpoint
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct IbkrPosition {
    #[serde(rename = "conid")]
    pub contract_id: i64,
    pub ticker: Option<String>,
    pub position: Option<f64>,
    #[serde(rename = "avgCost")]
    pub avg_cost: Option<f64>,
    #[serde(rename = "mktValue")]
    pub market_value: Option<f64>,
    #[serde(rename = "unrealizedPnl")]
    pub unrealized_pnl: Option<f64>,
    #[serde(rename = "realizedPnl")]
    pub realized_pnl: Option<f64>,
    pub currency: Option<String>,
}

/// IBKR order request
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct IbkrOrderRequest {
    #[serde(rename = "conid")]
    pub contract_id: i64,
    #[serde(rename = "orderType")]
    pub order_type: String,
    pub side: String, // "BUY" or "SELL"
    pub quantity: f64,
    pub tif: String, // "DAY", "GTC"
}

/// IBKR order response
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct IbkrOrderResponse {
    pub order_id: Option<String>,
    pub order_status: Option<String>,
}

/// IBKR order status
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct IbkrOrder {
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub ticker: Option<String>,
    pub side: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "orderType")]
    pub order_type: Option<String>,
    #[serde(rename = "filledQuantity")]
    pub filled_qty: Option<f64>,
    #[serde(rename = "remainingQuantity")]
    pub remaining_qty: Option<f64>,
    #[serde(rename = "avgPrice")]
    pub avg_price: Option<f64>,
    #[serde(rename = "totalQuantity")]
    pub total_qty: Option<f64>,
}

/// Contract search result (for symbol -> conid resolution)
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct IbkrContract {
    #[serde(rename = "conid")]
    pub contract_id: i64,
    #[serde(rename = "companyName")]
    pub company_name: Option<String>,
    pub symbol: Option<String>,
}
