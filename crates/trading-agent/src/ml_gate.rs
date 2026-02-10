use std::collections::HashMap;
use std::time::Duration;

use analysis_core::UnifiedAnalysis;
use ml_client::SignalModelsClient;

use crate::types::{GateDecision, TradingSignal};

pub struct MLTradeGate {
    client: SignalModelsClient,
}

impl MLTradeGate {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: SignalModelsClient::new(base_url.to_string(), Duration::from_secs(5)),
        }
    }

    /// Evaluate a trade signal using the ML meta-model.
    /// Falls back to confidence threshold if ML service is unavailable.
    pub async fn evaluate_trade(
        &self,
        signal: &TradingSignal,
        analysis: &UnifiedAnalysis,
    ) -> GateDecision {
        let features = self.build_features(signal, analysis);

        match self.client.predict_trade(&features).await {
            Ok(prediction) => {
                let approved = prediction.probability > 0.5;
                GateDecision {
                    approved,
                    probability: prediction.probability,
                    reasoning: format!(
                        "ML model: P(profitable)={:.2}, expected_return={:.2}%, rec={}",
                        prediction.probability,
                        prediction.expected_return,
                        prediction.recommendation
                    ),
                }
            }
            Err(e) => {
                tracing::warn!("ML gate unavailable ({}), falling back to confidence threshold", e);
                let approved = signal.confidence >= 0.75;
                GateDecision {
                    approved,
                    probability: signal.confidence,
                    reasoning: format!(
                        "ML fallback: confidence={:.2} {} threshold 0.75",
                        signal.confidence,
                        if approved { ">=" } else { "<" }
                    ),
                }
            }
        }
    }

    /// Build the 23-feature vector matching the format used by the orchestrator's
    /// `log_analysis_features()` for ML training.
    fn build_features(
        &self,
        signal: &TradingSignal,
        analysis: &UnifiedAnalysis,
    ) -> HashMap<String, f64> {
        let mut features = HashMap::new();

        // Signal scores (-100..100 normalized to -1..1)
        let tech_score = analysis
            .technical
            .as_ref()
            .map(|r| r.signal.to_score() as f64 / 100.0)
            .unwrap_or(0.0);
        let fund_score = analysis
            .fundamental
            .as_ref()
            .map(|r| r.signal.to_score() as f64 / 100.0)
            .unwrap_or(0.0);
        let quant_score = analysis
            .quantitative
            .as_ref()
            .map(|r| r.signal.to_score() as f64 / 100.0)
            .unwrap_or(0.0);
        let sent_score = analysis
            .sentiment
            .as_ref()
            .map(|r| r.signal.to_score() as f64 / 100.0)
            .unwrap_or(0.0);

        features.insert("tech_signal".to_string(), tech_score);
        features.insert("fund_signal".to_string(), fund_score);
        features.insert("quant_signal".to_string(), quant_score);
        features.insert("sent_signal".to_string(), sent_score);

        // Confidences
        features.insert(
            "tech_confidence".to_string(),
            analysis.technical.as_ref().map(|r| r.confidence).unwrap_or(0.0),
        );
        features.insert(
            "fund_confidence".to_string(),
            analysis.fundamental.as_ref().map(|r| r.confidence).unwrap_or(0.0),
        );
        features.insert(
            "quant_confidence".to_string(),
            analysis.quantitative.as_ref().map(|r| r.confidence).unwrap_or(0.0),
        );
        features.insert(
            "sent_confidence".to_string(),
            analysis.sentiment.as_ref().map(|r| r.confidence).unwrap_or(0.0),
        );

        // Technical metrics
        let tech_metrics = analysis
            .technical
            .as_ref()
            .map(|r| &r.metrics)
            .cloned()
            .unwrap_or_default();
        features.insert(
            "rsi".to_string(),
            tech_metrics["rsi"].as_f64().unwrap_or(50.0),
        );
        features.insert(
            "macd_histogram".to_string(),
            tech_metrics["macd_histogram"].as_f64().unwrap_or(0.0),
        );
        features.insert(
            "bb_width".to_string(),
            tech_metrics["bb_width"].as_f64().unwrap_or(0.0),
        );
        features.insert(
            "adx".to_string(),
            tech_metrics["adx"].as_f64().unwrap_or(0.0),
        );

        // Fundamental metrics
        let fund_metrics = analysis
            .fundamental
            .as_ref()
            .map(|r| &r.metrics)
            .cloned()
            .unwrap_or_default();
        features.insert(
            "pe_ratio".to_string(),
            fund_metrics["pe_ratio"].as_f64().unwrap_or(0.0),
        );
        features.insert(
            "debt_to_equity".to_string(),
            fund_metrics["debt_to_equity"].as_f64().unwrap_or(0.0),
        );

        // Quant metrics
        let quant_metrics = analysis
            .quantitative
            .as_ref()
            .map(|r| &r.metrics)
            .cloned()
            .unwrap_or_default();
        features.insert(
            "sharpe_ratio".to_string(),
            quant_metrics["sharpe_ratio"].as_f64().unwrap_or(0.0),
        );
        features.insert(
            "var_95".to_string(),
            quant_metrics["var_95"].as_f64().unwrap_or(0.0),
        );
        features.insert(
            "max_drawdown".to_string(),
            quant_metrics["max_drawdown"].as_f64().unwrap_or(0.0),
        );
        features.insert(
            "volatility".to_string(),
            quant_metrics["volatility"].as_f64().unwrap_or(0.0),
        );

        // Sentiment metrics
        features.insert(
            "news_sentiment_score".to_string(),
            signal.sentiment_score.unwrap_or(0.0),
        );

        // Market context
        let regime_val = match analysis.market_regime.as_deref() {
            Some("high_volatility") => -0.5,
            Some("low_volatility") => 0.5,
            Some("normal") => 0.0,
            _ => 0.0,
        };
        features.insert("regime".to_string(), regime_val);

        // SPY return and VIX proxy — use 0 as defaults since we don't have direct access here
        features.insert("spy_return".to_string(), 0.0);
        features.insert("vix_proxy".to_string(), 0.0);

        features
    }
}
