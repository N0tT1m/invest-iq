use anyhow::Result;
use log::{info, warn, debug};
use ml_client::{MLClient, MLConfig, PriceData};
use crate::config::AgentConfig;
use crate::market_scanner::MarketOpportunity;
use crate::llm_client::TradingSignal;

/// ML-Enhanced Strategy Manager
///
/// This integrates three ML models:
/// 1. FinBERT sentiment analysis for news
/// 2. Bayesian adaptive strategy weights
/// 3. PatchTST price direction predictor
pub struct MLStrategyManager {
    config: AgentConfig,
    ml_client: MLClient,
}

impl MLStrategyManager {
    pub fn new(config: AgentConfig) -> Result<Self> {
        let ml_config = MLConfig::default();
        let ml_client = MLClient::new(ml_config);

        Ok(Self {
            config,
            ml_client,
        })
    }

    pub fn strategy_count(&self) -> usize {
        5  // Momentum, Mean Reversion, Breakout, Sentiment, High Risk
    }

    /// Generate trading signals with ML enhancements
    pub async fn generate_signals(&self, opportunities: &[MarketOpportunity]) -> Result<Vec<TradingSignal>> {
        let mut signals = Vec::new();

        // Get current strategy weights from Bayesian model
        let strategy_weights = self.ml_client.bayesian
            .get_weights(true)
            .await
            .unwrap_or_default();

        info!("Strategy weights: {:?}", strategy_weights);

        for opp in opportunities {
            // 1. Run traditional strategies
            if let Some(signal) = self.momentum_strategy(opp).await? {
                signals.push(signal);
            }

            if let Some(signal) = self.mean_reversion_strategy(opp).await? {
                signals.push(signal);
            }

            if let Some(signal) = self.breakout_strategy(opp).await? {
                signals.push(signal);
            }

            // 2. Enhanced sentiment strategy using FinBERT
            if let Some(signal) = self.ml_sentiment_strategy(opp).await? {
                signals.push(signal);
            }
        }

        // 3. Apply Bayesian weights to signals
        let weighted_signals = self.apply_bayesian_weights(signals, strategy_weights);

        // 4. Enhance with price prediction
        let enhanced_signals = self.enhance_with_price_prediction(weighted_signals).await?;

        Ok(enhanced_signals)
    }

    /// Enhanced sentiment strategy using FinBERT
    async fn ml_sentiment_strategy(&self, opp: &MarketOpportunity) -> Result<Option<TradingSignal>> {
        // Check if we have news for this symbol
        if opp.news_headlines.is_empty() {
            return Ok(None);
        }

        // Analyze news sentiment using FinBERT
        match self.ml_client.sentiment.analyze_news(
            opp.news_headlines.clone(),
            Some(opp.news_descriptions.clone()),
            Some(opp.symbol.clone())
        ).await {
            Ok(sentiment) => {
                info!("FinBERT sentiment for {}: {} (score: {:.3}, confidence: {:.3})",
                      opp.symbol, sentiment.overall_sentiment, sentiment.score, sentiment.confidence);

                // Generate signal based on sentiment
                let action = match sentiment.overall_sentiment.as_str() {
                    "positive" if sentiment.confidence > 0.6 => "buy",
                    "negative" if sentiment.confidence > 0.6 => "sell",
                    _ => return Ok(None),
                };

                // Calculate confidence based on sentiment metrics
                let confidence = sentiment.confidence *
                                (sentiment.positive_ratio.max(sentiment.negative_ratio));

                // Only generate signal if confidence is high enough
                if confidence < 0.5 {
                    return Ok(None);
                }

                let signal = TradingSignal {
                    symbol: opp.symbol.clone(),
                    action: action.to_string(),
                    confidence,
                    strategy_name: "ml_sentiment".to_string(),
                    reasoning: format!(
                        "FinBERT sentiment: {} (score: {:.2}, confidence: {:.2}%, {}/{} articles positive)",
                        sentiment.overall_sentiment,
                        sentiment.score,
                        sentiment.confidence * 100.0,
                        (sentiment.article_count as f64 * sentiment.positive_ratio) as i32,
                        sentiment.article_count
                    ),
                    target_price: None,
                    stop_loss: None,
                    historical_win_rate: None,
                };

                Ok(Some(signal))
            }
            Err(e) => {
                warn!("Failed to get FinBERT sentiment for {}: {}", opp.symbol, e);
                Ok(None)
            }
        }
    }

