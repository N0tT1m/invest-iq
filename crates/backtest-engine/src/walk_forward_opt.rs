use rayon::prelude::*;

use crate::engine::{BacktestEngine, WalkForwardRunner};
use crate::models::*;

/// Run walk-forward optimization with parameter grid search.
///
/// For each in-sample fold, searches over `param_space` to find the best
/// parameters by Sharpe ratio, then tests on the out-of-sample fold.
///
/// Grid size is capped at ~100 combinations to keep runtime reasonable.
pub fn run_optimized_walk_forward(
    base_config: &BacktestConfig,
    folds: Vec<WalkForwardFoldData>,
    param_space: &ParamSearchSpace,
) -> Result<OptimizedWalkForwardResult, String> {
    if folds.is_empty() {
        return Err("No folds provided".to_string());
    }

    // Generate parameter grid
    let configs = generate_param_grid(param_space);
    if configs.is_empty() {
        // No search space → fall back to standard walk-forward
        let result = WalkForwardRunner::run(base_config, folds)?;
        return Ok(OptimizedWalkForwardResult {
            walk_forward: result,
            optimized_params: Vec::new(),
            best_params: OptimizedParams {
                fold_number: 0,
                confidence_threshold: base_config.confidence_threshold,
                position_size_percent: base_config.position_size_percent,
                stop_loss_percent: base_config.stop_loss_percent,
                take_profit_percent: base_config.take_profit_percent,
                in_sample_sharpe: None,
            },
        });
    }

    let mut optimized_params_list: Vec<OptimizedParams> = Vec::new();
    let mut all_oos_equity: Vec<EquityPoint> = Vec::new();
    let mut total_oos_trades = 0i32;
    let mut total_oos_wins = 0i32;
    let mut fold_results: Vec<WalkForwardFold> = Vec::new();
    let mut cumulative_capital = base_config.initial_capital;

    for (i, fold) in folds.into_iter().enumerate() {
        // Grid search on in-sample data — parallel over parameter configs
        let train_start = fold
            .train_data
            .values()
            .next()
            .and_then(|v| v.first())
            .map(|b| b.date.clone())
            .unwrap_or_default();
        let train_end = fold
            .train_data
            .values()
            .next()
            .and_then(|v| v.last())
            .map(|b| b.date.clone())
            .unwrap_or_default();

        let grid_results: Vec<(usize, f64)> = configs
            .par_iter()
            .enumerate()
            .map(|(ci, params)| {
                let mut is_config = base_config.clone();
                is_config.confidence_threshold = params.0;
                is_config.position_size_percent = params.1;
                is_config.stop_loss_percent = params.2;
                is_config.take_profit_percent = params.3;
                is_config.start_date = train_start.clone();
                is_config.end_date = train_end.clone();

                let mut engine = BacktestEngine::new(is_config);
                let sharpe = if let Ok(result) = engine.run(fold.train_data.clone(), fold.train_signals.clone()) {
                    result.sharpe_ratio.unwrap_or(f64::NEG_INFINITY)
                } else {
                    f64::NEG_INFINITY
                };
                (ci, sharpe)
            })
            .collect();

        let (best_config_idx, best_sharpe) = grid_results
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((0, f64::NEG_INFINITY));

        let best = &configs[best_config_idx];

        optimized_params_list.push(OptimizedParams {
            fold_number: (i + 1) as i32,
            confidence_threshold: best.0,
            position_size_percent: best.1,
            stop_loss_percent: best.2,
            take_profit_percent: best.3,
            in_sample_sharpe: if best_sharpe > f64::NEG_INFINITY {
                Some(best_sharpe)
            } else {
                None
            },
        });

        // Run in-sample with best params for reporting
        let mut is_config = base_config.clone();
        is_config.confidence_threshold = best.0;
        is_config.position_size_percent = best.1;
        is_config.stop_loss_percent = best.2;
        is_config.take_profit_percent = best.3;
        is_config.start_date = fold
            .train_data
            .values()
            .next()
            .and_then(|v| v.first())
            .map(|b| b.date.clone())
            .unwrap_or_default();
        is_config.end_date = fold
            .train_data
            .values()
            .next()
            .and_then(|v| v.last())
            .map(|b| b.date.clone())
            .unwrap_or_default();

        let mut is_engine = BacktestEngine::new(is_config);
        let is_result = is_engine.run(fold.train_data, fold.train_signals)?;

        // Run out-of-sample with best params
        let mut oos_config = base_config.clone();
        oos_config.initial_capital = cumulative_capital;
        oos_config.confidence_threshold = best.0;
        oos_config.position_size_percent = best.1;
        oos_config.stop_loss_percent = best.2;
        oos_config.take_profit_percent = best.3;
        oos_config.start_date = fold
            .test_data
            .values()
            .next()
            .and_then(|v| v.first())
            .map(|b| b.date.clone())
            .unwrap_or_default();
        oos_config.end_date = fold
            .test_data
            .values()
            .next()
            .and_then(|v| v.last())
            .map(|b| b.date.clone())
            .unwrap_or_default();

        let mut oos_engine = BacktestEngine::new(oos_config);
        let oos_result = oos_engine.run(fold.test_data, fold.test_signals)?;

        cumulative_capital = oos_result.final_capital;
        total_oos_trades += oos_result.total_trades;
        total_oos_wins += oos_result.winning_trades;
        all_oos_equity.extend(oos_result.equity_curve.clone());

        fold_results.push(WalkForwardFold {
            fold_number: (i + 1) as i32,
            train_start: is_result.start_date.clone(),
            train_end: is_result.end_date.clone(),
            test_start: oos_result.start_date.clone(),
            test_end: oos_result.end_date.clone(),
            in_sample_return: is_result.total_return_percent,
            out_of_sample_return: oos_result.total_return_percent,
            in_sample_sharpe: is_result.sharpe_ratio,
            out_of_sample_sharpe: oos_result.sharpe_ratio,
            in_sample_trades: is_result.total_trades,
            out_of_sample_trades: oos_result.total_trades,
        });
    }

    // Aggregate WF metrics
    let avg_is = fold_results
        .iter()
        .map(|f| f.in_sample_return)
        .sum::<f64>()
        / fold_results.len().max(1) as f64;
    let avg_oos = fold_results
        .iter()
        .map(|f| f.out_of_sample_return)
        .sum::<f64>()
        / fold_results.len().max(1) as f64;
    let overfitting_ratio = if avg_oos.abs() > 0.001 {
        avg_is / avg_oos
    } else {
        f64::INFINITY
    };
    let oos_win_rate = if total_oos_trades > 0 {
        total_oos_wins as f64 / total_oos_trades as f64 * 100.0
    } else {
        0.0
    };
    let oos_sharpe = {
        let sharpes: Vec<f64> = fold_results
            .iter()
            .filter_map(|f| f.out_of_sample_sharpe)
            .collect();
        if sharpes.is_empty() {
            None
        } else {
            Some(sharpes.iter().sum::<f64>() / sharpes.len() as f64)
        }
    };

    // Pick overall best params (from fold with best OOS Sharpe)
    let best_params = optimized_params_list
        .iter()
        .max_by(|a, b| {
            let a_sharpe = fold_results
                .get(a.fold_number as usize - 1)
                .and_then(|f| f.out_of_sample_sharpe)
                .unwrap_or(f64::NEG_INFINITY);
            let b_sharpe = fold_results
                .get(b.fold_number as usize - 1)
                .and_then(|f| f.out_of_sample_sharpe)
                .unwrap_or(f64::NEG_INFINITY);
            a_sharpe
                .partial_cmp(&b_sharpe)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
        .unwrap_or(OptimizedParams {
            fold_number: 0,
            confidence_threshold: base_config.confidence_threshold,
            position_size_percent: base_config.position_size_percent,
            stop_loss_percent: base_config.stop_loss_percent,
            take_profit_percent: base_config.take_profit_percent,
            in_sample_sharpe: None,
        });

    Ok(OptimizedWalkForwardResult {
        walk_forward: WalkForwardResult {
            folds: fold_results,
            avg_in_sample_return: avg_is,
            avg_out_of_sample_return: avg_oos,
            overfitting_ratio,
            out_of_sample_win_rate: oos_win_rate,
            out_of_sample_sharpe: oos_sharpe,
            combined_equity_curve: all_oos_equity,
            total_oos_trades,
        },
        optimized_params: optimized_params_list,
        best_params,
    })
}

/// Generate a parameter grid from the search space, capped at ~100 combos.
fn generate_param_grid(
    space: &ParamSearchSpace,
) -> Vec<(f64, f64, Option<f64>, Option<f64>)> {
    let conf = if space.confidence_thresholds.is_empty() {
        vec![0.5]
    } else {
        space.confidence_thresholds.clone()
    };
    let pos = if space.position_size_percents.is_empty() {
        vec![50.0]
    } else {
        space.position_size_percents.clone()
    };
    let sl = if space.stop_loss_percents.is_empty() {
        vec![None]
    } else {
        space.stop_loss_percents.iter().map(|v| Some(*v)).collect()
    };
    let tp = if space.take_profit_percents.is_empty() {
        vec![None]
    } else {
        space.take_profit_percents.iter().map(|v| Some(*v)).collect()
    };

    let max_combos = 100;
    let mut grid = Vec::new();

    for &c in &conf {
        for &p in &pos {
            for &s in &sl {
                for &t in &tp {
                    grid.push((c, p, s, t));
                    if grid.len() >= max_combos {
                        return grid;
                    }
                }
            }
        }
    }

    grid
}
