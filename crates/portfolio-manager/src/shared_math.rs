/// Pure mathematical utilities for portfolio analytics.
/// Stateless functions — no DB, no async, no external dependencies.

/// Compute daily returns from a value series.
pub fn daily_returns(values: &[f64]) -> Vec<f64> {
    if values.len() < 2 {
        return Vec::new();
    }
    values
        .windows(2)
        .filter_map(|w| {
            if w[0] != 0.0 {
                Some((w[1] - w[0]) / w[0])
            } else {
                None
            }
        })
        .collect()
}

/// Sharpe ratio: (mean_return - rf_daily) / std * sqrt(252).
/// rf_annual default 0.02 (2%).
pub fn sharpe_ratio(returns: &[f64], rf_annual: f64) -> Option<f64> {
    if returns.len() < 3 {
        return None;
    }
    let rf_daily = rf_annual / 252.0;
    let n = returns.len() as f64;
    let mean: f64 = returns.iter().sum::<f64>() / n;
    let excess = mean - rf_daily;
    let variance: f64 = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let std_dev = variance.sqrt();
    if std_dev < 1e-12 {
        return None;
    }
    Some((excess / std_dev) * 252.0_f64.sqrt())
}

/// Sortino ratio: downside deviation only.
pub fn sortino_ratio(returns: &[f64], rf_annual: f64) -> Option<f64> {
    if returns.len() < 3 {
        return None;
    }
    let rf_daily = rf_annual / 252.0;
    let n = returns.len() as f64;
    let mean: f64 = returns.iter().sum::<f64>() / n;
    let excess = mean - rf_daily;

    let downside: Vec<f64> = returns
        .iter()
        .filter(|&&r| r < rf_daily)
        .map(|r| (r - rf_daily).powi(2))
        .collect();

    if downside.is_empty() {
        return if excess > 0.0 { Some(99.99) } else { None };
    }
    let downside_dev = (downside.iter().sum::<f64>() / (n - 1.0)).sqrt();
    if downside_dev < 1e-12 {
        return if excess > 0.0 { Some(99.99) } else { None };
    }
    Some((excess / downside_dev) * 252.0_f64.sqrt())
}

