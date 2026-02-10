//! Wash Sale Rule Monitoring
//!
//! Tracks and prevents wash sale violations across jurisdictions.

use crate::tax_calculator::TaxRules;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Status of a wash sale check
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WashSaleStatus {
    /// Safe to trade - no wash sale risk
    Safe,
    /// In wash sale window - trade would trigger violation
    InWindow,
    /// Violation occurred - loss disallowed
    Violated,
    /// Pending - waiting for window to expire
    Pending,
}

impl std::fmt::Display for WashSaleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WashSaleStatus::Safe => write!(f, "Safe"),
            WashSaleStatus::InWindow => write!(f, "In Window"),
            WashSaleStatus::Violated => write!(f, "Violated"),
            WashSaleStatus::Pending => write!(f, "Pending"),
        }
    }
}

/// A wash sale window period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WashSaleWindow {
    /// Symbol affected
    pub symbol: String,
    /// ID of the sale that created this window
    pub sale_id: String,
    /// Start of wash sale window
    pub window_start: NaiveDate,
    /// End of wash sale window
    pub window_end: NaiveDate,
    /// Loss amount at risk
    pub loss_amount: f64,
    /// Current status
    pub status: WashSaleStatus,
    /// Whether a purchase occurred in the window
    pub triggered: bool,
    /// Triggering purchase date (if any)
    pub triggering_purchase_date: Option<NaiveDate>,
    /// Amount of loss disallowed
    pub disallowed_loss: f64,
}

impl WashSaleWindow {
    /// Create a new wash sale window
    pub fn new(
        symbol: String,
        sale_id: String,
        sale_date: NaiveDate,
        loss_amount: f64,
        rules: &TaxRules,
    ) -> Self {
        let window_days = rules.wash_sale_window_days as i64;

        Self {
            symbol,
            sale_id,
            window_start: sale_date - Duration::days(window_days),
            window_end: sale_date + Duration::days(window_days),
            loss_amount,
            status: WashSaleStatus::Pending,
            triggered: false,
            triggering_purchase_date: None,
            disallowed_loss: 0.0,
        }
    }

    /// Check if a date is within this window
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.window_start && date <= self.window_end
    }

    /// Check if window has expired
    pub fn is_expired(&self, as_of: NaiveDate) -> bool {
        as_of > self.window_end
    }

    /// Days remaining in window
    pub fn days_remaining(&self, as_of: NaiveDate) -> i64 {
        (self.window_end - as_of).num_days().max(0)
    }

    /// Mark as triggered by a purchase
    pub fn trigger(&mut self, purchase_date: NaiveDate) {
        self.triggered = true;
        self.triggering_purchase_date = Some(purchase_date);
        self.status = WashSaleStatus::Violated;
        self.disallowed_loss = self.loss_amount;
    }

    /// Update status based on current date
    pub fn update_status(&mut self, as_of: NaiveDate) {
        if self.triggered {
            self.status = WashSaleStatus::Violated;
        } else if self.is_expired(as_of) {
            self.status = WashSaleStatus::Safe;
        } else {
            self.status = WashSaleStatus::Pending;
        }
    }
}

/// A wash sale violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WashSaleViolation {
    /// Symbol
    pub symbol: String,
    /// Original sale date
    pub sale_date: NaiveDate,
    /// Triggering purchase date
    pub purchase_date: NaiveDate,
    /// Original loss amount
    pub original_loss: f64,
    /// Disallowed loss
    pub disallowed_loss: f64,
    /// Adjustment added to new cost basis
    pub basis_adjustment: f64,
    /// Sale lot ID
    pub sale_lot_id: String,
    /// Purchase lot ID
    pub purchase_lot_id: String,
}

/// Monitor for wash sale rules
pub struct WashSaleMonitor {
    rules: TaxRules,
    windows: Vec<WashSaleWindow>,
    violations: Vec<WashSaleViolation>,
}

