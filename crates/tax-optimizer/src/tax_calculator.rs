//! Tax Calculator
//!
//! Calculates tax implications for different jurisdictions.

use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Supported tax jurisdictions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaxJurisdiction {
    /// United States - wash sale (30 days), short/long term (1 year)
    US,
    /// United Kingdom - bed and breakfast rule (30 days)
    UK,
    /// Canada - superficial loss (30 days before/after)
    Canada,
    /// Australia - CGT discount (12 months)
    Australia,
    /// Germany - flat tax, no wash sale equivalent
    Germany,
    /// Custom rules
    Custom,
}

impl Default for TaxJurisdiction {
    fn default() -> Self {
        Self::US
    }
}

impl std::fmt::Display for TaxJurisdiction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaxJurisdiction::US => write!(f, "United States"),
            TaxJurisdiction::UK => write!(f, "United Kingdom"),
            TaxJurisdiction::Canada => write!(f, "Canada"),
            TaxJurisdiction::Australia => write!(f, "Australia"),
            TaxJurisdiction::Germany => write!(f, "Germany"),
            TaxJurisdiction::Custom => write!(f, "Custom"),
        }
    }
}

/// Tax rules for a jurisdiction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxRules {
    /// Jurisdiction
    pub jurisdiction: TaxJurisdiction,
    /// Wash sale window in days (before and after)
    pub wash_sale_window_days: u32,
    /// Days to qualify for long-term treatment
    pub long_term_threshold_days: u32,
    /// Short-term capital gains tax rate
    pub short_term_rate: f64,
    /// Long-term capital gains tax rate
    pub long_term_rate: f64,
    /// Maximum loss that can be deducted in a year (None = unlimited)
    pub annual_loss_limit: Option<f64>,
    /// Years losses can be carried forward (None = unlimited)
    pub loss_carryforward_years: Option<u32>,
    /// Whether the jurisdiction has a wash sale equivalent rule
    pub has_wash_sale_rule: bool,
    /// Description of special rules
    pub special_notes: Option<String>,
}

impl TaxRules {
    /// Get rules for US tax jurisdiction
    pub fn us() -> Self {
        Self {
            jurisdiction: TaxJurisdiction::US,
            wash_sale_window_days: 30,
            long_term_threshold_days: 365,
            short_term_rate: 0.37, // Top marginal rate
            long_term_rate: 0.20, // Top rate
            annual_loss_limit: Some(3000.0),
            loss_carryforward_years: None, // Unlimited
            has_wash_sale_rule: true,
            special_notes: Some("Wash sale applies 30 days before and after. $3,000 annual loss limit against ordinary income.".to_string()),
        }
    }

    /// Get rules for UK tax jurisdiction
    pub fn uk() -> Self {
        Self {
            jurisdiction: TaxJurisdiction::UK,
            wash_sale_window_days: 30,
            long_term_threshold_days: 0, // No distinction
            short_term_rate: 0.20, // Higher rate
            long_term_rate: 0.20,
            annual_loss_limit: None,
            loss_carryforward_years: None,
            has_wash_sale_rule: true,
            special_notes: Some("Bed and breakfast rule: 30-day matching rule for same/similar securities.".to_string()),
        }
    }

    /// Get rules for Canada tax jurisdiction
    pub fn canada() -> Self {
        Self {
            jurisdiction: TaxJurisdiction::Canada,
            wash_sale_window_days: 30, // Before AND after
            long_term_threshold_days: 0, // No distinction
            short_term_rate: 0.25, // Approximate
            long_term_rate: 0.25, // 50% inclusion rate
            annual_loss_limit: None,
            loss_carryforward_years: Some(3), // Can carry back 3 years
            has_wash_sale_rule: true,
            special_notes: Some("Superficial loss rule: 30 days before or after. Only 50% of capital gains are taxable.".to_string()),
        }
    }

    /// Get rules for Australia tax jurisdiction
    pub fn australia() -> Self {
        Self {
            jurisdiction: TaxJurisdiction::Australia,
            wash_sale_window_days: 0, // No formal rule, but ATO may challenge
            long_term_threshold_days: 365,
            short_term_rate: 0.45, // Top marginal
            long_term_rate: 0.225, // 50% CGT discount
            annual_loss_limit: None,
            loss_carryforward_years: None,
            has_wash_sale_rule: false, // But ATO can challenge artificial schemes
            special_notes: Some("50% CGT discount for assets held >12 months. No formal wash sale rule but ATO can challenge artificial schemes.".to_string()),
        }
    }

