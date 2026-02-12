use chrono::NaiveDate;
use rayon::prelude::*;

use crate::advanced_risk;
use crate::models::*;

/// Compute extended performance metrics from equity curve, trades, and benchmark returns.
pub fn compute_extended_metrics(
    equity_curve: &[EquityPoint],
    _trades: &[BacktestTrade],
    benchmark_returns: Option<&[f64]>,
    total_return_percent: f64,
    annualized_return_percent: Option<f64>,
) -> ExtendedMetrics {
    let returns = equity_returns(equity_curve);
    let rf_daily = 0.02 / 252.0;

    // Treynor ratio: (Rp - Rf) / Beta
    let treynor_ratio = benchmark_returns.and_then(|br| {
        let beta = compute_beta(&returns, br)?;
        if beta.abs() < 1e-10 {
            return None;
        }
        let n = returns.len() as f64;
        let mean_ret = returns.iter().sum::<f64>() / n;
        Some(((mean_ret - rf_daily) * 252.0) / beta)
    });

    // Jensen's alpha: Rp - [Rf + Beta * (Rm - Rf)]
    let jensens_alpha = benchmark_returns.and_then(|br| {
        let beta = compute_beta(&returns, br)?;
        let n = returns.len() as f64;
        let mean_ret = returns.iter().sum::<f64>() / n;
        let mean_bench = br.iter().sum::<f64>() / br.len().max(1) as f64;
        Some((mean_ret - (rf_daily + beta * (mean_bench - rf_daily))) * 252.0)
    });

    // Omega ratio: sum of gains above threshold / sum of losses below threshold
    let omega_ratio = compute_omega(&returns, rf_daily);

    // Tail ratio: 95th percentile / abs(5th percentile)
    let tail_ratio = compute_tail_ratio(&returns);

    // Skewness and kurtosis
    let skewness = compute_skewness(&returns);
    let kurtosis = compute_excess_kurtosis(&returns);

    // Top 5 drawdown events
    let top_drawdown_events = find_top_drawdowns(equity_curve, 5);

    // Max drawdown duration
    let max_drawdown_duration_days = top_drawdown_events
        .iter()
        .map(|e| e.duration_days + e.recovery_days.unwrap_or(0))
        .max();

    // Monthly returns
    let monthly_returns = compute_monthly_returns(equity_curve);

    // Rolling Sharpe (63-day / ~3 months)
    let rolling_sharpe = compute_rolling_sharpe(&returns, equity_curve, 63);

    // Advanced risk metrics (institutional-grade)
    let cdar_95 = advanced_risk::conditional_drawdown_at_risk(equity_curve, 0.05);
    let ulcer_index = advanced_risk::ulcer_index(equity_curve);
    let pain_index = advanced_risk::pain_index(equity_curve);
    let gain_to_pain_ratio = advanced_risk::gain_to_pain_ratio(total_return_percent, equity_curve);
    let burke_ratio = advanced_risk::burke_ratio(total_return_percent, equity_curve, 3);
    let sterling_ratio = annualized_return_percent
        .and_then(|cagr| advanced_risk::sterling_ratio(cagr, equity_curve, 3));

    ExtendedMetrics {
        treynor_ratio,
        jensens_alpha,
        omega_ratio,
        tail_ratio,
        skewness,
        kurtosis,
        top_drawdown_events,
        monthly_returns,
        rolling_sharpe,
        max_drawdown_duration_days,
        cdar_95,
        ulcer_index,
        pain_index,
        gain_to_pain_ratio,
        burke_ratio,
        sterling_ratio,
    }
}

/// Extract daily returns from equity curve.
pub fn equity_returns(equity_curve: &[EquityPoint]) -> Vec<f64> {
    equity_curve
        .windows(2)
        .map(|w| {
            let e0 = rust_decimal::prelude::ToPrimitive::to_f64(&w[0].equity).unwrap_or(1.0);
            let e1 = rust_decimal::prelude::ToPrimitive::to_f64(&w[1].equity).unwrap_or(1.0);
            (e1 / e0) - 1.0
        })
        .collect()
}

