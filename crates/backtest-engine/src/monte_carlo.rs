use rand::seq::SliceRandom;
use rand::thread_rng;
use rand::Rng;
use rayon::prelude::*;
use rust_decimal::prelude::*;

use crate::models::{BacktestTrade, MonteCarloConfig, MonteCarloResult};

/// Run Monte Carlo simulation by reshuffling the trade sequence.
///
/// This generates `num_simulations` equity curves by randomly reordering the
/// trades from a completed backtest.  Each simulation starts with
/// `initial_capital` and replays the trades in a random order to produce a
/// distribution of outcomes (final return, max drawdown, Sharpe).
pub fn run_monte_carlo(
    trades: &[BacktestTrade],
    initial_capital: Decimal,
    num_simulations: i32,
) -> MonteCarloResult {
    if trades.is_empty() || num_simulations <= 0 {
        return MonteCarloResult {
            simulations: 0,
            median_return: 0.0,
            mean_return: 0.0,
            std_dev_return: 0.0,
            percentile_5: 0.0,
            percentile_25: 0.0,
            percentile_75: 0.0,
            percentile_95: 0.0,
            probability_of_profit: 0.0,
            probability_of_ruin: 0.0,
            expected_max_drawdown_95: 0.0,
            median_max_drawdown: 0.0,
            median_sharpe: 0.0,
            return_distribution: Vec::new(),
            drawdown_distribution: Vec::new(),
        };
    }

    let trade_pcts: Vec<f64> = trades.iter().map(|t| t.profit_loss_percent / 100.0).collect();
    let initial_capital_f64 = initial_capital.to_f64().unwrap_or(100000.0);

    let n_trades = trade_pcts.len();

    // Run simulations in parallel using rayon
    let sim_results: Vec<(f64, f64, Option<f64>)> = (0..num_simulations)
        .into_par_iter()
        .map(|_| {
            let mut rng = thread_rng();
            // Bootstrap: sample N trades WITH REPLACEMENT
            let sampled: Vec<f64> = (0..n_trades)
                .map(|_| trade_pcts[rng.gen_range(0..n_trades)])
                .collect();

            let mut equity = initial_capital_f64;
            let mut peak = initial_capital_f64;
            let mut max_dd = 0.0_f64;
            let mut daily_returns: Vec<f64> = Vec::with_capacity(sampled.len());

            for pct in &sampled {
                let prev = equity;
                equity *= 1.0 + pct;
                if equity > peak {
                    peak = equity;
                }
                let dd = if peak > 0.0 {
                    (peak - equity) / peak * 100.0
                } else {
                    0.0
                };
                if dd > max_dd {
                    max_dd = dd;
                }
                daily_returns.push(equity / prev - 1.0);
            }

            let total_return_pct = (equity / initial_capital_f64 - 1.0) * 100.0;

            // Sharpe for this simulation
            let sharpe = if daily_returns.len() > 1 {
                let n = daily_returns.len() as f64;
                let mean = daily_returns.iter().sum::<f64>() / n;
                let var = daily_returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
                let std = var.sqrt();
                if std > 0.0 {
                    Some((mean / std) * (n.min(252.0)).sqrt())
                } else {
                    None
                }
            } else {
                None
            };

            (total_return_pct, max_dd, sharpe)
        })
        .collect();

    // Aggregate results from parallel simulations
    let mut returns: Vec<f64> = Vec::with_capacity(num_simulations as usize);
    let mut drawdowns: Vec<f64> = Vec::with_capacity(num_simulations as usize);
    let mut sharpes: Vec<f64> = Vec::with_capacity(num_simulations as usize);
    let mut profitable_count = 0u32;
    let mut ruin_count = 0u32;

    for (total_return_pct, max_dd, sharpe) in sim_results {
        returns.push(total_return_pct);
        drawdowns.push(max_dd);
        if let Some(s) = sharpe {
            sharpes.push(s);
        }
        if total_return_pct > 0.0 {
            profitable_count += 1;
        }
        if total_return_pct < -20.0 {
            ruin_count += 1;
        }
    }

    returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    drawdowns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sharpes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = returns.len();
    let percentile = |sorted: &[f64], p: f64| -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    };

    let mean_return = returns.iter().sum::<f64>() / n as f64;
    let var = returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>() / n as f64;

    // Sample up to 200 points for histograms
    let sample_dist = |sorted: &[f64], max_pts: usize| -> Vec<f64> {
        if sorted.len() <= max_pts {
            return sorted.to_vec();
        }
        let step = sorted.len() as f64 / max_pts as f64;
        (0..max_pts)
            .map(|i| sorted[(i as f64 * step) as usize])
            .collect()
    };

    MonteCarloResult {
        simulations: num_simulations,
        median_return: percentile(&returns, 50.0),
        mean_return,
        std_dev_return: var.sqrt(),
        percentile_5: percentile(&returns, 5.0),
        percentile_25: percentile(&returns, 25.0),
        percentile_75: percentile(&returns, 75.0),
        percentile_95: percentile(&returns, 95.0),
        probability_of_profit: if n > 0 { profitable_count as f64 / n as f64 * 100.0 } else { 0.0 },
        probability_of_ruin: if n > 0 { ruin_count as f64 / n as f64 * 100.0 } else { 0.0 },
        expected_max_drawdown_95: percentile(&drawdowns, 95.0),
        median_max_drawdown: percentile(&drawdowns, 50.0),
        median_sharpe: if !sharpes.is_empty() { percentile(&sharpes, 50.0) } else { 0.0 },
        return_distribution: sample_dist(&returns, 200),
        drawdown_distribution: sample_dist(&drawdowns, 200),
    }
}

