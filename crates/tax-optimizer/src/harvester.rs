//! Tax-Loss Harvesting Engine
//!
//! Finds opportunities to realize losses for tax purposes.

use crate::substitutes::SubstituteSecurity;
use crate::tax_calculator::{GainType, TaxCalculator, TaxLot, TaxRules};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Priority level for harvesting
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HarvestPriority {
    /// Low priority - small potential savings
    Low,
    /// Medium priority - moderate savings
    Medium,
    /// High priority - significant savings
    High,
    /// Urgent - time-sensitive (e.g., year-end)
    Urgent,
}

impl HarvestPriority {
    /// Determine priority based on tax savings
    pub fn from_savings(savings: f64, is_year_end: bool) -> Self {
        if is_year_end && savings > 100.0 {
            return Self::Urgent;
        }

        if savings >= 1000.0 {
            Self::High
        } else if savings >= 250.0 {
            Self::Medium
        } else {
            Self::Low
        }
    }
}

/// A tax-loss harvesting opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestOpportunity {
    /// Symbol to harvest
    pub symbol: String,
    /// Tax lot ID
    pub lot_id: String,
    /// Number of shares
    pub shares: f64,
    /// Current price
    pub current_price: f64,
    /// Cost basis
    pub cost_basis: f64,
    /// Unrealized loss (positive number)
    pub unrealized_loss: f64,
    /// Estimated tax savings
    pub estimated_tax_savings: f64,
    /// Type of loss (short/long term)
    pub loss_type: GainType,
    /// Priority level
    pub priority: HarvestPriority,
    /// Substitute securities to buy
    pub substitutes: Vec<SubstituteSecurity>,
    /// Date when wash sale safe period begins
    pub wash_sale_safe_date: NaiveDate,
    /// Days until long-term treatment
    pub days_until_long_term: Option<i64>,
    /// Holding period in days
    pub holding_days: i64,
    /// Reason/explanation
    pub reason: String,
}

/// Result of a harvest execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestResult {
    /// Whether harvest was successful
    pub success: bool,
    /// Symbol harvested
    pub symbol: String,
    /// Shares sold
    pub shares_sold: f64,
    /// Sale price
    pub sale_price: f64,
    /// Loss realized
    pub loss_realized: f64,
    /// Tax savings achieved
    pub tax_savings: f64,
    /// Substitute purchased
    pub substitute_symbol: Option<String>,
    /// Substitute shares
    pub substitute_shares: Option<f64>,
    /// Wash sale window end date
    pub wash_sale_window_ends: NaiveDate,
    /// Message
    pub message: String,
}

/// Harvesting engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestingConfig {
    /// Minimum loss to consider (in dollars)
    pub min_loss_threshold: f64,
    /// Minimum tax savings to consider
    pub min_savings_threshold: f64,
    /// Whether to include short-term losses
    pub include_short_term: bool,
    /// Whether to include long-term losses
    pub include_long_term: bool,
    /// Maximum number of opportunities to return
    pub max_opportunities: usize,
    /// Whether year-end is approaching
    pub is_year_end: bool,
}

impl Default for HarvestingConfig {
    fn default() -> Self {
        Self {
            min_loss_threshold: 50.0,
            min_savings_threshold: 10.0,
            include_short_term: true,
            include_long_term: true,
            max_opportunities: 20,
            is_year_end: false,
        }
    }
}

/// Engine for finding and executing tax-loss harvesting opportunities
pub struct HarvestingEngine {
    calculator: TaxCalculator,
    config: HarvestingConfig,
}

