use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::models::{BacktestTrade, MonteCarloResult};

/// Run Monte Carlo simulation by reshuffling the trade sequence.
///
/// This generates `num_simulations` equity curves by randomly reordering the
/// trades from a completed backtest.  Each simulation starts with
/// `initial_capital` and replays the trades in a random order to produce a
/// distribution of outcomes (final return, max drawdown, Sharpe).
pub fn run_monte_carlo(
    trades: &[BacktestTrade],
    initial_capital: f64,
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

    let mut rng = thread_rng();
    let mut returns: Vec<f64> = Vec::with_capacity(num_simulations as usize);
    let mut drawdowns: Vec<f64> = Vec::with_capacity(num_simulations as usize);
    let mut sharpes: Vec<f64> = Vec::with_capacity(num_simulations as usize);
    let mut profitable_count = 0u32;
    let mut ruin_count = 0u32; // losing > 50%

    let trade_pcts: Vec<f64> = trades.iter().map(|t| t.profit_loss_percent / 100.0).collect();

    for _ in 0..num_simulations {
        let mut shuffled = trade_pcts.clone();
        shuffled.shuffle(&mut rng);

        let mut equity = initial_capital;
        let mut peak = initial_capital;
        let mut max_dd = 0.0_f64;
        let mut daily_returns: Vec<f64> = Vec::with_capacity(shuffled.len());

        for pct in &shuffled {
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

        let total_return_pct = (equity / initial_capital - 1.0) * 100.0;
        returns.push(total_return_pct);
        drawdowns.push(max_dd);

        if total_return_pct > 0.0 {
            profitable_count += 1;
        }
        if total_return_pct < -50.0 {
            ruin_count += 1;
        }

        // Sharpe for this simulation
        if daily_returns.len() > 1 {
            let n = daily_returns.len() as f64;
            let mean = daily_returns.iter().sum::<f64>() / n;
            let var = daily_returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
            let std = var.sqrt();
            if std > 0.0 {
                sharpes.push((mean / std) * (n.min(252.0)).sqrt());
            }
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
