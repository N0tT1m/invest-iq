use analysis_core::{Bar, Financials, NewsArticle, AnalysisError, ConsensusRating, AnalystRating};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;

const BASE_URL: &str = "https://api.polygon.io";

/// Sliding-window rate limiter: at most `max_requests` per `window` duration.
#[derive(Clone)]
struct RateLimiter {
    timestamps: Arc<Mutex<VecDeque<Instant>>>,
    max_requests: usize,
    window: std::time::Duration,
}

impl RateLimiter {
    fn new(max_requests: usize, window: std::time::Duration) -> Self {
        Self {
            timestamps: Arc::new(Mutex::new(VecDeque::new())),
            max_requests,
            window,
        }
    }

    async fn acquire(&self) {
        loop {
            let mut ts = self.timestamps.lock().await;
            let now = Instant::now();

            // Remove timestamps outside the window
            while let Some(&front) = ts.front() {
                if now.duration_since(front) >= self.window {
                    ts.pop_front();
                } else {
                    break;
                }
            }

            if ts.len() < self.max_requests {
                ts.push_back(now);
                return;
            }

            // Need to wait until the oldest request falls out of the window
            let wait_until = ts.front().unwrap().checked_add(self.window).unwrap();
            let sleep_dur = wait_until.duration_since(now) + std::time::Duration::from_millis(50);
            drop(ts);
            tracing::debug!("Rate limiter: waiting {:.1}s for Polygon API slot", sleep_dur.as_secs_f64());
            tokio::time::sleep(sleep_dur).await;
        }
    }
}

#[derive(Clone)]
pub struct PolygonClient {
    api_key: String,
    client: Client,
    rate_limiter: RateLimiter,
}

impl PolygonClient {
    pub fn new(api_key: String) -> Self {
        // Default 500 req/min for Starter plan. Polygon recommends <100 req/sec (~6000/min).
        // Free tier users should set POLYGON_RATE_LIMIT=5.
        let rate_limit: usize = std::env::var("POLYGON_RATE_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(500);

        let client = Client::builder()
            .timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            api_key,
            client,
            rate_limiter: RateLimiter::new(rate_limit, Duration::from_secs(60)),
        }
    }

    /// Send a request with rate limiting and automatic 429 retry.
    async fn send_request(&self, builder: reqwest::RequestBuilder) -> Result<reqwest::Response, AnalysisError> {
        let request = builder.build().map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        for attempt in 0..3u32 {
            self.rate_limiter.acquire().await;
            let req_clone = request.try_clone()
                .ok_or_else(|| AnalysisError::ApiError("Cannot clone request".to_string()))?;
            let response = self.client.execute(req_clone).await
                .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

            if response.status().as_u16() != 429 {
                return Ok(response);
            }

            let wait_secs = 15u64;
            tracing::warn!("Polygon 429 rate limited, waiting {}s before retry {}/3", wait_secs, attempt + 1);
            tokio::time::sleep(Duration::from_secs(wait_secs)).await;
        }

        Err(AnalysisError::ApiError("Rate limited by Polygon after 3 retries".to_string()))
    }