fn compute_beta(returns: &[f64], benchmark_returns: &[f64]) -> Option<f64> {
    let n = returns.len().min(benchmark_returns.len());
    if n < 3 {
        return None;
    }

    let r = &returns[..n];
    let b = &benchmark_returns[..n];

    let mean_r = r.iter().sum::<f64>() / n as f64;
    let mean_b = b.iter().sum::<f64>() / n as f64;

    let cov: f64 = r
        .iter()
        .zip(b.iter())
        .map(|(ri, bi)| (ri - mean_r) * (bi - mean_b))
        .sum::<f64>()
        / (n - 1) as f64;

    let var_b: f64 = b.iter().map(|bi| (bi - mean_b).powi(2)).sum::<f64>() / (n - 1) as f64;

    if var_b > 1e-15 {
        Some(cov / var_b)
    } else {
        None
    }
}

fn compute_omega(returns: &[f64], threshold: f64) -> Option<f64> {
    if returns.is_empty() {
        return None;
    }
    let gains: f64 = returns.iter().map(|r| (r - threshold).max(0.0)).sum();
    let losses: f64 = returns.iter().map(|r| (threshold - r).max(0.0)).sum();
    if losses > 1e-15 {
        Some(gains / losses)
    } else if gains > 0.0 {
        Some(f64::INFINITY)
    } else {
        None
    }
}

fn compute_tail_ratio(returns: &[f64]) -> Option<f64> {
    if returns.len() < 20 {
        return None;
    }
    let mut sorted = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95 = percentile_sorted(&sorted, 95.0);
    let p5 = percentile_sorted(&sorted, 5.0);
    if p5.abs() > 1e-10 {
        Some(p95 / p5.abs())
    } else {
        None
    }
}

fn compute_skewness(returns: &[f64]) -> Option<f64> {
    let n = returns.len() as f64;
    if n < 3.0 {
        return None;
    }
    let mean = returns.iter().sum::<f64>() / n;
    let m2: f64 = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
    let m3: f64 = returns.iter().map(|r| (r - mean).powi(3)).sum::<f64>() / n;
    let std = m2.sqrt();
    if std > 1e-15 {
        Some(m3 / std.powi(3))
    } else {
        None
    }
}

fn compute_excess_kurtosis(returns: &[f64]) -> Option<f64> {
    let n = returns.len() as f64;
    if n < 4.0 {
        return None;
    }
    let mean = returns.iter().sum::<f64>() / n;
    let m2: f64 = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
    let m4: f64 = returns.iter().map(|r| (r - mean).powi(4)).sum::<f64>() / n;
    if m2 > 1e-15 {
        Some(m4 / m2.powi(2) - 3.0)
    } else {
        None
    }
}

