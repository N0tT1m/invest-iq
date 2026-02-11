use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Tracks margin utilization during a backtest.
pub struct MarginTracker {
    multiplier: Decimal,
    peak_utilization: f64,
}

impl MarginTracker {
    pub fn new(multiplier: f64) -> Self {
        Self {
            multiplier: Decimal::from_f64(multiplier.max(1.0)).unwrap_or(Decimal::ONE),
            peak_utilization: 0.0,
        }
    }

    /// Get the effective buying power given available cash.
    pub fn buying_power(&self, cash: Decimal) -> Decimal {
        cash * self.multiplier
    }

    /// Update margin utilization tracking.
    ///
    /// `positions_value` = total notional value of all open positions.
    /// `equity` = cash + positions value.
    pub fn update_utilization(&mut self, positions_value: Decimal, equity: Decimal) {
        let equity_f64 = equity.to_f64().unwrap_or(1.0);
        if equity_f64 > 0.0 {
            let utilization = positions_value.to_f64().unwrap_or(0.0) / equity_f64;
            if utilization > self.peak_utilization {
                self.peak_utilization = utilization;
            }
        }
    }

    /// Get the peak margin utilization (as a ratio, e.g. 1.5 = 150% of equity).
    pub fn peak_utilization(&self) -> f64 {
        self.peak_utilization
    }
}
