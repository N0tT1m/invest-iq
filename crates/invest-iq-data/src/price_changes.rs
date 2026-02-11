use crate::client::PolygonFetcher;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;

/// Fetch recent bars and compute N-day price return for multiple symbols concurrently.
/// Returns {symbol: percent_change}.
pub async fn fetch_price_changes_impl(
    fetcher: Arc<PolygonFetcher>,
    symbols: Vec<String>,
    days: i64,
) -> HashMap<String, f64> {
    // Fetch a few extra days to ensure we have enough trading days
    let extra_days = days + 10;
    let end = Utc::now().format("%Y-%m-%d").to_string();
    let start = (Utc::now() - Duration::days(extra_days))
        .format("%Y-%m-%d")
        .to_string();

    let mut handles = Vec::with_capacity(symbols.len());

    for symbol in symbols {
        let fetcher = Arc::clone(&fetcher);
        let start = start.clone();
        let end = end.clone();

        handles.push(tokio::spawn(async move {
            let url = format!(
                "{}?adjusted=true&sort=asc&limit={}",
                PolygonFetcher::polygon_url(&format!(
                    "/v2/aggs/ticker/{symbol}/range/1/day/{start}/{end}"
                )),
                extra_days + 1,
            );

            match fetcher.get(&url).await {
                Ok(data) => {
                    let bars = data["results"].as_array();
                    match bars {
                        Some(arr) if arr.len() >= 2 => {
                            // Use the bar `days` positions from the end as the reference
                            let idx = if arr.len() > days as usize {
                                arr.len() - 1 - days as usize
                            } else {
                                0
                            };
                            let start_close = arr[idx]["c"].as_f64().unwrap_or(0.0);
                            let end_close = arr.last().and_then(|b| b["c"].as_f64()).unwrap_or(0.0);

                            if start_close > 0.0 {
                                let pct = (end_close - start_close) / start_close * 100.0;
                                Some((symbol, pct))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                }
                Err(_) => None,
            }
        }));
    }

    let mut results = HashMap::new();
    for handle in handles {
        if let Ok(Some((symbol, pct))) = handle.await {
            results.insert(symbol, pct);
        }
    }

    results
}
