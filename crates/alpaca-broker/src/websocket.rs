use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const ALPACA_PAPER_WS: &str = "wss://paper-api.alpaca.markets/stream";
const ALPACA_LIVE_WS: &str = "wss://api.alpaca.markets/stream";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdate {
    pub event: String,
    pub order_id: String,
    pub symbol: String,
    pub side: String,
    pub qty: String,
    pub filled_qty: String,
    pub filled_avg_price: Option<String>,
    pub status: String,
    pub timestamp: String,
}

pub struct AlpacaWebSocket {
    api_key: String,
    api_secret: String,
    paper: bool,
    tx: broadcast::Sender<OrderUpdate>,
    shutdown: Arc<tokio::sync::Notify>,
}

impl AlpacaWebSocket {
    pub fn new(
        api_key: String,
        api_secret: String,
        paper: bool,
    ) -> (Self, broadcast::Receiver<OrderUpdate>) {
        let (tx, rx) = broadcast::channel(256);
        let ws = Self {
            api_key,
            api_secret,
            paper,
            tx,
            shutdown: Arc::new(tokio::sync::Notify::new()),
        };
        (ws, rx)
    }

    pub fn sender(&self) -> broadcast::Sender<OrderUpdate> {
        self.tx.clone()
    }

    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }

    pub async fn run(&self) {
        loop {
            match self.connect_and_stream().await {
                Ok(()) => {
                    tracing::info!("Alpaca WS disconnected gracefully");
                    break;
                }
                Err(e) => {
                    tracing::warn!("Alpaca WS error: {}, reconnecting in 5s", e);
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {},
                        _ = self.shutdown.notified() => {
                            tracing::info!("Alpaca WS shutdown requested");
                            return;
                        }
                    }
                }
            }
        }
    }

    async fn connect_and_stream(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = if self.paper {
            ALPACA_PAPER_WS
        } else {
            ALPACA_LIVE_WS
        };
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();
        tracing::info!("Connected to Alpaca WebSocket (paper={})", self.paper);

        // Authenticate
        let auth_msg = serde_json::json!({
            "action": "authenticate",
            "data": {
                "key_id": self.api_key,
                "secret_key": self.api_secret,
            }
        });
        write.send(Message::Text(auth_msg.to_string())).await?;

        // Wait for auth response
        if let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    tracing::debug!("Alpaca WS auth response: {}", text);
                }
                Ok(_) => {}
                Err(e) => return Err(Box::new(e)),
            }
        }

        // Subscribe to trade updates
        let sub_msg = serde_json::json!({
            "action": "listen",
            "data": { "streams": ["trade_updates"] }
        });
        write.send(Message::Text(sub_msg.to_string())).await?;
        tracing::info!("Subscribed to Alpaca trade_updates");

        // Stream messages
        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_message(&text);
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            tracing::info!("Alpaca WS connection closed");
                            return Ok(());
                        }
                        Some(Err(e)) => {
                            return Err(Box::new(e));
                        }
                        _ => {}
                    }
                }
                _ = self.shutdown.notified() => {
                    let _ = write.send(Message::Close(None)).await;
                    return Ok(());
                }
            }
        }
    }

    fn handle_message(&self, text: &str) {
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(text) {
            let stream = msg.get("stream").and_then(|s| s.as_str()).unwrap_or("");
            if stream == "trade_updates" {
                if let Some(data) = msg.get("data") {
                    let event = data
                        .get("event")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let order = data.get("order").unwrap_or(data);

                    let update = OrderUpdate {
                        event: event.to_string(),
                        order_id: order
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        symbol: order
                            .get("symbol")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        side: order
                            .get("side")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        qty: order
                            .get("qty")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .to_string(),
                        filled_qty: order
                            .get("filled_qty")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .to_string(),
                        filled_avg_price: order
                            .get("filled_avg_price")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        status: order
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        timestamp: order
                            .get("updated_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    };

                    tracing::info!(
                        "Order update: {} {} {} ({})",
                        update.event,
                        update.side,
                        update.symbol,
                        update.status
                    );
                    let _ = self.tx.send(update);
                }
            }
        }
    }
}
