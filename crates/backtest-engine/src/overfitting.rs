use crate::models::EquityPoint;
use rand::{thread_rng, Rng};
use rust_decimal::prelude::ToPrimitive;

/// Probability of Backtest Overfitting (PBO) from Bailey et al. (2015).
///
/// PBO estimates the probability that an optimized strategy's in-sample performance
/// is due to overfitting rather than genuine skill. It compares in-sample vs
/// out-of-sample performance across multiple train/test splits.
///
/// Algorithm:
/// 1. Split data into S train/test folds (e.g., S=16)
/// 2. For each fold, compute in-sample Sharpe (IS) and out-of-sample Sharpe (OOS)
/// 3. Rank all strategies by IS performance
/// 4. Partition into top half (n* best IS) and bottom half
/// 5. Compute the relative frequency where OOS[best IS] < median(OOS[all])
/// 6. PBO = frequency of OOS underperformance in the "best" strategies
///
/// PBO > 0.5 indicates high overfitting risk.
#[derive(Debug, Clone)]
pub struct PboResult {
    /// Probability of backtest overfitting (0.0-1.0).
    pub pbo: f64,
    /// Number of train/test splits used.
    pub num_splits: usize,
    /// Median OOS Sharpe across all strategies.
    pub median_oos_sharpe: f64,
    /// Fraction of best-IS strategies that underperformed median OOS.
    pub underperformance_rate: f64,
    /// Stochastic dominance statistic (higher = more overfitting).
    pub stochastic_dominance: f64,
}

