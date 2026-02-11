use crate::models::EquityPoint;

/// Conditional Drawdown at Risk (CDaR) - expected drawdown in the worst α% of cases.
///
/// CDaR is the average of the worst α% of drawdowns. It's more robust than max
/// drawdown alone because it considers the distribution of large drawdowns.
///
/// # Arguments
/// * `equity_curve` - The equity curve from the backtest
/// * `alpha` - Confidence level (e.g., 0.05 for 95% CDaR)
///
/// # Returns
/// CDaR as a percentage, or None if insufficient data.
pub fn conditional_drawdown_at_risk(equity_curve: &[EquityPoint], alpha: f64) -> Option<f64> {
    if equity_curve.len() < 10 {
        return None;
    }

    // Collect all drawdown values
    let drawdowns: Vec<f64> = equity_curve
        .iter()
        .map(|p| p.drawdown_percent)
        .collect();

    // Sort descending (largest drawdowns first)
    let mut sorted_dd = drawdowns.clone();
    sorted_dd.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    // Take worst alpha% and average them
    let cutoff_idx = ((alpha * sorted_dd.len() as f64).ceil() as usize).max(1);
    let worst_drawdowns = &sorted_dd[..cutoff_idx.min(sorted_dd.len())];

    if worst_drawdowns.is_empty() {
        return None;
    }

    let cdar = worst_drawdowns.iter().sum::<f64>() / worst_drawdowns.len() as f64;
    Some(cdar)
}

/// Ulcer Index - measures the depth and duration of drawdowns.
///
/// UI = sqrt(mean((DD_i)^2))
///
/// Lower is better. Unlike max drawdown, Ulcer Index penalizes prolonged drawdowns
/// more than brief sharp ones.
pub fn ulcer_index(equity_curve: &[EquityPoint]) -> Option<f64> {
    if equity_curve.len() < 3 {
        return None;
    }

    let n = equity_curve.len() as f64;
    let sum_sq_dd: f64 = equity_curve
        .iter()
        .map(|p| p.drawdown_percent.powi(2))
        .sum();

    let mean_sq_dd = sum_sq_dd / n;
    Some(mean_sq_dd.sqrt())
}

/// Pain Index - average of all squared drawdowns (similar to Ulcer but no sqrt).
pub fn pain_index(equity_curve: &[EquityPoint]) -> Option<f64> {
    if equity_curve.is_empty() {
        return None;
    }

    let n = equity_curve.len() as f64;
    let sum_sq_dd: f64 = equity_curve
        .iter()
        .map(|p| p.drawdown_percent.powi(2))
        .sum();

    Some(sum_sq_dd / n)
}

/// Gain-to-Pain Ratio - total return divided by Pain Index.
///
/// Measures reward per unit of drawdown pain. Higher is better.
pub fn gain_to_pain_ratio(
    total_return_percent: f64,
    equity_curve: &[EquityPoint],
) -> Option<f64> {
    let pain = pain_index(equity_curve)?;
    if pain > 1e-10 {
        Some(total_return_percent / pain)
    } else if total_return_percent > 0.0 {
        Some(f64::INFINITY)
    } else {
        None
    }
}

