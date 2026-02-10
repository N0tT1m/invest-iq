use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use crate::error::{MLError, MLResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub vwap: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionPrediction {
    pub direction: String,  // "up", "down", or "neutral"
    pub confidence: f64,
    pub probabilities: HashMap<String, f64>,
    pub horizon_steps: i32,
    pub predicted_prices: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
struct PredictionRequest {
    symbol: String,
    history: Vec<PriceData>,
    horizon_steps: i32,
}

#[derive(Clone)]
pub struct PricePredictorClient {
    client: reqwest::Client,
    base_url: String,
}

impl PricePredictorClient {
    pub fn new(base_url: String, timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self { client, base_url }
    }

    /// Predict price direction for next N steps
    pub async fn predict(
        &self,
        symbol: String,
        history: Vec<PriceData>,
        horizon_steps: i32,
    ) -> MLResult<DirectionPrediction> {
        let request = PredictionRequest {
            symbol,
            history,
            horizon_steps,
        };

        let response = self
            .client
            .post(&format!("{}/predict", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
                return Err(MLError::ModelNotLoaded);
            }
            return Err(MLError::ServiceUnavailable(format!("Status: {}", status)));
        }

        let result = response.json::<DirectionPrediction>().await?;
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

    /// Evaluate model accuracy
    pub async fn evaluate(&self, symbol: String, days: i32) -> MLResult<HashMap<String, f64>> {
        let response = self
            .client
            .get(&format!("{}/evaluate/{}", self.base_url, symbol))
            .query(&[("days", days)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        #[derive(Deserialize)]
        struct EvalResponse {
            metrics: HashMap<String, f64>,
        }

        let result = response.json::<EvalResponse>().await?;
        Ok(result.metrics)
    }
}