/// Max drawdown % and current drawdown % from an equity curve.
/// Returns (max_drawdown_pct, current_drawdown_pct) as positive numbers (e.g. 0.15 = 15%).
pub fn max_drawdown(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mut peak = values[0];
    let mut max_dd = 0.0_f64;
    for &v in values {
        if v > peak {
            peak = v;
        }
        if peak > 0.0 {
            let dd = (peak - v) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }
    let current_dd = if peak > 0.0 {
        (peak - values[values.len() - 1]) / peak
    } else {
        0.0
    };
    (max_dd, current_dd)
}

/// Rolling annualized volatility over the last `window` returns.
pub fn rolling_volatility(returns: &[f64], window: usize) -> Option<f64> {
    if returns.len() < window || window < 2 {
        return None;
    }
    let tail = &returns[returns.len() - window..];
    let n = tail.len() as f64;
    let mean: f64 = tail.iter().sum::<f64>() / n;
    let variance: f64 = tail.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
    Some(variance.sqrt() * 252.0_f64.sqrt())
}

/// Historical VaR at given confidence (e.g. 0.95 for 95%).
pub fn var_historical(returns: &[f64], confidence: f64) -> Option<f64> {
    if returns.len() < 10 {
        return None;
    }
    let mut sorted: Vec<f64> = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((1.0 - confidence) * sorted.len() as f64).floor() as usize;
    let idx = idx.min(sorted.len() - 1);
    Some(-sorted[idx]) // Return as positive loss
}

/// Historical CVaR (expected shortfall) at given confidence.
pub fn cvar_historical(returns: &[f64], confidence: f64) -> Option<f64> {
    if returns.len() < 10 {
        return None;
    }
    let mut sorted: Vec<f64> = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let cutoff = ((1.0 - confidence) * sorted.len() as f64).floor() as usize;
    let cutoff = cutoff.max(1).min(sorted.len());
    let tail = &sorted[..cutoff];
    let mean: f64 = tail.iter().sum::<f64>() / tail.len() as f64;
    Some(-mean) // Return as positive loss
}

/// OLS regression: y = alpha + beta * x.
/// Returns (alpha, beta, r_squared).
pub fn ols_regression(y: &[f64], x: &[f64]) -> (f64, f64, f64) {
    let n = y.len().min(x.len());
    if n < 3 {
        return (0.0, 1.0, 0.0);
    }
    let nf = n as f64;
    let x_mean: f64 = x[..n].iter().sum::<f64>() / nf;
    let y_mean: f64 = y[..n].iter().sum::<f64>() / nf;

    let mut ss_xy = 0.0;
    let mut ss_xx = 0.0;
    let mut ss_yy = 0.0;
    for i in 0..n {
        let dx = x[i] - x_mean;
        let dy = y[i] - y_mean;
        ss_xy += dx * dy;
        ss_xx += dx * dx;
        ss_yy += dy * dy;
    }

    if ss_xx < 1e-15 {
        return (y_mean, 0.0, 0.0);
    }

    let beta = ss_xy / ss_xx;
    let alpha = y_mean - beta * x_mean;
    let r_squared = if ss_yy > 1e-15 {
        (ss_xy * ss_xy) / (ss_xx * ss_yy)
    } else {
        0.0
    };

    (alpha, beta, r_squared)
}

/// Tracking error: std dev of return differences.
pub fn tracking_error(portfolio_returns: &[f64], benchmark_returns: &[f64]) -> f64 {
    let n = portfolio_returns.len().min(benchmark_returns.len());
    if n < 2 {
        return 0.0;
    }
    let diffs: Vec<f64> = (0..n)
        .map(|i| portfolio_returns[i] - benchmark_returns[i])
        .collect();
    let mean: f64 = diffs.iter().sum::<f64>() / n as f64;
    let variance: f64 = diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    variance.sqrt() * 252.0_f64.sqrt()
}

/// Monthly returns from dated equity values.
/// Returns Vec<(year, month, return_percent)>.
pub fn monthly_returns(dates: &[String], values: &[f64]) -> Vec<(i32, u32, f64)> {
    if dates.len() != values.len() || dates.len() < 2 {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut month_start_value = values[0];
    let mut current_ym = parse_ym(&dates[0]);

    for i in 1..dates.len() {
        let ym = parse_ym(&dates[i]);
        if ym != current_ym {
            let prev_value = values[i - 1];
            if month_start_value > 0.0 {
                let ret = (prev_value - month_start_value) / month_start_value * 100.0;
                result.push((current_ym.0, current_ym.1, ret));
            }
            month_start_value = prev_value;
            current_ym = ym;
        }
    }
    // Last period
    let last = values[values.len() - 1];
    if month_start_value > 0.0 {
        let ret = (last - month_start_value) / month_start_value * 100.0;
        result.push((current_ym.0, current_ym.1, ret));
    }

    result
}

fn parse_ym(date_str: &str) -> (i32, u32) {
    let parts: Vec<&str> = date_str.split(|c| c == '-' || c == 'T' || c == ' ').collect();
    let year = parts.first().and_then(|s| s.parse().ok()).unwrap_or(2000);
    let month = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    (year, month)
}

/// Herfindahl index from weights (0-1 scale). Higher = more concentrated.
pub fn herfindahl_index(weights: &[f64]) -> f64 {
    weights.iter().map(|w| w * w).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daily_returns() {
        let values = vec![100.0, 105.0, 103.0, 110.0];
        let returns = daily_returns(&values);
        assert_eq!(returns.len(), 3);
        assert!((returns[0] - 0.05).abs() < 1e-10);
        assert!((returns[1] - (-2.0 / 105.0)).abs() < 1e-10);
    }

    #[test]
    fn test_sharpe_ratio() {
        let returns = vec![0.01, 0.02, -0.01, 0.015, 0.005, -0.005, 0.01, 0.02, -0.01, 0.015];
        let sharpe = sharpe_ratio(&returns, 0.02);
        assert!(sharpe.is_some());
    }

    #[test]
    fn test_max_drawdown() {
        let values = vec![100.0, 110.0, 105.0, 95.0, 100.0, 115.0, 108.0];
        let (max_dd, current_dd) = max_drawdown(&values);
        // Max DD: peak 110, trough 95 => (110-95)/110 ≈ 0.1364
        assert!((max_dd - 15.0 / 110.0).abs() < 1e-6);
        // Current DD: peak 115, current 108 => (115-108)/115 ≈ 0.0609
        assert!((current_dd - 7.0 / 115.0).abs() < 1e-6);
    }

    #[test]
    fn test_ols_regression_identity() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let (alpha, beta, r2) = ols_regression(&y, &x);
        assert!((alpha - 0.0).abs() < 1e-10);
        assert!((beta - 2.0).abs() < 1e-10);
        assert!((r2 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_var_historical() {
        let returns = vec![-0.05, -0.03, -0.01, 0.0, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06];
        let var = var_historical(&returns, 0.95).unwrap();
        assert!(var > 0.0); // Should be positive (loss)
    }

    #[test]
    fn test_monthly_returns() {
        let dates = vec![
            "2025-01-01".to_string(),
            "2025-01-15".to_string(),
            "2025-02-01".to_string(),
            "2025-02-15".to_string(),
        ];
        let values = vec![100.0, 105.0, 110.0, 108.0];
        let monthly = monthly_returns(&dates, &values);
        assert_eq!(monthly.len(), 2);
        assert_eq!(monthly[0].0, 2025);
        assert_eq!(monthly[0].1, 1);
    }

    #[test]
    fn test_herfindahl_index() {
        // Equal weight across 4 positions = 4 * 0.25^2 = 0.25
        let weights = vec![0.25, 0.25, 0.25, 0.25];
        assert!((herfindahl_index(&weights) - 0.25).abs() < 1e-10);

        // Single position = 1.0
        let weights = vec![1.0];
        assert!((herfindahl_index(&weights) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_tracking_error() {
        let port = vec![0.01, 0.02, -0.01, 0.015];
        let bench = vec![0.01, 0.02, -0.01, 0.015];
        let te = tracking_error(&port, &bench);
        assert!(te < 1e-10); // Identical returns = 0 tracking error
    }
}