    /// Get aggregates (bars) for a symbol
    pub async fn get_aggregates(
        &self,
        symbol: &str,
        multiplier: u32,
        timespan: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Bar>, AnalysisError> {
        let url = format!(
            "{}/v2/aggs/ticker/{}/range/{}/{}/{}/{}",
            BASE_URL,
            symbol,
            multiplier,
            timespan,
            from.format("%Y-%m-%d"),
            to.format("%Y-%m-%d")
        );

        let response = self.send_request(
            self.client.get(&url).query(&[("apiKey", &self.api_key), ("adjusted", &"true".to_string())])
        ).await?;

        if !response.status().is_success() {
            return Err(AnalysisError::ApiError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let agg_response: AggregateResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(agg_response
            .results
            .into_iter()
            .map(|r| Bar {
                timestamp: DateTime::from_timestamp_millis(r.t)
                    .unwrap_or_else(|| Utc::now()),
                open: r.o,
                high: r.h,
                low: r.l,
                close: r.c,
                volume: r.v,
            })
            .collect())
    }

    /// Get company financials
    pub async fn get_financials(&self, symbol: &str) -> Result<Vec<Financials>, AnalysisError> {
        let url = format!("{}/vX/reference/financials", BASE_URL);

        let response = self.send_request(
            self.client.get(&url).query(&[
                ("ticker", symbol),
                ("timeframe", "quarterly"),
                ("apiKey", &self.api_key),
                ("limit", "10"),
            ])
        ).await?;

        if !response.status().is_success() {
            if response.status().as_u16() == 403 || response.status().as_u16() == 401 {
                return Ok(Vec::new());
            }
            return Err(AnalysisError::ApiError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let fin_response: FinancialsResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(fin_response
            .results
            .into_iter()
            .map(|r| {
                let income = r.financials.income_statement;
                let balance = r.financials.balance_sheet;
                let cash_flow = r.financials.cash_flow_statement;

                Financials {
                    symbol: symbol.to_string(),
                    fiscal_period: r.fiscal_period,
                    fiscal_year: r.fiscal_year.parse().unwrap_or(0),
                    revenue: income.get("revenues").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    gross_profit: income.get("gross_profit").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    operating_income: income.get("operating_income_loss").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    net_income: income.get("net_income_loss").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    eps: income.get("basic_earnings_per_share").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    total_assets: balance.get("assets").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    total_liabilities: balance.get("liabilities").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    shareholders_equity: balance.get("equity").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    cash_flow_operating: cash_flow.get("net_cash_flow_from_operating_activities").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    cash_flow_investing: cash_flow.get("net_cash_flow_from_investing_activities").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                    cash_flow_financing: cash_flow.get("net_cash_flow_from_financing_activities").and_then(|v| v.get("value")).and_then(|v| v.as_f64()),
                }
            })
            .collect())
    }

    /// Get news articles
    pub async fn get_news(
        &self,
        symbol: Option<&str>,
        limit: u32,
    ) -> Result<Vec<NewsArticle>, AnalysisError> {
        let url = format!("{}/v2/reference/news", BASE_URL);

        let mut query = vec![("apiKey", self.api_key.clone()), ("limit", limit.to_string())];
        if let Some(sym) = symbol {
            query.push(("ticker", sym.to_string()));
        }

        let response = self.send_request(
            self.client.get(&url).query(&query)
        ).await?;

        if !response.status().is_success() {
            return Err(AnalysisError::ApiError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let news_response: NewsResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(news_response
            .results
            .into_iter()
            .map(|r| NewsArticle {
                id: r.id,
                title: r.title,
                author: r.author,
                published_utc: DateTime::parse_from_rfc3339(&r.published_utc)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                article_url: r.article_url,
                description: r.description,
                keywords: r.keywords.unwrap_or_default(),
                tickers: r.tickers,
            })
            .collect())
    }

    /// Get ticker details
    pub async fn get_ticker_details(&self, symbol: &str) -> Result<TickerDetails, AnalysisError> {
        let url = format!("{}/v3/reference/tickers/{}", BASE_URL, symbol);

        let response = self.send_request(
            self.client.get(&url).query(&[("apiKey", &self.api_key)])
        ).await?;

        if !response.status().is_success() {
            return Err(AnalysisError::ApiError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let details_response: TickerDetailsResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(details_response.results)
    }

    /// Get dividend history for a symbol
    pub async fn get_dividends(&self, symbol: &str, limit: u32) -> Result<Vec<DividendInfo>, AnalysisError> {
        let url = format!("{}/v3/reference/dividends", BASE_URL);

        let response = self.send_request(
            self.client.get(&url).query(&[
                ("ticker", symbol),
                ("apiKey", &self.api_key as &str),
                ("limit", &limit.to_string()),
                ("order", "desc"),
            ])
        ).await?;

        if !response.status().is_success() {
            if response.status().as_u16() == 403 || response.status().as_u16() == 401 {
                return Ok(Vec::new());
            }
            return Err(AnalysisError::ApiError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let div_response: DividendResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(div_response.results)
    }

    /// Get options chain snapshot for an underlying symbol
    pub async fn get_options_snapshot(&self, underlying: &str) -> Result<Vec<OptionsContractSnapshot>, AnalysisError> {
        let url = format!("{}/v3/snapshot/options/{}", BASE_URL, underlying);

        let response = self.send_request(
            self.client.get(&url).query(&[("apiKey", &self.api_key), ("limit", &"250".to_string())])
        ).await?;

        if !response.status().is_success() {
            if response.status().as_u16() == 403 || response.status().as_u16() == 401 {
                return Ok(Vec::new());
            }
            return Err(AnalysisError::ApiError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let snap_response: OptionsSnapshotResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(snap_response.results.unwrap_or_default())
    }

    /// Get insider transactions for a symbol
    pub async fn get_insider_transactions(&self, symbol: &str, limit: u32) -> Result<Vec<InsiderTransaction>, AnalysisError> {
        let url = format!("{}/vX/reference/insiders", BASE_URL);

        let response = self.send_request(
            self.client.get(&url).query(&[
                ("ticker", symbol),
                ("apiKey", &self.api_key as &str),
                ("limit", &limit.to_string()),
            ])
        ).await?;

        if !response.status().is_success() {
            if response.status().as_u16() == 403 || response.status().as_u16() == 401 {
                return Ok(Vec::new());
            }
            return Err(AnalysisError::ApiError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let insider_response: InsiderResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(insider_response.results.unwrap_or_default())
    }

    /// Get snapshot for a ticker (near-real-time last trade, today's OHLCV, prev day)
    pub async fn get_snapshot(&self, symbol: &str) -> Result<SnapshotTicker, AnalysisError> {
        let url = format!(
            "{}/v2/snapshot/locale/us/markets/stocks/tickers/{}",
            BASE_URL, symbol
        );

        let response = self.send_request(
            self.client.get(&url).query(&[("apiKey", &self.api_key)])
        ).await?;

        if !response.status().is_success() {
            return Err(AnalysisError::ApiError(format!(
                "Snapshot HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let snap_response: SnapshotResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(snap_response.ticker)
    }

    /// Get SMA (Simple Moving Average) from Polygon technical indicators API
    pub async fn get_sma(
        &self,
        symbol: &str,
        window: u32,
        timespan: &str,
        limit: u32,
    ) -> Result<Vec<IndicatorValue>, AnalysisError> {
        let url = format!("{}/v1/indicators/sma/{}", BASE_URL, symbol);

        let response = self.send_request(
            self.client.get(&url).query(&[
                ("apiKey", self.api_key.as_str()),
                ("window", &window.to_string()),
                ("timespan", timespan),
                ("limit", &limit.to_string()),
                ("series_type", "close"),
            ])
        ).await?;

        if !response.status().is_success() {
            return Err(AnalysisError::ApiError(format!(
                "SMA HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let ind_response: IndicatorResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(ind_response.results.values.unwrap_or_default())
    }

    /// Get RSI (Relative Strength Index) from Polygon technical indicators API
    pub async fn get_rsi(
        &self,
        symbol: &str,
        window: u32,
        timespan: &str,
        limit: u32,
    ) -> Result<Vec<IndicatorValue>, AnalysisError> {
        let url = format!("{}/v1/indicators/rsi/{}", BASE_URL, symbol);

        let response = self.send_request(
            self.client.get(&url).query(&[
                ("apiKey", self.api_key.as_str()),
                ("window", &window.to_string()),
                ("timespan", timespan),
                ("limit", &limit.to_string()),
                ("series_type", "close"),
            ])
        ).await?;

        if !response.status().is_success() {
            return Err(AnalysisError::ApiError(format!(
                "RSI HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let ind_response: IndicatorResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(ind_response.results.values.unwrap_or_default())
    }

    /// Get MACD from Polygon technical indicators API
    pub async fn get_macd(
        &self,
        symbol: &str,
        timespan: &str,
        limit: u32,
    ) -> Result<Vec<MacdValue>, AnalysisError> {
        let url = format!("{}/v1/indicators/macd/{}", BASE_URL, symbol);

        let response = self.send_request(
            self.client.get(&url).query(&[
                ("apiKey", self.api_key.as_str()),
                ("timespan", timespan),
                ("limit", &limit.to_string()),
                ("series_type", "close"),
            ])
        ).await?;

        if !response.status().is_success() {
            return Err(AnalysisError::ApiError(format!(
                "MACD HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let macd_response: MacdResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(macd_response.results.values.unwrap_or_default())
    }

    /// Get Benzinga consensus ratings for a ticker.
    /// Returns Ok(None) on 403/401 (subscription not available).
    pub async fn get_consensus_ratings(&self, symbol: &str) -> Result<Option<ConsensusRating>, AnalysisError> {
        let url = format!("{}/benzinga/v1/consensus-ratings/{}", BASE_URL, symbol);

        let response = self.send_request(
            self.client.get(&url).query(&[("apiKey", &self.api_key)])
        ).await?;

        let status = response.status().as_u16();
        if status == 403 || status == 401 {
            tracing::info!("Benzinga consensus ratings not available (HTTP {}), skipping", status);
            return Ok(None);
        }

        if !response.status().is_success() {
            tracing::warn!("Benzinga consensus HTTP {}: ignoring", status);
            return Ok(None);
        }

        let body: BenzingaConsensusResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(body.results.into_iter().next().map(|r| ConsensusRating {
            consensus_rating: r.consensus_rating,
            consensus_price_target: r.consensus_price_target,
            high_price_target: r.high_price_target,
            low_price_target: r.low_price_target,
            buy_count: r.buy_count,
            hold_count: r.hold_count,
            sell_count: r.sell_count,
            contributors: r.contributors,
        }))
    }

    /// Get recent Benzinga analyst ratings for a ticker.
    /// Returns Ok(vec![]) on 403/401 (subscription not available).
    pub async fn get_analyst_ratings(&self, symbol: &str, limit: u32) -> Result<Vec<AnalystRating>, AnalysisError> {
        let url = format!("{}/benzinga/v1/ratings", BASE_URL);

        let response = self.send_request(
            self.client.get(&url).query(&[
                ("apiKey", self.api_key.as_str()),
                ("ticker", symbol),
                ("sort", "date.desc"),
                ("limit", &limit.to_string()),
            ])
        ).await?;

        let status = response.status().as_u16();
        if status == 403 || status == 401 {
            tracing::info!("Benzinga analyst ratings not available (HTTP {}), skipping", status);
            return Ok(Vec::new());
        }

        if !response.status().is_success() {
            tracing::warn!("Benzinga ratings HTTP {}: ignoring", status);
            return Ok(Vec::new());
        }

        let body: BenzingaRatingsResponse = response
            .json()
            .await
            .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

        Ok(body.results.into_iter().map(|r| AnalystRating {
            price_target: r.price_target,
            rating: r.rating,
            rating_action: r.rating_action,
            analyst: r.analyst,
            firm: r.firm,
            date: r.date,
        }).collect())
    }

    /// List active US common stock tickers from Polygon reference API.
    /// Paginates automatically. Returns up to `max_tickers` symbols.
    pub async fn list_tickers(&self, max_tickers: usize) -> Result<Vec<String>, AnalysisError> {
        let mut tickers = Vec::new();
        let mut cursor: Option<String> = None;
        let page_limit = 1000;

        loop {
            let mut builder = self.client.get(&format!("{}/v3/reference/tickers", BASE_URL))
                .query(&[
                    ("apiKey", self.api_key.as_str()),
                    ("market", "stocks"),
                    ("active", "true"),
                    ("type", "CS"),
                    ("limit", &page_limit.to_string()),
                    ("order", "asc"),
                    ("sort", "ticker"),
                ]);

            if let Some(ref c) = cursor {
                builder = builder.query(&[("cursor", c.as_str())]);
            }

            let response = self.send_request(builder).await?;
            if !response.status().is_success() {
                break;
            }

            let body: TickerListResponse = response
                .json()
                .await
                .map_err(|e| AnalysisError::ApiError(e.to_string()))?;

            for t in &body.results {
                // Filter: only US exchanges, skip OTC / weird tickers
                if t.ticker.contains('.') || t.ticker.contains('-') || t.ticker.len() > 5 {
                    continue;
                }
                tickers.push(t.ticker.clone());
                if tickers.len() >= max_tickers {
                    return Ok(tickers);
                }
            }

            // Follow pagination cursor
            match body.next_url {
                Some(ref next) => {
                    // Extract cursor param from next_url
                    cursor = next
                        .split("cursor=")
                        .nth(1)
                        .map(|s| s.split('&').next().unwrap_or(s).to_string());
                    if cursor.is_none() {
                        break;
                    }
                }
                None => break,
            }
        }

        Ok(tickers)
    }
}

// Ticker list response
#[derive(Debug, Deserialize)]
struct TickerListResponse {
    #[serde(default)]
    results: Vec<TickerListEntry>,
    next_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TickerListEntry {
    ticker: String,
}

// Benzinga response structures
#[derive(Debug, Deserialize)]
struct BenzingaConsensusResponse {
    #[serde(default)]
    results: Vec<BenzingaConsensusResult>,
}

#[derive(Debug, Deserialize)]
struct BenzingaConsensusResult {
    #[serde(default)]
    consensus_rating: Option<String>,
    #[serde(default)]
    consensus_price_target: Option<f64>,
    #[serde(default)]
    high_price_target: Option<f64>,
    #[serde(default)]
    low_price_target: Option<f64>,
    #[serde(default)]
    buy_count: Option<i32>,
    #[serde(default)]
    hold_count: Option<i32>,
    #[serde(default)]
    sell_count: Option<i32>,
    #[serde(default)]
    contributors: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct BenzingaRatingsResponse {
    #[serde(default)]
    results: Vec<BenzingaRatingResult>,
}

#[derive(Debug, Deserialize)]
struct BenzingaRatingResult {
    #[serde(default)]
    price_target: Option<f64>,
    #[serde(default)]
    rating: Option<String>,
    #[serde(default)]
    rating_action: Option<String>,
    #[serde(default)]
    analyst: Option<String>,
    #[serde(default)]
    firm: Option<String>,
    #[serde(default)]
    date: Option<String>,
}

// Response structures
#[derive(Debug, Deserialize)]
struct AggregateResponse {
    #[serde(default)]
    results: Vec<AggregateResult>,
}

#[derive(Debug, Deserialize)]
struct AggregateResult {
    t: i64, // timestamp
    o: f64, // open
    h: f64, // high
    l: f64, // low
    c: f64, // close
    v: f64, // volume
}

#[derive(Debug, Deserialize)]
struct FinancialsResponse {
    #[serde(default)]
    results: Vec<FinancialResult>,
}

#[derive(Debug, Deserialize)]
struct FinancialResult {
    fiscal_period: String,
    fiscal_year: String,
    financials: FinancialStatements,
}

#[derive(Debug, Deserialize)]
struct FinancialStatements {
    income_statement: HashMap<String, serde_json::Value>,
    balance_sheet: HashMap<String, serde_json::Value>,
    cash_flow_statement: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct NewsResponse {
    results: Vec<NewsResult>,
}

#[derive(Debug, Deserialize)]
struct NewsResult {
    id: String,
    title: String,
    author: Option<String>,
    published_utc: String,
    article_url: String,
    description: Option<String>,
    keywords: Option<Vec<String>>,
    tickers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TickerDetailsResponse {
    results: TickerDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerDetails {
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub locale: String,
    pub primary_exchange: String,
    #[serde(rename = "type")]
    pub ticker_type: String,
    pub active: bool,
    pub currency_name: Option<String>,
    pub market_cap: Option<f64>,
    pub share_class_shares_outstanding: Option<f64>,
    pub weighted_shares_outstanding: Option<f64>,
}

// Dividend types
#[derive(Debug, Deserialize)]
struct DividendResponse {
    #[serde(default)]
    results: Vec<DividendInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DividendInfo {
    pub cash_amount: Option<f64>,
    pub ex_dividend_date: Option<String>,
    pub pay_date: Option<String>,
    pub declaration_date: Option<String>,
    pub frequency: Option<i32>,
    #[serde(default)]
    pub dividend_type: Option<String>,
}

// Options types
#[derive(Debug, Deserialize)]
struct OptionsSnapshotResponse {
    results: Option<Vec<OptionsContractSnapshot>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsContractSnapshot {
    #[serde(default)]
    pub details: Option<OptionsDetails>,
    #[serde(default)]
    pub greeks: Option<OptionsGreeks>,
    pub implied_volatility: Option<f64>,
    pub open_interest: Option<i64>,
    #[serde(default)]
    pub day: Option<OptionsDay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsDetails {
    pub contract_type: Option<String>,
    pub strike_price: Option<f64>,
    pub expiration_date: Option<String>,
    pub ticker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsGreeks {
    pub delta: Option<f64>,
    pub gamma: Option<f64>,
    pub theta: Option<f64>,
    pub vega: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsDay {
    pub volume: Option<i64>,
    pub open: Option<f64>,
    pub close: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
}

// Insider types
#[derive(Debug, Deserialize)]
struct InsiderResponse {
    results: Option<Vec<InsiderTransaction>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsiderTransaction {
    pub filing_date: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub transaction_type: Option<String>,
    #[serde(default)]
    pub shares: Option<f64>,
    #[serde(default)]
    pub price_per_share: Option<f64>,
    #[serde(default)]
    pub total_value: Option<f64>,
}

// Snapshot types
#[derive(Debug, Deserialize)]
struct SnapshotResponse {
    ticker: SnapshotTicker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotTicker {
    pub day: Option<SnapshotDay>,
    #[serde(rename = "lastTrade")]
    pub last_trade: Option<SnapshotLastTrade>,
    #[serde(rename = "prevDay")]
    pub prev_day: Option<SnapshotDay>,
    #[serde(rename = "todaysChange")]
    pub todays_change: Option<f64>,
    #[serde(rename = "todaysChangePerc")]
    pub todays_change_perc: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDay {
    pub o: Option<f64>,
    pub h: Option<f64>,
    pub l: Option<f64>,
    pub c: Option<f64>,
    pub v: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotLastTrade {
    pub p: Option<f64>,
    pub s: Option<i64>,
    pub t: Option<i64>,
}

// Technical indicator types
#[derive(Debug, Deserialize)]
struct IndicatorResponse {
    results: IndicatorResults,
}

#[derive(Debug, Deserialize)]
struct IndicatorResults {
    values: Option<Vec<IndicatorValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorValue {
    pub timestamp: Option<i64>,
    pub value: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct MacdResponse {
    results: MacdResults,
}

#[derive(Debug, Deserialize)]
struct MacdResults {
    values: Option<Vec<MacdValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacdValue {
    pub timestamp: Option<i64>,
    pub value: Option<f64>,
    pub signal: Option<f64>,
    pub histogram: Option<f64>,
}
