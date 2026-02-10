//! Opportunity Ranking Module
//!
//! Ranks opportunities by personal relevance based on user preferences.

use crate::models::{Opportunity, OpportunitySignal, UserPreference};

/// Weights for ranking factors
#[derive(Debug, Clone)]
pub struct RankingWeights {
    /// Weight for signal strength
    pub signal_weight: f64,
    /// Weight for confidence
    pub confidence_weight: f64,
    /// Weight for symbol affinity
    pub affinity_weight: f64,
    /// Weight for sector preference
    pub sector_weight: f64,
    /// Weight for event importance
    pub event_weight: f64,
    /// Weight for potential return
    pub return_weight: f64,
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self {
            signal_weight: 0.25,
            confidence_weight: 0.20,
            affinity_weight: 0.20,
            sector_weight: 0.15,
            event_weight: 0.10,
            return_weight: 0.10,
        }
    }
}

/// Ranks opportunities by personal relevance
pub struct OpportunityRanker {
    weights: RankingWeights,
}

impl Default for OpportunityRanker {
    fn default() -> Self {
        Self::new()
    }
}

impl OpportunityRanker {
    /// Create a new ranker with default weights
    pub fn new() -> Self {
        Self {
            weights: RankingWeights::default(),
        }
    }

    /// Create ranker with custom weights
    pub fn with_weights(weights: RankingWeights) -> Self {
        Self { weights }
    }

