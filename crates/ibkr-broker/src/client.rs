use crate::models::*;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use broker_trait::{
    BrokerAccount, BrokerClient, BrokerOrder, BrokerOrderRequest, BrokerOrderSide, BrokerPosition,
};
use chrono::Utc;
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;

/// Interactive Brokers client using the Client Portal API.
///
/// Requires the IBKR Client Portal Gateway to be running locally.
/// See: https://www.interactivebrokers.com/en/trading/ib-api.php
pub struct IbkrClient {
    client: Client,
    /// Client Portal Gateway URL (default: https://localhost:5000)
    gateway_url: String,
    account_id: String,
    /// Cache: symbol -> contract ID
    conid_cache: std::sync::Mutex<HashMap<String, i64>>,
    is_paper: bool,
}

impl IbkrClient {
    pub fn new(gateway_url: String, account_id: String, is_paper: bool) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .danger_accept_invalid_certs(true) // IBKR gateway uses self-signed certs
            .build()?;

        Ok(Self {
            client,
            gateway_url,
            account_id,
            conid_cache: std::sync::Mutex::new(HashMap::new()),
            is_paper,
        })
    }

    pub fn from_env() -> Result<Self> {
        let gateway_url = std::env::var("IBKR_GATEWAY_URL")
            .unwrap_or_else(|_| "https://localhost:5000".to_string());
        let account_id =
            std::env::var("IBKR_ACCOUNT_ID").map_err(|_| anyhow!("IBKR_ACCOUNT_ID not set"))?;
        let is_paper = std::env::var("IBKR_PAPER")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true);

        Self::new(gateway_url, account_id, is_paper)
    }

    /// Resolve a stock symbol to an IBKR contract ID.
    async fn resolve_conid(&self, symbol: &str) -> Result<i64> {
        // Check cache first
        if let Ok(cache) = self.conid_cache.lock() {
            if let Some(&conid) = cache.get(symbol) {
                return Ok(conid);
            }
        }

        let url = format!("{}/v1/api/iserver/secdef/search", self.gateway_url);
        let body = serde_json::json!({ "symbol": symbol, "secType": "STK" });

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("IBKR contract search failed: {}", text));
        }

        let contracts: Vec<IbkrContract> = response.json().await?;
        let contract = contracts
            .into_iter()
            .find(|c| c.symbol.as_deref() == Some(symbol))
            .ok_or_else(|| anyhow!("No contract found for {}", symbol))?;

        let conid = contract.contract_id;

        // Cache it
        if let Ok(mut cache) = self.conid_cache.lock() {
            cache.insert(symbol.to_string(), conid);
        }

        Ok(conid)
    }
}

