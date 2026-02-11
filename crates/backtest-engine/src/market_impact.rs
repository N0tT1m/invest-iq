use rust_decimal::Decimal;
use rust_decimal::prelude::*;

/// Market impact modeling using the square root model.
///
/// The square root model estimates price impact based on order size relative to
/// average daily volume (ADV). This is the industry-standard approach used by
/// institutional traders and academics.
///
/// Model: Impact = Volatility × Gamma × (OrderSize / ADV)^0.5
///
/// Where:
/// - Volatility: recent price volatility (annualized)
/// - Gamma: market impact coefficient (typically 0.1-0.3 for liquid stocks)
/// - OrderSize: number of shares in the order
/// - ADV: average daily volume over recent period

/// Configuration for market impact calculation.
#[derive(Debug, Clone)]
pub struct MarketImpactConfig {
    /// Market impact coefficient (gamma). Typical range: 0.1 (highly liquid) to 0.5 (illiquid).
    pub gamma: f64,
    /// Lookback period for ADV calculation (trading days).
    pub adv_lookback_days: usize,
    /// Lookback period for volatility calculation (trading days).
    pub vol_lookback_days: usize,
    /// Permanent vs temporary impact split. 0.0-1.0 (0.5 = 50% permanent, 50% temporary).
    pub permanent_impact_fraction: f64,
    /// Minimum ADV fraction to apply full impact (orders below this get reduced impact).
    pub min_participation_for_full_impact: f64,
}

impl Default for MarketImpactConfig {
    fn default() -> Self {
        Self {
            gamma: 0.2, // Moderate liquidity assumption
            adv_lookback_days: 20,
            vol_lookback_days: 20,
            permanent_impact_fraction: 0.6, // 60% permanent, 40% temporary
            min_participation_for_full_impact: 0.01, // 1% of ADV
        }
    }
}

/// Market impact result.
#[derive(Debug, Clone)]
pub struct MarketImpact {
    /// Total price impact as a fraction (e.g., 0.002 = 0.2% = 20 bps).
    pub total_impact: f64,
    /// Permanent impact (affects fill price and subsequent bars).
    pub permanent_impact: f64,
    /// Temporary impact (only affects fill price, mean-reverts).
    pub temporary_impact: f64,
    /// Participation rate (order size / ADV).
    pub participation_rate: f64,
}

/// Compute market impact using the square root model.
///
/// # Arguments
/// * `order_shares` - Number of shares in the order
/// * `recent_volumes` - Recent daily volumes (for ADV calculation)
/// * `recent_prices` - Recent daily close prices (for volatility calculation)
/// * `config` - Market impact configuration
///
/// # Returns
/// `MarketImpact` with breakdown of permanent/temporary impact, or None if insufficient data.
pub fn compute_market_impact(
    order_shares: Decimal,
    recent_volumes: &[f64],
    recent_prices: &[Decimal],
    config: &MarketImpactConfig,
) -> Option<MarketImpact> {
    if recent_volumes.is_empty() || recent_prices.len() < 2 {
        return None;
    }

    let order_size = order_shares.to_f64().unwrap_or(0.0);
    if order_size <= 0.0 {
        return None;
    }

    // 1. Calculate ADV (average daily volume)
    let adv_lookback = config.adv_lookback_days.min(recent_volumes.len());
    if adv_lookback == 0 {
        return None;
    }
    let adv: f64 = recent_volumes[recent_volumes.len().saturating_sub(adv_lookback)..]
        .iter()
        .sum::<f64>()
        / adv_lookback as f64;

    if adv <= 0.0 {
        return None;
    }

    // 2. Calculate participation rate
    let participation_rate = order_size / adv;

    // 3. Calculate volatility (annualized)
    let vol_lookback = config.vol_lookback_days.min(recent_prices.len() - 1);
    if vol_lookback == 0 {
        return None;
    }

    let returns: Vec<f64> = recent_prices[recent_prices.len().saturating_sub(vol_lookback + 1)..]
        .windows(2)
        .map(|w| {
            let p0 = w[0].to_f64().unwrap_or(1.0);
            let p1 = w[1].to_f64().unwrap_or(1.0);
            if p0 > 0.0 && p1 > 0.0 {
                (p1 / p0).ln()  // Correct log return formula
            } else {
                0.0
            }
        })
        .collect();

    if returns.is_empty() {
        return None;
    }

    let mean_ret = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns
        .iter()
        .map(|r| (r - mean_ret).powi(2))
        .sum::<f64>()
        / returns.len().max(1) as f64;
    let daily_vol = variance.sqrt();
    let annualized_vol = daily_vol * 252.0_f64.sqrt();

    // 4. Square root model: Impact = Vol × Gamma × sqrt(OrderSize / ADV)
    let sqrt_participation = participation_rate.sqrt();
    let base_impact = annualized_vol * config.gamma * sqrt_participation;

    // 5. Adjust for very small orders (concave function below min participation)
    let adjusted_impact = if participation_rate < config.min_participation_for_full_impact {
        // Linear scaling for small orders
        let scale = participation_rate / config.min_participation_for_full_impact;
        base_impact * scale
    } else {
        base_impact
    };

    // 6. Split into permanent and temporary
    let permanent = adjusted_impact * config.permanent_impact_fraction;
    let temporary = adjusted_impact * (1.0 - config.permanent_impact_fraction);

    Some(MarketImpact {
        total_impact: adjusted_impact,
        permanent_impact: permanent,
        temporary_impact: temporary,
        participation_rate,
    })
}

