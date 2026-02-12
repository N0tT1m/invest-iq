use crate::error::{MLError, MLResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsNlpResponse {
    pub symbol: String,
    pub overall_tone: String,
    pub tone_score: f64,
    pub confidence: f64,
    pub key_topics: Vec<String>,
    pub guidance_sentiment: String,
    pub guidance_keywords: Vec<String>,
    pub tone_shift: Option<String>,
    pub forward_looking_count: i64,
    pub risk_mentions: i64,
    pub data_source: String,
    pub processing_time_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
struct TranscriptAnalysisRequest {
    symbol: String,
    text: Option<String>,
}

#[derive(Clone)]
pub struct EarningsNlpClient {
    client: reqwest::Client,
    base_url: String,
}

impl EarningsNlpClient {
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

    /// Fetch and analyze earnings transcript for a symbol.
    pub async fn analyze_earnings(&self, symbol: &str) -> MLResult<EarningsNlpResponse> {
        let url = format!("{}/earnings-nlp/{}", self.base_url, symbol);
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| MLError::ServiceUnavailable(e.to_string()))?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Earnings NLP service returned {}",
                response.status()
            )));
        }

        response
            .json::<EarningsNlpResponse>()
            .await
            .map_err(|e| MLError::InvalidResponse(e.to_string()))
    }

    /// Analyze provided transcript text directly.
    pub async fn analyze_transcript(
        &self,
        symbol: &str,
        text: &str,
    ) -> MLResult<EarningsNlpResponse> {
        let url = format!("{}/analyze-transcript", self.base_url);
        let request = TranscriptAnalysisRequest {
            symbol: symbol.to_string(),
            text: Some(text.to_string()),
        };

        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| MLError::ServiceUnavailable(e.to_string()))?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Earnings NLP service returned {}",
                response.status()
            )));
        }

        response
            .json::<EarningsNlpResponse>()
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
