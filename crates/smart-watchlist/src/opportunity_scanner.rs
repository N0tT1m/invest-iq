//! Opportunity Scanner Module
//!
//! Scans the market universe for trading opportunities.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};

use crate::models::{EventType, Opportunity, OpportunitySignal};

/// Configuration for scanning
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Minimum confidence threshold
    pub min_confidence: f64,
    /// Include only actionable signals (Buy/Sell)
    pub actionable_only: bool,
    /// Maximum number of results
    pub limit: usize,
    /// Include symbols with upcoming events
    pub include_events: bool,
    /// Event horizon in days (how far ahead to look for events)
    pub event_horizon_days: i32,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,
            actionable_only: true,
            limit: 50,
            include_events: true,
            event_horizon_days: 7,
        }
    }
}

/// Trait for analysis providers
#[async_trait]
pub trait AnalysisProvider: Send + Sync {
    /// Analyze a symbol and return opportunity data
    async fn analyze(&self, symbol: &str) -> Result<SymbolAnalysis>;

    /// Get list of symbols to scan
    async fn get_universe(&self) -> Result<Vec<String>>;
}

/// Analysis result for a symbol
#[derive(Debug, Clone)]
pub struct SymbolAnalysis {
    pub symbol: String,
    pub name: Option<String>,
    pub signal_score: f64,
    pub confidence: f64,
    pub sector: Option<String>,
    pub current_price: Option<f64>,
    pub price_target: Option<f64>,
    pub summary: String,
    pub tags: Vec<String>,
}

/// Scans for opportunities across the market
pub struct OpportunityScanner {
    /// Universe of symbols to scan
    universe: Vec<String>,
    /// Known upcoming events
    events: Vec<UpcomingEvent>,
}

#[derive(Debug, Clone)]
pub struct UpcomingEvent {
    pub symbol: String,
    pub event_type: EventType,
    pub date: chrono::NaiveDate,
    pub description: Option<String>,
}

impl OpportunityScanner {
    /// Create a new scanner with a default universe
    pub fn new() -> Self {
        Self {
            universe: Self::default_universe(),
            events: Vec::new(),
        }
    }

    /// Create scanner with custom universe
    pub fn with_universe(universe: Vec<String>) -> Self {
        Self {
            universe,
            events: Vec::new(),
        }
    }

    /// Set upcoming events
    pub fn with_events(mut self, events: Vec<UpcomingEvent>) -> Self {
        self.events = events;
        self
    }

    /// Default stock universe (popular stocks)
    fn default_universe() -> Vec<String> {
        vec![
            "AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "META", "TSLA", "AMD", "NFLX", "CRM",
            "ORCL", "ADBE", "INTC", "QCOM", "AVGO", "TXN", "MU", "AMAT", "LRCX", "KLAC",
            "JPM", "BAC", "WFC", "GS", "MS", "C", "USB", "PNC", "TFC", "COF",
            "JNJ", "UNH", "PFE", "MRK", "ABBV", "TMO", "LLY", "BMY", "AMGN", "GILD",
            "DIS", "CMCSA", "VZ", "T", "TMUS", "NFLX", "CHTR", "FOXA", "WBD", "PARA",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }

    /// Scan using provided analysis function
    pub async fn scan<F, Fut>(&self, config: &ScanConfig, analyze_fn: F) -> Result<Vec<Opportunity>>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<SymbolAnalysis>>,
    {
        let mut opportunities = Vec::new();

        for symbol in &self.universe {
            match analyze_fn(symbol.clone()).await {
                Ok(analysis) => {
                    if analysis.confidence < config.min_confidence {
                        continue;
                    }

                    let signal = OpportunitySignal::from_score(analysis.signal_score);

                    if config.actionable_only
                        && signal != OpportunitySignal::StrongBuy
                        && signal != OpportunitySignal::Buy
                        && signal != OpportunitySignal::StrongSell
                        && signal != OpportunitySignal::Sell
                    {
                        continue;
                    }

                    // Check for upcoming events
                    let (event_type, event_date) = self.find_upcoming_event(symbol, config.event_horizon_days);

                    let potential_return = if let (Some(target), Some(current)) =
                        (analysis.price_target, analysis.current_price)
                    {
                        Some((target - current) / current * 100.0)
                    } else {
                        None
                    };

                    // Generate reason based on analysis
                    let reason = self.generate_reason(&analysis, &signal, &event_type);

                    let opportunity = Opportunity {
                        symbol: analysis.symbol,
                        name: analysis.name,
                        signal,
                        confidence: analysis.confidence,
                        reason,
                        summary: analysis.summary,
                        event_type,
                        event_date,
                        relevance_score: 50.0, // Will be personalized by ranker
                        current_price: analysis.current_price,
                        price_target: analysis.price_target,
                        potential_return,
                        sector: analysis.sector,
                        tags: analysis.tags,
                        detected_at: Utc::now(),
                        expires_at: Some(Utc::now() + Duration::days(1)),
                    };

                    opportunities.push(opportunity);
                }
                Err(e) => {
                    tracing::warn!("Failed to analyze {}: {}", symbol, e);
                }
            }
        }

        // Sort by confidence and limit
        opportunities.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        opportunities.truncate(config.limit);

        Ok(opportunities)
    }

    /// Find upcoming event for a symbol
    fn find_upcoming_event(
        &self,
        symbol: &str,
        horizon_days: i32,
    ) -> (Option<EventType>, Option<chrono::NaiveDate>) {
        let today = Utc::now().date_naive();
        let horizon = today + Duration::days(horizon_days as i64);

        for event in &self.events {
            if event.symbol == symbol && event.date >= today && event.date <= horizon {
                return (Some(event.event_type.clone()), Some(event.date));
            }
        }

        (None, None)
    }

    /// Generate reason text for the opportunity
    fn generate_reason(
        &self,
        analysis: &SymbolAnalysis,
        signal: &OpportunitySignal,
        event: &Option<EventType>,
    ) -> String {
        let mut parts: Vec<String> = Vec::new();

        match signal {
            OpportunitySignal::StrongBuy => parts.push("Strong bullish signals across multiple indicators".to_string()),
            OpportunitySignal::Buy => parts.push("Positive technical and fundamental setup".to_string()),
            OpportunitySignal::Neutral => parts.push("Mixed signals, wait for confirmation".to_string()),
            OpportunitySignal::Sell => parts.push("Bearish indicators suggest caution".to_string()),
            OpportunitySignal::StrongSell => parts.push("Multiple warning signs, consider reducing exposure".to_string()),
        }

        if let Some(event_type) = event {
            parts.push(format!("Upcoming {}", event_type.as_str()));
        }

        if analysis.confidence >= 0.8 {
            parts.push("High confidence signal".to_string());
        }

        parts.join(". ") + "."
    }

    /// Get the current universe
    pub fn universe(&self) -> &[String] {
        &self.universe
    }

    /// Update the universe
    pub fn set_universe(&mut self, universe: Vec<String>) {
        self.universe = universe;
    }
}

impl Default for OpportunityScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_universe() {
        let scanner = OpportunityScanner::new();
        assert!(!scanner.universe.is_empty());
        assert!(scanner.universe.contains(&"AAPL".to_string()));
    }

    #[test]
    fn test_scan_config_default() {
        let config = ScanConfig::default();
        assert!(config.min_confidence > 0.0);
        assert!(config.actionable_only);
    }
}