    /// Get rules for Germany tax jurisdiction
    pub fn germany() -> Self {
        Self {
            jurisdiction: TaxJurisdiction::Germany,
            wash_sale_window_days: 0,
            long_term_threshold_days: 0,
            short_term_rate: 0.26375, // 25% + solidarity surcharge
            long_term_rate: 0.26375,
            annual_loss_limit: None, // But offset only against gains
            loss_carryforward_years: None,
            has_wash_sale_rule: false,
            special_notes: Some("Flat 25% Abgeltungssteuer + solidarity surcharge. Losses only offset against same type of gains.".to_string()),
        }
    }

    /// Get rules for a jurisdiction
    pub fn for_jurisdiction(jurisdiction: TaxJurisdiction) -> Self {
        match jurisdiction {
            TaxJurisdiction::US => Self::us(),
            TaxJurisdiction::UK => Self::uk(),
            TaxJurisdiction::Canada => Self::canada(),
            TaxJurisdiction::Australia => Self::australia(),
            TaxJurisdiction::Germany => Self::germany(),
            TaxJurisdiction::Custom => Self::us(), // Default to US
        }
    }
}

/// Holding period classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HoldingPeriod {
    ShortTerm,
    LongTerm,
}

/// Type of gain/loss
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GainType {
    ShortTermGain,
    ShortTermLoss,
    LongTermGain,
    LongTermLoss,
}

impl GainType {
    pub fn is_loss(&self) -> bool {
        matches!(self, GainType::ShortTermLoss | GainType::LongTermLoss)
    }

    pub fn is_long_term(&self) -> bool {
        matches!(self, GainType::LongTermGain | GainType::LongTermLoss)
    }
}

/// A tax lot representing a specific purchase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxLot {
    /// Unique ID
    pub id: String,
    /// Symbol
    pub symbol: String,
    /// Number of shares
    pub shares: f64,
    /// Cost basis per share
    pub cost_basis_per_share: f64,
    /// Total cost basis
    pub total_cost_basis: f64,
    /// Purchase date
    pub purchase_date: NaiveDate,
    /// Sale date (if sold)
    pub sale_date: Option<NaiveDate>,
    /// Sale price per share (if sold)
    pub sale_price_per_share: Option<f64>,
    /// Wash sale adjustment
    pub wash_sale_adjustment: f64,
    /// Whether this lot is closed
    pub is_closed: bool,
}

impl TaxLot {
    /// Create a new tax lot
    pub fn new(
        id: String,
        symbol: String,
        shares: f64,
        cost_basis_per_share: f64,
        purchase_date: NaiveDate,
    ) -> Self {
        Self {
            id,
            symbol,
            shares,
            cost_basis_per_share,
            total_cost_basis: shares * cost_basis_per_share,
            purchase_date,
            sale_date: None,
            sale_price_per_share: None,
            wash_sale_adjustment: 0.0,
            is_closed: false,
        }
    }

    /// Get holding period as of a date
    pub fn holding_period(&self, as_of: NaiveDate, rules: &TaxRules) -> HoldingPeriod {
        let end_date = self.sale_date.unwrap_or(as_of);
        let days_held = (end_date - self.purchase_date).num_days();

        if days_held >= rules.long_term_threshold_days as i64 {
            HoldingPeriod::LongTerm
        } else {
            HoldingPeriod::ShortTerm
        }
    }

    /// Get days held
    pub fn days_held(&self, as_of: NaiveDate) -> i64 {
        let end_date = self.sale_date.unwrap_or(as_of);
        (end_date - self.purchase_date).num_days()
    }

    /// Get days until long-term
    pub fn days_until_long_term(&self, rules: &TaxRules) -> Option<i64> {
        let today = Utc::now().date_naive();
        let days_held = self.days_held(today);
        let threshold = rules.long_term_threshold_days as i64;

        if days_held >= threshold {
            None
        } else {
            Some(threshold - days_held)
        }
    }

    /// Calculate unrealized gain/loss
    pub fn unrealized_gain_loss(&self, current_price: f64) -> f64 {
        let current_value = self.shares * current_price;
        let adjusted_basis = self.total_cost_basis + self.wash_sale_adjustment;
        current_value - adjusted_basis
    }

