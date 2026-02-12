pub mod bayesian;
pub mod earnings_nlp;
pub mod error;
pub mod price_predictor;
pub mod provider;
pub mod sentiment;
pub mod signal_models;
pub mod social_sentiment;

pub use bayesian::BayesianClient;
pub use earnings_nlp::EarningsNlpClient;
pub use error::{MLError, MLResult};
pub use price_predictor::PricePredictorClient;
pub use provider::{HttpMLProvider, MLProvider};
pub use sentiment::SentimentClient;
pub use signal_models::SignalModelsClient;
pub use social_sentiment::SocialSentimentClient;

use std::time::Duration;

/// Configuration for ML services
#[derive(Debug, Clone)]
pub struct MLConfig {
    pub sentiment_url: String,
    pub bayesian_url: String,
    pub price_predictor_url: String,
    pub signal_models_url: String,
    pub social_sentiment_url: String,
    pub earnings_nlp_url: String,
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
            social_sentiment_url: std::env::var("ML_SOCIAL_SENTIMENT_URL")
                .unwrap_or_else(|_| "http://localhost:8006".to_string()),
            earnings_nlp_url: std::env::var("ML_EARNINGS_NLP_URL")
                .unwrap_or_else(|_| "http://localhost:8005".to_string()),
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
    pub social_sentiment: SocialSentimentClient,
    pub earnings_nlp: EarningsNlpClient,
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
            price_predictor: PricePredictorClient::with_client(
                shared_client.clone(),
                config.price_predictor_url,
            ),
            signal_models: SignalModelsClient::with_client(
                shared_client.clone(),
                config.signal_models_url,
            ),
            social_sentiment: SocialSentimentClient::with_client(
                shared_client.clone(),
                config.social_sentiment_url,
            ),
            earnings_nlp: EarningsNlpClient::with_client(shared_client, config.earnings_nlp_url),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(MLConfig::default())
    }
}
