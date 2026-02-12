use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use analysis_core::{Timeframe, UnifiedAnalysis};
use analysis_orchestrator::AnalysisOrchestrator;
use ml_client::SignalModelsClient;

use crate::types::{GateDecision, TradingSignal};

pub struct MLTradeGate {
    client: SignalModelsClient,
    orchestrator: Arc<AnalysisOrchestrator>,
}

impl MLTradeGate {
    pub fn new(base_url: &str, orchestrator: Arc<AnalysisOrchestrator>) -> Self {
        Self {
            client: SignalModelsClient::new(base_url.to_string(), Duration::from_secs(5)),
            orchestrator,
        }
    }

    /// Evaluate a trade signal using the ML meta-model.
    /// Falls back to confidence threshold if ML service is unavailable.
    pub async fn evaluate_trade(
        &self,
        signal: &TradingSignal,
        analysis: &UnifiedAnalysis,
    ) -> GateDecision {
        // Calibrate engine confidences before building features
        let calibrated = self.calibrate_confidences(analysis).await;
        let features = self.build_features(signal, analysis, &calibrated).await;

        // Determine regime-conditional threshold
        let regime = analysis.market_regime.as_deref().unwrap_or("normal");
        let ml_threshold = regime_ml_threshold(regime);

        match self.client.predict_trade(&features).await {
            Ok(prediction) => {
                let avg_calibrated = if calibrated.is_empty() {
                    signal.confidence
                } else {
                    calibrated.values().sum::<f64>() / calibrated.len() as f64
                };

                // Three paths to approval:
                // 1. ML model confident: P(profitable) > regime threshold
                // 2. ML model not negative + orchestrator confident: P >= 0.45 AND signal.confidence >= 0.70
                // 3. Strong consensus: avg_calibrated >= 0.65 AND P >= 0.40 (ML not bearish)
                let ml_confident = prediction.probability > ml_threshold;
                let orchestrator_override =
                    prediction.probability >= 0.45 && signal.confidence >= 0.70;
                let consensus_override = avg_calibrated >= 0.65 && prediction.probability >= 0.40;
                let approved = ml_confident || orchestrator_override || consensus_override;

                let gate_reason = if ml_confident {
                    "ML_PASS"
                } else if orchestrator_override {
                    "ORCH_OVERRIDE"
                } else if consensus_override {
                    "CONSENSUS"
                } else {
                    "FAIL"
                };

                GateDecision {
                    approved,
                    probability: prediction.probability,
                    reasoning: format!(
                        "ML: P(win)={:.2} (thr={:.2}, regime={}), \
                         exp_ret={:.2}%, rec={}, avg_cal={:.2}, orch_conf={:.2}, gate={}",
                        prediction.probability,
                        ml_threshold,
                        regime,
                        prediction.expected_return,
                        prediction.recommendation,
                        avg_calibrated,
                        signal.confidence,
                        gate_reason,
                    ),
                    features: Some(features),
                }
            }
            Err(e) => {
                tracing::warn!(
                    "ML gate unavailable ({}), falling back to confidence threshold",
                    e
                );
                // Use the same regime thresholds as the ML model + small buffer
                // (slightly stricter than ML since we lack the meta-model's nuance)
                let fallback_threshold = match regime {
                    r if r.contains("bear") || r.contains("high_vol") => 0.65,
                    r if r.contains("bull") && r.contains("low_vol") => 0.50,
                    _ => 0.55,
                };
                let approved = signal.confidence >= fallback_threshold;
                GateDecision {
                    approved,
                    probability: signal.confidence,
                    reasoning: format!(
                        "ML fallback: confidence={:.2} {} threshold {:.2} (regime={})",
                        signal.confidence,
                        if approved { ">=" } else { "<" },
                        fallback_threshold,
                        regime
                    ),
                    features: None,
                }
            }
        }
    }