    /// Get gain type based on current price
    pub fn gain_type(&self, current_price: f64, rules: &TaxRules) -> GainType {
        let gain_loss = self.unrealized_gain_loss(current_price);
        let today = Utc::now().date_naive();
        let is_long_term = self.holding_period(today, rules) == HoldingPeriod::LongTerm;

        match (gain_loss >= 0.0, is_long_term) {
            (true, true) => GainType::LongTermGain,
            (true, false) => GainType::ShortTermGain,
            (false, true) => GainType::LongTermLoss,
            (false, false) => GainType::ShortTermLoss,
        }
    }
}

/// Tax estimate for a potential transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxEstimate {
    /// Gain or loss amount
    pub gain_loss: f64,
    /// Type of gain/loss
    pub gain_type: GainType,
    /// Applicable tax rate
    pub tax_rate: f64,
    /// Estimated tax impact (negative = savings)
    pub tax_impact: f64,
    /// Whether this would trigger wash sale
    pub wash_sale_risk: bool,
    /// Holding period classification
    pub holding_period: HoldingPeriod,
    /// Days held
    pub days_held: i64,
    /// Days until long-term (if applicable)
    pub days_until_long_term: Option<i64>,
}

/// Year-end tax summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YearEndSummary {
    /// Tax year
    pub tax_year: i32,
    /// Jurisdiction
    pub jurisdiction: TaxJurisdiction,
    /// Short-term gains
    pub short_term_gains: f64,
    /// Short-term losses
    pub short_term_losses: f64,
    /// Long-term gains
    pub long_term_gains: f64,
    /// Long-term losses
    pub long_term_losses: f64,
    /// Net short-term
    pub net_short_term: f64,
    /// Net long-term
    pub net_long_term: f64,
    /// Total net gain/loss
    pub total_net: f64,
    /// Estimated tax liability
    pub estimated_tax: f64,
    /// Harvested losses this year
    pub harvested_losses: f64,
    /// Wash sale disallowed losses
    pub wash_sale_disallowed: f64,
    /// Loss carryforward available
    pub loss_carryforward: f64,
    /// Detailed lot-by-lot breakdown
    pub lot_details: Vec<LotTaxDetail>,
}

/// Tax detail for a single lot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotTaxDetail {
    pub symbol: String,
    pub shares: f64,
    pub purchase_date: NaiveDate,
    pub sale_date: NaiveDate,
    pub proceeds: f64,
    pub cost_basis: f64,
    pub gain_loss: f64,
    pub gain_type: GainType,
    pub wash_sale_adjustment: f64,
}

/// Tax calculator for a specific jurisdiction
pub struct TaxCalculator {
    rules: TaxRules,
}

impl TaxCalculator {
    /// Create a new tax calculator
    pub fn new(jurisdiction: TaxJurisdiction) -> Self {
        Self {
            rules: TaxRules::for_jurisdiction(jurisdiction),
        }
    }

    /// Create with custom rules
    pub fn with_rules(rules: TaxRules) -> Self {
        Self { rules }
    }

    /// Get the tax rules
    pub fn rules(&self) -> &TaxRules {
        &self.rules
    }

    /// Estimate tax impact for selling a lot
    pub fn estimate_sale(&self, lot: &TaxLot, current_price: f64) -> TaxEstimate {
        let today = Utc::now().date_naive();
        let gain_loss = lot.unrealized_gain_loss(current_price);
        let gain_type = lot.gain_type(current_price, &self.rules);
        let holding_period = lot.holding_period(today, &self.rules);
        let days_held = lot.days_held(today);

        let tax_rate = match gain_type {
            GainType::ShortTermGain | GainType::ShortTermLoss => self.rules.short_term_rate,
            GainType::LongTermGain | GainType::LongTermLoss => self.rules.long_term_rate,
        };

        // Tax impact: positive = tax owed, negative = tax savings
        let tax_impact = if gain_loss >= 0.0 {
            gain_loss * tax_rate
        } else {
            gain_loss * tax_rate // Negative, so this is savings
        };

        TaxEstimate {
            gain_loss,
            gain_type,
            tax_rate,
            tax_impact,
            wash_sale_risk: self.rules.has_wash_sale_rule,
            holding_period,
            days_held,
            days_until_long_term: lot.days_until_long_term(&self.rules),
        }
    }

