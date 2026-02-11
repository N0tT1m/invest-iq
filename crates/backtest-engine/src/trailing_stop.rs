use std::collections::HashMap;

use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Manages trailing stop prices for open positions.
pub struct TrailingStopManager {
    /// symbol → current trailing stop price
    stops: HashMap<String, Decimal>,
    trailing_pct: Decimal,
}

impl TrailingStopManager {
    pub fn new(trailing_stop_percent: f64) -> Self {
        Self {
            stops: HashMap::new(),
            trailing_pct: Decimal::from_f64(trailing_stop_percent).unwrap_or(Decimal::ZERO),
        }
    }

    /// Initialize a trailing stop for a new position.
    pub fn init(&mut self, symbol: &str, entry_price: Decimal) {
        let stop = entry_price * (Decimal::ONE - self.trailing_pct);
        self.stops.insert(symbol.to_string(), stop);
    }

    /// Update the trailing stop based on the bar's high price.
    /// The stop ratchets up but never down.
    /// Returns the current stop price.
    pub fn update(&mut self, symbol: &str, bar_high: Decimal) -> Option<Decimal> {
        if let Some(stop) = self.stops.get_mut(symbol) {
            let new_stop = bar_high * (Decimal::ONE - self.trailing_pct);
            if new_stop > *stop {
                *stop = new_stop;
            }
            Some(*stop)
        } else {
            None
        }
    }

    /// Get the current trailing stop price for a symbol.
    pub fn get(&self, symbol: &str) -> Option<Decimal> {
        self.stops.get(symbol).copied()
    }

    /// Remove the trailing stop for a closed position.
    pub fn remove(&mut self, symbol: &str) {
        self.stops.remove(symbol);
    }
}