    /// Calibrate engine confidences via ML service (P6)
    async fn calibrate_confidences(&self, analysis: &UnifiedAnalysis) -> HashMap<String, f64> {
        let mut engines = HashMap::new();
        if let Some(t) = &analysis.technical {
            engines.insert("technical".to_string(), t.confidence);
        }
        if let Some(f) = &analysis.fundamental {
            engines.insert("fundamental".to_string(), f.confidence);
        }
        if let Some(q) = &analysis.quantitative {
            engines.insert("quantitative".to_string(), q.confidence);
        }
        if let Some(s) = &analysis.sentiment {
            engines.insert("sentiment".to_string(), s.confidence);
        }

        if engines.is_empty() {
            return HashMap::new();
        }

        let regime = analysis.market_regime.as_deref().unwrap_or("normal");

        match self.client.batch_calibrate(&engines, regime).await {
            Ok(calibrations) => {
                let mut result = HashMap::new();
                for (engine, resp) in &calibrations {
                    result.insert(engine.clone(), resp.calibrated_confidence);
                }
                tracing::debug!("Calibrated confidences: {:?}", result);
                result
            }
            Err(e) => {
                tracing::debug!("Calibration unavailable ({}), using raw confidences", e);
                engines
            }
        }
    }

    /// Compute SPY 1-day return from real bars (kept for potential future use)
    #[allow(dead_code)]
    async fn compute_spy_return(&self) -> f64 {
        match self.orchestrator.get_bars("SPY", Timeframe::Day1, 5).await {
            Ok(bars) if bars.len() >= 2 => {
                let prev = bars[bars.len() - 2].close;
                let curr = bars[bars.len() - 1].close;
                if prev > 0.0 {
                    (curr - prev) / prev
                } else {
                    0.0
                }
            }
            Ok(_) => 0.0,
            Err(e) => {
                tracing::debug!("Failed to fetch SPY bars for ML features: {}", e);
                0.0
            }
        }
    }

    /// Compute VIX proxy from SPY bar volatility (stddev of returns × sqrt(252))
    async fn compute_vix_proxy(&self) -> f64 {
        match self.orchestrator.get_bars("SPY", Timeframe::Day1, 22).await {
            Ok(bars) if bars.len() >= 5 => {
                let returns: Vec<f64> = bars
                    .windows(2)
                    .map(|w| (w[1].close - w[0].close) / w[0].close)
                    .collect();
                let mean = returns.iter().sum::<f64>() / returns.len() as f64;
                let variance =
                    returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
                let daily_vol = variance.sqrt();
                // Annualize: daily_vol * sqrt(252), then scale to VIX-like (0-100 range via *100)
                daily_vol * 252.0_f64.sqrt() * 100.0
            }
            Ok(_) => 0.0,
            Err(e) => {
                tracing::debug!("Failed to fetch SPY bars for VIX proxy: {}", e);
                0.0
            }
        }
    }

