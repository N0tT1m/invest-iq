use crate::error::{MLError, MLResult};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
struct CachedPrediction {
    prediction: TradePrediction,
    cached_at: Instant,
}

#[derive(Clone)]
struct CachedCalibration {
    calibrations: HashMap<String, CalibrateResponse>,
    cached_at: Instant,
}

#[derive(Clone)]
struct CachedWeights {
    weights: EngineWeights,
    cached_at: Instant,
}

#[derive(Clone)]
pub struct SignalModelsClient {
    client: reqwest::Client,
    base_url: String,
    prediction_cache: Arc<DashMap<String, CachedPrediction>>,
    calibration_cache: Arc<DashMap<String, CachedCalibration>>,
    weights_cache: Arc<DashMap<String, CachedWeights>>,
    cache_ttl: Duration,
}

impl SignalModelsClient {
    pub fn new(base_url: String, timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url,
            prediction_cache: Arc::new(DashMap::new()),
            calibration_cache: Arc::new(DashMap::new()),
            weights_cache: Arc::new(DashMap::new()),
            cache_ttl: Duration::from_secs(300), // 5 minutes
        }
    }

    pub fn with_client(client: reqwest::Client, base_url: String) -> Self {
        Self {
            client,
            base_url,
            prediction_cache: Arc::new(DashMap::new()),
            calibration_cache: Arc::new(DashMap::new()),
            weights_cache: Arc::new(DashMap::new()),
            cache_ttl: Duration::from_secs(300),
        }
    }

    /// Meta-model: should we take this trade?
    pub async fn predict_trade(
        &self,
        features: &HashMap<String, f64>,
    ) -> MLResult<TradePrediction> {
        // Generate cache key from sorted features
        let cache_key = self.generate_cache_key(features);

        // Check cache first
        if let Some(cached) = self.prediction_cache.get(&cache_key) {
            let age = cached.cached_at.elapsed();
            if age < self.cache_ttl {
                tracing::debug!("Cache hit for prediction (age: {:?})", age);
                return Ok(cached.prediction.clone());
            } else {
                // Remove stale entry
                drop(cached);
                self.prediction_cache.remove(&cache_key);
            }
        }

        // Cache miss - make the HTTP call
        tracing::debug!("Cache miss for prediction, calling ML service");
        let request = PredictRequest {
            features: features.clone(),
        };

        let response = self
            .client
            .post(format!("{}/predict", self.base_url))
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

        // Cache the result
        self.prediction_cache.insert(
            cache_key,
            CachedPrediction {
                prediction: result.clone(),
                cached_at: Instant::now(),
            },
        );

        Ok(result)
    }

    /// Generate deterministic cache key from features HashMap
    fn generate_cache_key(&self, features: &HashMap<String, f64>) -> String {
        let mut sorted_pairs: Vec<_> = features.iter().collect();
        sorted_pairs.sort_by_key(|(k, _)| *k);

        sorted_pairs
            .iter()
            .map(|(k, v)| format!("{}:{:.6}", k, v))
            .collect::<Vec<_>>()
            .join("|")
    }

    /// Clear all caches (predictions, calibrations, weights)
    pub fn clear_cache(&self) {
        self.prediction_cache.clear();
        self.calibration_cache.clear();
        self.weights_cache.clear();
        tracing::info!("All ML caches cleared");
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, String) {
        let pred = self.prediction_cache.len();
        let cal = self.calibration_cache.len();
        let wt = self.weights_cache.len();
        let total = pred + cal + wt;
        let ttl_secs = self.cache_ttl.as_secs();
        (
            total,
            format!(
                "predictions: {}, calibrations: {}, weights: {}, TTL: {}s",
                pred, cal, wt, ttl_secs
            ),
        )
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
            .post(format!("{}/calibrate", self.base_url))
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
        // Generate cache key from engines + regime
        let cache_key = {
            let mut pairs: Vec<_> = engines.iter().collect();
            pairs.sort_by_key(|(k, _)| *k);
            let engines_str: String = pairs
                .iter()
                .map(|(k, v)| format!("{}:{:.6}", k, v))
                .collect::<Vec<_>>()
                .join("|");
            format!("cal:{}:{}", engines_str, regime)
        };

        // Check cache
        if let Some(cached) = self.calibration_cache.get(&cache_key) {
            if cached.cached_at.elapsed() < self.cache_ttl {
                tracing::debug!(
                    "Cache hit for batch_calibrate (age: {:?})",
                    cached.cached_at.elapsed()
                );
                return Ok(cached.calibrations.clone());
            } else {
                drop(cached);
                self.calibration_cache.remove(&cache_key);
            }
        }

        let request = BatchCalibrateRequest {
            engines: engines.clone(),
            market_regime: regime.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/batch-calibrate", self.base_url))
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

        // Cache the result
        self.calibration_cache.insert(
            cache_key,
            CachedCalibration {
                calibrations: result.calibrations.clone(),
                cached_at: Instant::now(),
            },
        );

        Ok(result.calibrations)
    }

    /// Get optimal engine weights for current conditions
    pub async fn get_optimal_weights(
        &self,
        features: &HashMap<String, f64>,
    ) -> MLResult<EngineWeights> {
        // Generate cache key from features
        let cache_key = format!("wt:{}", self.generate_cache_key(features));

        // Check cache
        if let Some(cached) = self.weights_cache.get(&cache_key) {
            if cached.cached_at.elapsed() < self.cache_ttl {
                tracing::debug!(
                    "Cache hit for get_optimal_weights (age: {:?})",
                    cached.cached_at.elapsed()
                );
                return Ok(cached.weights.clone());
            } else {
                drop(cached);
                self.weights_cache.remove(&cache_key);
            }
        }

        let request = WeightsRequest {
            features: features.clone(),
        };

        let response = self
            .client
            .post(format!("{}/weights", self.base_url))
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

        // Cache the result
        self.weights_cache.insert(
            cache_key,
            CachedWeights {
                weights: result.clone(),
                cached_at: Instant::now(),
            },
        );

        Ok(result)
    }

    /// Check service health
    pub async fn health(&self) -> MLResult<bool> {
        let response = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}
