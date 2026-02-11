use analysis_core::{adaptive, AnalysisError, AnalysisResult, Bar, QuantAnalyzer, SignalStrength};
use async_trait::async_trait;
use chrono::{Datelike, Utc};
use rayon::prelude::*;
use serde_json::json;
use statrs::statistics::Statistics;

pub struct QuantAnalysisEngine;

impl QuantAnalysisEngine {
    pub fn new() -> Self {
        Self
    }

    /// Calculate returns from prices
    fn calculate_returns(&self, prices: &[f64]) -> Vec<f64> {
        prices
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect()
    }

    /// Calculate Sharpe Ratio (annualized)
    #[allow(dead_code)]
    fn calculate_sharpe_ratio(&self, returns: &[f64], risk_free_rate: f64) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let mean_return = returns.mean();
        let std_dev = returns.std_dev();

        if std_dev == 0.0 {
            return 0.0;
        }

        // Annualize: assuming daily returns
        let annualized_return = mean_return * 252.0;
        let annualized_volatility = std_dev * (252.0_f64).sqrt();

        (annualized_return - risk_free_rate) / annualized_volatility
    }

    /// Calculate maximum drawdown
    fn calculate_max_drawdown(&self, prices: &[f64]) -> f64 {
        if prices.is_empty() {
            return 0.0;
        }

        let mut max_price = prices[0];
        let mut max_dd = 0.0;

        for &price in prices.iter() {
            if price > max_price {
                max_price = price;
            }

            let drawdown = (max_price - price) / max_price;
            if drawdown > max_dd {
                max_dd = drawdown;
            }
        }

        max_dd * 100.0 // Return as percentage
    }

    /// Calculate volatility (annualized)
    fn calculate_volatility(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let std_dev = returns.std_dev();
        std_dev * (252.0_f64).sqrt() * 100.0 // Annualized and as percentage
    }

    /// Calculate Beta (market sensitivity)
    /// For simplicity, this uses a mock market return
    fn calculate_beta(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() {
            return 1.0;
        }

        // In production, you'd compare against actual market index returns (e.g., SPY)
        // For now, we'll use a simplified calculation
        let volatility = returns.std_dev();
        let market_volatility = 0.15 / (252.0_f64).sqrt(); // Assume 15% annual market volatility

        if market_volatility == 0.0 {
            return 1.0;
        }

        volatility / market_volatility
    }

    /// Calculate win rate for a simple momentum strategy
    fn calculate_win_rate(&self, bars: &[Bar]) -> f64 {
        if bars.len() < 2 {
            return 0.5;
        }

        let mut wins = 0;
        let mut total = 0;

        for i in 1..bars.len() {
            // Simple momentum: if previous close > close before that, expect price to go up
            if i >= 2 {
                let prev_momentum = bars[i - 1].close > bars[i - 2].close;
                let actual_up = bars[i].close > bars[i - 1].close;

                total += 1;
                if prev_momentum == actual_up {
                    wins += 1;
                }
            }
        }

        if total > 0 {
            wins as f64 / total as f64
        } else {
            0.5
        }
    }

    /// Calculate Value at Risk (VaR) at 95% confidence
    fn calculate_var(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let mut sorted_returns = returns.to_vec();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let index = (returns.len() as f64 * 0.05) as usize;
        if index < sorted_returns.len() {
            sorted_returns[index].abs() * 100.0 // As percentage
        } else {
            0.0
        }
    }

    /// Calculate Sortino Ratio (uses downside deviation only)
    fn calculate_sortino_ratio(&self, returns: &[f64], risk_free_rate: f64) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let mean_return = returns.mean();
        let annualized_return = mean_return * 252.0;

        // Downside deviation: std dev of returns below risk-free daily rate
        let daily_rf = risk_free_rate / 252.0;
        let downside_returns: Vec<f64> = returns
            .iter()
            .filter(|&&r| r < daily_rf)
            .map(|&r| (r - daily_rf).powi(2))
            .collect();

        if downside_returns.is_empty() {
            return 3.0; // No downside = excellent
        }

        let downside_variance = downside_returns.iter().sum::<f64>() / returns.len() as f64;
        let downside_dev = downside_variance.sqrt() * (252.0_f64).sqrt();

        if downside_dev == 0.0 {
            return 3.0;
        }

        (annualized_return - risk_free_rate) / downside_dev
    }

    /// Calculate real beta using covariance with benchmark returns
    fn calculate_real_beta(&self, stock_returns: &[f64], benchmark_returns: &[f64]) -> f64 {
        let n = stock_returns.len().min(benchmark_returns.len());
        if n < 2 {
            return 1.0;
        }

        let stock = &stock_returns[stock_returns.len() - n..];
        let bench = &benchmark_returns[benchmark_returns.len() - n..];

        let stock_mean = stock.iter().sum::<f64>() / n as f64;
        let bench_mean = bench.iter().sum::<f64>() / n as f64;

        let mut covariance = 0.0;
        let mut bench_variance = 0.0;

        for i in 0..n {
            let stock_diff = stock[i] - stock_mean;
            let bench_diff = bench[i] - bench_mean;
            covariance += stock_diff * bench_diff;
            bench_variance += bench_diff * bench_diff;
        }

        if bench_variance == 0.0 {
            return 1.0;
        }

        covariance / bench_variance
    }

    /// Calculate win rate for mean-reversion strategy (10-SMA crossover)
    fn calculate_mean_reversion_win_rate(&self, bars: &[Bar]) -> f64 {
        if bars.len() < 12 {
            return 0.5;
        }

        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let sma_10: Vec<f64> = {
            let mut result = Vec::new();
            for i in 9..closes.len() {
                let sum: f64 = closes[i + 1 - 10..=i].iter().sum();
                result.push(sum / 10.0);
            }
            result
        };

        if sma_10.len() < 3 {
            return 0.5;
        }

        let mut wins = 0;
        let mut total = 0;

        // Mean reversion: price below SMA → expect it to revert up (and vice versa)
        for i in 1..sma_10.len() - 1 {
            let price_idx = i + 9; // offset for SMA warmup
            if price_idx + 1 >= closes.len() {
                break;
            }
            let below_sma = closes[price_idx] < sma_10[i];
            let next_up = closes[price_idx + 1] > closes[price_idx];

            total += 1;
            // Mean reversion wins when below_sma predicts next_up (and above predicts next_down)
            if below_sma == next_up {
                wins += 1;
            }
        }

        if total > 0 {
            wins as f64 / total as f64
        } else {
            0.5
        }
    }

    /// CVaR (Conditional VaR) / Expected Shortfall at 95% confidence
    fn calculate_cvar(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() { return 0.0; }
        let mut sorted = returns.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let cutoff = (returns.len() as f64 * 0.05).ceil() as usize;
        if cutoff == 0 { return 0.0; }
        let tail = &sorted[..cutoff];
        (tail.iter().sum::<f64>() / tail.len() as f64).abs() * 100.0
    }

    /// Hurst Exponent via Rescaled Range (R/S) analysis.
    /// Uses rayon to parallelize R/S computation over sub-series sizes.
    fn calculate_hurst_exponent(&self, prices: &[f64]) -> f64 {
        let log_returns: Vec<f64> = prices.windows(2)
            .map(|w| if w[0] > 0.0 { (w[1] / w[0]).ln() } else { 0.0 })
            .collect();
        if log_returns.len() < 20 { return 0.5; }

        let sizes: Vec<usize> = [8usize, 16, 32, 64, 128]
            .iter()
            .copied()
            .filter(|&s| s <= log_returns.len())
            .collect();

        // Compute R/S for each sub-series size in parallel
        let rs_results: Vec<Option<(f64, f64)>> = sizes
            .par_iter()
            .map(|&size| {
                let num_sub = log_returns.len() / size;
                if num_sub == 0 { return None; }

                let mut rs_values = Vec::new();
                for s in 0..num_sub {
                    let sub = &log_returns[s * size..(s + 1) * size];
                    let mean = sub.iter().sum::<f64>() / size as f64;
                    let deviations: Vec<f64> = sub.iter().map(|r| r - mean).collect();
                    let mut cum = Vec::with_capacity(size);
                    let mut sum = 0.0;
                    for d in &deviations { sum += d; cum.push(sum); }
                    let range = cum.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                        - cum.iter().cloned().fold(f64::INFINITY, f64::min);
                    let std_dev = (deviations.iter().map(|d| d * d).sum::<f64>() / size as f64).sqrt();
                    if std_dev > 0.0 { rs_values.push(range / std_dev); }
                }
                if !rs_values.is_empty() {
                    let avg_rs = rs_values.iter().sum::<f64>() / rs_values.len() as f64;
                    if avg_rs > 0.0 {
                        return Some((avg_rs.ln(), (size as f64).ln()));
                    }
                }
                None
            })
            .collect();

        let mut log_rs = Vec::new();
        let mut log_n = Vec::new();
        for result in rs_results.into_iter().flatten() {
            log_rs.push(result.0);
            log_n.push(result.1);
        }

        if log_rs.len() < 2 { return 0.5; }
        let n = log_rs.len() as f64;
        let sum_x: f64 = log_n.iter().sum();
        let sum_y: f64 = log_rs.iter().sum();
        let sum_xy: f64 = log_n.iter().zip(log_rs.iter()).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = log_n.iter().map(|x| x * x).sum();
        let denom = n * sum_x2 - sum_x * sum_x;
        if denom == 0.0 { return 0.5; }
        let slope = (n * sum_xy - sum_x * sum_y) / denom;
        slope.clamp(0.0, 1.0)
    }

    /// Autocorrelation at a given lag
    fn calculate_autocorrelation(&self, returns: &[f64], lag: usize) -> f64 {
        if returns.len() <= lag { return 0.0; }
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let var: f64 = returns.iter().map(|r| (r - mean).powi(2)).sum();
        if var == 0.0 { return 0.0; }
        let cov: f64 = returns[lag..].iter().zip(returns.iter())
            .map(|(r1, r0)| (r1 - mean) * (r0 - mean))
            .sum();
        cov / var
    }

    /// GARCH(1,1) one-step volatility forecast (annualized %)
    fn forecast_volatility_garch(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() { return 0.0; }
        let omega = 0.00001;
        let alpha = 0.1;
        let beta = 0.85;

        let var = returns.iter().map(|r| r * r).sum::<f64>() / returns.len() as f64;
        let mut sigma2 = var;
        for r in returns {
            sigma2 = omega + alpha * r * r + beta * sigma2;
        }
        let last_r = returns.last().unwrap_or(&0.0);
        let forecast = omega + alpha * last_r * last_r + beta * sigma2;
        forecast.sqrt() * (252.0_f64).sqrt() * 100.0
    }

    /// Kelly Criterion optimal fraction
    fn calculate_kelly(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() { return 0.0; }
        let wins: Vec<f64> = returns.iter().filter(|&&r| r > 0.0).copied().collect();
        let losses: Vec<f64> = returns.iter().filter(|&&r| r < 0.0).copied().collect();
        if wins.is_empty() || losses.is_empty() { return 0.0; }
        let win_rate = wins.len() as f64 / returns.len() as f64;
        let avg_win = wins.iter().sum::<f64>() / wins.len() as f64;
        let avg_loss = losses.iter().map(|l| l.abs()).sum::<f64>() / losses.len() as f64;
        if avg_loss == 0.0 { return 0.0; }
        let r = avg_win / avg_loss;
        (win_rate - (1.0 - win_rate) / r).clamp(-1.0, 1.0)
    }

    /// Momentum factor: 12-month return minus last month
    fn calculate_momentum_factor(&self, prices: &[f64]) -> Option<f64> {
        if prices.len() < 252 { return None; }
        let n = prices.len();
        let ret_12m = (prices[n - 1] - prices[n - 252]) / prices[n - 252];
        let ret_1m = (prices[n - 1] - prices[n - 21.min(n)]) / prices[n - 21.min(n)];
        Some(ret_12m - ret_1m)
    }

    /// Omega Ratio: probability-weighted ratio of gains to losses relative to threshold
    /// More comprehensive than Sharpe as it considers entire return distribution
    fn calculate_omega_ratio(&self, returns: &[f64], threshold: f64) -> f64 {
        if returns.is_empty() { return 1.0; }
        let daily_threshold = threshold / 252.0; // Convert annual to daily

        let gains: f64 = returns.iter().filter(|&&r| r > daily_threshold).map(|r| r - daily_threshold).sum();
        let losses: f64 = returns.iter().filter(|&&r| r < daily_threshold).map(|r| daily_threshold - r).sum();

        if losses > 0.0 { gains / losses } else if gains > 0.0 { 99.99 } else { 1.0 }
    }

    /// Mean Reversion Speed (Half-Life of Price Deviations from SMA)
    /// Measures how quickly price returns to its moving average
    fn calculate_mean_reversion_speed(&self, prices: &[f64]) -> Option<f64> {
        if prices.len() < 50 { return None; }

        // Use 20-SMA as equilibrium
        let sma_20: Vec<f64> = {
            let mut result = Vec::new();
            for i in 19..prices.len() {
                let sum: f64 = prices[i + 1 - 20..=i].iter().sum();
                result.push(sum / 20.0);
            }
            result
        };

        // Calculate deviations from SMA
        let deviations: Vec<f64> = prices[19..].iter().zip(sma_20.iter())
            .map(|(p, sma)| if *sma > 0.0 { (p - sma) / sma } else { 0.0 })
            .collect();

        if deviations.is_empty() { return None; }

        // Estimate AR(1) coefficient: dev[t] = rho * dev[t-1] + epsilon
        // Half-life = -ln(2) / ln(rho)
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;
        for i in 1..deviations.len() {
            sum_xy += deviations[i - 1] * deviations[i];
            sum_x2 += deviations[i - 1] * deviations[i - 1];
        }

        if sum_x2 == 0.0 { return None; }
        let rho = sum_xy / sum_x2;

        if rho > 0.0 && rho < 1.0 {
            let half_life = -std::f64::consts::LN_2 / rho.ln();
            Some(half_life.clamp(0.1, 100.0)) // Clamp to reasonable range (days)
        } else {
            None // No mean reversion (rho >= 1) or oscillatory (rho <= 0)
        }
    }

    /// Jump Diffusion Detection: identifies days with abnormal returns (potential jumps)
    /// Returns (jump_days, jump_intensity)
    fn detect_jumps(&self, returns: &[f64], threshold_sigma: f64) -> (usize, f64) {
        if returns.len() < 20 { return (0, 0.0); }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let std_dev = {
            let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
            variance.sqrt()
        };

        if std_dev == 0.0 { return (0, 0.0); }

        let jumps: Vec<f64> = returns.iter()
            .filter_map(|&r| {
                let z = (r - mean) / std_dev;
                if z.abs() > threshold_sigma { Some(r) } else { None }
            })
            .collect();

        let jump_days = jumps.len();
        let jump_intensity = if jump_days > 0 {
            jumps.iter().map(|j| j.abs()).sum::<f64>() / returns.len() as f64
        } else {
            0.0
        };

        (jump_days, jump_intensity)
    }

    /// Rachev Ratio: ratio of expected tail gains to tail losses (like CVaR-based Sharpe)
    fn calculate_rachev_ratio(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() { return 1.0; }

        let mut sorted = returns.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let cutoff = (returns.len() as f64 * 0.05).ceil() as usize;
        if cutoff == 0 || cutoff >= sorted.len() { return 1.0; }

        // Expected tail loss (5% worst returns)
        let tail_loss = sorted[..cutoff].iter().sum::<f64>() / cutoff as f64;
        // Expected tail gain (5% best returns)
        let tail_gain = sorted[sorted.len() - cutoff..].iter().sum::<f64>() / cutoff as f64;

        if tail_loss < 0.0 {
            tail_gain / tail_loss.abs()
        } else if tail_gain > 0.0 {
            99.99
        } else {
            1.0
        }
    }

    /// Enhanced analysis with optional SPY benchmark for real beta
    pub fn analyze_with_benchmark(
        &self,
        symbol: &str,
        bars: &[Bar],
        spy_bars: Option<&[Bar]>,
    ) -> Result<AnalysisResult, AnalysisError> {
        self.analyze_with_benchmark_and_rate(symbol, bars, spy_bars, None)
    }

    /// Full analysis with optional SPY benchmark and optional dynamic risk-free rate
    pub fn analyze_with_benchmark_and_rate(
        &self,
        symbol: &str,
        bars: &[Bar],
        spy_bars: Option<&[Bar]>,
        dynamic_risk_free_rate: Option<f64>,
    ) -> Result<AnalysisResult, AnalysisError> {
        if bars.len() < 30 {
            return Err(AnalysisError::InsufficientData(
                "Need at least 30 bars for quantitative analysis".to_string(),
            ));
        }

        let prices: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let returns = self.calculate_returns(&prices);
        let risk_free_rate = dynamic_risk_free_rate.unwrap_or(0.045);

        let mut signals = Vec::new();

        // Sharpe Ratio (updated risk-free rate)
        let sharpe = {
            let r = returns.as_slice();
            let mean_return = r.mean();
            let std_dev = r.std_dev();
            if std_dev == 0.0 {
                0.0
            } else {
                let annualized_return = mean_return * 252.0;
                let annualized_volatility = std_dev * (252.0_f64).sqrt();
                (annualized_return - risk_free_rate) / annualized_volatility
            }
        };
        // Adaptive Sharpe threshold: rolling 60-day windows
        if returns.len() >= 60 {
            let mut rolling_sharpes = Vec::new();
            for i in 60..=returns.len() {
                let window = &returns[i-60..i];
                let mean_r = window.mean();
                let std_r = window.std_dev();
                if std_r > 0.0 {
                    let ann_ret = mean_r * 252.0;
                    let ann_vol = std_r * (252.0_f64).sqrt();
                    rolling_sharpes.push((ann_ret - risk_free_rate) / ann_vol);
                }
            }
            if !rolling_sharpes.is_empty() {
                let sharpe_pct = adaptive::percentile_rank(sharpe, &rolling_sharpes);
                let sharpe_z = adaptive::z_score_of(sharpe, &rolling_sharpes);
                let sharpe_weight = adaptive::z_score_to_weight(sharpe_z);
                if sharpe_pct > 0.80 {
                    signals.push(("Good Risk-Adjusted Return", sharpe_weight, true));
                } else if sharpe_pct < 0.20 {
                    signals.push(("Poor Risk-Adjusted Return", sharpe_weight, false));
                }
            }
        } else if sharpe > 1.0 {
            signals.push(("Good Risk-Adjusted Return", 3, true));
        } else if sharpe < 0.0 {
            signals.push(("Poor Risk-Adjusted Return", 2, false));
        }

        // Sortino Ratio
        let sortino = self.calculate_sortino_ratio(&returns, risk_free_rate);
        // Adaptive Sortino: use z-score vs benchmark distribution
        let sortino_benchmarks = vec![0.0, 0.5, 1.0, 1.5, 2.0];
        let sortino_z = adaptive::z_score_of(sortino, &sortino_benchmarks);
        let sortino_weight = adaptive::z_score_to_weight(sortino_z);
        if sortino_z > 1.0 {
            signals.push(("Strong Downside Protection", sortino_weight, true));
        } else if sortino < 0.0 {
            signals.push(("Poor Downside Profile", sortino_weight.max(1), false));
        }

        // Volatility
        let volatility = self.calculate_volatility(&returns);
        // Adaptive volatility: rolling 30-day windows
        if returns.len() >= 30 {
            let mut rolling_vols = Vec::new();
            for i in 30..=returns.len() {
                let window = &returns[i-30..i];
                let std_dev = window.std_dev();
                rolling_vols.push(std_dev * (252.0_f64).sqrt() * 100.0);
            }
            if !rolling_vols.is_empty() {
                let vol_pct = adaptive::percentile_rank(volatility, &rolling_vols);
                let vol_z = adaptive::z_score_of(volatility, &rolling_vols);
                let vol_weight = adaptive::z_score_to_weight(vol_z);
                if vol_pct > 0.85 {
                    signals.push(("High Volatility", vol_weight, false));
                } else if vol_pct < 0.15 {
                    signals.push(("Low Volatility", vol_weight, true));
                }
            }
        } else if volatility < 20.0 {
            signals.push(("Low Volatility", 1, true));
        } else if volatility > 40.0 {
            signals.push(("High Volatility", 2, false));
        }

        // Max Drawdown
        let max_dd = self.calculate_max_drawdown(&prices);
        // Adaptive max drawdown: rolling 60-day windows
        if prices.len() >= 60 {
            let mut rolling_dds = Vec::new();
            for i in 60..=prices.len() {
                let window = &prices[i-60..i];
                let mut max_price = window[0];
                let mut max_dd_window = 0.0;
                for &price in window.iter() {
                    if price > max_price {
                        max_price = price;
                    }
                    let drawdown = (max_price - price) / max_price * 100.0;
                    if drawdown > max_dd_window {
                        max_dd_window = drawdown;
                    }
                }
                rolling_dds.push(max_dd_window);
            }
            if !rolling_dds.is_empty() {
                let dd_pct = adaptive::percentile_rank(max_dd, &rolling_dds);
                let dd_z = adaptive::z_score_of(max_dd, &rolling_dds);
                let dd_weight = adaptive::z_score_to_weight(dd_z);
                if dd_pct > 0.85 {
                    signals.push(("High Drawdown", dd_weight, false));
                } else if dd_pct < 0.15 {
                    signals.push(("Low Drawdown", dd_weight, true));
                }
            }
        } else if max_dd < 10.0 {
            signals.push(("Low Drawdown", 2, true));
        } else if max_dd > 25.0 {
            signals.push(("High Drawdown", 2, false));
        }

        // Beta — real calculation if SPY bars available
        let beta = if let Some(spy) = spy_bars {
            let spy_prices: Vec<f64> = spy.iter().map(|b| b.close).collect();
            let spy_returns = self.calculate_returns(&spy_prices);
            self.calculate_real_beta(&returns, &spy_returns)
        } else {
            self.calculate_beta(&returns)
        };
        if beta > 1.2 {
            signals.push(("High Beta (Aggressive)", 1, false));
        } else if beta < 0.8 {
            signals.push(("Low Beta (Defensive)", 1, true));
        }

        // Win Rate — test both strategies, report the better one
        let momentum_wr = self.calculate_win_rate(bars);
        let mean_rev_wr = self.calculate_mean_reversion_win_rate(bars);
        let (best_strategy, best_wr) = if momentum_wr >= mean_rev_wr {
            ("momentum", momentum_wr)
        } else {
            ("mean_reversion", mean_rev_wr)
        };
        if best_wr > 0.55 {
            signals.push(("High Win Rate", 2, true));
        } else if best_wr < 0.45 {
            signals.push(("Low Win Rate", 2, false));
        }

        // VaR — generate signals for extreme risk
        let var = self.calculate_var(&returns);
        // Adaptive VaR: rolling 30-day windows
        if returns.len() >= 30 {
            let mut rolling_vars = Vec::new();
            for i in 30..=returns.len() {
                let window = &returns[i-30..i];
                let mut sorted_window = window.to_vec();
                sorted_window.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let index = (window.len() as f64 * 0.05) as usize;
                if index < sorted_window.len() {
                    rolling_vars.push(sorted_window[index].abs() * 100.0);
                }
            }
            if !rolling_vars.is_empty() {
                let var_pct = adaptive::percentile_rank(var, &rolling_vars);
                let var_z = adaptive::z_score_of(var, &rolling_vars);
                let var_weight = adaptive::z_score_to_weight(var_z);
                if var_pct > 0.85 {
                    signals.push(("Extreme VaR Risk", var_weight, false));
                } else if var_pct > 0.70 {
                    signals.push(("Elevated VaR Risk", var_weight, false));
                } else if var_pct < 0.20 {
                    signals.push(("Low VaR Risk", var_weight, true));
                }
            }
        } else if var > 10.0 {
            signals.push(("Extreme VaR Risk", 2, false));
        } else if var > 5.0 {
            signals.push(("Elevated VaR Risk", 1, false));
        } else if var < 2.0 {
            signals.push(("Low VaR Risk", 1, true));
        }

        // Tiered Momentum with Mean-Reversion Awareness
        let recent_return = if prices.len() >= 20 {
            (prices[prices.len() - 1] - prices[prices.len() - 20]) / prices[prices.len() - 20]
        } else {
            0.0
        };
        // Adaptive momentum: compute all 20-day rolling returns
        if prices.len() >= 40 {
            let mut rolling_rets = Vec::new();
            for i in 20..prices.len() {
                let ret = (prices[i] - prices[i-20]) / prices[i-20];
                rolling_rets.push(ret);
            }
            if !rolling_rets.is_empty() {
                let mom_pct = adaptive::percentile_rank(recent_return, &rolling_rets);
                let mom_z = adaptive::z_score_of(recent_return, &rolling_rets);
                let mom_weight = adaptive::z_score_to_weight(mom_z.abs());
                if mom_pct > 0.95 {
                    signals.push(("Extreme Momentum — Reversion Risk", mom_weight, false));
                } else if mom_pct > 0.85 {
                    signals.push(("Strong Momentum — Extended", mom_weight, false));
                } else if mom_pct > 0.70 {
                    signals.push(("Positive Momentum", mom_weight, true));
                } else if mom_pct < 0.05 {
                    signals.push(("Extreme Sell-off — Bounce Risk", mom_weight, true));
                } else if mom_pct < 0.15 {
                    signals.push(("Heavy Selling — Oversold", mom_weight, true));
                } else if mom_pct < 0.30 {
                    signals.push(("Negative Momentum", mom_weight, false));
                }
            }
        } else if recent_return > 0.20 {
            signals.push(("Extreme Momentum — Reversion Risk", 2, false));
        } else if recent_return > 0.10 {
            signals.push(("Strong Momentum — Extended", 1, false));
        } else if recent_return > 0.05 {
            signals.push(("Positive Momentum", 2, true));
        } else if recent_return < -0.20 {
            signals.push(("Extreme Sell-off — Bounce Risk", 2, true));
        } else if recent_return < -0.10 {
            signals.push(("Heavy Selling — Oversold", 1, true));
        } else if recent_return < -0.05 {
            signals.push(("Negative Momentum", 2, false));
        }

        // --- CVaR / Expected Shortfall ---
        let cvar = self.calculate_cvar(&returns);
        // Adaptive CVaR: rolling 30-day windows
        if returns.len() >= 30 {
            let mut rolling_cvars = Vec::new();
            for i in 30..=returns.len() {
                let window = &returns[i-30..i];
                let mut sorted_window = window.to_vec();
                sorted_window.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let cutoff = (window.len() as f64 * 0.05).ceil() as usize;
                if cutoff > 0 && cutoff <= sorted_window.len() {
                    let tail = &sorted_window[..cutoff];
                    let cvar_val = (tail.iter().sum::<f64>() / tail.len() as f64).abs() * 100.0;
                    rolling_cvars.push(cvar_val);
                }
            }
            if !rolling_cvars.is_empty() {
                let cvar_pct = adaptive::percentile_rank(cvar, &rolling_cvars);
                let cvar_z = adaptive::z_score_of(cvar, &rolling_cvars);
                let cvar_weight = adaptive::z_score_to_weight(cvar_z);
                if cvar_pct > 0.85 {
                    signals.push(("Extreme Tail Risk (CVaR)", cvar_weight, false));
                } else if cvar_pct > 0.70 {
                    signals.push(("Elevated Tail Risk (CVaR)", cvar_weight, false));
                } else if cvar_pct < 0.20 {
                    signals.push(("Low Tail Risk (CVaR)", cvar_weight, true));
                }
            }
        } else if cvar > 15.0 {
            signals.push(("Extreme Tail Risk (CVaR)", 2, false));
        } else if cvar > 8.0 {
            signals.push(("Elevated Tail Risk (CVaR)", 1, false));
        } else if cvar < 3.0 {
            signals.push(("Low Tail Risk (CVaR)", 1, true));
        }

        // --- Hurst Exponent ---
        let hurst = self.calculate_hurst_exponent(&prices);
        let hurst_regime = if hurst > 0.65 { "trending" } else if hurst < 0.35 { "mean_reverting" } else { "random" };
        if hurst > 0.65 {
            // Trending — signal aligns with current direction
            signals.push(("Trending Market (Hurst)", 2, recent_return > 0.0));
        } else if hurst < 0.35 {
            // Mean-reverting — contrarian signal
            signals.push(("Mean-Reverting Market (Hurst)", 2, recent_return < 0.0));
        }

        // --- Autocorrelation ---
        let ac1 = self.calculate_autocorrelation(&returns, 1);
        // Statistical significance: |ac1| > 2.0 / sqrt(n) at 95% confidence
        let ac_threshold = 2.0 / (returns.len() as f64).sqrt();
        if ac1.abs() > ac_threshold {
            let ac_z = ac1 / (1.0 / (returns.len() as f64).sqrt());
            let ac_weight = adaptive::z_score_to_weight(ac_z.abs());
            if ac1 > 0.0 {
                signals.push(("Positive Serial Correlation", ac_weight, recent_return > 0.0));
            } else {
                signals.push(("Negative Serial Correlation", ac_weight, recent_return < 0.0));
            }
        }

        // --- GARCH Volatility Forecast ---
        let garch_vol = self.forecast_volatility_garch(&returns);
        // Adaptive GARCH: percentile of garch/realized ratio
        if volatility > 0.0 && returns.len() >= 30 {
            let mut rolling_ratios = Vec::new();
            for i in 30..=returns.len() {
                let window = &returns[i-30..i];
                let realized_vol = window.std_dev() * (252.0_f64).sqrt() * 100.0;
                let garch_forecast = self.forecast_volatility_garch(window);
                if realized_vol > 0.0 {
                    rolling_ratios.push(garch_forecast / realized_vol);
                }
            }
            if !rolling_ratios.is_empty() {
                let current_ratio = garch_vol / volatility;
                let ratio_pct = adaptive::percentile_rank(current_ratio, &rolling_ratios);
                let ratio_z = adaptive::z_score_of(current_ratio, &rolling_ratios);
                let ratio_weight = adaptive::z_score_to_weight(ratio_z.abs());
                if ratio_pct > 0.85 {
                    signals.push(("Volatility Expected to Increase", ratio_weight, false));
                } else if ratio_pct < 0.15 {
                    signals.push(("Volatility Expected to Decrease", ratio_weight, true));
                }
            }
        } else if volatility > 0.0 && garch_vol > volatility * 1.3 {
            signals.push(("Volatility Expected to Increase", 1, false));
        } else if volatility > 0.0 && garch_vol < volatility * 0.7 {
            signals.push(("Volatility Expected to Decrease", 1, true));
        }

        // --- Kelly Criterion ---
        let kelly = self.calculate_kelly(&returns);
        // Adaptive Kelly: percentile of 30-day rolling Kelly estimates
        if returns.len() >= 30 {
            let mut rolling_kellys = Vec::new();
            for i in 30..=returns.len() {
                let window = &returns[i-30..i];
                rolling_kellys.push(self.calculate_kelly(window));
            }
            if !rolling_kellys.is_empty() {
                let kelly_pct = adaptive::percentile_rank(kelly, &rolling_kellys);
                let kelly_z = adaptive::z_score_of(kelly, &rolling_kellys);
                let kelly_weight = adaptive::z_score_to_weight(kelly_z.abs());
                if kelly > 0.0 && kelly_pct > 0.85 {
                    signals.push(("Favorable Risk/Reward (Kelly)", kelly_weight, true));
                } else if kelly > 0.0 && kelly_pct > 0.60 {
                    signals.push(("Moderate Edge (Kelly)", kelly_weight, true));
                } else if kelly < 0.0 {
                    signals.push(("Negative Edge (Kelly)", kelly_weight.max(2), false));
                }
            }
        } else if kelly > 0.25 {
            signals.push(("Favorable Risk/Reward (Kelly)", 2, true));
        } else if kelly > 0.10 {
            signals.push(("Moderate Edge (Kelly)", 1, true));
        } else if kelly < 0.0 {
            signals.push(("Negative Edge (Kelly)", 2, false));
        }

        // --- Momentum Factor ---
        let momentum_factor = self.calculate_momentum_factor(&prices);
        if let Some(mf) = momentum_factor {
            // Adaptive momentum factor: z-score of rolling estimates
            if prices.len() >= 252 + 30 {
                let mut rolling_mfs = Vec::new();
                for i in 252..prices.len() {
                    if let Some(rolling_mf) = self.calculate_momentum_factor(&prices[..=i]) {
                        rolling_mfs.push(rolling_mf);
                    }
                }
                if !rolling_mfs.is_empty() {
                    let mf_z = adaptive::z_score_of(mf, &rolling_mfs);
                    let mf_weight = adaptive::z_score_to_weight(mf_z.abs());
                    if mf_z > 1.0 {
                        signals.push(("Positive Momentum Factor", mf_weight, true));
                    } else if mf_z < -1.0 {
                        signals.push(("Negative Momentum Factor", mf_weight, false));
                    }
                }
            } else if mf > 0.15 {
                signals.push(("Positive Momentum Factor", 2, true));
            } else if mf < -0.15 {
                signals.push(("Negative Momentum Factor", 2, false));
            }
        }

        // --- Skewness & Kurtosis ---
        let skewness = if returns.len() >= 30 {
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let std_dev = volatility / (252.0_f64).sqrt(); // daily vol
            if std_dev > 0.0 {
                let n = returns.len() as f64;
                let m3 = returns.iter().map(|r| ((r - mean) / std_dev).powi(3)).sum::<f64>() / n;
                Some(m3)
            } else { None }
        } else { None };

        let kurtosis = if returns.len() >= 30 {
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let std_dev = volatility / (252.0_f64).sqrt();
            if std_dev > 0.0 {
                let n = returns.len() as f64;
                let m4 = returns.iter().map(|r| ((r - mean) / std_dev).powi(4)).sum::<f64>() / n;
                Some(m4 - 3.0) // excess kurtosis (normal = 0)
            } else { None }
        } else { None };

        if let Some(sk) = skewness {
            if sk < -0.5 {
                signals.push(("Negative Skew (Crash Risk)", 2, false));
            } else if sk > 0.5 {
                signals.push(("Positive Skew (Lottery-Ticket)", 1, false));
            }
        }
        if let Some(kurt) = kurtosis {
            if kurt > 3.0 {
                signals.push(("Fat Tails (Extreme Moves Likely)", 2, false));
            }
        }

        // --- Correlation Regime Shift (rolling beta stability) ---
        let beta_stability = if let Some(spy) = spy_bars {
            let spy_prices: Vec<f64> = spy.iter().map(|b| b.close).collect();
            let spy_returns = self.calculate_returns(&spy_prices);
            let min_len = returns.len().min(spy_returns.len());
            if min_len >= 60 {
                // Compare beta from first half vs second half
                let half = min_len / 2;
                let beta_first = {
                    let sr = &spy_returns[..half];
                    let ar = &returns[..half];
                    let sm = sr.iter().sum::<f64>() / sr.len() as f64;
                    let cov = sr.iter().zip(ar.iter()).map(|(s, a)| (s - sm) * a).sum::<f64>() / sr.len() as f64;
                    let var = sr.iter().map(|s| (s - sm).powi(2)).sum::<f64>() / sr.len() as f64;
                    if var > 0.0 { cov / var } else { 0.0 }
                };
                let beta_second = {
                    let sr = &spy_returns[half..min_len];
                    let ar = &returns[half..min_len];
                    let sm = sr.iter().sum::<f64>() / sr.len() as f64;
                    let cov = sr.iter().zip(ar.iter()).map(|(s, a)| (s - sm) * a).sum::<f64>() / sr.len() as f64;
                    let var = sr.iter().map(|s| (s - sm).powi(2)).sum::<f64>() / sr.len() as f64;
                    if var > 0.0 { cov / var } else { 0.0 }
                };
                let shift = (beta_second - beta_first).abs();
                // Adaptive beta shift: z-score of shift magnitude
                let mut rolling_shifts = Vec::new();
                let min_window = 60;
                for i in min_window*2..min_len {
                    let half = i / 2;
                    let b1 = {
                        let sr = &spy_returns[..half];
                        let ar = &returns[..half];
                        let sm = sr.iter().sum::<f64>() / sr.len() as f64;
                        let cov = sr.iter().zip(ar.iter()).map(|(s, a)| (s - sm) * a).sum::<f64>() / sr.len() as f64;
                        let var = sr.iter().map(|s| (s - sm).powi(2)).sum::<f64>() / sr.len() as f64;
                        if var > 0.0 { cov / var } else { 0.0 }
                    };
                    let b2 = {
                        let sr = &spy_returns[half..i];
                        let ar = &returns[half..i];
                        let sm = sr.iter().sum::<f64>() / sr.len() as f64;
                        let cov = sr.iter().zip(ar.iter()).map(|(s, a)| (s - sm) * a).sum::<f64>() / sr.len() as f64;
                        let var = sr.iter().map(|s| (s - sm).powi(2)).sum::<f64>() / sr.len() as f64;
                        if var > 0.0 { cov / var } else { 0.0 }
                    };
                    rolling_shifts.push((b2 - b1).abs());
                }
                if !rolling_shifts.is_empty() {
                    let shift_z = adaptive::z_score_of(shift, &rolling_shifts);
                    let shift_weight = adaptive::z_score_to_weight(shift_z.abs());
                    if shift_z.abs() > 1.5 {
                        signals.push(("Correlation Regime Shift", shift_weight, false));
                    }
                } else if shift > 0.5 {
                    signals.push(("Correlation Regime Shift", 2, false));
                }
                Some(shift)
            } else { None }
        } else { None };

        // --- Omega Ratio ---
        let omega_ratio = self.calculate_omega_ratio(&returns, risk_free_rate);
        // Adaptive Omega: z-score vs rolling 60-day windows
        if returns.len() >= 60 {
            let mut rolling_omegas = Vec::new();
            for i in 60..=returns.len() {
                let window = &returns[i-60..i];
                rolling_omegas.push(self.calculate_omega_ratio(window, risk_free_rate));
            }
            if !rolling_omegas.is_empty() {
                let omega_z = adaptive::z_score_of(omega_ratio, &rolling_omegas);
                let omega_weight = adaptive::z_score_to_weight(omega_z);
                if omega_z > 1.0 {
                    signals.push(("Superior Omega Ratio", omega_weight, true));
                } else if omega_z < -1.0 {
                    signals.push(("Poor Omega Ratio", omega_weight, false));
                }
            }
        } else if omega_ratio > 1.5 {
            signals.push(("Superior Omega Ratio", 2, true));
        } else if omega_ratio < 0.8 {
            signals.push(("Poor Omega Ratio", 2, false));
        }

        // --- Mean Reversion Speed ---
        let mean_rev_half_life = self.calculate_mean_reversion_speed(&prices);
        if let Some(hl) = mean_rev_half_life {
            // Fast mean reversion (< 5 days) favors contrarian strategies
            // Slow/no mean reversion (> 20 days) favors trend-following
            if hl < 5.0 {
                signals.push(("Fast Mean Reversion", 2, recent_return < 0.0));
            } else if hl > 20.0 {
                signals.push(("Weak Mean Reversion (Trending)", 1, recent_return > 0.0));
            }
        }

        // --- Jump Diffusion Detection ---
        let (jump_days, jump_intensity) = self.detect_jumps(&returns, 3.0); // 3-sigma threshold
        if jump_days as f64 / returns.len() as f64 > 0.05 {
            signals.push(("Frequent Jumps (High Event Risk)", 2, false));
        }

        // --- Rachev Ratio ---
        let rachev_ratio = self.calculate_rachev_ratio(&returns);
        // Adaptive Rachev: z-score vs rolling 60-day windows
        if returns.len() >= 60 {
            let mut rolling_rachevs = Vec::new();
            for i in 60..=returns.len() {
                let window = &returns[i-60..i];
                rolling_rachevs.push(self.calculate_rachev_ratio(window));
            }
            if !rolling_rachevs.is_empty() {
                let rachev_z = adaptive::z_score_of(rachev_ratio, &rolling_rachevs);
                let rachev_weight = adaptive::z_score_to_weight(rachev_z.abs());
                if rachev_z > 1.0 {
                    signals.push(("Favorable Tail Risk Profile (Rachev)", rachev_weight, true));
                } else if rachev_z < -1.0 {
                    signals.push(("Unfavorable Tail Risk Profile (Rachev)", rachev_weight, false));
                }
            }
        } else if rachev_ratio > 1.5 {
            signals.push(("Favorable Tail Risk Profile (Rachev)", 2, true));
        } else if rachev_ratio < 0.8 {
            signals.push(("Unfavorable Tail Risk Profile (Rachev)", 2, false));
        }

        // --- Seasonality (month-of-year effect) ---
        let seasonality_signal = if bars.len() >= 252 {
            // Check if current month historically positive or negative
            let current_month = bars.last().map(|b| b.timestamp.month()).unwrap_or(0);
            if current_month > 0 {
                let mut month_returns: Vec<f64> = Vec::new();
                for window in bars.windows(2) {
                    if window[1].timestamp.month() == current_month {
                        let r = (window[1].close - window[0].close) / window[0].close;
                        month_returns.push(r);
                    }
                }
                if month_returns.len() >= 5 {
                    let avg = month_returns.iter().sum::<f64>() / month_returns.len() as f64;
                    let std_dev = {
                        let variance = month_returns.iter()
                            .map(|r| (r - avg).powi(2))
                            .sum::<f64>() / month_returns.len() as f64;
                        variance.sqrt()
                    };
                    // Statistical significance: z-score = avg / (std_dev / sqrt(n))
                    let season_z = if std_dev > 0.0 {
                        avg / (std_dev / (month_returns.len() as f64).sqrt())
                    } else {
                        0.0
                    };
                    let season_weight = adaptive::z_score_to_weight(season_z.abs());
                    if season_z.abs() > 1.5 {
                        if avg > 0.0 {
                            signals.push(("Positive Seasonal Tendency", season_weight, true));
                        } else {
                            signals.push(("Negative Seasonal Tendency", season_weight, false));
                        }
                    }
                    Some(avg * 100.0)
                } else { None }
            } else { None }
        } else { None };

        // --- Low Vol Factor (relative to SPY) ---
        let low_vol_factor = if let Some(spy) = spy_bars {
            let spy_prices: Vec<f64> = spy.iter().map(|b| b.close).collect();
            let spy_returns = self.calculate_returns(&spy_prices);
            let spy_vol = self.calculate_volatility(&spy_returns);
            if spy_vol > 0.0 {
                let ratio = volatility / spy_vol;
                // Adaptive vol factor: z-score of volatility ratio
                if returns.len() >= 30 && spy_returns.len() >= 30 {
                    let mut rolling_ratios = Vec::new();
                    let min_len = returns.len().min(spy_returns.len());
                    for i in 30..=min_len {
                        let stock_window = &returns[i-30..i];
                        let spy_window = &spy_returns[i-30..i];
                        let stock_vol = stock_window.std_dev() * (252.0_f64).sqrt() * 100.0;
                        let spy_vol_window = spy_window.std_dev() * (252.0_f64).sqrt() * 100.0;
                        if spy_vol_window > 0.0 {
                            rolling_ratios.push(stock_vol / spy_vol_window);
                        }
                    }
                    if !rolling_ratios.is_empty() {
                        let ratio_z = adaptive::z_score_of(ratio, &rolling_ratios);
                        let ratio_weight = adaptive::z_score_to_weight(ratio_z.abs());
                        if ratio_z < -1.0 {
                            signals.push(("Low Volatility Factor", ratio_weight, true));
                        } else if ratio_z > 1.0 {
                            signals.push(("High Volatility Factor", ratio_weight, false));
                        }
                    }
                } else if ratio < 0.8 {
                    signals.push(("Low Volatility Factor", 1, true));
                } else if ratio > 1.5 {
                    signals.push(("High Volatility Factor", 1, false));
                }
                Some(ratio)
            } else { None }
        } else { None };

        // Calculate overall signal
        let mut total_score = 0;
        let mut total_weight = 0;
        for (_, weight, bullish) in &signals {
            total_weight += weight;
            total_score += if *bullish { *weight } else { -weight };
        }

        let normalized_score = if total_weight > 0 {
            (total_score as f64 / total_weight as f64) * 100.0
        } else {
            0.0
        };

        let signal = SignalStrength::from_score(normalized_score as i32);

        // Dynamic confidence: data quantity (60%) + signal agreement (40%)
        let data_confidence = if bars.len() >= 90 {
            0.8
        } else if bars.len() >= 60 {
            0.6
        } else if bars.len() >= 30 {
            0.4
        } else {
            0.2
        };
        let bullish_count = signals.iter().filter(|(_, _, b)| *b).count();
        let bearish_count = signals.iter().filter(|(_, _, b)| !*b).count();
        let total_signals = bullish_count + bearish_count;
        let agreement = if total_signals > 0 {
            bullish_count.max(bearish_count) as f64 / total_signals as f64
        } else {
            0.5
        };
        let confidence = (data_confidence * 0.6 + agreement * 0.4).min(0.95);

        let reason = signals
            .iter()
            .map(|(name, _, bullish)| {
                format!("{} {}", if *bullish { "+" } else { "-" }, name)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let metrics = json!({
            "sharpe_ratio": sharpe,
            "sortino_ratio": sortino,
            "volatility": volatility,
            "max_drawdown": max_dd,
            "beta": beta,
            "win_rate": best_wr,
            "best_strategy": best_strategy,
            "momentum_win_rate": momentum_wr,
            "mean_reversion_win_rate": mean_rev_wr,
            "var_95": var,
            "cvar_95": cvar,
            "recent_return": recent_return * 100.0,
            "risk_free_rate": risk_free_rate,
            "hurst_exponent": hurst,
            "hurst_regime": hurst_regime,
            "autocorrelation_lag1": ac1,
            "garch_forecast_vol": garch_vol,
            "kelly_fraction": kelly,
            "momentum_factor": momentum_factor,
            "low_vol_factor_ratio": low_vol_factor,
            "skewness": skewness,
            "excess_kurtosis": kurtosis,
            "beta_stability_shift": beta_stability,
            "seasonality_avg_return": seasonality_signal,
            "omega_ratio": omega_ratio,
            "mean_reversion_half_life": mean_rev_half_life,
            "jump_days": jump_days,
            "jump_intensity": jump_intensity,
            "rachev_ratio": rachev_ratio,
        });

        Ok(AnalysisResult {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            signal,
            confidence,
            reason,
            metrics,
        })
    }

    fn analyze_sync(&self, symbol: &str, bars: &[Bar]) -> Result<AnalysisResult, AnalysisError> {
        // Delegate to the full implementation with no benchmark and default rate
        self.analyze_with_benchmark_and_rate(symbol, bars, None, None)
    }
}

#[async_trait]
impl QuantAnalyzer for QuantAnalysisEngine {
    async fn analyze(&self, symbol: &str, bars: &[Bar]) -> Result<AnalysisResult, AnalysisError> {
        self.analyze_sync(symbol, bars)
    }
}

impl Default for QuantAnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}
