use std::sync::Arc;

use analysis_core::UnifiedAnalysis;
use analysis_core::{SignalStrength, Timeframe};
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
    #[allow(dead_code)]
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

    /// Run the orchestrator on each opportunity concurrently.
    /// Concurrency controlled by `max_concurrent_analyses` config (default 20).
    pub async fn generate_signals(
        &self,
        opportunities: &[MarketOpportunity],
    ) -> Result<Vec<SignalWithAnalysis>> {
        use tokio::sync::Semaphore;

        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_analyses));
        let mut handles = Vec::new();

        for opp in opportunities {
            let sem = Arc::clone(&semaphore);
            let orch = Arc::clone(&self.orchestrator);
            let symbol = opp.symbol.clone();

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                match orch.analyze(&symbol, Timeframe::Day1, 365).await {
                    Ok(analysis) => {
                        // Fetch bars for ATR calculation
                        let bars = orch.get_bars(&symbol, Timeframe::Day1, 20).await.ok();
                        Some((analysis, bars))
                    }
                    Err(e) => {
                        tracing::warn!("Orchestrator analysis failed for {}: {}", symbol, e);
                        None
                    }
                }
            }));
        }

        let mut results = Vec::new();
        for (i, handle) in handles.into_iter().enumerate() {
            if let Ok(Some((analysis, bars))) = handle.await {
                let opp = &opportunities[i];
                if let Some(sig) = self.analysis_to_signal(&analysis, opp, bars.as_deref()) {
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
        bars: Option<&[analysis_core::Bar]>,
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
        let regime = analysis.market_regime.clone();

        // ATR-based dynamic stops (P2)
        let (stop_loss, take_profit, atr_val) =
            compute_atr_stops(entry_price, action, &regime, bars);

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

        // Start with base confidence
        let mut confidence = analysis.overall_confidence;
        let mut adjustments = Vec::new();

        // Supplementary signal integration (P5)
        if let Some(ref supp) = analysis.supplementary_signals {
            // Smart money composite
            if let Some(smart_money) = supp.get("smart_money") {
                if let Some(score) = smart_money.get("composite_score").and_then(|v| v.as_f64()) {
                    if score > 0.3 {
                        confidence = (confidence + 0.05).min(0.98);
                        adjustments.push(format!("smart_money_boost(+0.05, score={:.2})", score));
                    } else if score < -0.3 {
                        confidence = (confidence - 0.05).max(0.05);
                        adjustments.push(format!("smart_money_penalty(-0.05, score={:.2})", score));
                    }
                }
            }

            // Insider signal
            if let Some(insiders) = supp.get("insiders") {
                if let Some(exec_buys) =
                    insiders.get("executive_buy_count").and_then(|v| v.as_i64())
                {
                    if exec_buys >= 2 {
                        adjustments.push(format!("insider_buying(exec_buys={})", exec_buys));
                    }
                }
            }

            // Options-implied volatility check
            if let Some(options) = supp.get("options") {
                if let Some(iv_pct) = options.get("iv_percentile").and_then(|v| v.as_f64()) {
                    if iv_pct > 80.0 && action == "BUY" {
                        confidence = (confidence - 0.03).max(0.05);
                        adjustments.push(format!("high_iv_penalty(-0.03, iv_pct={:.0})", iv_pct));
                    }
                }
                if let Some(pcr) = options.get("put_call_ratio").and_then(|v| v.as_f64()) {
                    if pcr > 1.5 && action == "BUY" {
                        confidence = (confidence - 0.03).max(0.05);
                        adjustments.push(format!("high_put_call_penalty(-0.03, pcr={:.2})", pcr));
                    }
                }
            }

            // Gap analysis
            if let Some(intraday) = supp.get("intraday") {
                if let Some(gap_dir) = intraday.get("gap_direction").and_then(|v| v.as_str()) {
                    if gap_dir == "gap_up" && action == "BUY" {
                        confidence = (confidence + 0.02).min(0.98);
                        adjustments.push("gap_up_boost(+0.02)".to_string());
                    } else if gap_dir == "gap_down" && action == "SELL" {
                        confidence = (confidence + 0.02).min(0.98);
                        adjustments.push("gap_down_boost(+0.02)".to_string());
                    }
                }
            }
        }

        if !adjustments.is_empty() {
            tracing::info!(
                "Signal adjustments for {}: {:?} (confidence {:.2} -> {:.2})",
                opp.symbol,
                adjustments,
                analysis.overall_confidence,
                confidence
            );
        }

        Some(TradingSignal {
            symbol: analysis.symbol.clone(),
            action: action.to_string(),
            confidence,
            strategy_name: "orchestrator".to_string(),
            entry_price,
            stop_loss,
            take_profit,
            historical_win_rate: win_rate,
            technical_reason,
            fundamental_reason,
            sentiment_score,
            atr: atr_val,
            regime,
            signal_adjustments: adjustments,
            conviction_tier: analysis.conviction_tier.clone(),
        })
    }
}

/// Compute 14-period ATR from bars and derive stop-loss / take-profit.
/// Returns (stop_loss, take_profit, Option<atr>).
/// Falls back to 5%/10% percentage-based stops if < 14 bars available.
fn compute_atr_stops(
    entry_price: f64,
    action: &str,
    regime: &Option<String>,
    bars: Option<&[analysis_core::Bar]>,
) -> (f64, f64, Option<f64>) {
    if let Some(bars) = bars {
        if bars.len() >= 14 {
            // Calculate 14-period ATR
            let mut tr_sum = 0.0;
            for i in (bars.len() - 14)..bars.len() {
                let high = bars[i].high;
                let low = bars[i].low;
                let prev_close = if i > 0 {
                    bars[i - 1].close
                } else {
                    bars[i].open
                };
                let tr = (high - low)
                    .max((high - prev_close).abs())
                    .max((low - prev_close).abs());
                tr_sum += tr;
            }
            let atr = tr_sum / 14.0;

            if atr > 0.0 && entry_price > 0.0 {
                // Regime-adaptive multipliers
                let (sl_mult, tp_mult) = match regime.as_deref() {
                    Some(r) if r.contains("high_vol") => (2.5, 3.5),
                    Some(r) if r.contains("low_vol") => (1.5, 2.5),
                    _ => (2.0, 3.0),
                };

                let (stop_loss, take_profit) = if action == "BUY" {
                    (entry_price - sl_mult * atr, entry_price + tp_mult * atr)
                } else {
                    (entry_price + sl_mult * atr, entry_price - tp_mult * atr)
                };

                tracing::debug!(
                    "ATR stops: atr={:.2}, sl_mult={}, tp_mult={}, SL={:.2}, TP={:.2}",
                    atr,
                    sl_mult,
                    tp_mult,
                    stop_loss,
                    take_profit
                );

                return (stop_loss, take_profit, Some(atr));
            }
        }
    }

    // Fallback: 5% stop-loss, 10% take-profit
    let (stop_loss, take_profit) = if action == "BUY" {
        (entry_price * 0.95, entry_price * 1.10)
    } else {
        (entry_price * 1.05, entry_price * 0.90)
    };

    (stop_loss, take_profit, None)
}