fn find_top_drawdowns(equity_curve: &[EquityPoint], top_n: usize) -> Vec<DrawdownEvent> {
    if equity_curve.is_empty() {
        return Vec::new();
    }

    // Find all drawdown periods
    let mut events: Vec<DrawdownEvent> = Vec::new();
    let mut peak_idx = 0usize;
    let mut peak_val =
        rust_decimal::prelude::ToPrimitive::to_f64(&equity_curve[0].equity).unwrap_or(1.0);
    let mut in_drawdown = false;
    let mut dd_start_idx = 0usize;
    let mut trough_idx = 0usize;
    let mut trough_val = peak_val;

    for (i, point) in equity_curve.iter().enumerate() {
        let val = rust_decimal::prelude::ToPrimitive::to_f64(&point.equity).unwrap_or(0.0);

        if val >= peak_val {
            if in_drawdown {
                // Recovery — record the drawdown event
                let dd_pct = (peak_val - trough_val) / peak_val * 100.0;
                let start_date = equity_curve[dd_start_idx].timestamp.clone();
                let trough_date = equity_curve[trough_idx].timestamp.clone();
                let recovery_date = Some(point.timestamp.clone());
                let duration = date_diff_days(&start_date, &trough_date);
                let recovery = date_diff_days(&trough_date, &point.timestamp);

                events.push(DrawdownEvent {
                    start_date,
                    trough_date,
                    recovery_date,
                    drawdown_percent: dd_pct,
                    duration_days: duration,
                    recovery_days: Some(recovery),
                });
                in_drawdown = false;
            }
            peak_val = val;
            peak_idx = i;
        } else {
            if !in_drawdown {
                in_drawdown = true;
                dd_start_idx = peak_idx;
                trough_idx = i;
                trough_val = val;
            }
            if val < trough_val {
                trough_val = val;
                trough_idx = i;
            }
        }
    }

    // Handle ongoing drawdown at end
    if in_drawdown {
        let dd_pct = (peak_val - trough_val) / peak_val * 100.0;
        let start_date = equity_curve[dd_start_idx].timestamp.clone();
        let trough_date = equity_curve[trough_idx].timestamp.clone();
        let duration = date_diff_days(&start_date, &trough_date);

        events.push(DrawdownEvent {
            start_date,
            trough_date,
            recovery_date: None,
            drawdown_percent: dd_pct,
            duration_days: duration,
            recovery_days: None,
        });
    }

    // Sort by magnitude, take top N
    events.sort_by(|a, b| {
        b.drawdown_percent
            .partial_cmp(&a.drawdown_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    events.truncate(top_n);
    events
}

fn compute_monthly_returns(equity_curve: &[EquityPoint]) -> Vec<MonthlyReturn> {
    if equity_curve.is_empty() {
        return Vec::new();
    }

    let mut monthly: Vec<MonthlyReturn> = Vec::new();
    let mut month_start_equity =
        rust_decimal::prelude::ToPrimitive::to_f64(&equity_curve[0].equity).unwrap_or(1.0);
    let mut prev_ym: Option<(i32, u32)> = None;

    for point in equity_curve {
        if let Ok(date) = NaiveDate::parse_from_str(&point.timestamp, "%Y-%m-%d") {
            let ym = (date.year(), date.month());

            match prev_ym {
                Some(prev) if prev != ym => {
                    // Month changed — record the previous month
                    let equity =
                        rust_decimal::prelude::ToPrimitive::to_f64(&point.equity).unwrap_or(1.0);
                    if month_start_equity > 0.0 {
                        monthly.push(MonthlyReturn {
                            year: prev.0,
                            month: prev.1 as i32,
                            return_percent: (equity / month_start_equity - 1.0) * 100.0,
                        });
                    }
                    month_start_equity = equity;
                }
                None => {
                    // First point
                }
                _ => {}
            }
            prev_ym = Some(ym);
        }
    }

    // Record the last month
    if let (Some(prev), Some(last)) = (prev_ym, equity_curve.last()) {
        let equity = rust_decimal::prelude::ToPrimitive::to_f64(&last.equity).unwrap_or(1.0);
        if month_start_equity > 0.0 {
            monthly.push(MonthlyReturn {
                year: prev.0,
                month: prev.1 as i32,
                return_percent: (equity / month_start_equity - 1.0) * 100.0,
            });
        }
    }

    monthly
}

fn compute_rolling_sharpe(
    returns: &[f64],
    equity_curve: &[EquityPoint],
    window: usize,
) -> Vec<RollingSharpePoint> {
    if returns.len() < window {
        return Vec::new();
    }

    let rf_daily = 0.02 / 252.0;

    // Compute rolling Sharpe values in parallel using rayon
    let indices: Vec<usize> = (window..=returns.len()).collect();
    let sharpe_values: Vec<(usize, f64)> = indices
        .par_iter()
        .map(|&i| {
            let window_rets = &returns[i - window..i];
            let n = window_rets.len() as f64;
            let mean = window_rets.iter().sum::<f64>() / n;
            let var =
                window_rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
            let std = var.sqrt();

            let sharpe = if std > 1e-10 {
                ((mean - rf_daily) / std) * 252.0_f64.sqrt()
            } else {
                0.0
            };
            (i, sharpe)
        })
        .collect();

    // Build result points (must be in order)
    let mut points = Vec::with_capacity(sharpe_values.len());
    for (i, sharpe) in sharpe_values {
        if i < equity_curve.len() {
            points.push(RollingSharpePoint {
                date: equity_curve[i].timestamp.clone(),
                sharpe,
            });
        }
    }

    points
}

use chrono::Datelike;

fn date_diff_days(from: &str, to: &str) -> i64 {
    let parse = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    match (parse(from), parse(to)) {
        (Some(a), Some(b)) => (b - a).num_days().abs(),
        _ => 0,
    }
}

fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
