use crate::client::PolygonFetcher;
use chrono::{Duration, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Fetch OHLCV bars for multiple symbols concurrently.
/// Returns {symbol: [{timestamp, open, high, low, close, volume, vwap}, ...]}.
pub async fn fetch_bars_multi_impl(
    fetcher: Arc<PolygonFetcher>,
    symbols: Vec<String>,
    days: i64,
    timespan: &str,
) -> HashMap<String, Vec<Value>> {
    let end = Utc::now().format("%Y-%m-%d").to_string();
    let start = (Utc::now() - Duration::days(days))
        .format("%Y-%m-%d")
        .to_string();

    let mut handles = Vec::with_capacity(symbols.len());

    for symbol in symbols {
        let fetcher = Arc::clone(&fetcher);
        let start = start.clone();
        let end = end.clone();
        let timespan = timespan.to_string();

        handles.push(tokio::spawn(async move {
            let url = format!(
                "{}?adjusted=true&sort=asc&limit=50000",
                PolygonFetcher::polygon_url(&format!(
                    "/v2/aggs/ticker/{symbol}/range/1/{timespan}/{start}/{end}"
                )),
            );

            match fetcher.get(&url).await {
                Ok(data) => {
                    let bars = data["results"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .map(|bar| {
                                    serde_json::json!({
                                        "timestamp": bar["t"],
                                        "open": bar["o"],
                                        "high": bar["h"],
                                        "low": bar["l"],
                                        "close": bar["c"],
                                        "volume": bar["v"],
                                        "vwap": bar.get("vw").unwrap_or(&Value::Null),
                                    })
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    (symbol, bars)
                }
                Err(_) => (symbol, Vec::new()),
            }
        }));
    }

    let mut results = HashMap::new();
    for handle in handles {
        if let Ok((symbol, bars)) = handle.await {
            if !bars.is_empty() {
                results.insert(symbol, bars);
            }
        }
    }

    results
}
