use crate::models::*;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use broker_trait::{
    BrokerAccount, BrokerClient, BrokerOrder, BrokerOrderRequest, BrokerOrderSide, BrokerPosition,
};
use reqwest::{header, Client};
use std::time::Duration;

pub struct AlpacaClient {
    client: Client,
    base_url: String,
    api_key: String,
    secret_key: String,
}

impl AlpacaClient {
    /// Create a new Alpaca client
    pub fn new(api_key: String, secret_key: String, base_url: String) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(15)).build()?;

        Ok(Self {
            client,
            base_url,
            api_key,
            secret_key,
        })
    }

    /// Create client from environment variables.
    /// Accepts both APCA_API_KEY_ID / APCA_API_SECRET_KEY (standard Alpaca names)
    /// and ALPACA_API_KEY / ALPACA_SECRET_KEY as fallbacks.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("APCA_API_KEY_ID")
            .or_else(|_| std::env::var("ALPACA_API_KEY"))
            .map_err(|_| anyhow!("APCA_API_KEY_ID (or ALPACA_API_KEY) not set"))?;
        let secret_key = std::env::var("APCA_API_SECRET_KEY")
            .or_else(|_| std::env::var("ALPACA_SECRET_KEY"))
            .map_err(|_| anyhow!("APCA_API_SECRET_KEY (or ALPACA_SECRET_KEY) not set"))?;
        let base_url = std::env::var("ALPACA_BASE_URL")
            .unwrap_or_else(|_| "https://paper-api.alpaca.markets".to_string());

        Self::new(api_key, secret_key, base_url)
    }

    /// Get authorization headers
    fn auth_headers(&self) -> header::HeaderMap {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "APCA-API-KEY-ID",
            header::HeaderValue::from_str(&self.api_key)
                .expect("API key contains invalid header characters"),
        );
        headers.insert(
            "APCA-API-SECRET-KEY",
            header::HeaderValue::from_str(&self.secret_key)
                .expect("Secret key contains invalid header characters"),
        );
        headers
    }

    /// Get account information
    pub async fn get_account(&self) -> Result<Account> {
        let url = format!("{}/v2/account", self.base_url);

        let response = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Alpaca API error: {}", error_text));
        }

        let account = response.json::<Account>().await?;
        Ok(account)
    }

    /// Submit an order
    pub async fn submit_order(&self, order: OrderRequest) -> Result<Order> {
        let url = format!("{}/v2/orders", self.base_url);

        tracing::info!("Submitting order to Alpaca: {:?}", order);

        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .json(&order)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Alpaca order failed: {}", error_text));
        }

        let order_response = response.json::<Order>().await?;
        tracing::info!("Order submitted successfully: {}", order_response.id);
        Ok(order_response)
    }

    /// Submit a market order (convenience method)
    pub async fn submit_market_order(&self, order: MarketOrderRequest) -> Result<Order> {
        self.submit_order(order.to_order_request()).await
    }

    /// Get an order by ID
    pub async fn get_order(&self, order_id: &str) -> Result<Order> {
        let url = format!("{}/v2/orders/{}", self.base_url, order_id);

        let response = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get order: {}", error_text));
        }

        let order = response.json::<Order>().await?;
        Ok(order)
    }

    /// Get all orders
    pub async fn get_orders(&self, limit: Option<usize>) -> Result<Vec<Order>> {
        let mut url = format!("{}/v2/orders?status=all", self.base_url);
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={}", lim));
        }

        let response = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get orders: {}", error_text));
        }

        let orders = response.json::<Vec<Order>>().await?;
        Ok(orders)
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let url = format!("{}/v2/orders/{}", self.base_url, order_id);

        let response = self
            .client
            .delete(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to cancel order: {}", error_text));
        }

        tracing::info!("Order {} canceled successfully", order_id);
        Ok(())
    }

    /// Get all positions
    pub async fn get_positions(&self) -> Result<Vec<Position>> {
        let url = format!("{}/v2/positions", self.base_url);

        let response = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get positions: {}", error_text));
        }

        let positions = response.json::<Vec<Position>>().await?;
        Ok(positions)
    }

    /// Get a specific position
    pub async fn get_position(&self, symbol: &str) -> Result<Option<Position>> {
        let url = format!("{}/v2/positions/{}", self.base_url, symbol);

        let response = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get position: {}", error_text));
        }

        let position = response.json::<Position>().await?;
        Ok(Some(position))
    }

    /// Close a position (sell all shares)
    pub async fn close_position(&self, symbol: &str) -> Result<Order> {
        let url = format!("{}/v2/positions/{}", self.base_url, symbol);

        let response = self
            .client
            .delete(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to close position: {}", error_text));
        }

        let order = response.json::<Order>().await?;
        tracing::info!("Position {} closed successfully", symbol);
        Ok(order)
    }

    /// Check if trading is available
    pub async fn is_trading_available(&self) -> Result<bool> {
        let account = self.get_account().await?;
        Ok(!account.trading_blocked && !account.account_blocked)
    }

    /// Check if this client is connected to the paper trading environment
    pub fn is_paper(&self) -> bool {
        self.base_url.contains("paper-api")
    }

    /// Get the base URL (for logging/diagnostics)
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers: Alpaca types -> unified BrokerClient types
// ---------------------------------------------------------------------------

