use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Backtest circuit breaker: halts trading when drawdown exceeds a threshold.
pub struct CircuitBreaker {
    max_drawdown_halt_pct: f64,
    halted: bool,
}

impl CircuitBreaker {
    pub fn new(max_drawdown_halt_percent: f64) -> Self {
        Self {
            max_drawdown_halt_pct: max_drawdown_halt_percent,
            halted: false,
        }
    }

    /// Check if trading should be halted based on current equity vs peak.
    pub fn check(&mut self, equity: Decimal, peak_equity: Decimal) -> bool {
        if self.halted {
            return true;
        }

        let peak_f64 = peak_equity.to_f64().unwrap_or(1.0);
        let equity_f64 = equity.to_f64().unwrap_or(0.0);

        if peak_f64 > 0.0 {
            let drawdown_pct = (peak_f64 - equity_f64) / peak_f64 * 100.0;
            if drawdown_pct >= self.max_drawdown_halt_pct {
                self.halted = true;
                return true;
            }
        }

        false
    }

    pub fn is_halted(&self) -> bool {
        self.halted
    }
}
