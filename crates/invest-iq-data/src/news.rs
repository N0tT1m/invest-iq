use crate::client::PolygonFetcher;
use chrono::{Duration, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Fetch news articles for multiple symbols concurrently.
/// Returns {symbol: [{title, description, published_utc, tickers}, ...]}.
pub async fn fetch_news_multi_impl(
    fetcher: Arc<PolygonFetcher>,
    symbols: Vec<String>,
    limit_per_symbol: usize,
) -> HashMap<String, Vec<Value>> {
    let end = Utc::now().format("%Y-%m-%d").to_string();
    let start = (Utc::now() - Duration::days(365))
        .format("%Y-%m-%d")
        .to_string();

    let mut handles = Vec::with_capacity(symbols.len());

    for symbol in symbols {
        let fetcher = Arc::clone(&fetcher);
        let start = start.clone();
        let end = end.clone();

        handles.push(tokio::spawn(async move {
            let mut articles = Vec::new();
            let page_limit = limit_per_symbol.min(1000);

            let url = format!(
                "{}?ticker={}&published_utc.gte={}&published_utc.lte={}&limit={}&sort=published_utc",
                PolygonFetcher::polygon_url("/v2/reference/news"),
                symbol,
                start,
                end,
                page_limit,
            );

            // Fetch first page (and follow pagination if needed)
            let mut current_url = url;
            while let Ok(data) = fetcher.get(&current_url).await {
                if let Some(results) = data["results"].as_array() {
                    for article in results {
                        articles.push(serde_json::json!({
                            "title": article["title"],
                            "description": article.get("description").unwrap_or(&Value::Null),
                            "published_utc": article["published_utc"],
                            "tickers": article.get("tickers").unwrap_or(&Value::Null),
                        }));
                    }
                }

                if articles.len() >= limit_per_symbol {
                    articles.truncate(limit_per_symbol);
                    break;
                }

                // Follow pagination
                match data["next_url"].as_str() {
                    Some(next) if !next.is_empty() => {
                        current_url = next.to_string();
                    }
                    _ => break,
                }
            }

            (symbol, articles)
        }));
    }

    let mut results = HashMap::new();
    for handle in handles {
        if let Ok((symbol, articles)) = handle.await {
            if !articles.is_empty() {
                results.insert(symbol, articles);
            }
        }
    }

    results
}
