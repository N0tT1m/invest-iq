use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

const POLYGON_BASE: &str = "https://api.polygon.io";

/// Sliding-window rate limiter that tracks request timestamps.
pub struct RateLimiter {
    window: Duration,
    max_requests: usize,
    timestamps: Mutex<Vec<Instant>>,
}

impl RateLimiter {
    pub fn new(max_per_minute: usize) -> Self {
        Self {
            window: Duration::from_secs(60),
            max_requests: max_per_minute,
            timestamps: Mutex::new(Vec::new()),
        }
    }

    pub async fn acquire(&self) {
        loop {
            let mut ts = self.timestamps.lock().await;
            let now = Instant::now();

            // Evict timestamps outside the window
            ts.retain(|t| now.duration_since(*t) < self.window);

            if ts.len() < self.max_requests {
                ts.push(now);
                return;
            }

            // Calculate how long to sleep until the oldest entry expires
            let oldest = ts[0];
            let sleep_dur = self.window.saturating_sub(now.duration_since(oldest));
            drop(ts);
            tokio::time::sleep(sleep_dur + Duration::from_millis(10)).await;
        }
    }
}

/// Shared HTTP client with rate limiting, concurrency control, and retry logic.
pub struct PolygonFetcher {
    pub client: Client,
    pub api_key: String,
    pub semaphore: Arc<Semaphore>,
    pub rate_limiter: Arc<RateLimiter>,
}

impl PolygonFetcher {
    pub fn new(api_key: String, max_concurrent: usize, rate_per_minute: usize) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            api_key,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            rate_limiter: Arc::new(RateLimiter::new(rate_per_minute)),
        }
    }

    /// Make a GET request with concurrency + rate limiting + retry on 429.
    pub async fn get(&self, url: &str) -> Result<serde_json::Value, String> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| format!("Semaphore error: {e}"))?;

        let backoffs = [2, 4, 8];
        let mut last_err = String::new();

        for (attempt, &backoff_secs) in std::iter::once(&0u64).chain(backoffs.iter()).enumerate() {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
            }

            self.rate_limiter.acquire().await;

            let sep = if url.contains('?') { "&" } else { "?" };
            let full_url = format!("{url}{sep}apiKey={}", self.api_key);

            match self.client.get(&full_url).send().await {
                Ok(resp) => {
                    if resp.status() == 429 {
                        last_err = "Rate limited (429)".to_string();
                        continue;
                    }
                    if !resp.status().is_success() {
                        return Err(format!("HTTP {}", resp.status()));
                    }
                    return resp.json().await.map_err(|e| format!("JSON parse error: {e}"));
                }
                Err(e) => {
                    last_err = format!("Request error: {e}");
                    if e.is_timeout() {
                        continue;
                    }
                    return Err(last_err);
                }
            }
        }

        Err(last_err)
    }

    pub fn polygon_url(path: &str) -> String {
        format!("{POLYGON_BASE}{path}")
    }
}