/// Burke Ratio - return divided by sqrt(sum of squared drawdowns).
///
/// Similar to Ulcer Index ratio but uses total return instead of excess return.
pub fn burke_ratio(
    total_return_percent: f64,
    equity_curve: &[EquityPoint],
    num_drawdowns: usize,
) -> Option<f64> {
    if equity_curve.len() < 3 || num_drawdowns == 0 {
        return None;
    }

    // Find top N drawdown events
    let mut drawdown_magnitudes: Vec<f64> = Vec::new();
    let mut in_drawdown = false;
    let mut max_dd_in_current = 0.0_f64;

    for point in equity_curve {
        let dd = point.drawdown_percent;
        if dd > 0.0 {
            if !in_drawdown {
                in_drawdown = true;
                max_dd_in_current = dd;
            } else {
                max_dd_in_current = max_dd_in_current.max(dd);
            }
        } else {
            if in_drawdown {
                drawdown_magnitudes.push(max_dd_in_current);
                in_drawdown = false;
                max_dd_in_current = 0.0;
            }
        }
    }

    // Handle ongoing drawdown at end
    if in_drawdown {
        drawdown_magnitudes.push(max_dd_in_current);
    }

    if drawdown_magnitudes.is_empty() {
        return None;
    }

    // Sort descending and take top N
    drawdown_magnitudes.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let top_n = &drawdown_magnitudes[..num_drawdowns.min(drawdown_magnitudes.len())];

    let sum_sq: f64 = top_n.iter().map(|dd| dd.powi(2)).sum();
    let denom = sum_sq.sqrt();

    if denom > 1e-10 {
        Some(total_return_percent / denom)
    } else if total_return_percent > 0.0 {
        Some(f64::INFINITY)
    } else {
        None
    }
}

/// Sterling Ratio - annualized return divided by average of worst N drawdowns.
///
/// Traditional Sterling uses N=3 drawdowns.
pub fn sterling_ratio(
    annualized_return_percent: f64,
    equity_curve: &[EquityPoint],
    num_drawdowns: usize,
) -> Option<f64> {
    if equity_curve.len() < 3 || num_drawdowns == 0 {
        return None;
    }

    // Find all drawdown events
    let mut drawdown_magnitudes: Vec<f64> = Vec::new();
    let mut in_drawdown = false;
    let mut max_dd_in_current = 0.0_f64;

    for point in equity_curve {
        let dd = point.drawdown_percent;
        if dd > 0.0 {
            if !in_drawdown {
                in_drawdown = true;
                max_dd_in_current = dd;
            } else {
                max_dd_in_current = max_dd_in_current.max(dd);
            }
        } else {
            if in_drawdown {
                drawdown_magnitudes.push(max_dd_in_current);
                in_drawdown = false;
                max_dd_in_current = 0.0;
            }
        }
    }

    if in_drawdown {
        drawdown_magnitudes.push(max_dd_in_current);
    }

    if drawdown_magnitudes.is_empty() {
        return None;
    }

    // Sort descending and take worst N
    drawdown_magnitudes.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let worst_n = &drawdown_magnitudes[..num_drawdowns.min(drawdown_magnitudes.len())];

    let avg_worst_dd = worst_n.iter().sum::<f64>() / worst_n.len() as f64;

    // Add 10% penalty (traditional Sterling adjustment)
    let adjusted_dd = avg_worst_dd + 10.0;

    if adjusted_dd > 1e-10 {
        Some(annualized_return_percent / adjusted_dd)
    } else if annualized_return_percent > 0.0 {
        Some(f64::INFINITY)
    } else {
        None
    }
}

/// Drawdown recovery analysis - compute metrics about recovery times.
#[derive(Debug, Clone)]
pub struct DrawdownRecovery {
    /// Average recovery time in days (bars).
    pub avg_recovery_days: f64,
    /// Maximum recovery time in days.
    pub max_recovery_days: i64,
    /// Number of drawdowns that fully recovered.
    pub num_recovered: usize,
    /// Number of drawdowns still ongoing.
    pub num_ongoing: usize,
    /// Percentage of time spent in drawdown.
    pub time_in_drawdown_percent: f64,
}

