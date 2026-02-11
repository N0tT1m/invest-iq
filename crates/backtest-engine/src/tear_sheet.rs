use chrono::Datelike;
use serde_json::json;

use crate::models::*;

/// Generate a structured tear sheet as a JSON value combining all analytics.
pub fn generate_tear_sheet(result: &BacktestResult) -> serde_json::Value {
    let mut sheet = json!({
        "summary": {
            "strategy_name": result.strategy_name,
            "symbols": result.symbols,
            "period": format!("{} to {}", result.start_date, result.end_date),
            "initial_capital": result.initial_capital.to_string(),
            "final_capital": result.final_capital.to_string(),
            "total_return_percent": result.total_return_percent,
            "annualized_return_percent": result.annualized_return_percent,
            "total_trades": result.total_trades,
            "win_rate": result.win_rate,
            "profit_factor": result.profit_factor,
            "short_trades": result.short_trades.unwrap_or(0),
        },
        "risk_metrics": {
            "sharpe_ratio": result.sharpe_ratio,
            "sortino_ratio": result.sortino_ratio,
            "calmar_ratio": result.calmar_ratio,
            "max_drawdown_percent": result.max_drawdown,
            "recovery_factor": result.recovery_factor,
            "exposure_time_percent": result.exposure_time_percent,
            "margin_used_peak": result.margin_used_peak,
        },
        "trade_analysis": {
            "average_win": result.average_win.map(|v| v.to_string()),
            "average_loss": result.average_loss.map(|v| v.to_string()),
            "largest_win": result.largest_win.map(|v| v.to_string()),
            "largest_loss": result.largest_loss.map(|v| v.to_string()),
            "avg_holding_period_days": result.avg_holding_period_days,
            "max_consecutive_wins": result.max_consecutive_wins,
            "max_consecutive_losses": result.max_consecutive_losses,
            "avg_trade_return_percent": result.avg_trade_return_percent,
        },
        "costs": {
            "total_commission": result.total_commission_paid.to_string(),
            "total_slippage": result.total_slippage_cost.to_string(),
        },
    });

    // Add extended metrics if available
    if let Some(ref ext) = result.extended_metrics {
        sheet["extended_metrics"] = json!({
            "treynor_ratio": ext.treynor_ratio,
            "jensens_alpha": ext.jensens_alpha,
            "omega_ratio": ext.omega_ratio,
            "tail_ratio": ext.tail_ratio,
            "skewness": ext.skewness,
            "kurtosis": ext.kurtosis,
            "max_drawdown_duration_days": ext.max_drawdown_duration_days,
            "num_drawdown_events": ext.top_drawdown_events.len(),
            "num_monthly_returns": ext.monthly_returns.len(),
        });

        // Trade analysis by day of week
        let mut by_day: Vec<serde_json::Value> = Vec::new();
        let days = ["Mon", "Tue", "Wed", "Thu", "Fri"];
        for (i, day) in days.iter().enumerate() {
            let day_trades: Vec<&BacktestTrade> = result
                .trades
                .iter()
                .filter(|t| {
                    chrono::NaiveDate::parse_from_str(&t.entry_date, "%Y-%m-%d")
                        .map(|d| d.weekday().num_days_from_monday() as usize == i)
                        .unwrap_or(false)
                })
                .collect();

            if !day_trades.is_empty() {
                let wins = day_trades
                    .iter()
                    .filter(|t| {
                        rust_decimal::prelude::ToPrimitive::to_f64(&t.profit_loss).unwrap_or(0.0)
                            > 0.0
                    })
                    .count();
                let avg_ret = day_trades
                    .iter()
                    .map(|t| t.profit_loss_percent)
                    .sum::<f64>()
                    / day_trades.len() as f64;

                by_day.push(json!({
                    "day": day,
                    "trades": day_trades.len(),
                    "win_rate": wins as f64 / day_trades.len() as f64 * 100.0,
                    "avg_return_percent": avg_ret,
                }));
            }
        }
        sheet["trade_analysis_by_day"] = json!(by_day);

        // Holding period distribution
        let holding_periods: Vec<i64> = result.trades.iter().map(|t| t.holding_period_days).collect();
        if !holding_periods.is_empty() {
            let min_hp = *holding_periods.iter().min().unwrap_or(&0);
            let max_hp = *holding_periods.iter().max().unwrap_or(&0);
            let median_hp = {
                let mut sorted = holding_periods.clone();
                sorted.sort();
                sorted[sorted.len() / 2]
            };
            sheet["holding_period_distribution"] = json!({
                "min_days": min_hp,
                "max_days": max_hp,
                "median_days": median_hp,
            });
        }
    }

    // Add factor attribution if available
    if let Some(ref fa) = result.factor_attribution {
        sheet["factor_attribution"] = json!({
            "beta": fa.beta,
            "alpha_annualized": fa.alpha_annualized,
            "r_squared": fa.r_squared,
            "tracking_error": fa.tracking_error,
            "residual_risk": fa.residual_risk,
        });
    }

    // Add confidence intervals if available
    if let Some(ref ci) = result.confidence_intervals {
        sheet["confidence_intervals"] = json!({
            "sharpe_95ci": [ci.sharpe_ci_lower, ci.sharpe_ci_upper],
            "win_rate_95ci": [ci.win_rate_ci_lower, ci.win_rate_ci_upper],
            "profit_factor_95ci": [ci.profit_factor_ci_lower, ci.profit_factor_ci_upper],
            "bootstrap_samples": ci.bootstrap_samples,
        });
    }

    // Add data quality if available
    if let Some(ref dq) = result.data_quality_report {
        sheet["data_quality"] = json!({
            "total_bars": dq.total_bars,
            "missing_dates": dq.missing_dates,
            "zero_volume_bars": dq.zero_volume_bars,
            "price_spikes": dq.price_spike_count,
            "corporate_events": dq.corporate_events.len(),
            "warnings": dq.warnings.len(),
        });
    }

    sheet
}
