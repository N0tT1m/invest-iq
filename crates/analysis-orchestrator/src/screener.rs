use super::AnalysisOrchestrator;
use analysis_core::{SignalStrength, UnifiedAnalysis};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::task::JoinSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSuggestion {
    pub symbol: String,
    pub signal: SignalStrength,
    pub confidence: f64,
    pub score: f64, // Combined score for ranking
    pub recommendation: String,
    pub key_highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenerResult {
    pub suggestions: Vec<StockSuggestion>,
    pub total_analyzed: usize,
    pub total_passed_filters: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub enum StockUniverse {
    Custom(Vec<String>),
    PopularStocks,
    TechStocks,
    BlueChips,
}

impl StockUniverse {
    pub fn get_symbols(&self) -> Vec<String> {
        match self {
            StockUniverse::Custom(symbols) => symbols.clone(),
            StockUniverse::PopularStocks => vec![
                "AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "TSLA", "META", "BRK.B", "V", "JPM",
                "WMT", "MA", "PG", "HD", "DIS", "NFLX", "ADBE", "CRM", "CSCO", "INTC", "AMD",
                "PYPL", "COST", "PEP", "TMO", "MRK", "ABBV", "NKE", "CVX", "MCD",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            StockUniverse::TechStocks => vec![
                "AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "TSLA", "META", "NFLX", "ADBE", "CRM",
                "CSCO", "INTC", "AMD", "PYPL", "ORCL", "IBM", "QCOM", "NOW", "SNOW", "ZM",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            StockUniverse::BlueChips => vec![
                "AAPL", "MSFT", "JPM", "JNJ", "V", "WMT", "PG", "MA", "HD", "DIS", "CVX", "MCD",
                "KO", "PEP", "CSCO", "VZ", "INTC", "MRK", "ABBV", "NKE",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScreenerFilters {
    pub min_confidence: f64,
    pub min_signal_strength: i32, // -3 to 3 (StrongSell to StrongBuy)
    pub limit: usize,
}

impl Default for ScreenerFilters {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,
            min_signal_strength: 0, // Neutral or better
            limit: 10,
        }
    }
}

pub struct StockScreener {
    orchestrator: Arc<AnalysisOrchestrator>,
}

impl StockScreener {
    pub fn new(orchestrator: Arc<AnalysisOrchestrator>) -> Self {
        Self { orchestrator }
    }

    pub async fn screen(
        &self,
        universe: StockUniverse,
        filters: ScreenerFilters,
    ) -> Result<ScreenerResult, anyhow::Error> {
        let symbols = universe.get_symbols();
        let total_analyzed = symbols.len();

        tracing::info!("📊 Starting stock screen of {} symbols", total_analyzed);

        // Analyze all stocks concurrently
        let mut tasks = JoinSet::new();

        for symbol in symbols {
            let orchestrator = Arc::clone(&self.orchestrator);
            tasks.spawn(async move {
                let result = orchestrator
                    .analyze(&symbol, analysis_core::Timeframe::Day1, 365)
                    .await;
                (symbol, result)
            });
        }

        // Collect results
        let mut suggestions = Vec::new();

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok((_symbol, Ok(analysis))) => {
                    // Apply filters
                    if analysis.overall_confidence >= filters.min_confidence
                        && analysis.overall_signal.to_score() >= filters.min_signal_strength
                    {
                        if let Some(suggestion) = self.create_suggestion(analysis) {
                            suggestions.push(suggestion);
                        }
                    }
                }
                Ok((symbol, Err(e))) => {
                    tracing::warn!("Failed to analyze {}: {}", symbol, e);
                }
                Err(e) => {
                    tracing::error!("Task error: {}", e);
                }
            }
        }

        let total_passed_filters = suggestions.len();

        // Sort by score (highest first)
        suggestions.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit results
        suggestions.truncate(filters.limit);

        tracing::info!(
            "✅ Screen complete: {}/{} stocks passed filters, returning top {}",
            total_passed_filters,
            total_analyzed,
            suggestions.len()
        );

        Ok(ScreenerResult {
            suggestions,
            total_analyzed,
            total_passed_filters,
            timestamp: chrono::Utc::now(),
        })
    }

    fn create_suggestion(&self, analysis: UnifiedAnalysis) -> Option<StockSuggestion> {
        // Calculate composite score (0-100)
        let signal_score = (analysis.overall_signal.to_score() + 100) as f64 / 200.0; // Normalize -100..100 to 0..1
        let score =
            ((signal_score * 0.6 + analysis.overall_confidence * 0.4) * 100.0).clamp(0.0, 100.0);

        // Extract key highlights
        let mut highlights = Vec::new();

        // Technical highlights
        if let Some(tech) = &analysis.technical {
            if tech.confidence > 0.6 {
                highlights.push(format!(
                    "Technical: {:?} ({}% conf)",
                    tech.signal,
                    (tech.confidence * 100.0) as i32
                ));
            }
        }

        // Fundamental highlights
        if let Some(fund) = &analysis.fundamental {
            if fund.confidence > 0.6 {
                highlights.push(format!(
                    "Fundamental: {:?} ({}% conf)",
                    fund.signal,
                    (fund.confidence * 100.0) as i32
                ));
            }
        }

        // Quantitative highlights
        if let Some(quant) = &analysis.quantitative {
            if let Some(sharpe) = quant.metrics.get("sharpe_ratio") {
                if let Some(sharpe_val) = sharpe.as_f64() {
                    if sharpe_val > 1.0 {
                        highlights.push(format!("Strong Sharpe Ratio: {:.2}", sharpe_val));
                    }
                }
            }
        }

        // Sentiment highlights
        if let Some(sent) = &analysis.sentiment {
            if sent.confidence > 0.6 {
                highlights.push(format!("Sentiment: {:?}", sent.signal));
            }
        }

        Some(StockSuggestion {
            symbol: analysis.symbol,
            signal: analysis.overall_signal,
            confidence: analysis.overall_confidence,
            score,
            recommendation: analysis.recommendation,
            key_highlights: highlights,
        })
    }
}
