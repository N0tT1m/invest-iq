use crate::models::FactorAttribution;

/// Compute CAPM factor attribution via hand-rolled OLS regression.
///
/// `strategy_returns`: daily returns of the strategy.
/// `benchmark_returns`: daily returns of the benchmark (SPY).
///
/// Model: R_strategy = alpha + beta * R_benchmark + epsilon
pub fn compute_factor_attribution(
    strategy_returns: &[f64],
    benchmark_returns: &[f64],
) -> Option<FactorAttribution> {
    let n = strategy_returns.len().min(benchmark_returns.len());
    if n < 10 {
        return None;
    }

    let r = &strategy_returns[..n];
    let b = &benchmark_returns[..n];
    let nf = n as f64;

    // OLS: beta = Cov(r, b) / Var(b), alpha = mean(r) - beta * mean(b)
    let mean_r = r.iter().sum::<f64>() / nf;
    let mean_b = b.iter().sum::<f64>() / nf;

    let cov: f64 = r
        .iter()
        .zip(b.iter())
        .map(|(ri, bi)| (ri - mean_r) * (bi - mean_b))
        .sum::<f64>()
        / (nf - 1.0);

    let var_b: f64 = b.iter().map(|bi| (bi - mean_b).powi(2)).sum::<f64>() / (nf - 1.0);

    if var_b < 1e-15 {
        return None;
    }

    let beta = cov / var_b;
    let alpha_daily = mean_r - beta * mean_b;
    let alpha_annualized = alpha_daily * 252.0;

    // R-squared: 1 - SS_res / SS_tot
    let ss_res: f64 = r
        .iter()
        .zip(b.iter())
        .map(|(ri, bi)| {
            let predicted = alpha_daily + beta * bi;
            (ri - predicted).powi(2)
        })
        .sum();

    let ss_tot: f64 = r.iter().map(|ri| (ri - mean_r).powi(2)).sum();

    let r_squared = if ss_tot > 1e-15 {
        1.0 - ss_res / ss_tot
    } else {
        0.0
    };

    // Tracking error: std dev of (strategy - benchmark) returns, annualized
    let diffs: Vec<f64> = r
        .iter()
        .zip(b.iter())
        .map(|(ri, bi)| ri - bi)
        .collect();
    let mean_diff = diffs.iter().sum::<f64>() / nf;
    let var_diff = diffs
        .iter()
        .map(|d| (d - mean_diff).powi(2))
        .sum::<f64>()
        / (nf - 1.0);
    let tracking_error = var_diff.sqrt() * 252.0_f64.sqrt();

    // Residual risk: std dev of residuals, annualized
    let residuals: Vec<f64> = r
        .iter()
        .zip(b.iter())
        .map(|(ri, bi)| ri - (alpha_daily + beta * bi))
        .collect();
    let mean_resid = residuals.iter().sum::<f64>() / nf;
    let var_resid = residuals
        .iter()
        .map(|e| (e - mean_resid).powi(2))
        .sum::<f64>()
        / (nf - 1.0);
    let residual_risk = var_resid.sqrt() * 252.0_f64.sqrt();

    Some(FactorAttribution {
        beta,
        alpha_annualized,
        r_squared,
        tracking_error,
        residual_risk,
    })
}
