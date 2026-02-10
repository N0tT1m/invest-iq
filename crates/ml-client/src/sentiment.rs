use serde::{Deserialize, Serialize};
use std::time::Duration;
use crate::error::{MLError, MLResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentPrediction {
    pub label: String,
    pub positive: f64,
    pub negative: f64,
    pub neutral: f64,
    pub confidence: f64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentResponse {
    pub predictions: Vec<SentimentPrediction>,
    pub processing_time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsSentimentResponse {
    pub overall_sentiment: String,
    pub score: f64,
    pub confidence: f64,
    pub positive_ratio: f64,
    pub negative_ratio: f64,
    pub neutral_ratio: f64,
    pub article_count: usize,
    pub processing_time_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
struct SentimentRequest {
    texts: Vec<String>,
    symbol: Option<String>,
    use_cache: bool,
}

#[derive(Debug, Clone, Serialize)]
struct NewsSentimentRequest {
    headlines: Vec<String>,
    descriptions: Option<Vec<String>>,
    symbol: Option<String>,
}

#[derive(Clone)]
pub struct SentimentClient {
    client: reqwest::Client,
    base_url: String,
}

impl SentimentClient {
    pub fn new(base_url: String, timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self { client, base_url }
    }

    /// Predict sentiment for text(s)
    pub async fn predict(
        &self,
        texts: Vec<String>,
        symbol: Option<String>,
    ) -> MLResult<SentimentResponse> {
        let request = SentimentRequest {
            texts,
            symbol,
            use_cache: true,
        };

        let response = self
            .client
            .post(&format!("{}/predict", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        let result = response.json::<SentimentResponse>().await?;
        Ok(result)
    }

    /// Analyze sentiment from news articles
    pub async fn analyze_news(
        &self,
        headlines: Vec<String>,
        descriptions: Option<Vec<String>>,
        symbol: Option<String>,
    ) -> MLResult<NewsSentimentResponse> {
        let request = NewsSentimentRequest {
            headlines,
            descriptions,
            symbol,
        };

        let response = self
            .client
            .post(&format!("{}/analyze-news", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        let result = response.json::<NewsSentimentResponse>().await?;
        Ok(result)
    }

    /// Check service health
    pub async fn health(&self) -> MLResult<bool> {
        let response = self
            .client
            .get(&format!("{}/health", self.base_url))
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}
