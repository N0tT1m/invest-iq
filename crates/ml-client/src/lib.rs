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

/// Complete ML client with all services
#[derive(Clone)]
pub struct MLClient {
    pub sentiment: SentimentClient,
    pub bayesian: BayesianClient,
    pub price_predictor: PricePredictorClient,
    pub signal_models: SignalModelsClient,
}

impl MLClient {
    pub fn new(config: MLConfig) -> Self {
        Self {
            sentiment: SentimentClient::new(config.sentiment_url.clone(), config.timeout),
            bayesian: BayesianClient::new(config.bayesian_url.clone(), config.timeout),
            price_predictor: PricePredictorClient::new(config.price_predictor_url.clone(), config.timeout),
            signal_models: SignalModelsClient::new(config.signal_models_url.clone(), config.timeout),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(MLConfig::default())
    }
}
