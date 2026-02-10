use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use crate::error::{MLError, MLResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradePrediction {
    pub probability: f64,
    pub expected_return: f64,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrateResponse {
    pub calibrated_confidence: f64,
    pub reliability_tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineWeights {
    pub weights: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCalibrateResponse {
    pub calibrations: HashMap<String, CalibrateResponse>,
}

#[derive(Debug, Clone, Serialize)]
struct PredictRequest {
    features: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize)]
struct CalibrateRequest {
    engine: String,
    raw_confidence: f64,
    signal_strength: i32,
    market_regime: String,
}

#[derive(Debug, Clone, Serialize)]
struct BatchCalibrateRequest {
    engines: HashMap<String, f64>,
    market_regime: String,
}

#[derive(Debug, Clone, Serialize)]
struct WeightsRequest {
    features: HashMap<String, f64>,
}

#[derive(Clone)]
pub struct SignalModelsClient {
    client: reqwest::Client,
    base_url: String,
}

impl SignalModelsClient {
    pub fn new(base_url: String, timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self { client, base_url }
    }

    /// Meta-model: should we take this trade?
    pub async fn predict_trade(
        &self,
        features: &HashMap<String, f64>,
    ) -> MLResult<TradePrediction> {
        let request = PredictRequest {
            features: features.clone(),
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

        let result = response.json::<TradePrediction>().await?;
        Ok(result)
    }

    /// Calibrate a raw confidence score for a specific engine
    pub async fn calibrate_confidence(
        &self,
        engine: &str,
        raw_confidence: f64,
        signal_strength: i32,
        regime: &str,
    ) -> MLResult<f64> {
        let request = CalibrateRequest {
            engine: engine.to_string(),
            raw_confidence,
            signal_strength,
            market_regime: regime.to_string(),
        };

        let response = self
            .client
            .post(&format!("{}/calibrate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        let result = response.json::<CalibrateResponse>().await?;
        Ok(result.calibrated_confidence)
    }

    /// Calibrate all 4 engine confidences at once
    pub async fn batch_calibrate(
        &self,
        engines: &HashMap<String, f64>,
        regime: &str,
    ) -> MLResult<HashMap<String, CalibrateResponse>> {
        let request = BatchCalibrateRequest {
            engines: engines.clone(),
            market_regime: regime.to_string(),
        };

        let response = self
            .client
            .post(&format!("{}/batch-calibrate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        let result = response.json::<BatchCalibrateResponse>().await?;
        Ok(result.calibrations)
    }

    /// Get optimal engine weights for current conditions
    pub async fn get_optimal_weights(
        &self,
        features: &HashMap<String, f64>,
    ) -> MLResult<EngineWeights> {
        let request = WeightsRequest {
            features: features.clone(),
        };

        let response = self
            .client
            .post(&format!("{}/weights", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MLError::ServiceUnavailable(format!(
                "Status: {}",
                response.status()
            )));
        }

        let result = response.json::<EngineWeights>().await?;
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