fn account_to_broker(a: Account) -> BrokerAccount {
    BrokerAccount {
        id: a.id,
        account_number: a.account_number,
        status: a.status,
        currency: a.currency,
        buying_power: a.buying_power,
        cash: a.cash,
        portfolio_value: a.portfolio_value,
        pattern_day_trader: a.pattern_day_trader,
        trading_blocked: a.trading_blocked,
        daytrade_count: a.daytrade_count,
    }
}

fn position_to_broker(p: Position) -> BrokerPosition {
    BrokerPosition {
        symbol: p.symbol,
        qty: p.qty,
        side: p.side,
        avg_entry_price: p.avg_entry_price,
        market_value: p.market_value,
        cost_basis: p.cost_basis,
        unrealized_pl: p.unrealized_pl,
        unrealized_plpc: p.unrealized_plpc,
        unrealized_intraday_pl: p.unrealized_intraday_pl,
        current_price: p.current_price,
        lastday_price: p.lastday_price,
        change_today: p.change_today,
    }
}

fn order_to_broker(o: Order) -> BrokerOrder {
    BrokerOrder {
        id: o.id,
        client_order_id: o.client_order_id,
        created_at: o.created_at,
        filled_at: o.filled_at,
        symbol: o.symbol,
        qty: o.quantity,
        filled_qty: o.filled_quantity,
        filled_avg_price: o.filled_avg_price,
        order_type: o.order_type,
        side: o.side,
        status: o.status,
    }
}

#[async_trait]
impl BrokerClient for AlpacaClient {
    async fn get_account(&self) -> Result<BrokerAccount> {
        self.get_account().await.map(account_to_broker)
    }

    async fn get_positions(&self) -> Result<Vec<BrokerPosition>> {
        self.get_positions()
            .await
            .map(|ps| ps.into_iter().map(position_to_broker).collect())
    }

    async fn get_position(&self, symbol: &str) -> Result<Option<BrokerPosition>> {
        self.get_position(symbol)
            .await
            .map(|p| p.map(position_to_broker))
    }

    async fn submit_market_order(&self, order: BrokerOrderRequest) -> Result<BrokerOrder> {
        let side = match order.side {
            BrokerOrderSide::Buy => OrderSide::Buy,
            BrokerOrderSide::Sell => OrderSide::Sell,
        };
        let alpaca_order = MarketOrderRequest {
            symbol: order.symbol,
            qty: order.qty,
            side,
        };
        self.submit_market_order(alpaca_order)
            .await
            .map(order_to_broker)
    }

    async fn get_order(&self, order_id: &str) -> Result<BrokerOrder> {
        self.get_order(order_id).await.map(order_to_broker)
    }

    async fn get_orders(&self, limit: Option<usize>) -> Result<Vec<BrokerOrder>> {
        self.get_orders(limit)
            .await
            .map(|os| os.into_iter().map(order_to_broker).collect())
    }

    async fn cancel_order(&self, order_id: &str) -> Result<()> {
        self.cancel_order(order_id).await
    }

    async fn close_position(&self, symbol: &str) -> Result<BrokerOrder> {
        self.close_position(symbol).await.map(order_to_broker)
    }

    fn is_paper(&self) -> bool {
        self.is_paper()
    }

    fn broker_name(&self) -> &str {
        "alpaca"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[tokio::test]
    #[ignore] // Only run with valid credentials
    async fn test_get_account() {
        let client = AlpacaClient::from_env().unwrap();
        let account = client.get_account().await.unwrap();

        println!("Account ID: {}", account.id);
        println!("Buying Power: ${}", account.buying_power);
        println!("Portfolio Value: ${}", account.portfolio_value);

        assert!(!account.id.is_empty());
    }

    #[tokio::test]
    #[ignore] // Only run with valid credentials
    async fn test_submit_market_order() {
        let client = AlpacaClient::from_env().unwrap();

        // Submit a small test order
        let order = MarketOrderRequest::buy("AAPL", Decimal::from_str("1.0").unwrap());
        let result = client.submit_market_order(order).await.unwrap();

        println!("Order submitted: {}", result.id);
        println!("Status: {}", result.status);

        // Cancel the order immediately (for paper trading)
        client.cancel_order(&result.id).await.unwrap();
    }
}
