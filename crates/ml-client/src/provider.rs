use async_trait::async_trait;
use std::collections::HashMap;

use crate::bayesian::{RecommendationResponse, StrategyStats};
use crate::error::MLResult;
use crate::price_predictor::{DirectionPrediction, PriceData};
use crate::sentiment::{NewsSentimentResponse, SentimentResponse};
use crate::signal_models::{CalibrateResponse, EngineWeights, TradePrediction};
use crate::MLClient;

/// Backend-agnostic interface for ML inference.
///
/// Implemented by both the HTTP client (microservice) and PyO3 embedded engine.
#[async_trait]
pub trait MLProvider: Send + Sync {
    // -- Signal Models -------------------------------------------------------
    async fn predict_trade(&self, features: &HashMap<String, f64>) -> MLResult<TradePrediction>;

    async fn batch_calibrate(
        &self,
        engines: &HashMap<String, f64>,
        regime: &str,
    ) -> MLResult<HashMap<String, CalibrateResponse>>;

    async fn get_optimal_weights(&self, features: &HashMap<String, f64>)
        -> MLResult<EngineWeights>;

    // -- Sentiment -----------------------------------------------------------
    async fn predict_sentiment(&self, texts: Vec<String>) -> MLResult<SentimentResponse>;

    async fn analyze_news(
        &self,
        headlines: Vec<String>,
        descriptions: Option<Vec<String>>,
    ) -> MLResult<NewsSentimentResponse>;

    // -- Price Predictor -----------------------------------------------------
    async fn predict_price(
        &self,
        symbol: &str,
        history: Vec<PriceData>,
        horizon: i32,
    ) -> MLResult<DirectionPrediction>;

    // -- Bayesian ------------------------------------------------------------
    async fn update_strategy(&self, name: &str, outcome: i32, pnl: Option<f64>) -> MLResult<()>;

    async fn get_strategy_weights(&self, normalize: bool) -> MLResult<HashMap<String, f64>>;

    async fn get_all_strategy_stats(&self) -> MLResult<Vec<StrategyStats>>;

    async fn get_recommendation(&self, name: &str) -> MLResult<RecommendationResponse>;

    // -- Earnings NLP --------------------------------------------------------
    async fn analyze_earnings(&self, _symbol: &str) -> MLResult<serde_json::Value> {
        Err(crate::error::MLError::ServiceUnavailable(
            "earnings NLP not available".into(),
        ))
    }

    // -- Social Sentiment ----------------------------------------------------
    async fn get_social_sentiment(&self, _symbol: &str) -> MLResult<serde_json::Value> {
        Err(crate::error::MLError::ServiceUnavailable(
            "social sentiment not available".into(),
        ))
    }

    // -- Meta ----------------------------------------------------------------
    fn backend_name(&self) -> &'static str;
}

/// HTTP-backed implementation that delegates to the existing `MLClient`.
pub struct HttpMLProvider {
    client: MLClient,
}

impl HttpMLProvider {
    pub fn new(client: MLClient) -> Self {
        Self { client }
    }
}

impl From<MLClient> for HttpMLProvider {
    fn from(client: MLClient) -> Self {
        Self::new(client)
    }
}

#[async_trait]
impl MLProvider for HttpMLProvider {
    async fn predict_trade(&self, features: &HashMap<String, f64>) -> MLResult<TradePrediction> {
        self.client.signal_models.predict_trade(features).await
    }

    async fn batch_calibrate(
        &self,
        engines: &HashMap<String, f64>,
        regime: &str,
    ) -> MLResult<HashMap<String, CalibrateResponse>> {
        self.client
            .signal_models
            .batch_calibrate(engines, regime)
            .await
    }

    async fn get_optimal_weights(
        &self,
        features: &HashMap<String, f64>,
    ) -> MLResult<EngineWeights> {
        self.client
            .signal_models
            .get_optimal_weights(features)
            .await
    }

    async fn predict_sentiment(&self, texts: Vec<String>) -> MLResult<SentimentResponse> {
        self.client.sentiment.predict(texts, None).await
    }

    async fn analyze_news(
        &self,
        headlines: Vec<String>,
        descriptions: Option<Vec<String>>,
    ) -> MLResult<NewsSentimentResponse> {
        self.client
            .sentiment
            .analyze_news(headlines, descriptions, None)
            .await
    }

    async fn predict_price(
        &self,
        symbol: &str,
        history: Vec<PriceData>,
        horizon: i32,
    ) -> MLResult<DirectionPrediction> {
        self.client
            .price_predictor
            .predict(symbol.to_string(), history, horizon)
            .await
    }

    async fn update_strategy(&self, name: &str, outcome: i32, pnl: Option<f64>) -> MLResult<()> {
        self.client
            .bayesian
            .update_strategy(name.to_string(), outcome, pnl, None)
            .await
    }

    async fn get_strategy_weights(&self, normalize: bool) -> MLResult<HashMap<String, f64>> {
        self.client.bayesian.get_weights(normalize).await
    }

    async fn get_all_strategy_stats(&self) -> MLResult<Vec<StrategyStats>> {
        // The HTTP API returns stats per strategy; gather the well-known ones.
        let names = [
            "technical",
            "fundamental",
            "quantitative",
            "sentiment",
            "meta_model",
        ];
        let mut stats = Vec::new();
        for name in &names {
            if let Ok(s) = self.client.bayesian.get_strategy_stats(name).await {
                stats.push(s);
            }
        }
        Ok(stats)
    }

    async fn get_recommendation(&self, name: &str) -> MLResult<RecommendationResponse> {
        self.client.bayesian.get_recommendation(name).await
    }

    async fn analyze_earnings(&self, symbol: &str) -> MLResult<serde_json::Value> {
        let resp = self.client.earnings_nlp.analyze_earnings(symbol).await?;
        serde_json::to_value(resp)
            .map_err(|e| crate::error::MLError::InvalidResponse(e.to_string()))
    }

    async fn get_social_sentiment(&self, symbol: &str) -> MLResult<serde_json::Value> {
        let resp = self
            .client
            .social_sentiment
            .get_social_sentiment(symbol)
            .await?;
        serde_json::to_value(resp)
            .map_err(|e| crate::error::MLError::InvalidResponse(e.to_string()))
    }

    fn backend_name(&self) -> &'static str {
        "http"
    }
}