    /// Apply Bayesian adaptive weights to signals
    fn apply_bayesian_weights(
        &self,
        mut signals: Vec<TradingSignal>,
        weights: std::collections::HashMap<String, f64>
    ) -> Vec<TradingSignal> {
        for signal in &mut signals {
            // Get weight for this strategy
            let weight = weights.get(&signal.strategy_name).copied().unwrap_or(1.0);

            // Get recommendation from Bayesian model
            let recommendation_future = self.ml_client.bayesian.get_recommendation(&signal.strategy_name);

            // Apply weight to confidence
            let original_confidence = signal.confidence;
            signal.confidence *= weight;

            debug!("Applied Bayesian weight to {}: {:.3} -> {:.3} (weight: {:.3})",
                   signal.strategy_name, original_confidence, signal.confidence, weight);
        }

        signals
    }

    /// Enhance signals with price direction prediction
    async fn enhance_with_price_prediction(&self, mut signals: Vec<TradingSignal>) -> Result<Vec<TradingSignal>> {
        for signal in &mut signals {
            // Get price history for this symbol
            // In production, you would fetch this from your market data provider
            // For now, we'll skip if we can't get the data

            // Skip price prediction enhancement for now
            // In a real implementation, you would:
            // 1. Fetch recent price history (OHLCV data)
            // 2. Call ml_client.price_predictor.predict()
            // 3. Compare prediction direction with signal action
            // 4. Boost confidence if prediction agrees, reduce if it disagrees

            debug!("Price prediction enhancement skipped for {} (would require price history)", signal.symbol);
        }

        Ok(signals)
    }

    /// Helper function to enhance signal with price prediction
    /// This shows how to use the price predictor when you have price history
    #[allow(dead_code)]
    async fn apply_price_prediction_enhancement(
        &self,
        signal: &mut TradingSignal,
        price_history: Vec<PriceData>
    ) -> Result<()> {
        match self.ml_client.price_predictor.predict(
            signal.symbol.clone(),
            price_history,
            4  // Predict next 4 steps (1 hour for 15min bars)
        ).await {
            Ok(prediction) => {
                info!("Price prediction for {}: {} (confidence: {:.3})",
                      signal.symbol, prediction.direction, prediction.confidence);

                // Check if prediction agrees with signal
                let agrees = match (signal.action.as_str(), prediction.direction.as_str()) {
                    ("buy", "up") | ("sell", "down") => true,
                    _ => false,
                };

                if agrees && prediction.confidence > 0.6 {
                    // Boost confidence
                    signal.confidence *= 1.2;
                    signal.confidence = signal.confidence.min(0.95);
                    signal.reasoning = format!(
                        "{} | Price predictor confirms: {} (conf: {:.1}%)",
                        signal.reasoning,
                        prediction.direction,
                        prediction.confidence * 100.0
                    );
                } else if !agrees && prediction.confidence > 0.6 {
                    // Reduce confidence
                    signal.confidence *= 0.7;
                    signal.reasoning = format!(
                        "{} | Price predictor disagrees: {} (conf: {:.1}%)",
                        signal.reasoning,
                        prediction.direction,
                        prediction.confidence * 100.0
                    );
                }
            }
            Err(e) => {
                debug!("Price prediction failed for {}: {}", signal.symbol, e);
            }
        }

        Ok(())
    }

    /// Update strategy performance after trade closes
    pub async fn update_strategy_performance(
        &self,
        strategy_name: &str,
        outcome: bool,  // true for win, false for loss
        profit_loss: f64,
        trade_id: Option<i64>
    ) -> Result<()> {
        let outcome_int = if outcome { 1 } else { 0 };

        match self.ml_client.bayesian.update_strategy(
            strategy_name.to_string(),
            outcome_int,
            Some(profit_loss),
            trade_id
        ).await {
            Ok(_) => {
                info!("Updated Bayesian weights for strategy: {} (outcome: {}, P/L: ${:.2})",
                      strategy_name, if outcome { "WIN" } else { "LOSS" }, profit_loss);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to update Bayesian weights: {}", e);
                Err(e.into())
            }
        }
    }

    /// Get strategy recommendation
    pub async fn get_strategy_recommendation(&self, strategy_name: &str) -> Result<bool> {
        match self.ml_client.bayesian.get_recommendation(strategy_name).await {
            Ok(rec) => {
                info!("Strategy recommendation for {}: {} (confidence: {:.1}%, {})",
                      strategy_name,
                      if rec.use_strategy { "USE" } else { "SKIP" },
                      rec.confidence * 100.0,
                      rec.reason);
                Ok(rec.use_strategy)
            }
            Err(e) => {
                warn!("Failed to get strategy recommendation: {}", e);
                Ok(true)  // Default to allowing strategy
            }
        }
    }

    // Placeholder strategy implementations
    async fn momentum_strategy(&self, _opp: &MarketOpportunity) -> Result<Option<TradingSignal>> {
        Ok(None)
    }

    async fn mean_reversion_strategy(&self, _opp: &MarketOpportunity) -> Result<Option<TradingSignal>> {
        Ok(None)
    }

    async fn breakout_strategy(&self, _opp: &MarketOpportunity) -> Result<Option<TradingSignal>> {
        Ok(None)
    }
}
