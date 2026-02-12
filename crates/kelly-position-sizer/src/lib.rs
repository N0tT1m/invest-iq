use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Kelly Criterion position sizing calculator
///
/// The Kelly Criterion determines the optimal position size to maximize
/// long-term growth rate. Formula: f* = (bp - q) / b
/// where:
///   f* = optimal fraction of capital to wager
///   b = odds received (profit/loss ratio)
///   p = probability of winning
///   q = probability of losing (1 - p)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KellyPositionSizer {
    /// Maximum fraction of portfolio to risk (e.g., 0.25 = 25%)
    /// This caps the Kelly fraction to prevent over-leveraging
    pub max_kelly_fraction: f64,

    /// Fractional Kelly multiplier (e.g., 0.5 for half-Kelly)
    /// Conservative traders often use 0.25-0.5 to reduce volatility
    pub kelly_multiplier: f64,

    /// Minimum position size as fraction of portfolio (e.g., 0.01 = 1%)
    pub min_position_size: f64,

    /// Maximum position size as fraction of portfolio (e.g., 0.10 = 10%)
    pub max_position_size: f64,
}

/// Strategy performance metrics used for Kelly calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    /// Win rate (0.0 to 1.0)
    pub win_rate: f64,

    /// Average win amount (positive)
    pub avg_win: f64,

    /// Average loss amount (positive, will be treated as negative)
    pub avg_loss: f64,

    /// Number of trades in sample
    pub num_trades: usize,

    /// Confidence in the win rate estimate (0.0 to 1.0)
    /// Lower confidence will reduce position size
    pub confidence: f64,
}

/// Signal with confidence and risk parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub symbol: String,
    pub confidence: f64,
    pub strategy_name: String,
    pub expected_profit_target: f64,
    pub stop_loss_distance: f64,
}

/// Position sizing recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSize {
    /// Fraction of portfolio to allocate (0.0 to 1.0)
    pub fraction: f64,

    /// Dollar amount to invest
    pub dollar_amount: f64,

    /// Number of shares to buy (rounded down)
    pub shares: i64,

    /// Kelly fraction before constraints
    pub raw_kelly_fraction: f64,

    /// Reasoning for the position size
    pub reasoning: String,
}

impl Default for KellyPositionSizer {
    fn default() -> Self {
        Self {
            max_kelly_fraction: 0.25, // Never risk more than 25%
            kelly_multiplier: 0.5,    // Use half-Kelly for safety
            min_position_size: 0.01,  // Minimum 1% position
            max_position_size: 0.10,  // Maximum 10% position
        }
    }
}

impl KellyPositionSizer {
    pub fn new(
        max_kelly_fraction: f64,
        kelly_multiplier: f64,
        min_position_size: f64,
        max_position_size: f64,
    ) -> Result<Self> {
        if max_kelly_fraction <= 0.0 || max_kelly_fraction > 1.0 {
            bail!("max_kelly_fraction must be between 0 and 1");
        }
        if kelly_multiplier <= 0.0 || kelly_multiplier > 1.0 {
            bail!("kelly_multiplier must be between 0 and 1");
        }
        if min_position_size < 0.0 || min_position_size > max_position_size {
            bail!("min_position_size must be >= 0 and <= max_position_size");
        }
        if max_position_size <= 0.0 || max_position_size > 1.0 {
            bail!("max_position_size must be between 0 and 1");
        }

        Ok(Self {
            max_kelly_fraction,
            kelly_multiplier,
            min_position_size,
            max_position_size,
        })
    }

    /// Conservative constructor with safer defaults
    pub fn conservative() -> Self {
        Self {
            max_kelly_fraction: 0.15, // Max 15%
            kelly_multiplier: 0.25,   // Quarter-Kelly
            min_position_size: 0.005, // 0.5% minimum
            max_position_size: 0.05,  // 5% maximum
        }
    }

    /// Aggressive constructor for higher risk tolerance
    pub fn aggressive() -> Self {
        Self {
            max_kelly_fraction: 0.40, // Max 40%
            kelly_multiplier: 0.75,   // 3/4 Kelly
            min_position_size: 0.02,  // 2% minimum
            max_position_size: 0.20,  // 20% maximum
        }
    }