/// Apply market impact to a fill price.
///
/// For buy orders: price increases by impact.
/// For sell orders: price decreases by impact.
///
/// # Arguments
/// * `base_price` - The raw price before impact
/// * `is_buy` - True for buy orders, false for sell orders
/// * `impact` - The market impact result
///
/// # Returns
/// Adjusted fill price including market impact.
pub fn apply_market_impact(base_price: Decimal, is_buy: bool, impact: &MarketImpact) -> Decimal {
    let price_f64 = base_price.to_f64().unwrap_or(0.0);
    let impact_multiplier = if is_buy {
        1.0 + impact.total_impact
    } else {
        1.0 - impact.total_impact
    };
    Decimal::from_f64(price_f64 * impact_multiplier).unwrap_or(base_price)
}

/// Track cumulative permanent market impact across multiple trades.
///
/// Permanent impact persists across bars, so subsequent trades face higher/lower
/// prices depending on prior order flow.
pub struct PermanentImpactTracker {
    /// Symbol -> cumulative permanent price shift (as a fraction).
    impacts: std::collections::HashMap<String, f64>,
}

impl PermanentImpactTracker {
    pub fn new() -> Self {
        Self {
            impacts: std::collections::HashMap::new(),
        }
    }

    /// Record a permanent impact from a trade.
    pub fn record_impact(&mut self, symbol: &str, is_buy: bool, permanent_impact: f64) {
        let entry = self.impacts.entry(symbol.to_string()).or_insert(0.0);
        if is_buy {
            *entry += permanent_impact;
        } else {
            *entry -= permanent_impact;
        }
    }

    /// Get the cumulative permanent impact for a symbol (as a price multiplier).
    pub fn get_impact_multiplier(&self, symbol: &str) -> f64 {
        1.0 + self.impacts.get(symbol).copied().unwrap_or(0.0)
    }

