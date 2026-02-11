/// Adaptive threshold utilities for data-driven signal generation.
///
/// Instead of hardcoded thresholds (e.g., "RSI > 70 = overbought"), these functions
/// derive thresholds from the data's own distribution using percentile ranks and z-scores.
/// This makes signals self-calibrating: a stock with naturally high RSI won't constantly
/// trigger overbought, and a low-volatility stock won't be penalized by thresholds
/// designed for high-vol names.

/// Compute the mean of a data slice.
pub fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().sum::<f64>() / data.len() as f64
}

/// Compute sample standard deviation.
pub fn std_dev(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let m = mean(data);
    let variance = data.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (data.len() - 1) as f64;
    variance.sqrt()
}

/// Compute the percentile rank of `value` within `data` (returns 0.0 to 1.0).
/// Uses midpoint interpolation: ties count as half.
pub fn percentile_rank(value: f64, data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.5;
    }
    let count_below = data.iter().filter(|&&x| x < value).count();
    let count_equal = data.iter().filter(|&&x| (x - value).abs() < f64::EPSILON).count();
    (count_below as f64 + 0.5 * count_equal as f64) / data.len() as f64
}

/// Compute the z-score of `value` relative to `data`.
/// Returns 0.0 if data has insufficient variance.
pub fn z_score_of(value: f64, data: &[f64]) -> f64 {
    let sd = std_dev(data);
    if sd < f64::EPSILON {
        return 0.0;
    }
    (value - mean(data)) / sd
}

/// Convert a percentile (0.0-1.0) to a proportional signal score (-100 to 100).
///
/// Values in [neutral_low, neutral_high] map to 0 (dead zone).
/// Values above neutral_high scale linearly to +100.
/// Values below neutral_low scale linearly to -100.
///
/// If `invert` is true, the sign is flipped (high percentile = bearish).
pub fn percentile_to_signal(percentile: f64, neutral_low: f64, neutral_high: f64, invert: bool) -> f64 {
    let raw = if percentile > neutral_high {
        ((percentile - neutral_high) / (1.0 - neutral_high)) * 100.0
    } else if percentile < neutral_low {
        -((neutral_low - percentile) / neutral_low) * 100.0
    } else {
        0.0
    };
    if invert { -raw } else { raw }
}

/// Convert a z-score to a signal weight (1-4).
/// Larger absolute z-scores get more weight, reflecting stronger deviations.
pub fn z_score_to_weight(z: f64) -> i32 {
    let abs_z = z.abs();
    if abs_z > 2.5 {
        4
    } else if abs_z > 1.5 {
        3
    } else if abs_z > 1.0 {
        2
    } else {
        1
    }
}

/// Compute a specific percentile value from data (0-100 scale).
/// Sorts data internally (takes a mutable slice or clones).
pub fn percentile_value(data: &[f64], pct: f64) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Determine if a value is an outlier relative to data (|z| > threshold).
pub fn is_outlier(value: f64, data: &[f64], z_threshold: f64) -> bool {
    z_score_of(value, data).abs() > z_threshold
}

/// Adaptive signal generation: given a current value and its historical distribution,
/// returns (signal_score, weight, is_significant).
///
/// - signal_score: -100 to 100 based on percentile position
/// - weight: 1-4 based on z-score magnitude
/// - is_significant: whether the z-score exceeds 1.0 (worth generating a signal)
pub fn adaptive_signal(value: f64, history: &[f64], invert: bool) -> (f64, i32, bool) {
    if history.len() < 10 {
        return (0.0, 1, false);
    }
    let pct = percentile_rank(value, history);
    let z = z_score_of(value, history);
    let score = percentile_to_signal(pct, 0.25, 0.75, invert);
    let weight = z_score_to_weight(z);
    let significant = z.abs() > 1.0;
    (score, weight, significant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile_rank() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((percentile_rank(3.0, &data) - 0.5).abs() < 0.01);
        assert!(percentile_rank(5.0, &data) > 0.8);
        assert!(percentile_rank(1.0, &data) < 0.2);
    }

    #[test]
    fn test_z_score() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let z = z_score_of(30.0, &data);
        assert!(z.abs() < 0.01); // mean value should have z ≈ 0
    }

    #[test]
    fn test_percentile_to_signal() {
        // High percentile (0.9) should give positive signal
        let score = percentile_to_signal(0.9, 0.25, 0.75, false);
        assert!(score > 0.0);

        // Low percentile (0.1) should give negative signal
        let score = percentile_to_signal(0.1, 0.25, 0.75, false);
        assert!(score < 0.0);

        // Middle percentile should give zero
        let score = percentile_to_signal(0.5, 0.25, 0.75, false);
        assert!((score).abs() < 0.01);

        // Inverted: high percentile gives negative
        let score = percentile_to_signal(0.9, 0.25, 0.75, true);
        assert!(score < 0.0);
    }

    #[test]
    fn test_adaptive_signal() {
        let history: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let (score, weight, significant) = adaptive_signal(95.0, &history, false);
        assert!(score > 0.0);
        assert!(weight >= 2);
        assert!(significant);
    }
}