    /// Calculate position size based on strategy performance
    pub fn calculate_from_performance(
        &self,
        performance: &StrategyPerformance,
        portfolio_value: f64,
        current_price: f64,
    ) -> Result<PositionSize> {
        if performance.num_trades < 10 {
            // Not enough data - use minimum position
            return Ok(PositionSize {
                fraction: self.min_position_size,
                dollar_amount: portfolio_value * self.min_position_size,
                shares: ((portfolio_value * self.min_position_size) / current_price).floor() as i64,
                raw_kelly_fraction: 0.0,
                reasoning: format!(
                    "Insufficient trade history ({} trades). Using minimum position size.",
                    performance.num_trades
                ),
            });
        }

        // Validate performance metrics
        if performance.win_rate <= 0.0 || performance.win_rate >= 1.0 {
            bail!("Win rate must be between 0 and 1");
        }
        if performance.avg_win <= 0.0 {
            bail!("Average win must be positive");
        }
        if performance.avg_loss <= 0.0 {
            bail!("Average loss must be positive");
        }

        // Calculate Kelly fraction
        // f* = (p * b - q) / b
        // where b = avg_win / avg_loss (profit/loss ratio)
        let p = performance.win_rate;
        let q = 1.0 - p;
        let b = performance.avg_win / performance.avg_loss;

        let raw_kelly = (p * b - q) / b;

        // Adjust for confidence
        let confidence_adjusted = raw_kelly * performance.confidence;

        // Apply Kelly multiplier
        let fractional_kelly = confidence_adjusted * self.kelly_multiplier;

        // Apply constraints
        let constrained_kelly = fractional_kelly
            .max(0.0) // No negative positions
            .min(self.max_kelly_fraction);

        let final_fraction = constrained_kelly
            .max(self.min_position_size)
            .min(self.max_position_size);

        let dollar_amount = portfolio_value * final_fraction;
        let shares = (dollar_amount / current_price).floor() as i64;

        let reasoning = format!(
            "Kelly: {:.2}% (raw: {:.2}%, win_rate: {:.1}%, avg_win/loss: {:.2}, confidence: {:.0}%)",
            final_fraction * 100.0,
            raw_kelly * 100.0,
            p * 100.0,
            b,
            performance.confidence * 100.0
        );

        Ok(PositionSize {
            fraction: final_fraction,
            dollar_amount,
            shares,
            raw_kelly_fraction: raw_kelly,
            reasoning,
        })
    }

    /// Calculate position size based on individual signal
    pub fn calculate_from_signal(
        &self,
        signal: &TradingSignal,
        performance: &StrategyPerformance,
        portfolio_value: f64,
        current_price: f64,
    ) -> Result<PositionSize> {
        // Start with performance-based Kelly
        let mut base_position =
            self.calculate_from_performance(performance, portfolio_value, current_price)?;

        // Adjust based on signal confidence
        // Lower confidence = smaller position
        let confidence_multiplier = signal.confidence;
        base_position.fraction *= confidence_multiplier;

        // Ensure we stay within bounds
        base_position.fraction = base_position
            .fraction
            .max(self.min_position_size)
            .min(self.max_position_size);

        base_position.dollar_amount = portfolio_value * base_position.fraction;
        base_position.shares = (base_position.dollar_amount / current_price).floor() as i64;

        base_position.reasoning = format!(
            "{} | Signal confidence: {:.0}% | Adjusted to: {:.2}%",
            base_position.reasoning,
            signal.confidence * 100.0,
            base_position.fraction * 100.0
        );

        Ok(base_position)
    }