/// Compute PBO using combinatorial splits.
///
/// # Arguments
/// * `equity_curves` - Multiple backtest equity curves from parameter search
/// * `num_splits` - Number of train/test splits to generate (e.g., 16)
///
/// # Returns
/// PboResult with overfitting probability and diagnostics.
pub fn compute_pbo(
    equity_curves: &[Vec<EquityPoint>],
    num_splits: usize,
) -> Option<PboResult> {
    if equity_curves.len() < 4 || num_splits < 4 {
        return None;
    }

    let mut rng = thread_rng();
    let mut is_sharpes: Vec<Vec<f64>> = vec![Vec::new(); equity_curves.len()];
    let mut oos_sharpes: Vec<Vec<f64>> = vec![Vec::new(); equity_curves.len()];

    // For each split, divide data and compute IS/OOS Sharpe for each strategy
    for _ in 0..num_splits {
        for (strategy_idx, curve) in equity_curves.iter().enumerate() {
            if curve.len() < 10 {
                continue;
            }

            // Random split point (30-70% for train, remainder for test)
            let split_pct = 0.3 + rng.gen::<f64>() * 0.4; // 30-70%
            let split_idx = ((curve.len() as f64 * split_pct) as usize).max(5);

            let train_curve = &curve[..split_idx];
            let test_curve = &curve[split_idx..];

            if train_curve.len() < 5 || test_curve.len() < 5 {
                continue;
            }

            let is_sharpe = compute_sharpe_from_curve(train_curve);
            let oos_sharpe = compute_sharpe_from_curve(test_curve);

            is_sharpes[strategy_idx].push(is_sharpe);
            oos_sharpes[strategy_idx].push(oos_sharpe);
        }
    }

    // Average IS and OOS Sharpe for each strategy
    let avg_is: Vec<f64> = is_sharpes
        .iter()
        .map(|sharpes| {
            if sharpes.is_empty() {
                0.0
            } else {
                sharpes.iter().sum::<f64>() / sharpes.len() as f64
            }
        })
        .collect();

    let avg_oos: Vec<f64> = oos_sharpes
        .iter()
        .map(|sharpes| {
            if sharpes.is_empty() {
                0.0
            } else {
                sharpes.iter().sum::<f64>() / sharpes.len() as f64
            }
        })
        .collect();

    // Rank strategies by IS performance
    let mut ranked_indices: Vec<usize> = (0..equity_curves.len()).collect();
    ranked_indices.sort_by(|&a, &b| {
        avg_is[b]
            .partial_cmp(&avg_is[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Median OOS Sharpe
    let mut sorted_oos = avg_oos.clone();
    sorted_oos.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_oos = if sorted_oos.is_empty() {
        0.0
    } else {
        sorted_oos[sorted_oos.len() / 2]
    };

    // Split into top half (best IS) and bottom half
    let n_star = (ranked_indices.len() / 2).max(1);
    let best_is_indices = &ranked_indices[..n_star];

    // Count how many "best IS" strategies have OOS < median OOS
    let underperformers = best_is_indices
        .iter()
        .filter(|&&idx| avg_oos[idx] < median_oos)
        .count();

    let underperformance_rate = underperformers as f64 / n_star as f64;

    // PBO: probability that a randomly selected best-IS strategy underperforms
    let pbo = underperformance_rate;

    // Stochastic dominance: compare cumulative distributions
    // Higher value = more separation between IS-rank and OOS-performance
    let stochastic_dominance = compute_stochastic_dominance(&avg_is, &avg_oos);

    Some(PboResult {
        pbo,
        num_splits,
        median_oos_sharpe: median_oos,
        underperformance_rate,
        stochastic_dominance,
    })
}

/// Compute Sharpe ratio from an equity curve.
fn compute_sharpe_from_curve(curve: &[EquityPoint]) -> f64 {
    if curve.len() < 3 {
        return 0.0;
    }

    let returns: Vec<f64> = curve
        .windows(2)
        .map(|w| {
            let e0 = w[0].equity.to_f64().unwrap_or(1.0);
            let e1 = w[1].equity.to_f64().unwrap_or(1.0);
            (e1 / e0) - 1.0
        })
        .collect();

    let n = returns.len() as f64;
    let mean = returns.iter().sum::<f64>() / n;
    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    let std = var.sqrt();

    if std > 1e-10 {
        (mean / std) * 252.0_f64.sqrt()
    } else {
        0.0
    }
}

/// Compute stochastic dominance between IS and OOS rankings.
fn compute_stochastic_dominance(is_sharpes: &[f64], oos_sharpes: &[f64]) -> f64 {
    if is_sharpes.len() != oos_sharpes.len() || is_sharpes.is_empty() {
        return 0.0;
    }

    // Rank by IS (descending)
    let mut is_ranks: Vec<(usize, f64)> = is_sharpes.iter().copied().enumerate().collect();
    is_ranks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Compute cumulative OOS for each IS rank
    let n = is_ranks.len() as f64;
    let mut cumulative_oos_sum = 0.0;
    let mut integral = 0.0;

    for (i, (original_idx, _is_val)) in is_ranks.iter().enumerate() {
        cumulative_oos_sum += oos_sharpes[*original_idx];
        let expected_oos_at_rank = cumulative_oos_sum / (i + 1) as f64;

        // Ideally, expected OOS should decrease with IS rank (overfitting)
        // Integral measures area between ideal and actual
        integral += expected_oos_at_rank / n;
    }

    integral
}

/// Deflated Sharpe Ratio (DSR) from Bailey & López de Prado (2014).
///
/// Adjusts the Sharpe ratio for multiple testing bias. When testing N strategies,
/// the best observed Sharpe is inflated. DSR deflates it to account for selection bias.
///
/// DSR = (SR - E[max SR_null]) / std[max SR_null]
///
/// Where E[max SR_null] is the expected maximum Sharpe under the null hypothesis
/// (no skill, random returns), given N trials and T observations.
#[derive(Debug, Clone)]
pub struct DeflatedSharpeResult {
    /// Deflated Sharpe ratio (z-score).
    pub deflated_sharpe: f64,
    /// Original observed Sharpe ratio.
    pub observed_sharpe: f64,
    /// Expected maximum Sharpe under null (selection bias).
    pub expected_max_sharpe_null: f64,
    /// Standard deviation of max Sharpe under null.
    pub std_max_sharpe_null: f64,
    /// Number of trials (strategies tested).
    pub num_trials: i32,
    /// Number of observations (returns in backtest).
    pub num_observations: i32,
    /// p-value: probability of observing this Sharpe by chance.
    pub p_value: f64,
}

/// Compute deflated Sharpe ratio.
///
/// # Arguments
/// * `observed_sharpe` - The Sharpe ratio of the best strategy
/// * `num_trials` - Number of strategies tested (parameter combinations)
/// * `num_observations` - Number of returns in the backtest
/// * `skewness` - Skewness of returns (use 0.0 if unknown)
/// * `kurtosis` - Excess kurtosis of returns (use 0.0 if unknown)
///
/// # Returns
/// DeflatedSharpeResult with DSR and p-value.
pub fn deflated_sharpe_ratio(
    observed_sharpe: f64,
    num_trials: i32,
    num_observations: i32,
    skewness: f64,
    kurtosis: f64,
) -> DeflatedSharpeResult {
    let n = num_trials as f64;
    let t = num_observations as f64;

    if n < 1.0 || t < 3.0 {
        return DeflatedSharpeResult {
            deflated_sharpe: observed_sharpe,
            observed_sharpe,
            expected_max_sharpe_null: 0.0,
            std_max_sharpe_null: 1.0,
            num_trials,
            num_observations,
            p_value: 1.0,
        };
    }

    // Euler-Mascheroni constant
    let _gamma = 0.5772156649;

    // Expected value of max Sharpe under null hypothesis
    // E[max SR] ≈ sqrt(2 log N)
    let expected_max = (2.0 * n.ln()).sqrt();

    // Variance of max SR under null (from extreme value theory)
    // Var[max SR] ≈ 1 / (2 log N)
    let variance_max = 1.0 / (2.0 * n.ln());
    let std_max = variance_max.sqrt();

    // Adjust for non-Gaussianity (skewness and kurtosis)
    // Sharpe standard error: SE(SR) = sqrt((1 + SR²/2 - skew*SR + (kurt-1)*SR²/4) / T)
    let sr2 = observed_sharpe.powi(2);
    let se_adjustment = (1.0 + sr2 / 2.0 - skewness * observed_sharpe
        + (kurtosis) * sr2 / 4.0)
        / t;
    let se = se_adjustment.max(1.0 / t).sqrt();

    // Deflated Sharpe: z-score
    let deflated = (observed_sharpe - expected_max) / (std_max + se);

    // p-value from standard normal CDF
    let p_value = 2.0 * (1.0 - normal_cdf(deflated.abs()));

    DeflatedSharpeResult {
        deflated_sharpe: deflated,
        observed_sharpe,
        expected_max_sharpe_null: expected_max,
        std_max_sharpe_null: std_max,
        num_trials,
        num_observations,
        p_value,
    }
}

/// Out-of-sample degradation factor - measures how much performance decays OOS.
///
/// Degradation = (IS metric - OOS metric) / IS metric
///
/// Values close to 0 = low degradation (robust).
/// Values > 0.5 = high degradation (overfitting).
#[derive(Debug, Clone)]
pub struct OosDegradation {
    pub sharpe_degradation: f64,
    pub win_rate_degradation: f64,
    pub profit_factor_degradation: f64,
    pub overall_degradation_score: f64,
}

pub fn compute_oos_degradation(
    is_sharpe: f64,
    oos_sharpe: f64,
    is_win_rate: f64,
    oos_win_rate: f64,
    is_profit_factor: f64,
    oos_profit_factor: f64,
) -> OosDegradation {
    let sharpe_deg = if is_sharpe.abs() > 0.01 {
        (is_sharpe - oos_sharpe) / is_sharpe.abs()
    } else {
        0.0
    };

    let wr_deg = if is_win_rate > 0.01 {
        (is_win_rate - oos_win_rate) / is_win_rate
    } else {
        0.0
    };

    let pf_deg = if is_profit_factor > 0.01 {
        (is_profit_factor - oos_profit_factor) / is_profit_factor
    } else {
        0.0
    };

    // Overall: average degradation across metrics
    let overall = (sharpe_deg + wr_deg + pf_deg) / 3.0;

    OosDegradation {
        sharpe_degradation: sharpe_deg,
        win_rate_degradation: wr_deg,
        profit_factor_degradation: pf_deg,
        overall_degradation_score: overall,
    }
}

/// Minimum backtest length calculator - estimates required sample size.
///
/// Based on Bailey & López de Prado: T_min ≈ [(Z_α + Z_β) / SR*]²
///
/// Where:
/// - Z_α: significance level (e.g., 1.96 for 95% confidence)
/// - Z_β: power level (e.g., 0.84 for 80% power)
/// - SR*: expected Sharpe ratio
pub fn minimum_backtest_length(
    expected_sharpe: f64,
    confidence_level: f64,
    power: f64,
) -> i32 {
    let z_alpha = inverse_normal_cdf(1.0 - (1.0 - confidence_level) / 2.0);
    let z_beta = inverse_normal_cdf(power);

    if expected_sharpe.abs() < 0.01 {
        return 10000; // Infinite if Sharpe near zero
    }

    let t_min = ((z_alpha + z_beta) / expected_sharpe).powi(2);
    t_min.ceil() as i32
}

// --- Helper functions ---

use statrs::distribution::{ContinuousCDF, Normal};

fn normal_cdf(x: f64) -> f64 {
    let normal = Normal::new(0.0, 1.0).unwrap();
    normal.cdf(x)
}

fn inverse_normal_cdf(p: f64) -> f64 {
    let normal = Normal::new(0.0, 1.0).unwrap();
    normal.inverse_cdf(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal::prelude::FromPrimitive;

    fn mock_curve(returns: &[f64]) -> Vec<EquityPoint> {
        let mut equity = 100000.0;
        let mut peak = equity;
        let mut curve = vec![EquityPoint {
            timestamp: "2024-01-01".to_string(),
            equity: Decimal::from_f64(equity).unwrap(),
            drawdown_percent: 0.0,
        }];

        for (i, &ret) in returns.iter().enumerate() {
            equity *= 1.0 + ret;
            if equity > peak {
                peak = equity;
            }
            let dd = if peak > 0.0 {
                (peak - equity) / peak * 100.0
            } else {
                0.0
            };
            curve.push(EquityPoint {
                timestamp: format!("2024-01-{:02}", i + 2),
                equity: Decimal::from_f64(equity).unwrap(),
                drawdown_percent: dd,
            });
        }

        curve
    }

    #[test]
    fn test_deflated_sharpe_basic() {
        // Tested 100 strategies, best Sharpe = 2.0, over 252 returns
        let dsr = deflated_sharpe_ratio(2.0, 100, 252, 0.0, 0.0);

        // Expected max Sharpe under null ≈ sqrt(2 * ln(100)) ≈ 3.03
        let expected_max = (2.0 * 100.0_f64.ln()).sqrt();
        assert!((dsr.expected_max_sharpe_null - expected_max).abs() < 0.1);

        // Deflated Sharpe should be negative (observed < expected max)
        assert!(dsr.deflated_sharpe < 0.0);

        // With Sharpe=2.0 < expected_max=3.03, deflated Sharpe is significantly negative
        // This indicates the result is likely due to selection bias (multiple testing)
        // p-value should be small (significant), indicating high probability of false positive
        assert!(dsr.p_value < 0.05);
    }

    #[test]
    fn test_oos_degradation() {
        let deg = compute_oos_degradation(
            2.0,  // IS Sharpe
            1.5,  // OOS Sharpe
            65.0, // IS win rate
            55.0, // OOS win rate
            2.5,  // IS PF
            2.0,  // OOS PF
        );

        // Sharpe degradation: (2.0 - 1.5) / 2.0 = 0.25 = 25%
        assert!((deg.sharpe_degradation - 0.25).abs() < 0.01);

        // Win rate degradation: (65 - 55) / 65 ≈ 0.154
        assert!((deg.win_rate_degradation - (10.0 / 65.0)).abs() < 0.01);

        // PF degradation: (2.5 - 2.0) / 2.5 = 0.2
        assert!((deg.profit_factor_degradation - 0.2).abs() < 0.01);

        // Overall: average of 3
        let expected_overall = (0.25 + 10.0 / 65.0 + 0.2) / 3.0;
        assert!((deg.overall_degradation_score - expected_overall).abs() < 0.01);
    }

    #[test]
    fn test_minimum_backtest_length() {
        // For Sharpe = 1.0, 95% confidence, 80% power
        let min_len = minimum_backtest_length(1.0, 0.95, 0.80);

        // Formula: [(1.96 + 0.84) / 1.0]^2 = 2.8^2 = 7.84 → 8
        // (using approximate z-values)
        assert!(min_len >= 7 && min_len <= 10);

        // For Sharpe = 0.5, need 4x more data
        let min_len_low = minimum_backtest_length(0.5, 0.95, 0.80);
        assert!(min_len_low > min_len * 3);
    }

    #[test]
    fn test_pbo_basic() {
        // Create 4+ strategies with varying IS/OOS performance (need at least 4)
        let strategy1 = mock_curve(&vec![0.01; 100]); // Consistent positive
        let strategy2 = mock_curve(&vec![-0.005; 100]); // Consistent negative
        let strategy3 = mock_curve(&(0..100).map(|i| if i % 2 == 0 { 0.02 } else { -0.01 }).collect::<Vec<_>>()); // Volatile
        let strategy4 = mock_curve(&vec![0.005; 100]); // Modest positive

        let curves = vec![strategy1, strategy2, strategy3, strategy4];

        let pbo = compute_pbo(&curves, 8);
        assert!(pbo.is_some());

        let result = pbo.unwrap();
        assert!(result.pbo >= 0.0 && result.pbo <= 1.0);
        assert_eq!(result.num_splits, 8);
    }
}