/// Enhanced Monte Carlo with block bootstrap and parameter uncertainty.
///
/// - **Block bootstrap**: Instead of shuffling individual trades, resamples
///   contiguous blocks of `block_size` trades, preserving autocorrelation
///   (winning/losing streaks) present in the original trade sequence.
///
/// - **Parameter uncertainty**: On each simulation, randomly perturbs
///   commission and slippage costs by +/-20%, reflecting uncertainty in
///   real-world execution costs.
pub fn run_monte_carlo_enhanced(
    trades: &[BacktestTrade],
    initial_capital: Decimal,
    config: &MonteCarloConfig,
) -> MonteCarloResult {
    if trades.is_empty() || config.num_simulations <= 0 {
        return run_monte_carlo(trades, initial_capital, 0);
    }

    let block_size = config.block_size.max(1);
    let num_sims = config.num_simulations;

    // If block_size == 1 and no parameter uncertainty, delegate to standard
    if block_size <= 1 && !config.parameter_uncertainty {
        return run_monte_carlo(trades, initial_capital, num_sims);
    }

    let initial_f64 = initial_capital.to_f64().unwrap_or(100000.0);
    let n_trades = trades.len();

    // Pre-compute base trade returns and costs
    let base_pcts: Vec<f64> = trades.iter().map(|t| t.profit_loss_percent / 100.0).collect();
    let base_costs: Vec<f64> = trades
        .iter()
        .map(|t| {
            let comm = t.commission_cost.to_f64().unwrap_or(0.0);
            let slip = t.slippage_cost.to_f64().unwrap_or(0.0);
            let entry_val = (t.entry_price * t.shares).to_f64().unwrap_or(1.0);
            if entry_val > 0.0 {
                (comm + slip) / entry_val
            } else {
                0.0
            }
        })
        .collect();

    // Build blocks: split trades into blocks of block_size
    let n_blocks = (n_trades + block_size - 1) / block_size;
    let block_indices: Vec<usize> = (0..n_blocks).collect();
    let parameter_uncertainty = config.parameter_uncertainty;

    // Run simulations in parallel using rayon
    let sim_results: Vec<(f64, f64, Option<f64>)> = (0..num_sims)
        .into_par_iter()
        .map(|_| {
            let mut rng = thread_rng();

            // Block bootstrap: sample blocks with replacement
            let mut sim_sequence: Vec<usize> = Vec::with_capacity(n_trades);
            while sim_sequence.len() < n_trades {
                let block_idx = *block_indices.choose(&mut rng).unwrap();
                let start = block_idx * block_size;
                let end = (start + block_size).min(n_trades);
                for i in start..end {
                    if sim_sequence.len() < n_trades {
                        sim_sequence.push(i);
                    }
                }
            }

            // Parameter uncertainty: perturb cost ratio
            let cost_multiplier = if parameter_uncertainty {
                1.0 + rng.gen_range(-0.2..0.2)
            } else {
                1.0
            };

            let mut equity = initial_f64;
            let mut peak = initial_f64;
            let mut max_dd = 0.0_f64;
            let mut daily_returns: Vec<f64> = Vec::with_capacity(n_trades);

            for &idx in &sim_sequence {
                let base_return = base_pcts[idx];
                let base_cost = base_costs[idx];

                let adjusted_return = base_return + (1.0 - cost_multiplier) * base_cost;

                let prev = equity;
                equity *= 1.0 + adjusted_return;
                if equity > peak {
                    peak = equity;
                }
                let dd = if peak > 0.0 {
                    (peak - equity) / peak * 100.0
                } else {
                    0.0
                };
                if dd > max_dd {
                    max_dd = dd;
                }
                daily_returns.push(equity / prev - 1.0);
            }

            let total_return_pct = (equity / initial_f64 - 1.0) * 100.0;

            let sharpe = if daily_returns.len() > 1 {
                let n = daily_returns.len() as f64;
                let mean = daily_returns.iter().sum::<f64>() / n;
                let var = daily_returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
                let std = var.sqrt();
                if std > 0.0 {
                    Some((mean / std) * (n.min(252.0)).sqrt())
                } else {
                    None
                }
            } else {
                None
            };

            (total_return_pct, max_dd, sharpe)
        })
        .collect();

    // Aggregate results from parallel simulations
    let mut returns: Vec<f64> = Vec::with_capacity(num_sims as usize);
    let mut drawdowns: Vec<f64> = Vec::with_capacity(num_sims as usize);
    let mut sharpes: Vec<f64> = Vec::with_capacity(num_sims as usize);
    let mut profitable_count = 0u32;
    let mut ruin_count = 0u32;

    for (total_return_pct, max_dd, sharpe) in sim_results {
        returns.push(total_return_pct);
        drawdowns.push(max_dd);
        if let Some(s) = sharpe {
            sharpes.push(s);
        }
        if total_return_pct > 0.0 {
            profitable_count += 1;
        }
        if total_return_pct < -20.0 {
            ruin_count += 1;
        }
    }

    returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    drawdowns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sharpes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = returns.len();
    let percentile = |sorted: &[f64], p: f64| -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    };

    let mean_return = returns.iter().sum::<f64>() / n as f64;
    let var = returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>() / n as f64;

    let sample_dist = |sorted: &[f64], max_pts: usize| -> Vec<f64> {
        if sorted.len() <= max_pts {
            return sorted.to_vec();
        }
        let step = sorted.len() as f64 / max_pts as f64;
        (0..max_pts)
            .map(|i| sorted[(i as f64 * step) as usize])
            .collect()
    };

    MonteCarloResult {
        simulations: num_sims,
        median_return: percentile(&returns, 50.0),
        mean_return,
        std_dev_return: var.sqrt(),
        percentile_5: percentile(&returns, 5.0),
        percentile_25: percentile(&returns, 25.0),
        percentile_75: percentile(&returns, 75.0),
        percentile_95: percentile(&returns, 95.0),
        probability_of_profit: if n > 0 {
            profitable_count as f64 / n as f64 * 100.0
        } else {
            0.0
        },
        probability_of_ruin: if n > 0 {
            ruin_count as f64 / n as f64 * 100.0
        } else {
            0.0
        },
        expected_max_drawdown_95: percentile(&drawdowns, 95.0),
        median_max_drawdown: percentile(&drawdowns, 50.0),
        median_sharpe: if !sharpes.is_empty() {
            percentile(&sharpes, 50.0)
        } else {
            0.0
        },
        return_distribution: sample_dist(&returns, 200),
        drawdown_distribution: sample_dist(&drawdowns, 200),
    }
}
