use crate::models::*;
use crate::shared_math;
use rust_decimal::prelude::*;

pub struct BenchmarkComparer;

impl BenchmarkComparer {
    /// Compare portfolio performance vs benchmark.
    /// `benchmark_prices` is a slice of (date_string, price) tuples, sorted chronologically.
    pub fn compare(
        snapshots: &[PortfolioSnapshot],
        benchmark_prices: &[(String, f64)],
        benchmark_symbol: &str,
    ) -> BenchmarkAnalysis {
        let empty = BenchmarkAnalysis {
            benchmark_symbol: benchmark_symbol.to_string(),
            alpha: 0.0,
            beta: 1.0,
            r_squared: 0.0,
            tracking_error: 0.0,
            information_ratio: None,
            portfolio_return_percent: 0.0,
            benchmark_return_percent: 0.0,
            excess_return_percent: 0.0,
            portfolio_indexed: Vec::new(),
            benchmark_indexed: Vec::new(),
            rolling_alpha: Vec::new(),
        };

        if snapshots.len() < 3 || benchmark_prices.len() < 3 {
            return empty;
        }

        let port_values: Vec<f64> = snapshots
            .iter()
            .map(|s| s.total_value.to_f64().unwrap_or(0.0))
            .collect();
        let port_dates: Vec<String> = snapshots.iter().map(|s| s.snapshot_date.clone()).collect();
        let bench_values: Vec<f64> = benchmark_prices.iter().map(|(_, p)| *p).collect();
        let _bench_dates: Vec<String> = benchmark_prices.iter().map(|(d, _)| d.clone()).collect();

        // Align by date — use the shorter of the two series
        let n = port_values.len().min(bench_values.len());
        let port_vals = &port_values[port_values.len() - n..];
        let bench_vals = &bench_values[bench_values.len() - n..];
        let dates_for_output = &port_dates[port_dates.len() - n..];

        let port_returns = shared_math::daily_returns(port_vals);
        let bench_returns = shared_math::daily_returns(bench_vals);

        let n_returns = port_returns.len().min(bench_returns.len());
        let pr = &port_returns[..n_returns];
        let br = &bench_returns[..n_returns];

        // OLS regression: portfolio_return = alpha + beta * benchmark_return
        let (alpha_daily, beta, r_squared) = shared_math::ols_regression(pr, br);
        let alpha_annual = alpha_daily * 252.0;

        let te = shared_math::tracking_error(pr, br);
        let info_ratio = if te > 1e-12 {
            let excess_mean =
                pr.iter().zip(br.iter()).map(|(p, b)| p - b).sum::<f64>() / n_returns as f64;
            Some(excess_mean / (te / 252.0_f64.sqrt()))
        } else {
            None
        };

        // Total returns
        let port_total = if port_vals[0] > 0.0 {
            (port_vals[port_vals.len() - 1] - port_vals[0]) / port_vals[0] * 100.0
        } else {
            0.0
        };
        let bench_total = if bench_vals[0] > 0.0 {
            (bench_vals[bench_vals.len() - 1] - bench_vals[0]) / bench_vals[0] * 100.0
        } else {
            0.0
        };

        // Indexed performance (base = 100)
        let port_indexed: Vec<IndexedPoint> = dates_for_output
            .iter()
            .zip(port_vals.iter())
            .map(|(d, v)| IndexedPoint {
                date: d.clone(),
                value: if port_vals[0] > 0.0 {
                    v / port_vals[0] * 100.0
                } else {
                    100.0
                },
            })
            .collect();

        let bench_indexed: Vec<IndexedPoint> = dates_for_output
            .iter()
            .zip(bench_vals.iter())
            .map(|(d, v)| IndexedPoint {
                date: d.clone(),
                value: if bench_vals[0] > 0.0 {
                    v / bench_vals[0] * 100.0
                } else {
                    100.0
                },
            })
            .collect();

        // Rolling 63-day alpha
        let window = 63;
        let mut rolling_alpha = Vec::new();
        if n_returns > window {
            for i in window..n_returns {
                let pr_win = &pr[i - window..i];
                let br_win = &br[i - window..i];
                let (a, _, _) = shared_math::ols_regression(pr_win, br_win);
                // i is the index into returns; the corresponding date is at index (i + 1) in dates_for_output
                let date_idx = (i + 1).min(dates_for_output.len() - 1);
                rolling_alpha.push(RollingAlpha {
                    date: dates_for_output[date_idx].clone(),
                    alpha: a * 252.0,
                });
            }
        }

        BenchmarkAnalysis {
            benchmark_symbol: benchmark_symbol.to_string(),
            alpha: alpha_annual,
            beta,
            r_squared,
            tracking_error: te,
            information_ratio: info_ratio,
            portfolio_return_percent: port_total,
            benchmark_return_percent: bench_total,
            excess_return_percent: port_total - bench_total,
            portfolio_indexed: port_indexed,
            benchmark_indexed: bench_indexed,
            rolling_alpha,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_benchmark_comparison() {
        let snapshots: Vec<PortfolioSnapshot> = (0..30)
            .map(|i| PortfolioSnapshot {
                id: None,
                total_value: Decimal::from_f64(10000.0 + i as f64 * 50.0).unwrap_or_default(),
                total_cost: Decimal::from(9000),
                total_pnl: Decimal::from(1000),
                total_pnl_percent: 10.0,
                snapshot_date: format!("2025-01-{:02}", i + 1),
            })
            .collect();

        let benchmark: Vec<(String, f64)> = (0..30)
            .map(|i| (format!("2025-01-{:02}", i + 1), 400.0 + i as f64 * 2.0))
            .collect();

        let result = BenchmarkComparer::compare(&snapshots, &benchmark, "SPY");
        assert_eq!(result.benchmark_symbol, "SPY");
        assert!(result.portfolio_return_percent > 0.0);
        assert!(result.benchmark_return_percent > 0.0);
        assert_eq!(result.portfolio_indexed.len(), 30);
    }

    #[test]
    fn test_empty_benchmark() {
        let result = BenchmarkComparer::compare(&[], &[], "SPY");
        assert_eq!(result.alpha, 0.0);
        assert_eq!(result.beta, 1.0);
    }
}
