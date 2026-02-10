use std::sync::Arc;

use analysis_core::{SignalStrength, Timeframe};
use analysis_core::UnifiedAnalysis;
use analysis_orchestrator::AnalysisOrchestrator;
use anyhow::Result;

use crate::config::AgentConfig;
use crate::market_scanner::MarketOpportunity;
use crate::types::TradingSignal;

/// Result of signal generation: the trading signal plus the full analysis
/// (kept around for ML gating).
pub struct SignalWithAnalysis {
    pub signal: TradingSignal,
    pub analysis: UnifiedAnalysis,
}

pub struct StrategyManager {
    config: AgentConfig,
    orchestrator: Arc<AnalysisOrchestrator>,
}

impl StrategyManager {
    pub fn new(config: AgentConfig, orchestrator: Arc<AnalysisOrchestrator>) -> Self {
        Self {
            config,
            orchestrator,
        }
    }

    pub fn strategy_count(&self) -> usize {
        4 // technical + fundamental + quantitative + sentiment via orchestrator
    }

    /// Run the orchestrator on each opportunity concurrently (up to 5 at a time).
    pub async fn generate_signals(
        &self,
        opportunities: &[MarketOpportunity],
    ) -> Result<Vec<SignalWithAnalysis>> {
        use tokio::sync::Semaphore;

        let semaphore = Arc::new(Semaphore::new(5));
        let mut handles = Vec::new();

        for opp in opportunities {
            let sem = Arc::clone(&semaphore);
            let orch = Arc::clone(&self.orchestrator);
            let symbol = opp.symbol.clone();
            let current_price = opp.current_price;

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                match orch.analyze(&symbol, Timeframe::Day1, 365).await {
                    Ok(analysis) => Some((analysis, current_price)),
                    Err(e) => {
                        tracing::warn!("Orchestrator analysis failed for {}: {}", symbol, e);
                        None
                    }
                }
            }));
        }

        let mut results = Vec::new();
        for (i, handle) in handles.into_iter().enumerate() {
            if let Ok(Some((analysis, current_price))) = handle.await {
                let opp = &opportunities[i];
                if let Some(sig) = self.analysis_to_signal(&analysis, opp) {
                    results.push(SignalWithAnalysis {
                        signal: sig,
                        analysis,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Convert a `UnifiedAnalysis` into a `TradingSignal`, or `None` if Neutral / low confidence.
    fn analysis_to_signal(
        &self,
        analysis: &UnifiedAnalysis,
        opp: &MarketOpportunity,
    ) -> Option<TradingSignal> {
        // Determine action from signal strength
        let action = match analysis.overall_signal {
            SignalStrength::StrongBuy | SignalStrength::Buy | SignalStrength::WeakBuy => "BUY",
            SignalStrength::StrongSell | SignalStrength::Sell | SignalStrength::WeakSell => "SELL",
            SignalStrength::Neutral => return None,
        };

        // Skip low-confidence signals early
        if analysis.overall_confidence < 0.5 {
            return None;
        }

        let entry_price = analysis.current_price.unwrap_or(opp.current_price);

        // Default stop-loss / take-profit (5% / 10%)
        let (stop_loss, take_profit) = if action == "BUY" {
            (entry_price * 0.95, entry_price * 1.10)
        } else {
            (entry_price * 1.05, entry_price * 0.90)
        };

        // Extract win rate from quant metrics if available
        let win_rate = analysis
            .quantitative
            .as_ref()
            .and_then(|q| q.metrics["win_rate"].as_f64());

        let technical_reason = analysis
            .technical
            .as_ref()
            .map(|t| t.reason.clone())
            .unwrap_or_else(|| "No technical data".to_string());

        let fundamental_reason = analysis.fundamental.as_ref().map(|f| f.reason.clone());

        let sentiment_score = analysis.sentiment.as_ref().map(|s| s.confidence);

        Some(TradingSignal {
            symbol: analysis.symbol.clone(),
            action: action.to_string(),
            confidence: analysis.overall_confidence,
            strategy_name: "orchestrator".to_string(),
            entry_price,
            stop_loss,
            take_profit,
            historical_win_rate: win_rate,
            technical_reason,
            fundamental_reason,
            sentiment_score,
        })
    }
}
