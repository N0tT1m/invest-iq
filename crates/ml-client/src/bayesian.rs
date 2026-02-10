use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use crate::error::{MLError, MLResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStats {
    pub strategy_name: String,
    pub alpha: f64,
    pub beta: f64,
    pub total_samples: i32,
    pub win_rate: f64,
    pub weight: f64,
    pub credible_interval: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightsResponse {
    pub weights: HashMap<String, f64>,
    pub normalized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThompsonSamplingResponse {
    pub selected_strategies: Vec<String>,
    pub all_weights: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationResponse {
    pub use_strategy: bool,
    pub reason: String,
    pub confidence: f64,
    pub expected_win_rate: Option<f64>,
    pub credible_interval: Option<(f64, f64)>,
    pub samples: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateRequest {
    strategy_name: String,
    outcome: i32,
    profit_loss: Option<f64>,
    trade_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct ThompsonSamplingRequest {
    strategies: Vec<String>,
    n_samples: usize,
}

#[derive(Clone)]
pub struct BayesianClient {
    client: reqwest::Client,
    base_url: String,
}

impl BayesianClient {
    pub fn new(base_url: String, timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self { client, base_url }
    }

    /// Update strategy with trade outcome
    pub async fn update_strategy(
        &self,
        strategy_name: String,
        outcome: i32,
        profit_loss: Option<f64>,
        trade_id: Option<i64>,
    ) -> MLResult<()> {
        let request = UpdateRequest {
            strategy_name,
            outcome,
            profit_loss,
            trade_id,
        };

        let response = self
            .client
            .post(&format!("{}/update", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// Get current strategy weights
    pub async fn get_weights(&self, normalize: bool) -> MLResult<HashMap<String, f64>> {
        let response = self
            .client
            .get(&format!("{}/weights", self.base_url))
            .query(&[("normalize", normalize)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        let result = response.json::<WeightsResponse>().await?;
        Ok(result.weights)
    }

    /// Select strategies using Thompson sampling
    pub async fn thompson_sampling(
        &self,
        strategies: Vec<String>,
        n_samples: usize,
    ) -> MLResult<Vec<String>> {
        let request = ThompsonSamplingRequest {
            strategies,
            n_samples,
        };

        let response = self
            .client
            .post(&format!("{}/thompson-sampling", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        let result = response.json::<ThompsonSamplingResponse>().await?;
        Ok(result.selected_strategies)
    }

    /// Get recommendation for a strategy
    pub async fn get_recommendation(
        &self,
        strategy_name: &str,
    ) -> MLResult<RecommendationResponse> {
        let response = self
            .client
            .get(&format!("{}/recommendation/{}", self.base_url, strategy_name))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        let result = response.json::<RecommendationResponse>().await?;
        Ok(result)
    }

    /// Get statistics for a specific strategy
    pub async fn get_strategy_stats(
        &self,
        strategy_name: &str,
    ) -> MLResult<StrategyStats> {
        let response = self
            .client
            .get(&format!("{}/strategy/{}", self.base_url, strategy_name))
            .query(&[("include_ci", true)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        let result = response.json::<StrategyStats>().await?;
        Ok(result)
    }

    /// Sync from database
    pub async fn sync_from_database(&self, days: i32) -> MLResult<()> {
        let response = self
            .client
            .post(&format!("{}/sync-from-database", self.base_url))
            .query(&[("days", days)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        Ok(())
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