impl HarvestingEngine {
    /// Create a new harvesting engine
    pub fn new(calculator: TaxCalculator) -> Self {
        Self {
            calculator,
            config: HarvestingConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(calculator: TaxCalculator, config: HarvestingConfig) -> Self {
        Self { calculator, config }
    }

    /// Set configuration
    pub fn set_config(&mut self, config: HarvestingConfig) {
        self.config = config;
    }

    /// Find all harvesting opportunities in a portfolio
    pub fn find_opportunities(
        &self,
        lots: &[TaxLot],
        current_prices: &std::collections::HashMap<String, f64>,
    ) -> Vec<HarvestOpportunity> {
        let mut opportunities = Vec::new();
        let today = Utc::now().date_naive();
        let rules = self.calculator.rules();

        for lot in lots {
            if lot.is_closed {
                continue;
            }

            let current_price = match current_prices.get(&lot.symbol) {
                Some(&price) => price,
                None => continue,
            };

            let gain_loss = lot.unrealized_gain_loss(current_price);

            // Skip if not a loss
            if gain_loss >= 0.0 {
                continue;
            }

            let unrealized_loss = gain_loss.abs();

            // Skip if below threshold
            if unrealized_loss < self.config.min_loss_threshold {
                continue;
            }

            let estimate = self.calculator.estimate_sale(lot, current_price);

            // Skip based on term preference
            if !self.config.include_short_term && !estimate.gain_type.is_long_term() {
                continue;
            }
            if !self.config.include_long_term && estimate.gain_type.is_long_term() {
                continue;
            }

            let tax_savings = estimate.tax_impact.abs();

            // Skip if savings below threshold
            if tax_savings < self.config.min_savings_threshold {
                continue;
            }

            // Calculate wash sale safe date
            let wash_sale_safe_date = today + chrono::Duration::days(rules.wash_sale_window_days as i64 + 1);

            let priority = HarvestPriority::from_savings(tax_savings, self.config.is_year_end);

            let reason = self.generate_reason(&lot, unrealized_loss, tax_savings, estimate.days_until_long_term);

            opportunities.push(HarvestOpportunity {
                symbol: lot.symbol.clone(),
                lot_id: lot.id.clone(),
                shares: lot.shares,
                current_price,
                cost_basis: lot.total_cost_basis,
                unrealized_loss,
                estimated_tax_savings: tax_savings,
                loss_type: estimate.gain_type,
                priority,
                substitutes: Vec::new(), // Will be filled by SubstituteFinder
                wash_sale_safe_date,
                days_until_long_term: estimate.days_until_long_term,
                holding_days: estimate.days_held,
                reason,
            });
        }

        // Sort by priority (descending) then by savings (descending)
        opportunities.sort_by(|a, b| {
            b.priority.cmp(&a.priority)
                .then(b.estimated_tax_savings.partial_cmp(&a.estimated_tax_savings).unwrap())
        });

        // Limit results
        opportunities.truncate(self.config.max_opportunities);

        opportunities
    }

    /// Generate a human-readable reason for the opportunity
    fn generate_reason(
        &self,
        lot: &TaxLot,
        loss: f64,
        savings: f64,
        days_until_lt: Option<i64>,
    ) -> String {
        let mut reasons = Vec::new();

        reasons.push(format!(
            "Potential ${:.0} tax savings from ${:.0} loss",
            savings, loss
        ));

        if let Some(days) = days_until_lt {
            if days <= 30 {
                reasons.push(format!(
                    "Consider waiting {} days for long-term treatment",
                    days
                ));
            }
        }

        let loss_pct = (loss / lot.total_cost_basis) * 100.0;
        if loss_pct > 20.0 {
            reasons.push(format!("Significant {:.1}% loss from cost basis", loss_pct));
        }

        reasons.join(". ")
    }

    /// Simulate executing a harvest
    pub fn simulate_harvest(
        &self,
        opportunity: &HarvestOpportunity,
        substitute: Option<&SubstituteSecurity>,
    ) -> HarvestResult {
        let today = Utc::now().date_naive();
        let rules = self.calculator.rules();

        let wash_sale_window_ends = today + chrono::Duration::days(rules.wash_sale_window_days as i64);

        let message = if let Some(sub) = substitute {
            format!(
                "Sell {} shares of {} at ${:.2}, buy {} as substitute. Wash sale window ends {}.",
                opportunity.shares,
                opportunity.symbol,
                opportunity.current_price,
                sub.symbol,
                wash_sale_window_ends
            )
        } else {
            format!(
                "Sell {} shares of {} at ${:.2}. Wash sale window ends {}.",
                opportunity.shares,
                opportunity.symbol,
                opportunity.current_price,
                wash_sale_window_ends
            )
        };

        HarvestResult {
            success: true,
            symbol: opportunity.symbol.clone(),
            shares_sold: opportunity.shares,
            sale_price: opportunity.current_price,
            loss_realized: opportunity.unrealized_loss,
            tax_savings: opportunity.estimated_tax_savings,
            substitute_symbol: substitute.map(|s| s.symbol.clone()),
            substitute_shares: substitute.map(|_| opportunity.shares), // 1:1 substitution
            wash_sale_window_ends,
            message,
        }
    }

    /// Calculate total potential savings from all opportunities
    pub fn total_potential_savings(&self, opportunities: &[HarvestOpportunity]) -> f64 {
        opportunities.iter().map(|o| o.estimated_tax_savings).sum()
    }

    /// Get summary statistics
    pub fn get_summary(&self, opportunities: &[HarvestOpportunity]) -> HarvestSummary {
        let total_savings = self.total_potential_savings(opportunities);
        let total_losses = opportunities.iter().map(|o| o.unrealized_loss).sum();

        let short_term_count = opportunities
            .iter()
            .filter(|o| !o.loss_type.is_long_term())
            .count();

        let long_term_count = opportunities.len() - short_term_count;

        let high_priority_count = opportunities
            .iter()
            .filter(|o| o.priority >= HarvestPriority::High)
            .count();

        HarvestSummary {
            total_opportunities: opportunities.len(),
            total_potential_savings: total_savings,
            total_harvestable_losses: total_losses,
            short_term_opportunities: short_term_count,
            long_term_opportunities: long_term_count,
            high_priority_opportunities: high_priority_count,
        }
    }
}

/// Summary of harvesting opportunities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestSummary {
    pub total_opportunities: usize,
    pub total_potential_savings: f64,
    pub total_harvestable_losses: f64,
    pub short_term_opportunities: usize,
    pub long_term_opportunities: usize,
    pub high_priority_opportunities: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tax_calculator::TaxJurisdiction;
    use std::collections::HashMap;

    #[test]
    fn test_find_opportunities() {
        let calculator = TaxCalculator::new(TaxJurisdiction::US);
        let engine = HarvestingEngine::new(calculator);

        let lots = vec![
            TaxLot::new(
                "lot1".to_string(),
                "AAPL".to_string(),
                10.0,
                200.0,
                NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            ),
            TaxLot::new(
                "lot2".to_string(),
                "MSFT".to_string(),
                5.0,
                300.0,
                NaiveDate::from_ymd_opt(2023, 6, 1).unwrap(),
            ),
        ];

        let mut prices = HashMap::new();
        prices.insert("AAPL".to_string(), 150.0); // Loss
        prices.insert("MSFT".to_string(), 350.0); // Gain

        let opportunities = engine.find_opportunities(&lots, &prices);

        // Should find AAPL opportunity (loss), not MSFT (gain)
        assert_eq!(opportunities.len(), 1);
        assert_eq!(opportunities[0].symbol, "AAPL");
        assert!((opportunities[0].unrealized_loss - 500.0).abs() < 0.01); // 10 * (200 - 150)
    }

    #[test]
    fn test_priority_levels() {
        assert_eq!(HarvestPriority::from_savings(50.0, false), HarvestPriority::Low);
        assert_eq!(HarvestPriority::from_savings(500.0, false), HarvestPriority::Medium);
        assert_eq!(HarvestPriority::from_savings(2000.0, false), HarvestPriority::High);
        assert_eq!(HarvestPriority::from_savings(500.0, true), HarvestPriority::Urgent);
    }
}