    /// Build the 23-feature vector matching the format used by the orchestrator's
    /// `log_analysis_features()` for ML training.
    async fn build_features(
        &self,
        signal: &TradingSignal,
        analysis: &UnifiedAnalysis,
        calibrated: &HashMap<String, f64>,
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

        // Use canonical feature names matching FEATURE_NAMES in features.py
        features.insert("technical_score".to_string(), tech_score);
        features.insert("fundamental_score".to_string(), fund_score);
        features.insert("quant_score".to_string(), quant_score);
        features.insert("sentiment_score".to_string(), sent_score);

        // Use calibrated confidences if available, else raw
        features.insert(
            "technical_confidence".to_string(),
            *calibrated.get("technical").unwrap_or(
                &analysis
                    .technical
                    .as_ref()
                    .map(|r| r.confidence)
                    .unwrap_or(0.0),
            ),
        );
        features.insert(
            "fundamental_confidence".to_string(),
            *calibrated.get("fundamental").unwrap_or(
                &analysis
                    .fundamental
                    .as_ref()
                    .map(|r| r.confidence)
                    .unwrap_or(0.0),
            ),
        );
        features.insert(
            "quant_confidence".to_string(),
            *calibrated.get("quantitative").unwrap_or(
                &analysis
                    .quantitative
                    .as_ref()
                    .map(|r| r.confidence)
                    .unwrap_or(0.0),
            ),
        );
        features.insert(
            "sentiment_confidence".to_string(),
            *calibrated.get("sentiment").unwrap_or(
                &analysis
                    .sentiment
                    .as_ref()
                    .map(|r| r.confidence)
                    .unwrap_or(0.0),
            ),
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
            "bb_percent_b".to_string(),
            tech_metrics["bb_percent_b"].as_f64().unwrap_or(0.5),
        );
        features.insert(
            "adx".to_string(),
            tech_metrics["adx"].as_f64().unwrap_or(20.0),
        );
        let sma_20 = tech_metrics["sma_20"].as_f64().unwrap_or(0.0);
        let sma_50 = tech_metrics["sma_50"].as_f64().unwrap_or(0.0);
        let sma_20_vs_50 = if sma_20 > sma_50 {
            1.0
        } else if sma_50 > sma_20 {
            -1.0
        } else {
            0.0
        };
        features.insert("sma_20_vs_50".to_string(), sma_20_vs_50);

        // Fundamental metrics
        let fund_metrics = analysis
            .fundamental
            .as_ref()
            .map(|r| &r.metrics)
            .cloned()
            .unwrap_or_default();
        features.insert(
            "pe_ratio".to_string(),
            fund_metrics["pe_ratio"].as_f64().unwrap_or(20.0),
        );
        features.insert(
            "debt_to_equity".to_string(),
            fund_metrics["debt_to_equity"].as_f64().unwrap_or(1.0),
        );
        features.insert(
            "revenue_growth".to_string(),
            fund_metrics["revenue_growth"].as_f64().unwrap_or(0.0),
        );
        features.insert(
            "roic".to_string(),
            fund_metrics["roic"].as_f64().unwrap_or(0.0),
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
            "volatility".to_string(),
            quant_metrics["volatility"].as_f64().unwrap_or(0.2),
        );
        features.insert(
            "max_drawdown".to_string(),
            quant_metrics["max_drawdown"].as_f64().unwrap_or(0.0),
        );
        features.insert(
            "beta".to_string(),
            quant_metrics["beta"].as_f64().unwrap_or(1.0),
        );

        // Sentiment metrics
        features.insert(
            "normalized_sentiment_score".to_string(),
            signal.sentiment_score.unwrap_or(0.0),
        );
        let sent_metrics = analysis
            .sentiment
            .as_ref()
            .map(|r| &r.metrics)
            .cloned()
            .unwrap_or_default();
        features.insert(
            "article_count".to_string(),
            sent_metrics["article_count"].as_f64().unwrap_or(0.0),
        );
        features.insert(
            "direct_mention_ratio".to_string(),
            sent_metrics["direct_mention_ratio"].as_f64().unwrap_or(0.5),
        );

        // Market context
        let regime_val = encode_regime(analysis.market_regime.as_deref());
        features.insert("market_regime_encoded".to_string(), regime_val);

        // Inter-engine agreement (std dev of scores — lower = more agreement)
        let active_scores: Vec<f64> = [tech_score, fund_score, quant_score, sent_score]
            .iter()
            .filter(|&&s| s != 0.0)
            .copied()
            .collect();
        let agreement = if active_scores.len() >= 2 {
            let mean = active_scores.iter().sum::<f64>() / active_scores.len() as f64;
            let var = active_scores
                .iter()
                .map(|s| (s - mean).powi(2))
                .sum::<f64>()
                / active_scores.len() as f64;
            var.sqrt()
        } else {
            0.0
        };
        features.insert("inter_engine_agreement".to_string(), agreement);

        // VIX proxy from real data
        let vix_proxy = self.compute_vix_proxy().await;
        features.insert("vix_proxy".to_string(), vix_proxy);

        tracing::debug!(
            "ML features: vix_proxy={:.2}, regime={:.2}, agreement={:.2}",
            vix_proxy,
            regime_val,
            agreement,
        );

        features
    }
}

/// Encode 9-regime combinations to a numeric scale for the ML model
fn encode_regime(regime: Option<&str>) -> f64 {
    match regime {
        Some("bull_low_vol") => 1.0,
        Some("bull_normal") => 0.7,
        Some("bull_high_vol") => 0.4,
        Some("sideways_low_vol") => 0.1,
        Some("sideways_normal") | Some("normal") => 0.0,
        Some("sideways_high_vol") => -0.2,
        Some("bear_low_vol") => -0.4,
        Some("bear_normal") => -0.7,
        Some("bear_high_vol") => -1.0,
        // Legacy regime names
        Some("high_volatility") => -0.5,
        Some("low_volatility") => 0.5,
        _ => 0.0,
    }
}

/// Regime-conditional ML threshold for P(profitable)
fn regime_ml_threshold(regime: &str) -> f64 {
    if regime.contains("bear") || regime.contains("high_vol") {
        0.6
    } else if regime.contains("bull") && regime.contains("low_vol") {
        0.45
    } else {
        0.5
    }
}