    /// Calculate position size using risk-based approach
    /// This calculates position size based on stop loss distance
    pub fn calculate_risk_based(
        &self,
        portfolio_value: f64,
        max_risk_dollars: f64,
        current_price: f64,
        stop_loss_price: f64,
    ) -> Result<PositionSize> {
        if stop_loss_price >= current_price {
            bail!("Stop loss price must be below current price");
        }

        let risk_per_share = current_price - stop_loss_price;
        let shares = (max_risk_dollars / risk_per_share).floor() as i64;
        let dollar_amount = shares as f64 * current_price;
        let fraction = dollar_amount / portfolio_value;

        // Apply constraints
        let constrained_fraction = fraction
            .max(self.min_position_size)
            .min(self.max_position_size);

        let final_shares =
            ((constrained_fraction * portfolio_value) / current_price).floor() as i64;
        let final_dollar = final_shares as f64 * current_price;

        let reasoning = format!(
            "Risk-based sizing: ${:.2} risk, ${:.2} per share risk, {:.2}% position",
            max_risk_dollars,
            risk_per_share,
            constrained_fraction * 100.0
        );

        Ok(PositionSize {
            fraction: constrained_fraction,
            dollar_amount: final_dollar,
            shares: final_shares,
            raw_kelly_fraction: 0.0,
            reasoning,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_kelly_calculation_positive_edge() {
        let sizer = KellyPositionSizer::default();

        let performance = StrategyPerformance {
            win_rate: 0.60,
            avg_win: 100.0,
            avg_loss: 50.0,
            num_trades: 100,
            confidence: 1.0,
        };

        let result = sizer
            .calculate_from_performance(&performance, 10000.0, 100.0)
            .unwrap();

        // With 60% win rate, 2:1 win/loss ratio
        // Kelly = (0.6 * 2 - 0.4) / 2 = 0.4
        // Half-Kelly = 0.2
        // But max is 0.10, so should be capped
        assert_relative_eq!(result.fraction, 0.10, epsilon = 0.001);
    }

    #[test]
    fn test_kelly_calculation_no_edge() {
        let sizer = KellyPositionSizer::default();

        let performance = StrategyPerformance {
            win_rate: 0.50,
            avg_win: 100.0,
            avg_loss: 100.0,
            num_trades: 100,
            confidence: 1.0,
        };

        let result = sizer
            .calculate_from_performance(&performance, 10000.0, 100.0)
            .unwrap();

        // No edge = use minimum position
        assert_relative_eq!(result.fraction, 0.01, epsilon = 0.001);
    }

    #[test]
    fn test_insufficient_trades() {
        let sizer = KellyPositionSizer::default();

        let performance = StrategyPerformance {
            win_rate: 0.70,
            avg_win: 200.0,
            avg_loss: 50.0,
            num_trades: 5, // Not enough trades
            confidence: 1.0,
        };

        let result = sizer
            .calculate_from_performance(&performance, 10000.0, 100.0)
            .unwrap();

        // Should use minimum due to insufficient data
        assert_relative_eq!(result.fraction, 0.01, epsilon = 0.001);
        assert!(result.reasoning.contains("Insufficient trade history"));
    }

    #[test]
    fn test_confidence_adjustment() {
        let sizer = KellyPositionSizer::default();

        let performance = StrategyPerformance {
            win_rate: 0.60,
            avg_win: 100.0,
            avg_loss: 50.0,
            num_trades: 100,
            confidence: 0.5, // Low confidence
        };

        let result = sizer
            .calculate_from_performance(&performance, 10000.0, 100.0)
            .unwrap();

        // Low confidence should reduce position size
        assert!(result.fraction < 0.10);
    }

    #[test]
    fn test_risk_based_sizing() {
        let sizer = KellyPositionSizer::default();

        let result = sizer
            .calculate_risk_based(
                10000.0, // portfolio value
                200.0,   // max risk dollars
                100.0,   // current price
                95.0,    // stop loss price
            )
            .unwrap();

        // Risk per share = $5, max risk = $200
        // Shares = 200 / 5 = 40 shares
        // Dollar amount = 40 * 100 = $4000
        // Fraction = 4000 / 10000 = 0.40
        // But max is 0.10, so capped
        assert_eq!(result.shares, 10); // (0.10 * 10000) / 100
    }

    #[test]
    fn test_conservative_mode() {
        let sizer = KellyPositionSizer::conservative();

        assert_eq!(sizer.max_kelly_fraction, 0.15);
        assert_eq!(sizer.kelly_multiplier, 0.25);
        assert_eq!(sizer.max_position_size, 0.05);
    }

    #[test]
    fn test_aggressive_mode() {
        let sizer = KellyPositionSizer::aggressive();

        assert_eq!(sizer.max_kelly_fraction, 0.40);
        assert_eq!(sizer.kelly_multiplier, 0.75);
        assert_eq!(sizer.max_position_size, 0.20);
    }
}