impl WashSaleMonitor {
    /// Create a new wash sale monitor
    pub fn new(rules: TaxRules) -> Self {
        Self {
            rules,
            windows: Vec::new(),
            violations: Vec::new(),
        }
    }

    /// Check if a symbol is safe to purchase
    pub fn is_safe_to_purchase(&self, symbol: &str, as_of: NaiveDate) -> WashSaleStatus {
        // If no wash sale rule in this jurisdiction, always safe
        if !self.rules.has_wash_sale_rule {
            return WashSaleStatus::Safe;
        }

        for window in &self.windows {
            if window.symbol == symbol && window.contains(as_of) && !window.triggered {
                return WashSaleStatus::InWindow;
            }
        }

        WashSaleStatus::Safe
    }

    /// Get date when it's safe to purchase a symbol
    pub fn safe_purchase_date(&self, symbol: &str, as_of: NaiveDate) -> NaiveDate {
        if !self.rules.has_wash_sale_rule {
            return as_of;
        }

        let mut latest_window_end = as_of;

        for window in &self.windows {
            if window.symbol == symbol && !window.triggered && window.window_end > latest_window_end {
                latest_window_end = window.window_end;
            }
        }

        if latest_window_end > as_of {
            latest_window_end + Duration::days(1)
        } else {
            as_of
        }
    }

    /// Record a sale that creates a wash sale window
    pub fn record_sale(
        &mut self,
        symbol: String,
        sale_id: String,
        sale_date: NaiveDate,
        loss_amount: f64,
    ) {
        // Only create window if it's a loss
        if loss_amount <= 0.0 {
            return;
        }

        let window = WashSaleWindow::new(symbol, sale_id, sale_date, loss_amount, &self.rules);
        self.windows.push(window);
    }

    /// Record a purchase and check for wash sale violations
    pub fn record_purchase(
        &mut self,
        symbol: &str,
        purchase_lot_id: &str,
        purchase_date: NaiveDate,
    ) -> Vec<WashSaleViolation> {
        let mut new_violations = Vec::new();

        for window in &mut self.windows {
            if window.symbol == symbol && window.contains(purchase_date) && !window.triggered {
                window.trigger(purchase_date);

                let violation = WashSaleViolation {
                    symbol: symbol.to_string(),
                    sale_date: window.window_start + Duration::days(self.rules.wash_sale_window_days as i64),
                    purchase_date,
                    original_loss: window.loss_amount,
                    disallowed_loss: window.loss_amount,
                    basis_adjustment: window.loss_amount,
                    sale_lot_id: window.sale_id.clone(),
                    purchase_lot_id: purchase_lot_id.to_string(),
                };

                new_violations.push(violation.clone());
                self.violations.push(violation);
            }
        }

        new_violations
    }

    /// Get all active windows for a symbol
    pub fn get_active_windows(&self, symbol: &str, as_of: NaiveDate) -> Vec<&WashSaleWindow> {
        self.windows
            .iter()
            .filter(|w| w.symbol == symbol && !w.is_expired(as_of) && !w.triggered)
            .collect()
    }

    /// Get all windows (for display)
    pub fn all_windows(&self) -> &[WashSaleWindow] {
        &self.windows
    }

    /// Get all violations
    pub fn all_violations(&self) -> &[WashSaleViolation] {
        &self.violations
    }

    /// Get total disallowed losses
    pub fn total_disallowed(&self) -> f64 {
        self.violations.iter().map(|v| v.disallowed_loss).sum()
    }

    /// Clean up expired windows
    pub fn cleanup_expired(&mut self, as_of: NaiveDate) {
        // Update statuses first
        for window in &mut self.windows {
            window.update_status(as_of);
        }

        // Remove old, expired windows (keep for 90 days after expiry for reference)
        let cutoff = as_of - Duration::days(90);
        self.windows.retain(|w| w.window_end > cutoff);
    }

