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
                // Dual-gate: ML meta-model AND average calibrated confidence must agree
                let avg_calibrated = if calibrated.is_empty() {
                    signal.confidence
                } else {
                    calibrated.values().sum::<f64>() / calibrated.len() as f64
                };

                let ml_approved = prediction.probability > ml_threshold;
                let confidence_approved = avg_calibrated >= 0.60 || prediction.probability > 0.7;
                let approved = ml_approved && confidence_approved;

                GateDecision {
                    approved,
                    probability: prediction.probability,
                    reasoning: format!(
                        "ML model: P(profitable)={:.2} (threshold={:.2}, regime={}), \
                         expected_return={:.2}%, rec={}, avg_cal_conf={:.2}, dual_gate={}",
                        prediction.probability,
                        ml_threshold,
                        regime,
                        prediction.expected_return,
                        prediction.recommendation,
                        avg_calibrated,
                        if approved { "PASS" } else { "FAIL" }
                    ),
                }
            }
            Err(e) => {
                tracing::warn!("ML gate unavailable ({}), falling back to confidence threshold", e);
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
                }
            }
        }
    }

    /// Calibrate engine confidences via ML service (P6)
    async fn calibrate_confidences(
        &self,
        analysis: &UnifiedAnalysis,
    ) -> HashMap<String, f64> {
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

    /// Compute SPY 1-day return from real bars
    async fn compute_spy_return(&self) -> f64 {
        match self.orchestrator.get_bars("SPY", Timeframe::Day1, 5).await {
            Ok(bars) if bars.len() >= 2 => {
                let prev = bars[bars.len() - 2].close;
                let curr = bars[bars.len() - 1].close;
                if prev > 0.0 { (curr - prev) / prev } else { 0.0 }
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

        features.insert("tech_signal".to_string(), tech_score);
        features.insert("fund_signal".to_string(), fund_score);
        features.insert("quant_signal".to_string(), quant_score);
        features.insert("sent_signal".to_string(), sent_score);

        // Use calibrated confidences if available, else raw
        features.insert(
            "tech_confidence".to_string(),
            *calibrated.get("technical").unwrap_or(
                &analysis.technical.as_ref().map(|r| r.confidence).unwrap_or(0.0),
            ),
        );
        features.insert(
            "fund_confidence".to_string(),
            *calibrated.get("fundamental").unwrap_or(
                &analysis.fundamental.as_ref().map(|r| r.confidence).unwrap_or(0.0),
            ),
        );
        features.insert(
            "quant_confidence".to_string(),
            *calibrated.get("quantitative").unwrap_or(
                &analysis.quantitative.as_ref().map(|r| r.confidence).unwrap_or(0.0),
            ),
        );
        features.insert(
            "sent_confidence".to_string(),
            *calibrated.get("sentiment").unwrap_or(
                &analysis.sentiment.as_ref().map(|r| r.confidence).unwrap_or(0.0),
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

        // Market context — richer 9-regime encoding
        let regime_val = encode_regime(analysis.market_regime.as_deref());
        features.insert("regime".to_string(), regime_val);

        // Real SPY return and VIX proxy
        let (spy_return, vix_proxy) =
            tokio::join!(self.compute_spy_return(), self.compute_vix_proxy());
        features.insert("spy_return".to_string(), spy_return);
        features.insert("vix_proxy".to_string(), vix_proxy);

        tracing::debug!(
            "ML features: spy_return={:.4}, vix_proxy={:.2}, regime={:.2}",
            spy_return,
            vix_proxy,
            regime_val
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