    /// Decay permanent impacts (mean reversion over time).
    /// Typically called daily with a decay rate of 0.95-0.99.
    pub fn decay_impacts(&mut self, decay_rate: f64) {
        for impact in self.impacts.values_mut() {
            *impact *= decay_rate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_impact_basic() {
        let order_shares = Decimal::new(10000, 0); // 10,000 shares
        let volumes = vec![500_000.0; 20]; // ADV = 500k shares
        let prices: Vec<Decimal> = (0..20)
            .map(|i| Decimal::from_f64(100.0 + i as f64 * 0.1).unwrap())
            .collect();

        let config = MarketImpactConfig::default();
        let impact = compute_market_impact(order_shares, &volumes, &prices, &config).unwrap();

        // Participation rate = 10,000 / 500,000 = 2%
        assert!((impact.participation_rate - 0.02).abs() < 1e-6);

        // Impact should be positive
        assert!(impact.total_impact > 0.0);
        assert!(impact.permanent_impact > 0.0);
        assert!(impact.temporary_impact > 0.0);

        // Permanent + temporary = total
        let sum = impact.permanent_impact + impact.temporary_impact;
        assert!((sum - impact.total_impact).abs() < 1e-10);
    }

    #[test]
    fn test_apply_market_impact() {
        let base_price = Decimal::new(10000, 2); // $100.00
        let impact = MarketImpact {
            total_impact: 0.005, // 0.5%
            permanent_impact: 0.003,
            temporary_impact: 0.002,
            participation_rate: 0.02,
        };

        // Buy: price increases
        let buy_price = apply_market_impact(base_price, true, &impact);
        let expected_buy = 100.0 * 1.005;
        let actual_buy = buy_price.to_f64().unwrap();
        assert!((actual_buy - expected_buy).abs() < 0.01);

        // Sell: price decreases
        let sell_price = apply_market_impact(base_price, false, &impact);
        let expected_sell = 100.0 * 0.995;
        let actual_sell = sell_price.to_f64().unwrap();
        assert!((actual_sell - expected_sell).abs() < 0.01);
    }

    #[test]
    fn test_permanent_impact_tracker() {
        let mut tracker = PermanentImpactTracker::new();

        // Buy 10k shares → +0.3% permanent impact
        tracker.record_impact("AAPL", true, 0.003);
        assert!((tracker.get_impact_multiplier("AAPL") - 1.003).abs() < 1e-10);

        // Buy another 5k → +0.15% more
        tracker.record_impact("AAPL", true, 0.0015);
        assert!((tracker.get_impact_multiplier("AAPL") - 1.0045).abs() < 1e-10);

        // Sell 8k → -0.24%
        tracker.record_impact("AAPL", false, 0.0024);
        assert!((tracker.get_impact_multiplier("AAPL") - 1.0021).abs() < 1e-10);

        // Decay impacts
        tracker.decay_impacts(0.95);
        let decayed = (1.0021 - 1.0) * 0.95 + 1.0;
        assert!((tracker.get_impact_multiplier("AAPL") - decayed).abs() < 1e-8);
    }

    #[test]
    fn test_small_order_reduced_impact() {
        let config = MarketImpactConfig {
            gamma: 0.2,
            min_participation_for_full_impact: 0.01, // 1%
            ..Default::default()
        };

        let volumes = vec![1_000_000.0; 20]; // ADV = 1M shares

        // Prices with ~1% daily volatility to ensure non-zero impact
        let mut prices: Vec<Decimal> = Vec::new();
        let mut price = 100.0;
        for i in 0..20 {
            // Add small variation: alternating +/- 0.5%
            let change = if i % 2 == 0 { 1.005 } else { 0.995 };
            price *= change;
            prices.push(Decimal::from_f64(price).unwrap());
        }

        // Order 1: 5000 shares = 0.5% participation (below 1% threshold)
        let small_order = Decimal::new(5000, 0);
        let small_impact =
            compute_market_impact(small_order, &volumes, &prices, &config).unwrap();

        // Order 2: 20000 shares = 2% participation (above threshold)
        let large_order = Decimal::new(20000, 0);
        let large_impact =
            compute_market_impact(large_order, &volumes, &prices, &config).unwrap();

        // Small order should have proportionally reduced impact
        assert!(small_impact.total_impact < large_impact.total_impact);

        // Small order impact should be less than sqrt(participation) would suggest
        // due to linear scaling below min threshold
        // Formula: small gets base * 0.5 (linear scale), large gets full base
        // Ratio = 0.5 * sqrt(0.005/0.02) = 0.5 * 0.5 = 0.25
        let expected_ratio_if_full = (0.005_f64 / 0.02).sqrt(); // sqrt(5k/20k) = 0.5
        let actual_ratio = small_impact.total_impact / large_impact.total_impact;
        // Actual ratio should be 0.25, which is < 0.5
        assert!(actual_ratio < expected_ratio_if_full);
        // Verify it's approximately 0.25 (0.5 * sqrt ratio)
        assert!((actual_ratio - 0.25).abs() < 0.05);
    }
}
