pub mod sentiment;
pub mod bayesian;
pub mod price_predictor;
pub mod signal_models;
pub mod error;

pub use sentiment::SentimentClient;
pub use bayesian::BayesianClient;
pub use price_predictor::PricePredictorClient;
pub use signal_models::SignalModelsClient;
pub use error::{MLError, MLResult};

use std::time::Duration;

/// Configuration for ML services
#[derive(Debug, Clone)]
pub struct MLConfig {
    pub sentiment_url: String,
    pub bayesian_url: String,
    pub price_predictor_url: String,
    pub signal_models_url: String,
    pub timeout: Duration,
}

impl Default for MLConfig {
    fn default() -> Self {
        Self {
            sentiment_url: std::env::var("ML_SENTIMENT_URL")
                .unwrap_or_else(|_| "http://localhost:8001".to_string()),
            bayesian_url: std::env::var("ML_BAYESIAN_URL")
                .unwrap_or_else(|_| "http://localhost:8002".to_string()),
            price_predictor_url: std::env::var("ML_PRICE_PREDICTOR_URL")
                .unwrap_or_else(|_| "http://localhost:8003".to_string()),
            signal_models_url: std::env::var("ML_SIGNAL_MODELS_URL")
                .unwrap_or_else(|_| "http://localhost:8004".to_string()),
            timeout: Duration::from_secs(10),
        }
    }
}

/// Complete ML client with all services sharing a single connection pool.
#[derive(Clone)]
pub struct MLClient {
    pub sentiment: SentimentClient,
    pub bayesian: BayesianClient,
    pub price_predictor: PricePredictorClient,
    pub signal_models: SignalModelsClient,
}

impl MLClient {
    pub fn new(config: MLConfig) -> Self {
        // Single shared reqwest client for all ML services — one connection pool.
        let shared_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .pool_max_idle_per_host(20)
            .build()
            .expect("Failed to create shared ML HTTP client");

        Self {
            sentiment: SentimentClient::with_client(shared_client.clone(), config.sentiment_url),
            bayesian: BayesianClient::with_client(shared_client.clone(), config.bayesian_url),
            price_predictor: PricePredictorClient::with_client(shared_client.clone(), config.price_predictor_url),
            signal_models: SignalModelsClient::with_client(shared_client, config.signal_models_url),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(MLConfig::default())
    }
}
