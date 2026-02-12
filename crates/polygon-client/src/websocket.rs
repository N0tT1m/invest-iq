use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const POLYGON_WS_URL: &str = "wss://socket.polygon.io/stocks";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamQuote {
    pub symbol: String,
    pub price: f64,
    pub size: f64,
    pub timestamp: i64,
    #[serde(default)]
    pub bid: Option<f64>,
    #[serde(default)]
    pub ask: Option<f64>,
}

pub struct PolygonWebSocket {
    api_key: String,
    tx: broadcast::Sender<StreamQuote>,
    subscriptions: Arc<Mutex<HashSet<String>>>,
    shutdown: Arc<tokio::sync::Notify>,
}

impl PolygonWebSocket {
    pub fn new(api_key: String) -> (Self, broadcast::Receiver<StreamQuote>) {
        let (tx, rx) = broadcast::channel(1024);
        let ws = Self {
            api_key,
            tx,
            subscriptions: Arc::new(Mutex::new(HashSet::new())),
            shutdown: Arc::new(tokio::sync::Notify::new()),
        };
        (ws, rx)
    }

    pub fn sender(&self) -> broadcast::Sender<StreamQuote> {
        self.tx.clone()
    }

    pub async fn subscribe(&self, symbols: &[String]) {
        let mut subs = self.subscriptions.lock().await;
        for s in symbols {
            subs.insert(s.to_uppercase());
        }
    }

    pub async fn unsubscribe(&self, symbols: &[String]) {
        let mut subs = self.subscriptions.lock().await;
        for s in symbols {
            subs.remove(&s.to_uppercase());
        }
    }

    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }

    pub async fn run(&self) {
        loop {
            match self.connect_and_stream().await {
                Ok(()) => {
                    tracing::info!("Polygon WS disconnected gracefully");
                    break;
                }
                Err(e) => {
                    tracing::warn!("Polygon WS error: {}, reconnecting in 5s", e);
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {},
                        _ = self.shutdown.notified() => {
                            tracing::info!("Polygon WS shutdown requested");
                            return;
                        }
                    }
                }
            }
        }
    }

    async fn connect_and_stream(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (ws_stream, _) = connect_async(POLYGON_WS_URL).await?;
        let (mut write, mut read) = ws_stream.split();
        tracing::info!("Connected to Polygon WebSocket");

        // Authenticate
        let auth_msg = serde_json::json!({"action": "auth", "params": self.api_key});
        write.send(Message::Text(auth_msg.to_string())).await?;

        // Wait for auth confirmation
        if let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    tracing::debug!("Polygon WS auth response: {}", text);
                }
                Ok(_) => {}
                Err(e) => return Err(Box::new(e)),
            }
        }

        // Subscribe to current symbols
        let subs = self.subscriptions.lock().await;
        if !subs.is_empty() {
            let channels: Vec<String> = subs
                .iter()
                .flat_map(|s| vec![format!("T.{}", s), format!("Q.{}", s)])
                .collect();
            let sub_msg = serde_json::json!({"action": "subscribe", "params": channels.join(",")});
            write.send(Message::Text(sub_msg.to_string())).await?;
            tracing::info!("Subscribed to {} symbols", subs.len());
        }
        drop(subs);

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
                            tracing::info!("Polygon WS connection closed");
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
        // Polygon sends arrays of events
        if let Ok(events) = serde_json::from_str::<Vec<serde_json::Value>>(text) {
            for event in events {
                match event.get("ev").and_then(|e| e.as_str()) {
                    Some("T") => {
                        if let (Some(sym), Some(p), Some(s), Some(t)) = (
                            event.get("sym").and_then(|v| v.as_str()),
                            event.get("p").and_then(|v| v.as_f64()),
                            event.get("s").and_then(|v| v.as_f64()),
                            event.get("t").and_then(|v| v.as_i64()),
                        ) {
                            let _ = self.tx.send(StreamQuote {
                                symbol: sym.to_string(),
                                price: p,
                                size: s,
                                timestamp: t,
                                bid: None,
                                ask: None,
                            });
                        }
                    }
                    Some("Q") => {
                        if let (Some(sym), Some(bp), Some(ap), Some(t)) = (
                            event.get("sym").and_then(|v| v.as_str()),
                            event.get("bp").and_then(|v| v.as_f64()),
                            event.get("ap").and_then(|v| v.as_f64()),
                            event.get("t").and_then(|v| v.as_i64()),
                        ) {
                            let mid = (bp + ap) / 2.0;
                            let _ = self.tx.send(StreamQuote {
                                symbol: sym.to_string(),
                                price: mid,
                                size: 0.0,
                                timestamp: t,
                                bid: Some(bp),
                                ask: Some(ap),
                            });
                        }
                    }
                    Some("status") => {
                        if let Some(msg) = event.get("message").and_then(|v| v.as_str()) {
                            tracing::debug!("Polygon WS status: {}", msg);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