pub fn drawdown_recovery_analysis(equity_curve: &[EquityPoint]) -> Option<DrawdownRecovery> {
    if equity_curve.len() < 3 {
        return None;
    }

    let mut recovery_times: Vec<i64> = Vec::new();
    let mut in_drawdown = false;
    let mut _dd_start_idx = 0usize;
    let mut trough_idx = 0usize;
    let mut bars_in_drawdown = 0usize;

    for (i, point) in equity_curve.iter().enumerate() {
        let dd = point.drawdown_percent;
        if dd > 0.0 {
            bars_in_drawdown += 1;
            if !in_drawdown {
                in_drawdown = true;
                _dd_start_idx = i;
                trough_idx = i;
            } else {
                // Still in drawdown — track trough
                if dd > equity_curve[trough_idx].drawdown_percent {
                    trough_idx = i;
                }
            }
        } else {
            if in_drawdown {
                // Recovery — measure time from trough to recovery
                let recovery_bars = (i as i64) - (trough_idx as i64);
                recovery_times.push(recovery_bars);
                in_drawdown = false;
            }
        }
    }

    let num_ongoing = if in_drawdown { 1 } else { 0 };
    let num_recovered = recovery_times.len();

    let avg_recovery_days = if !recovery_times.is_empty() {
        recovery_times.iter().sum::<i64>() as f64 / recovery_times.len() as f64
    } else {
        0.0
    };

    let max_recovery_days = recovery_times.iter().copied().max().unwrap_or(0);

    let time_in_drawdown_percent =
        (bars_in_drawdown as f64 / equity_curve.len() as f64) * 100.0;

    Some(DrawdownRecovery {
        avg_recovery_days,
        max_recovery_days,
        num_recovered,
        num_ongoing,
        time_in_drawdown_percent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;

    fn make_equity_point(timestamp: &str, equity: f64, drawdown_percent: f64) -> EquityPoint {
        EquityPoint {
            timestamp: timestamp.to_string(),
            equity: rust_decimal::Decimal::from_f64(equity).unwrap(),
            drawdown_percent,
        }
    }

    #[test]
    fn test_conditional_drawdown_at_risk() {
        let curve = vec![
            make_equity_point("2024-01-01", 100000.0, 0.0),
            make_equity_point("2024-01-02", 98000.0, 2.0),
            make_equity_point("2024-01-03", 95000.0, 5.0),
            make_equity_point("2024-01-04", 92000.0, 8.0),
            make_equity_point("2024-01-05", 90000.0, 10.0),
            make_equity_point("2024-01-06", 93000.0, 7.0),
            make_equity_point("2024-01-07", 96000.0, 4.0),
            make_equity_point("2024-01-08", 99000.0, 1.0),
            make_equity_point("2024-01-09", 100500.0, 0.0),
            make_equity_point("2024-01-10", 101000.0, 0.0),
        ];

        // 95% CDaR = average of worst 5% (0.05 * 10 = 0.5 → 1 point)
        let cdar = conditional_drawdown_at_risk(&curve, 0.05).unwrap();
        // Worst drawdown is 10%
        assert!((cdar - 10.0).abs() < 0.1);

        // 50% CDaR = average of worst 50% (5 points)
        let cdar_50 = conditional_drawdown_at_risk(&curve, 0.5).unwrap();
        // Worst 5: 10, 8, 7, 5, 4 → avg = 6.8
        let expected = (10.0 + 8.0 + 7.0 + 5.0 + 4.0) / 5.0;
        assert!((cdar_50 - expected).abs() < 0.1);
    }

    #[test]
    fn test_ulcer_index() {
        let curve = vec![
            make_equity_point("2024-01-01", 100000.0, 0.0),
            make_equity_point("2024-01-02", 95000.0, 5.0),
            make_equity_point("2024-01-03", 90000.0, 10.0),
            make_equity_point("2024-01-04", 100000.0, 0.0),
        ];

        let ui = ulcer_index(&curve).unwrap();
        // sqrt(mean([0^2, 5^2, 10^2, 0^2])) = sqrt((0 + 25 + 100 + 0) / 4) = sqrt(31.25) ≈ 5.59
        let expected = ((0.0_f64 + 25.0 + 100.0 + 0.0) / 4.0).sqrt();
        assert!((ui - expected).abs() < 0.01);
    }

    #[test]
    fn test_pain_index() {
        let curve = vec![
            make_equity_point("2024-01-01", 100000.0, 0.0),
            make_equity_point("2024-01-02", 96000.0, 4.0),
            make_equity_point("2024-01-03", 92000.0, 8.0),
            make_equity_point("2024-01-04", 100000.0, 0.0),
        ];

        let pain = pain_index(&curve).unwrap();
        // Mean of squared DDs: (0 + 16 + 64 + 0) / 4 = 20
        assert!((pain - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_gain_to_pain_ratio() {
        let curve = vec![
            make_equity_point("2024-01-01", 100000.0, 0.0),
            make_equity_point("2024-01-02", 96000.0, 4.0),
            make_equity_point("2024-01-03", 108000.0, 0.0),
        ];

        // Total return = 8%, Pain = (0 + 16 + 0) / 3 = 5.33
        let ratio = gain_to_pain_ratio(8.0, &curve).unwrap();
        let expected_pain = (0.0 + 16.0 + 0.0) / 3.0;
        let expected_ratio = 8.0 / expected_pain;
        assert!((ratio - expected_ratio).abs() < 0.01);
    }

    #[test]
    fn test_drawdown_recovery_analysis() {
        let curve = vec![
            make_equity_point("2024-01-01", 100000.0, 0.0),
            make_equity_point("2024-01-02", 95000.0, 5.0), // DD starts
            make_equity_point("2024-01-03", 90000.0, 10.0), // Trough
            make_equity_point("2024-01-04", 95000.0, 5.0),
            make_equity_point("2024-01-05", 100000.0, 0.0), // Recovered (2 bars from trough)
            make_equity_point("2024-01-06", 98000.0, 2.0), // DD starts again
            make_equity_point("2024-01-07", 100000.0, 0.0), // Recovered (1 bar from trough)
        ];

        let recovery = drawdown_recovery_analysis(&curve).unwrap();

        assert_eq!(recovery.num_recovered, 2);
        assert_eq!(recovery.num_ongoing, 0);
        // Recoveries: 2 bars + 1 bar → avg = 1.5
        assert!((recovery.avg_recovery_days - 1.5).abs() < 0.01);
        assert_eq!(recovery.max_recovery_days, 2);
        // Bars in DD: indices 1,2,3,5 have DD>0 → 4 out of 7 → 57.14%
        let expected_pct = 4.0 / 7.0 * 100.0;
        assert!((recovery.time_in_drawdown_percent - expected_pct).abs() < 1.0);
    }

    #[test]
    fn test_burke_ratio() {
        let curve = vec![
            make_equity_point("2024-01-01", 100000.0, 0.0),
            make_equity_point("2024-01-02", 95000.0, 5.0),
            make_equity_point("2024-01-03", 100000.0, 0.0), // DD #1 recovered
            make_equity_point("2024-01-04", 90000.0, 10.0),
            make_equity_point("2024-01-05", 100000.0, 0.0), // DD #2 recovered
        ];

        // Two drawdowns: 5% and 10%
        // Burke(2) = return / sqrt(5^2 + 10^2) = 15 / sqrt(125) ≈ 15 / 11.18 ≈ 1.34
        let ratio = burke_ratio(15.0, &curve, 2).unwrap();
        let expected_denom = (25.0 + 100.0_f64).sqrt();
        let expected = 15.0 / expected_denom;
        assert!((ratio - expected).abs() < 0.01);
    }

    #[test]
    fn test_sterling_ratio() {
        let curve = vec![
            make_equity_point("2024-01-01", 100000.0, 0.0),
            make_equity_point("2024-01-02", 95000.0, 5.0),
            make_equity_point("2024-01-03", 100000.0, 0.0),
            make_equity_point("2024-01-04", 90000.0, 10.0),
            make_equity_point("2024-01-05", 100000.0, 0.0),
            make_equity_point("2024-01-06", 92000.0, 8.0),
            make_equity_point("2024-01-07", 100000.0, 0.0),
        ];

        // Three drawdowns: 5%, 10%, 8% → avg = 7.67%
        // Sterling = return / (avg_worst_3 + 10%) = 20 / 17.67 ≈ 1.13
        let ratio = sterling_ratio(20.0, &curve, 3).unwrap();
        let avg_dd = (5.0 + 10.0 + 8.0) / 3.0;
        let expected = 20.0 / (avg_dd + 10.0);
        assert!((ratio - expected).abs() < 0.01);
    }
}
