use rust_decimal::Decimal;

use crate::models::{PendingLimitOrder, Signal};

/// Manages pending limit orders.
#[derive(Default)]
pub struct LimitOrderManager {
    pub pending: Vec<PendingLimitOrder>,
}

impl LimitOrderManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new limit order from a signal.
    pub fn add_order(&mut self, signal: Signal, direction: &str) {
        let expiry = signal.limit_expiry_bars.unwrap_or(5);
        self.pending.push(PendingLimitOrder {
            signal,
            bars_remaining: expiry,
            direction: direction.to_string(),
        });
    }

    /// Check pending limit orders against current bar.
    /// Returns signals that should execute (limit price was reached).
    /// Decrements bars_remaining and removes expired orders.
    pub fn check_and_expire(
        &mut self,
        bar_low: Decimal,
        bar_high: Decimal,
    ) -> Vec<(Signal, String)> {
        let mut triggered = Vec::new();
        let mut remaining = Vec::new();

        for mut order in self.pending.drain(..) {
            let limit_price = order.signal.limit_price.unwrap_or(order.signal.price);

            let is_buy = order.direction == "buy";
            let is_triggered = if is_buy {
                // Buy limit: triggers when price drops to or below limit
                bar_low <= limit_price
            } else {
                // Sell limit: triggers when price rises to or above limit
                bar_high >= limit_price
            };

            if is_triggered {
                triggered.push((order.signal, order.direction));
            } else {
                order.bars_remaining -= 1;
                if order.bars_remaining > 0 {
                    remaining.push(order);
                }
                // else: expired, dropped
            }
        }

        self.pending = remaining;
        triggered
    }
}