    /// Calculate year-end summary
    pub fn calculate_year_end_summary(
        &self,
        tax_year: i32,
        closed_lots: &[TaxLot],
        harvested_losses: f64,
        wash_sale_disallowed: f64,
    ) -> YearEndSummary {
        let mut short_term_gains = 0.0;
        let mut short_term_losses = 0.0;
        let mut long_term_gains = 0.0;
        let mut long_term_losses = 0.0;
        let mut lot_details = Vec::new();

        for lot in closed_lots {
            if !lot.is_closed {
                continue;
            }

            let sale_date = match lot.sale_date {
                Some(d) if d.year() == tax_year => d,
                _ => continue,
            };

            let sale_price = lot.sale_price_per_share.unwrap_or(0.0);
            let proceeds = lot.shares * sale_price;
            let adjusted_basis = lot.total_cost_basis + lot.wash_sale_adjustment;
            let gain_loss = proceeds - adjusted_basis;

            let gain_type = if gain_loss >= 0.0 {
                if lot.holding_period(sale_date, &self.rules) == HoldingPeriod::LongTerm {
                    long_term_gains += gain_loss;
                    GainType::LongTermGain
                } else {
                    short_term_gains += gain_loss;
                    GainType::ShortTermGain
                }
            } else {
                if lot.holding_period(sale_date, &self.rules) == HoldingPeriod::LongTerm {
                    long_term_losses += gain_loss.abs();
                    GainType::LongTermLoss
                } else {
                    short_term_losses += gain_loss.abs();
                    GainType::ShortTermLoss
                }
            };

            lot_details.push(LotTaxDetail {
                symbol: lot.symbol.clone(),
                shares: lot.shares,
                purchase_date: lot.purchase_date,
                sale_date,
                proceeds,
                cost_basis: adjusted_basis,
                gain_loss,
                gain_type,
                wash_sale_adjustment: lot.wash_sale_adjustment,
            });
        }

        let net_short_term = short_term_gains - short_term_losses;
        let net_long_term = long_term_gains - long_term_losses;
        let total_net = net_short_term + net_long_term;

        // Calculate estimated tax
        let estimated_tax = if total_net >= 0.0 {
            // Pay tax on net gains
            let st_tax = if net_short_term > 0.0 {
                net_short_term * self.rules.short_term_rate
            } else {
                0.0
            };
            let lt_tax = if net_long_term > 0.0 {
                net_long_term * self.rules.long_term_rate
            } else {
                0.0
            };
            st_tax + lt_tax
        } else {
            // Net loss - potential deduction
            let deductible = match self.rules.annual_loss_limit {
                Some(limit) => total_net.abs().min(limit),
                None => total_net.abs(),
            };
            -deductible * self.rules.short_term_rate // Savings
        };

        // Loss carryforward
        let loss_carryforward = if total_net < 0.0 {
            match self.rules.annual_loss_limit {
                Some(limit) if total_net.abs() > limit => total_net.abs() - limit,
                _ => 0.0,
            }
        } else {
            0.0
        };

        YearEndSummary {
            tax_year,
            jurisdiction: self.rules.jurisdiction,
            short_term_gains,
            short_term_losses,
            long_term_gains,
            long_term_losses,
            net_short_term,
            net_long_term,
            total_net,
            estimated_tax,
            harvested_losses,
            wash_sale_disallowed,
            loss_carryforward,
            lot_details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_tax_rules_us() {
        let rules = TaxRules::us();
        assert_eq!(rules.jurisdiction, TaxJurisdiction::US);
        assert_eq!(rules.wash_sale_window_days, 30);
        assert_eq!(rules.long_term_threshold_days, 365);
    }

    #[test]
    fn test_holding_period() {
        let rules = TaxRules::us();
        let purchase_date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let lot = TaxLot::new(
            "test".to_string(),
            "AAPL".to_string(),
            10.0,
            150.0,
            purchase_date,
        );

        // After 100 days - short term
        let short_term_date = purchase_date + Duration::days(100);
        assert_eq!(lot.holding_period(short_term_date, &rules), HoldingPeriod::ShortTerm);

        // After 400 days - long term
        let long_term_date = purchase_date + Duration::days(400);
        assert_eq!(lot.holding_period(long_term_date, &rules), HoldingPeriod::LongTerm);
    }

    #[test]
    fn test_unrealized_gain_loss() {
        let lot = TaxLot::new(
            "test".to_string(),
            "AAPL".to_string(),
            10.0,
            150.0,
            NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        );

        // Gain
        assert!((lot.unrealized_gain_loss(160.0) - 100.0).abs() < 0.01);

        // Loss
        assert!((lot.unrealized_gain_loss(140.0) - (-100.0)).abs() < 0.01);
    }
}