#[async_trait]
impl BrokerClient for IbkrClient {
    async fn get_account(&self) -> Result<BrokerAccount> {
        let url = format!(
            "{}/v1/api/portfolio/{}/ledger",
            self.gateway_url, self.account_id
        );
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("IBKR get account failed: {}", text));
        }

        let ledger: HashMap<String, IbkrLedger> = response.json().await?;
        let usd = ledger.get("USD").or_else(|| ledger.get("BASE"));

        let (cash, buying_power, portfolio_value) = if let Some(l) = usd {
            (
                l.cash_balance.unwrap_or(0.0),
                l.buying_power.unwrap_or(0.0),
                l.net_liquidation.unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        Ok(BrokerAccount {
            id: self.account_id.clone(),
            account_number: self.account_id.clone(),
            status: "active".to_string(),
            currency: "USD".to_string(),
            buying_power: buying_power.to_string(),
            cash: cash.to_string(),
            portfolio_value: portfolio_value.to_string(),
            pattern_day_trader: false,
            trading_blocked: false,
            daytrade_count: 0,
        })
    }

    async fn get_positions(&self) -> Result<Vec<BrokerPosition>> {
        let url = format!(
            "{}/v1/api/portfolio/{}/positions/0",
            self.gateway_url, self.account_id
        );
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("IBKR get positions failed: {}", text));
        }

        let positions: Vec<IbkrPosition> = response.json().await?;
        Ok(positions
            .into_iter()
            .map(|p| {
                let qty = p.position.unwrap_or(0.0);
                let avg_cost = p.avg_cost.unwrap_or(0.0);
                let mkt_val = p.market_value.unwrap_or(0.0);
                let cost_basis = qty.abs() * avg_cost;
                let unrealized = p.unrealized_pnl.unwrap_or(0.0);
                let unrealized_pct = if cost_basis > 0.0 {
                    unrealized / cost_basis * 100.0
                } else {
                    0.0
                };
                let current_price = if qty.abs() > 0.0 {
                    mkt_val / qty.abs()
                } else {
                    0.0
                };

                BrokerPosition {
                    symbol: p.ticker.unwrap_or_default(),
                    qty: qty.to_string(),
                    side: if qty >= 0.0 {
                        "long".to_string()
                    } else {
                        "short".to_string()
                    },
                    avg_entry_price: avg_cost.to_string(),
                    market_value: mkt_val.to_string(),
                    cost_basis: cost_basis.to_string(),
                    unrealized_pl: unrealized.to_string(),
                    unrealized_plpc: unrealized_pct.to_string(),
                    unrealized_intraday_pl: "0".to_string(),
                    current_price: current_price.to_string(),
                    lastday_price: "0".to_string(),
                    change_today: "0".to_string(),
                }
            })
            .collect())
    }

    async fn get_position(&self, symbol: &str) -> Result<Option<BrokerPosition>> {
        let positions = self.get_positions().await?;
        Ok(positions.into_iter().find(|p| p.symbol == symbol))
    }

    async fn submit_market_order(&self, order: BrokerOrderRequest) -> Result<BrokerOrder> {
        let conid = self.resolve_conid(&order.symbol).await?;
        let side = match order.side {
            BrokerOrderSide::Buy => "BUY",
            BrokerOrderSide::Sell => "SELL",
        };

        let url = format!(
            "{}/v1/api/iserver/account/{}/orders",
            self.gateway_url, self.account_id
        );
        let body = serde_json::json!({
            "orders": [{
                "conid": conid,
                "orderType": "MKT",
                "side": side,
                "quantity": order.qty.to_string().parse::<f64>().unwrap_or(0.0),
                "tif": "DAY",
            }]
        });

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("IBKR order submission failed: {}", text));
        }

        let results: Vec<IbkrOrderResponse> = response.json().await?;
        let order_id = results
            .first()
            .and_then(|r| r.order_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(BrokerOrder {
            id: order_id.clone(),
            client_order_id: order_id,
            created_at: Utc::now(),
            filled_at: None,
            symbol: order.symbol,
            qty: Some(order.qty.to_string()),
            filled_qty: None,
            filled_avg_price: None,
            order_type: "market".to_string(),
            side: side.to_lowercase(),
            status: "submitted".to_string(),
        })
    }

    async fn get_order(&self, order_id: &str) -> Result<BrokerOrder> {
        let url = format!(
            "{}/v1/api/iserver/account/order/status/{}",
            self.gateway_url, order_id
        );
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("IBKR get order failed: {}", text));
        }

        let ibkr_order: IbkrOrder = response.json().await?;

        Ok(BrokerOrder {
            id: ibkr_order.order_id.clone(),
            client_order_id: ibkr_order.order_id,
            created_at: Utc::now(),
            filled_at: None,
            symbol: ibkr_order.ticker.unwrap_or_default(),
            qty: ibkr_order.total_qty.map(|q| q.to_string()),
            filled_qty: ibkr_order.filled_qty.map(|q| q.to_string()),
            filled_avg_price: ibkr_order.avg_price.map(|p| p.to_string()),
            order_type: ibkr_order
                .order_type
                .unwrap_or_else(|| "market".to_string())
                .to_lowercase(),
            side: ibkr_order.side.unwrap_or_default().to_lowercase(),
            status: ibkr_order
                .status
                .unwrap_or_else(|| "unknown".to_string())
                .to_lowercase(),
        })
    }

    async fn get_orders(&self, limit: Option<usize>) -> Result<Vec<BrokerOrder>> {
        let url = format!("{}/v1/api/iserver/account/orders", self.gateway_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("IBKR get orders failed: {}", text));
        }

        let body: serde_json::Value = response.json().await?;
        let orders_arr = body
            .get("orders")
            .and_then(|o| o.as_array())
            .cloned()
            .unwrap_or_default();

        let mut orders: Vec<BrokerOrder> = orders_arr
            .into_iter()
            .filter_map(|v| serde_json::from_value::<IbkrOrder>(v).ok())
            .map(|o| BrokerOrder {
                id: o.order_id.clone(),
                client_order_id: o.order_id,
                created_at: Utc::now(),
                filled_at: None,
                symbol: o.ticker.unwrap_or_default(),
                qty: o.total_qty.map(|q| q.to_string()),
                filled_qty: o.filled_qty.map(|q| q.to_string()),
                filled_avg_price: o.avg_price.map(|p| p.to_string()),
                order_type: o.order_type.unwrap_or_default().to_lowercase(),
                side: o.side.unwrap_or_default().to_lowercase(),
                status: o.status.unwrap_or_default().to_lowercase(),
            })
            .collect();

        if let Some(lim) = limit {
            orders.truncate(lim);
        }
        Ok(orders)
    }

    async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let url = format!(
            "{}/v1/api/iserver/account/{}/order/{}",
            self.gateway_url, self.account_id, order_id
        );
        let response = self.client.delete(&url).send().await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("IBKR cancel order failed: {}", text));
        }

        tracing::info!("IBKR order {} canceled", order_id);
        Ok(())
    }

    async fn close_position(&self, symbol: &str) -> Result<BrokerOrder> {
        let position = self
            .get_position(symbol)
            .await?
            .ok_or_else(|| anyhow!("No position found for {}", symbol))?;

        let qty: f64 = position.qty.parse().unwrap_or(0.0);
        let side = if qty > 0.0 {
            BrokerOrderSide::Sell
        } else {
            BrokerOrderSide::Buy
        };

        let order_req = BrokerOrderRequest {
            symbol: symbol.to_string(),
            qty: rust_decimal::Decimal::from_f64_retain(qty.abs()).unwrap_or_default(),
            side,
        };

        self.submit_market_order(order_req).await
    }

    fn is_paper(&self) -> bool {
        self.is_paper
    }

    fn broker_name(&self) -> &str {
        "ibkr"
    }
}