    /// Rank opportunities based on user preferences
    pub fn rank(
        &self,
        opportunities: &mut [Opportunity],
        preferences: &UserPreference,
    ) {
        for opp in opportunities.iter_mut() {
            opp.relevance_score = self.calculate_relevance(opp, preferences);
        }

        // Sort by relevance (descending)
        opportunities.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Calculate relevance score for a single opportunity
    fn calculate_relevance(&self, opp: &Opportunity, prefs: &UserPreference) -> f64 {
        let mut score = 0.0;

        // 1. Signal alignment score
        let signal_score = self.calculate_signal_score(opp, prefs);
        score += signal_score * self.weights.signal_weight;

        // 2. Confidence score
        score += (opp.confidence.clamp(0.0, 1.0) * 100.0) * self.weights.confidence_weight;

        // 3. Symbol affinity score
        let affinity_score = self.calculate_affinity_score(opp, prefs);
        score += affinity_score * self.weights.affinity_weight;

        // 4. Sector preference score
        let sector_score = self.calculate_sector_score(opp, prefs);
        score += sector_score * self.weights.sector_weight;

        // 5. Event importance score
        let event_score = self.calculate_event_score(opp);
        score += event_score * self.weights.event_weight;

        // 6. Potential return score
        let return_score = self.calculate_return_score(opp, prefs);
        score += return_score * self.weights.return_weight;

        // Ensure score is in 0-100 range
        score.clamp(0.0, 100.0)
    }

    /// Calculate signal alignment score
    fn calculate_signal_score(&self, opp: &Opportunity, prefs: &UserPreference) -> f64 {
        let signal_str = match opp.signal {
            OpportunitySignal::StrongBuy => "StrongBuy",
            OpportunitySignal::Buy => "Buy",
            OpportunitySignal::Neutral => "Neutral",
            OpportunitySignal::Sell => "Sell",
            OpportunitySignal::StrongSell => "StrongSell",
        };

        // Check if signal type is preferred
        if prefs.preferred_signals.contains(&signal_str.to_string()) {
            100.0
        } else if prefs.preferred_signals.is_empty() {
            // No preference set, use signal strength
            match opp.signal {
                OpportunitySignal::StrongBuy | OpportunitySignal::StrongSell => 100.0,
                OpportunitySignal::Buy | OpportunitySignal::Sell => 75.0,
                OpportunitySignal::Neutral => 25.0,
            }
        } else {
            25.0
        }
    }

    /// Calculate symbol affinity score based on past interactions
    fn calculate_affinity_score(&self, opp: &Opportunity, prefs: &UserPreference) -> f64 {
        // Check if this is an excluded symbol
        if prefs.excluded_symbols.contains(&opp.symbol) {
            return 0.0;
        }

        // Check symbol affinity from learned preferences
        if let Some(&affinity) = prefs.symbol_affinities.get(&opp.symbol) {
            // Convert -1 to 1 range to 0 to 100
            (affinity + 1.0) * 50.0
        } else {
            // No prior interaction, neutral score
            50.0
        }
    }

    /// Calculate sector preference score
    fn calculate_sector_score(&self, opp: &Opportunity, prefs: &UserPreference) -> f64 {
        if let Some(sector) = &opp.sector {
            // Check explicit preference
            if prefs.preferred_sectors.contains(sector) {
                return 100.0;
            }

            // Check learned sector affinity
            if let Some(&affinity) = prefs.sector_affinities.get(sector) {
                return (affinity + 1.0) * 50.0;
            }
        }

        // No preference or unknown sector
        50.0
    }

    /// Calculate event importance score
    fn calculate_event_score(&self, opp: &Opportunity) -> f64 {
        if let Some(ref event_type) = opp.event_type {
            match event_type {
                crate::models::EventType::Earnings => 100.0,
                crate::models::EventType::FDA => 90.0,
                crate::models::EventType::Dividend => 50.0,
                crate::models::EventType::Split => 40.0,
                crate::models::EventType::Conference => 30.0,
                crate::models::EventType::Other(_) => 25.0,
            }
        } else {
            0.0
        }
    }

    /// Calculate potential return score adjusted for risk tolerance
    fn calculate_return_score(&self, opp: &Opportunity, prefs: &UserPreference) -> f64 {
        if let Some(potential_return) = opp.potential_return {
            // Adjust score based on risk tolerance
            let base_score = (potential_return.abs().min(50.0) / 50.0) * 100.0;

            // High risk tolerance = reward higher potential returns more
            // Low risk tolerance = penalize very high returns (too risky)
            if prefs.risk_tolerance > 0.7 {
                base_score
            } else if prefs.risk_tolerance < 0.3 {
                // Conservative: cap benefit of high returns
                base_score.min(50.0)
            } else {
                base_score * 0.8
            }
        } else {
            25.0
        }
    }

    /// Filter opportunities that don't meet minimum relevance threshold
    pub fn filter_relevant(&self, opportunities: &[Opportunity], min_relevance: f64) -> Vec<Opportunity> {
        opportunities
            .iter()
            .filter(|o| o.relevance_score >= min_relevance)
            .cloned()
            .collect()
    }

    /// Get top N opportunities
    pub fn top_n(&self, opportunities: &[Opportunity], n: usize) -> Vec<Opportunity> {
        opportunities.iter().take(n).cloned().collect()
    }

    /// Group opportunities by category
    pub fn group_by_signal(
        &self,
        opportunities: &[Opportunity],
    ) -> std::collections::HashMap<String, Vec<Opportunity>> {
        let mut groups: std::collections::HashMap<String, Vec<Opportunity>> = std::collections::HashMap::new();

        for opp in opportunities {
            let key = match opp.signal {
                OpportunitySignal::StrongBuy => "Strong Buys",
                OpportunitySignal::Buy => "Buys",
                OpportunitySignal::Neutral => "Neutral",
                OpportunitySignal::Sell => "Sells",
                OpportunitySignal::StrongSell => "Strong Sells",
            }
            .to_string();

            groups.entry(key).or_default().push(opp.clone());
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_opportunity(symbol: &str, confidence: f64) -> Opportunity {
        Opportunity {
            symbol: symbol.to_string(),
            name: Some(format!("{} Inc", symbol)),
            signal: OpportunitySignal::Buy,
            confidence,
            reason: "Test".to_string(),
            summary: "Test opportunity".to_string(),
            event_type: None,
            event_date: None,
            relevance_score: 0.0,
            current_price: Some(100.0),
            price_target: Some(120.0),
            potential_return: Some(20.0),
            sector: Some("Technology".to_string()),
            tags: vec![],
            detected_at: Utc::now(),
            expires_at: None,
        }
    }

    #[test]
    fn test_ranking_basic() {
        let ranker = OpportunityRanker::new();
        let prefs = UserPreference::default();

        let mut opps = vec![
            create_test_opportunity("AAPL", 0.5),
            create_test_opportunity("MSFT", 0.9),
            create_test_opportunity("GOOGL", 0.7),
        ];

        ranker.rank(&mut opps, &prefs);

        // Higher confidence should generally rank higher
        assert!(opps[0].confidence >= opps[1].confidence || opps[0].relevance_score >= opps[1].relevance_score);
    }

    #[test]
    fn test_excluded_symbols() {
        let ranker = OpportunityRanker::new();
        let mut prefs = UserPreference::default();
        prefs.excluded_symbols = vec!["AAPL".to_string()];

        let mut opps = vec![
            create_test_opportunity("AAPL", 0.9),
            create_test_opportunity("MSFT", 0.7),
        ];

        ranker.rank(&mut opps, &prefs);

        // AAPL should be ranked lower despite higher confidence
        assert!(opps.last().unwrap().symbol == "AAPL");
    }

    #[test]
    fn test_filter_relevant() {
        let ranker = OpportunityRanker::new();

        let opps = vec![
            {
                let mut o = create_test_opportunity("AAPL", 0.9);
                o.relevance_score = 80.0;
                o
            },
            {
                let mut o = create_test_opportunity("MSFT", 0.7);
                o.relevance_score = 30.0;
                o
            },
        ];

        let filtered = ranker.filter_relevant(&opps, 50.0);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].symbol, "AAPL");
    }
}
