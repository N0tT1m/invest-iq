use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::{ApiResponse, AppState};

// ---------------------------------------------------------------------------
// Shared quote cache (latest quote per symbol, populated by WS or REST)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LiveQuote {
    pub symbol: String,
    pub price: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub size: f64,
    pub timestamp: i64,
}

pub type QuoteCache = Arc<RwLock<HashMap<String, LiveQuote>>>;

// ---------------------------------------------------------------------------
// Broadcast channels stored in AppState extension
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct WsBroadcast {
    pub quotes: broadcast::Sender<LiveQuote>,
    pub quote_cache: QuoteCache,
}

#[allow(dead_code)] // publish_quote called from Polygon WS bridge task
impl WsBroadcast {
    pub fn new() -> Self {
        let (quotes, _) = broadcast::channel(2048);
        Self {
            quotes,
            quote_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update the cache and broadcast
    pub async fn publish_quote(&self, quote: LiveQuote) {
        {
            let mut cache = self.quote_cache.write().await;
            cache.insert(quote.symbol.clone(), quote.clone());
        }
        let _ = self.quotes.send(quote);
    }
}

// ---------------------------------------------------------------------------
// WebSocket handler: /ws/quotes
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/ws/quotes",
    responses((status = 101, description = "WebSocket upgrade for streaming quotes")),
    tag = "System"
)]
async fn ws_quotes_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_quote_socket(socket, state))
}

async fn handle_quote_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let ws_broadcast = match &state.ws_broadcast {
        Some(wb) => wb.clone(),
        None => {
            let _ = sender
                .send(Message::Text(
                    serde_json::json!({"error": "WebSocket not enabled"}).to_string(),
                ))
                .await;
            return;
        }
    };

    let mut rx = ws_broadcast.quotes.subscribe();

    // Send current cache snapshot
    {
        let cache = ws_broadcast.quote_cache.read().await;
        if !cache.is_empty() {
            let snapshot: Vec<&LiveQuote> = cache.values().collect();
            if let Ok(json) = serde_json::to_string(&snapshot) {
                let _ = sender.send(Message::Text(json)).await;
            }
        }
    }

    // Fan out broadcast messages to this client
    let send_task = tokio::spawn(async move {
        while let Ok(quote) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&quote) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Read from client (handle subscribe/unsubscribe messages or just ping/pong)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(data) => {
                    // axum handles pong automatically
                    let _ = data;
                }
                _ => {} // Ignore other client messages for now
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

// ---------------------------------------------------------------------------
// REST fallback: /api/quotes/live (for Dash polling via dcc.Interval)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LiveQuotesQuery {
    #[serde(default)]
    pub symbols: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/quotes/live",
    params(("symbols" = Option<String>, Query, description = "Comma-separated symbols to filter")),
    responses((status = 200, description = "Latest cached quotes from WebSocket feed")),
    tag = "System"
)]
async fn get_live_quotes(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<LiveQuotesQuery>,
) -> Json<ApiResponse<Vec<LiveQuote>>> {
    let ws_broadcast = match &state.ws_broadcast {
        Some(wb) => wb,
        None => return Json(ApiResponse::success(vec![])),
    };

    let cache = ws_broadcast.quote_cache.read().await;

    let quotes: Vec<LiveQuote> = if let Some(symbols) = &query.symbols {
        symbols
            .split(',')
            .filter_map(|s| {
                let sym = s.trim().to_uppercase();
                cache.get(&sym).cloned()
            })
            .collect()
    } else {
        cache.values().cloned().collect()
    };

    Json(ApiResponse::success(quotes))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn ws_routes() -> Router<AppState> {
    Router::new()
        .route("/ws/quotes", get(ws_quotes_handler))
        .route("/api/quotes/live", get(get_live_quotes))
}
