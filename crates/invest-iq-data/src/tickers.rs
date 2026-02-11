use crate::client::PolygonFetcher;

/// Fetch all active tickers from Polygon, paginating through results.
/// Filters out tickers with ".", "/", or length > 5.
pub async fn fetch_active_tickers_impl(
    fetcher: &PolygonFetcher,
    market: &str,
    ticker_type: &str,
) -> Result<Vec<String>, String> {
    let mut tickers = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut url = format!(
            "{}?market={}&active=true&type={}&limit=1000&order=asc&sort=ticker",
            PolygonFetcher::polygon_url("/v3/reference/tickers"),
            market,
            ticker_type,
        );
        if let Some(ref c) = cursor {
            url.push_str(&format!("&cursor={c}"));
        }

        let data = fetcher.get(&url).await?;

        let results = data["results"].as_array();
        let results = match results {
            Some(r) if !r.is_empty() => r,
            _ => break,
        };

        for item in results {
            if let Some(ticker) = item["ticker"].as_str() {
                if !ticker.contains('.')
                    && !ticker.contains('/')
                    && ticker.len() <= 5
                    && !ticker.is_empty()
                {
                    tickers.push(ticker.to_string());
                }
            }
        }

        // Follow pagination
        let next_url = data["next_url"].as_str().unwrap_or("");
        if next_url.is_empty() {
            break;
        }

        // Extract cursor from next_url query string
        cursor = next_url
            .split("cursor=")
            .nth(1)
            .map(|s| s.split('&').next().unwrap_or(s).to_string());

        if cursor.is_none() {
            break;
        }
    }

    tickers.sort();
    Ok(tickers)
}
