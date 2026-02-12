use crate::error::{MLError, MLResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialSentimentResponse {
    pub symbol: String,
    pub mentions: i64,
    pub avg_sentiment: f64,
    pub sentiment_label: String,
    pub buzz_level: String,
    pub trending: bool,
    pub top_posts: Vec<SocialPost>,
    pub subreddits_searched: Vec<String>,
    pub data_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPost {
    pub title: String,
    pub subreddit: String,
    pub score: i64,
    pub num_comments: i64,
    pub sentiment: f64,
    #[serde(default)]
    pub url: String,
}

#[derive(Clone)]
pub struct SocialSentimentClient {
    client: reqwest::Client,
    base_url: String,
}

impl SocialSentimentClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("Failed to build HTTP client"),
            base_url: base_url.into(),
        }
    }

    pub fn with_client(client: reqwest::Client, base_url: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into(),
        }
    }

    pub async fn get_social_sentiment(&self, symbol: &str) -> MLResult<SocialSentimentResponse> {
        let url = format!("{}/social-sentiment/{}", self.base_url, symbol);
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| MLError::ServiceUnavailable(e.to_string()))?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Social sentiment service returned {}",
                response.status()
            )));
        }

        response
            .json::<SocialSentimentResponse>()
            .await
            .map_err(|e| MLError::InvalidResponse(e.to_string()))
    }

    pub async fn health(&self) -> MLResult<bool> {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}