    /// Get summary for a tax year
    pub fn year_summary(&self, year: i32) -> WashSaleSummary {
        let year_violations: Vec<_> = self
            .violations
            .iter()
            .filter(|v| v.sale_date.year() == year)
            .collect();

        let total_disallowed = year_violations.iter().map(|v| v.disallowed_loss).sum();
        let affected_symbols: std::collections::HashSet<_> =
            year_violations.iter().map(|v| v.symbol.clone()).collect();

        WashSaleSummary {
            tax_year: year,
            violation_count: year_violations.len(),
            total_disallowed_loss: total_disallowed,
            affected_symbols: affected_symbols.into_iter().collect(),
        }
    }
}

/// Summary of wash sale activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WashSaleSummary {
    pub tax_year: i32,
    pub violation_count: usize,
    pub total_disallowed_loss: f64,
    pub affected_symbols: Vec<String>,
}

/// Calendar view of wash sale windows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WashSaleCalendar {
    /// Symbol
    pub symbol: String,
    /// Windows for this symbol
    pub windows: Vec<WashSaleWindow>,
    /// Next safe purchase date
    pub next_safe_date: NaiveDate,
    /// Days until safe
    pub days_until_safe: i64,
}

impl WashSaleMonitor {
    /// Get calendar view for a symbol
    pub fn get_calendar(&self, symbol: &str) -> WashSaleCalendar {
        let today = Utc::now().date_naive();
        let windows: Vec<_> = self
            .windows
            .iter()
            .filter(|w| w.symbol == symbol)
            .cloned()
            .collect();

        let next_safe = self.safe_purchase_date(symbol, today);
        let days_until_safe = (next_safe - today).num_days();

        WashSaleCalendar {
            symbol: symbol.to_string(),
            windows,
            next_safe_date: next_safe,
            days_until_safe,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tax_calculator::TaxJurisdiction;

    #[test]
    fn test_wash_sale_window() {
        let rules = TaxRules::for_jurisdiction(TaxJurisdiction::US);
        let sale_date = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();

        let window = WashSaleWindow::new(
            "AAPL".to_string(),
            "sale1".to_string(),
            sale_date,
            500.0,
            &rules,
        );

        // Window should span 30 days before and after
        assert_eq!(window.window_start, NaiveDate::from_ymd_opt(2024, 5, 16).unwrap());
        assert_eq!(window.window_end, NaiveDate::from_ymd_opt(2024, 7, 15).unwrap());
    }

    #[test]
    fn test_wash_sale_detection() {
        let rules = TaxRules::for_jurisdiction(TaxJurisdiction::US);
        let mut monitor = WashSaleMonitor::new(rules);

        let sale_date = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
        monitor.record_sale("AAPL".to_string(), "sale1".to_string(), sale_date, 500.0);

        // Purchase within window should trigger violation
        let purchase_date = NaiveDate::from_ymd_opt(2024, 6, 20).unwrap();
        let violations = monitor.record_purchase("AAPL", "purchase1", purchase_date);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].disallowed_loss, 500.0);
    }

    #[test]
    fn test_safe_to_purchase() {
        let rules = TaxRules::for_jurisdiction(TaxJurisdiction::US);
        let mut monitor = WashSaleMonitor::new(rules);

        let sale_date = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
        monitor.record_sale("AAPL".to_string(), "sale1".to_string(), sale_date, 500.0);

        // Should be in window
        let in_window = NaiveDate::from_ymd_opt(2024, 6, 20).unwrap();
        assert_eq!(monitor.is_safe_to_purchase("AAPL", in_window), WashSaleStatus::InWindow);

        // Should be safe after window
        let after_window = NaiveDate::from_ymd_opt(2024, 7, 20).unwrap();
        assert_eq!(monitor.is_safe_to_purchase("AAPL", after_window), WashSaleStatus::Safe);

        // Different symbol should be safe
        assert_eq!(monitor.is_safe_to_purchase("MSFT", in_window), WashSaleStatus::Safe);
    }
}
