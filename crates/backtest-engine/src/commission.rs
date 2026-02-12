use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::models::CommissionModel;

/// Compute commission for a trade using the tiered commission model.
///
/// If no model is provided, falls back to a flat rate.
pub fn compute_tiered_commission(
    model: Option<&CommissionModel>,
    shares: Decimal,
    price: Decimal,
    flat_rate: Decimal,
    cumulative_monthly_shares: f64,
) -> Decimal {
    let model = match model {
        Some(m) => m,
        None => {
            // Flat rate fallback: notional * rate
            return price * shares * flat_rate;
        }
    };

    let shares_f64 = shares.to_f64().unwrap_or(0.0);

    // Find the applicable tier based on cumulative monthly volume
    let per_share_rate = model
        .tiers
        .iter()
        .rev()
        .find(|t| cumulative_monthly_shares >= t.volume_threshold)
        .map(|t| t.per_share_rate)
        .unwrap_or_else(|| {
            // If no tier matches, use the first tier's rate
            model
                .tiers
                .first()
                .map(|t| t.per_share_rate)
                .unwrap_or(0.005)
        });

    let raw_commission = shares_f64 * per_share_rate;

    // Apply min/max bounds
    let bounded = raw_commission
        .max(model.min_per_trade)
        .min(if model.max_per_trade > 0.0 {
            model.max_per_trade
        } else {
            f64::MAX
        });

    Decimal::from_f64(bounded).unwrap_or(Decimal::ZERO)
}
