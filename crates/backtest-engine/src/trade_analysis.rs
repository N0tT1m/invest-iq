use crate::models::BacktestTrade;
use rust_decimal::prelude::*;
use std::collections::HashMap;

/// Trade expectancy analysis - measures expected value per trade.
#[derive(Debug, Clone)]
pub struct ExpectancyAnalysis {
    /// Mathematical expectancy: avg_win * win_rate - avg_loss * loss_rate.
    pub expectancy: f64,
    /// Expectancy as a percentage of average trade size.
    pub expectancy_percent: f64,
    /// Kelly criterion optimal bet size (fraction of capital).
    pub kelly_fraction: f64,
    /// Payoff ratio: avg_win / avg_loss.
    pub payoff_ratio: f64,
    /// System quality number (SQN): sqrt(n) * expectancy / std_dev.
    pub sqn: f64,
}

pub fn compute_expectancy(trades: &[BacktestTrade]) -> Option<ExpectancyAnalysis> {
    if trades.len() < 5 {
        return None;
    }

    let winning_trades: Vec<&BacktestTrade> = trades
        .iter()
        .filter(|t| t.profit_loss > rust_decimal::Decimal::ZERO)
        .collect();

    let losing_trades: Vec<&BacktestTrade> = trades
        .iter()
        .filter(|t| t.profit_loss < rust_decimal::Decimal::ZERO)
        .collect();

    let n = trades.len() as f64;
    let win_count = winning_trades.len() as f64;
    let loss_count = losing_trades.len() as f64;

    if win_count == 0.0 && loss_count == 0.0 {
        return None;
    }

    let win_rate = win_count / n;
    let loss_rate = loss_count / n;

    let avg_win = if win_count > 0.0 {
        winning_trades
            .iter()
            .map(|t| t.profit_loss.to_f64().unwrap_or(0.0))
            .sum::<f64>()
            / win_count
    } else {
        0.0
    };

    let avg_loss = if loss_count > 0.0 {
        losing_trades
            .iter()
            .map(|t| t.profit_loss.to_f64().unwrap_or(0.0).abs())
            .sum::<f64>()
            / loss_count
    } else {
        0.0
    };

    // Expectancy = (Win% × Avg Win) - (Loss% × Avg Loss)
    let expectancy = win_rate * avg_win - loss_rate * avg_loss;

    // Average trade size (absolute value)
    let avg_trade_size = trades
        .iter()
        .map(|t| (t.entry_price * t.shares).to_f64().unwrap_or(0.0))
        .sum::<f64>()
        / n;

    let expectancy_percent = if avg_trade_size > 0.0 {
        (expectancy / avg_trade_size) * 100.0
    } else {
        0.0
    };

    // Payoff ratio
    let payoff_ratio = if avg_loss > 0.0 {
        avg_win / avg_loss
    } else {
        f64::INFINITY
    };

    // Kelly criterion: f* = (p*b - q) / b
    // where p = win_rate, q = loss_rate, b = avg_win/avg_loss
    let kelly_fraction = if avg_loss > 0.0 {
        let b = avg_win / avg_loss;
        let kelly = (win_rate * b - loss_rate) / b;
        kelly.clamp(0.0, 1.0) // Clamp to [0, 1]
    } else if win_rate > 0.0 {
        1.0 // No losses → full Kelly
    } else {
        0.0
    };

    // System Quality Number (Van Tharp): SQN = sqrt(n) * E[R] / std(R)
    let returns: Vec<f64> = trades.iter().map(|t| t.profit_loss_percent).collect();
    let mean_ret = returns.iter().sum::<f64>() / n;
    let var = returns.iter().map(|r| (r - mean_ret).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    let std_dev = var.sqrt();

    let sqn = if std_dev > 1e-10 {
        (n.sqrt() * mean_ret) / std_dev
    } else {
        0.0
    };

    Some(ExpectancyAnalysis {
        expectancy,
        expectancy_percent,
        kelly_fraction,
        payoff_ratio,
        sqn,
    })
}

/// Win/loss streak distribution analysis.
#[derive(Debug, Clone)]
pub struct StreakDistribution {
    /// Histogram of win streak lengths (length → count).
    pub win_streaks: HashMap<i32, i32>,
    /// Histogram of loss streak lengths (length → count).
    pub loss_streaks: HashMap<i32, i32>,
    /// Longest win streak.
    pub max_win_streak: i32,
    /// Longest loss streak.
    pub max_loss_streak: i32,
    /// Average win streak length.
    pub avg_win_streak: f64,
    /// Average loss streak length.
    pub avg_loss_streak: f64,
    /// Probability of win following a win (momentum).
    pub prob_win_after_win: f64,
    /// Probability of win following a loss (mean reversion).
    pub prob_win_after_loss: f64,
}

pub fn analyze_streaks(trades: &[BacktestTrade]) -> Option<StreakDistribution> {
    if trades.len() < 5 {
        return None;
    }

    let mut win_streaks: HashMap<i32, i32> = HashMap::new();
    let mut loss_streaks: HashMap<i32, i32> = HashMap::new();
    let mut current_streak = 0i32;
    let mut current_is_win = false;
    let mut max_win_streak = 0i32;
    let mut max_loss_streak = 0i32;

    // Track conditional probabilities
    let mut win_after_win = 0;
    let mut total_after_win = 0;
    let mut win_after_loss = 0;
    let mut total_after_loss = 0;

    for (i, trade) in trades.iter().enumerate() {
        let is_win = trade.profit_loss > rust_decimal::Decimal::ZERO;

        if i > 0 {
            let prev_was_win = trades[i - 1].profit_loss > rust_decimal::Decimal::ZERO;
            if prev_was_win {
                total_after_win += 1;
                if is_win {
                    win_after_win += 1;
                }
            } else {
                total_after_loss += 1;
                if is_win {
                    win_after_loss += 1;
                }
            }
        }

        if i == 0 || is_win != current_is_win {
            // Streak ended (or first trade)
            if i > 0 {
                if current_is_win {
                    *win_streaks.entry(current_streak).or_insert(0) += 1;
                    max_win_streak = max_win_streak.max(current_streak);
                } else {
                    *loss_streaks.entry(current_streak).or_insert(0) += 1;
                    max_loss_streak = max_loss_streak.max(current_streak);
                }
            }
            current_streak = 1;
            current_is_win = is_win;
        } else {
            current_streak += 1;
        }
    }

    // Record last streak
    if current_streak > 0 {
        if current_is_win {
            *win_streaks.entry(current_streak).or_insert(0) += 1;
            max_win_streak = max_win_streak.max(current_streak);
        } else {
            *loss_streaks.entry(current_streak).or_insert(0) += 1;
            max_loss_streak = max_loss_streak.max(current_streak);
        }
    }

    // Average streak lengths
    let total_win_streaks: i32 = win_streaks.values().sum();
    let weighted_win_sum: i32 = win_streaks.iter().map(|(len, count)| len * count).sum();
    let avg_win_streak = if total_win_streaks > 0 {
        weighted_win_sum as f64 / total_win_streaks as f64
    } else {
        0.0
    };

    let total_loss_streaks: i32 = loss_streaks.values().sum();
    let weighted_loss_sum: i32 = loss_streaks.iter().map(|(len, count)| len * count).sum();
    let avg_loss_streak = if total_loss_streaks > 0 {
        weighted_loss_sum as f64 / total_loss_streaks as f64
    } else {
        0.0
    };

    let prob_win_after_win = if total_after_win > 0 {
        win_after_win as f64 / total_after_win as f64
    } else {
        0.0
    };

    let prob_win_after_loss = if total_after_loss > 0 {
        win_after_loss as f64 / total_after_loss as f64
    } else {
        0.0
    };

    Some(StreakDistribution {
        win_streaks,
        loss_streaks,
        max_win_streak,
        max_loss_streak,
        avg_win_streak,
        avg_loss_streak,
        prob_win_after_win,
        prob_win_after_loss,
    })
}

/// Regime-based payoff analysis - breakdown by market regime.
#[derive(Debug, Clone)]
pub struct RegimePayoff {
    pub regime_name: String,
    pub num_trades: i32,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub payoff_ratio: f64,
    pub expectancy: f64,
}

/// Analyze payoff ratios by regime (requires regime labels per trade).
/// For this initial implementation, we'll analyze by holding period buckets as a proxy.
pub fn analyze_payoff_by_holding_period(trades: &[BacktestTrade]) -> Vec<RegimePayoff> {
    if trades.len() < 5 {
        return Vec::new();
    }

    // Buckets: 1 day, 2-5 days, 6-10 days, 11-20 days, 21+ days
    let buckets = vec![
        ("1 day", 0i64, 1i64),
        ("2-5 days", 2, 5),
        ("6-10 days", 6, 10),
        ("11-20 days", 11, 20),
        ("21+ days", 21, i64::MAX),
    ];

    let mut results = Vec::new();

    for (name, min_days, max_days) in buckets {
        let bucket_trades: Vec<&BacktestTrade> = trades
            .iter()
            .filter(|t| t.holding_period_days >= min_days && t.holding_period_days <= max_days)
            .collect();

        if bucket_trades.is_empty() {
            continue;
        }

        let winners: Vec<&BacktestTrade> = bucket_trades
            .iter()
            .copied()
            .filter(|t| t.profit_loss > rust_decimal::Decimal::ZERO)
            .collect();

        let losers: Vec<&BacktestTrade> = bucket_trades
            .iter()
            .copied()
            .filter(|t| t.profit_loss < rust_decimal::Decimal::ZERO)
            .collect();

        let num_trades = bucket_trades.len() as i32;
        let win_count = winners.len() as f64;
        let loss_count = losers.len() as f64;
        let n = num_trades as f64;

        let win_rate = if n > 0.0 {
            (win_count / n) * 100.0
        } else {
            0.0
        };

        let avg_win = if !winners.is_empty() {
            winners
                .iter()
                .map(|t| t.profit_loss.to_f64().unwrap_or(0.0))
                .sum::<f64>()
                / winners.len() as f64
        } else {
            0.0
        };

        let avg_loss = if !losers.is_empty() {
            losers
                .iter()
                .map(|t| t.profit_loss.to_f64().unwrap_or(0.0).abs())
                .sum::<f64>()
                / losers.len() as f64
        } else {
            0.0
        };

        let payoff_ratio = if avg_loss > 0.0 {
            avg_win / avg_loss
        } else {
            f64::INFINITY
        };

        let expectancy = (win_count / n) * avg_win - (loss_count / n) * avg_loss;

        results.push(RegimePayoff {
            regime_name: name.to_string(),
            num_trades,
            win_rate,
            avg_win,
            avg_loss,
            payoff_ratio,
            expectancy,
        });
    }

    results
}

/// Time-in-market analysis - measures how much time capital is deployed vs idle.
#[derive(Debug, Clone)]
pub struct TimeInMarketAnalysis {
    /// Percentage of calendar days with open positions.
    pub time_in_market_percent: f64,
    /// Average number of concurrent positions.
    pub avg_concurrent_positions: f64,
    /// Maximum number of concurrent positions.
    pub max_concurrent_positions: i32,
    /// Number of unique calendar days with at least one trade.
    pub active_trading_days: i32,
    /// Total calendar days in backtest period.
    pub total_calendar_days: i32,
}

pub fn analyze_time_in_market(trades: &[BacktestTrade]) -> Option<TimeInMarketAnalysis> {
    if trades.is_empty() {
        return None;
    }

    // Parse dates
    let parse_date = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();

    let first_entry = trades
        .iter()
        .filter_map(|t| parse_date(&t.entry_date))
        .min()?;

    let last_exit = trades
        .iter()
        .filter_map(|t| parse_date(&t.exit_date))
        .max()?;

    let total_calendar_days = (last_exit - first_entry).num_days() as i32 + 1;

    // Build a map of date → number of open positions
    let mut position_count: HashMap<chrono::NaiveDate, i32> = HashMap::new();

    for trade in trades {
        let entry = parse_date(&trade.entry_date)?;
        let exit = parse_date(&trade.exit_date)?;

        let mut current = entry;
        while current <= exit {
            *position_count.entry(current).or_insert(0) += 1;
            current += chrono::Duration::days(1);
        }
    }

    let days_in_market = position_count.len() as i32;
    let time_in_market_percent = if total_calendar_days > 0 {
        (days_in_market as f64 / total_calendar_days as f64) * 100.0
    } else {
        0.0
    };

    let total_position_days: i32 = position_count.values().sum();
    let avg_concurrent_positions = if days_in_market > 0 {
        total_position_days as f64 / days_in_market as f64
    } else {
        0.0
    };

    let max_concurrent_positions = position_count.values().copied().max().unwrap_or(0);

    // Active trading days (days with entry or exit)
    let mut active_days: std::collections::HashSet<chrono::NaiveDate> =
        std::collections::HashSet::new();
    for trade in trades {
        if let Some(entry) = parse_date(&trade.entry_date) {
            active_days.insert(entry);
        }
        if let Some(exit) = parse_date(&trade.exit_date) {
            active_days.insert(exit);
        }
    }
    let active_trading_days = active_days.len() as i32;

    Some(TimeInMarketAnalysis {
        time_in_market_percent,
        avg_concurrent_positions,
        max_concurrent_positions,
        active_trading_days,
        total_calendar_days,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn mock_trade(
        profit_loss: f64,
        holding_days: i64,
        entry_date: &str,
        exit_date: &str,
    ) -> BacktestTrade {
        BacktestTrade {
            id: None,
            backtest_id: None,
            symbol: "AAPL".to_string(),
            signal: "Buy".to_string(),
            confidence: 0.5,
            entry_date: entry_date.to_string(),
            exit_date: exit_date.to_string(),
            entry_price: Decimal::new(10000, 2),
            exit_price: Decimal::new(10000, 2),
            shares: Decimal::new(100, 0),
            profit_loss: Decimal::from_f64(profit_loss).unwrap(),
            profit_loss_percent: (profit_loss / 10000.0) * 100.0,
            holding_period_days: holding_days,
            commission_cost: Decimal::ZERO,
            slippage_cost: Decimal::ZERO,
            exit_reason: "test".to_string(),
            direction: Some("long".to_string()),
        }
    }

    #[test]
    fn test_expectancy_basic() {
        let trades = vec![
            mock_trade(100.0, 1, "2024-01-01", "2024-01-02"), // Win
            mock_trade(-50.0, 1, "2024-01-03", "2024-01-04"), // Loss
            mock_trade(150.0, 1, "2024-01-05", "2024-01-06"), // Win
            mock_trade(-75.0, 1, "2024-01-07", "2024-01-08"), // Loss
            mock_trade(200.0, 1, "2024-01-09", "2024-01-10"), // Win
        ];

        let exp = compute_expectancy(&trades).unwrap();

        // 3 wins: 100, 150, 200 → avg = 150
        // 2 losses: -50, -75 → avg = 62.5
        // Win rate = 3/5 = 0.6, Loss rate = 2/5 = 0.4
        // Expectancy = 0.6 * 150 - 0.4 * 62.5 = 90 - 25 = 65
        assert!((exp.expectancy - 65.0).abs() < 1.0);
        assert!((exp.payoff_ratio - (150.0 / 62.5)).abs() < 0.01);
    }

    #[test]
    fn test_streak_analysis() {
        let trades = vec![
            mock_trade(100.0, 1, "2024-01-01", "2024-01-02"), // W
            mock_trade(50.0, 1, "2024-01-03", "2024-01-04"),  // W (streak 2)
            mock_trade(-30.0, 1, "2024-01-05", "2024-01-06"), // L
            mock_trade(-40.0, 1, "2024-01-07", "2024-01-08"), // L
            mock_trade(-20.0, 1, "2024-01-09", "2024-01-10"), // L (streak 3)
            mock_trade(60.0, 1, "2024-01-11", "2024-01-12"),  // W
        ];

        let streaks = analyze_streaks(&trades).unwrap();

        assert_eq!(streaks.max_win_streak, 2);
        assert_eq!(streaks.max_loss_streak, 3);

        // Win streaks: one streak of length 2, one of length 1 → avg = (2 + 1) / 2 = 1.5
        assert!((streaks.avg_win_streak - 1.5).abs() < 0.01);

        // Loss streak: one of length 3 → avg = 3
        assert!((streaks.avg_loss_streak - 3.0).abs() < 0.01);

        // P(Win | Win): After trade 0 (W), got W at index 1. After trade 1 (W), got L at index 2.
        // So 1 win out of 2 times after a win → P(W|W) = 1/2 = 0.5
        // P(Win | Loss): After trade 2 (L), got L. After trade 3 (L), got L. After trade 4 (L), got W.
        // So 1 win out of 3 times after a loss → P(W|L) = 1/3 = 0.333...
        assert!((streaks.prob_win_after_win - 0.5).abs() < 0.01);
        assert!((streaks.prob_win_after_loss - (1.0 / 3.0)).abs() < 0.01);
    }

    #[test]
    fn test_time_in_market() {
        let trades = vec![
            mock_trade(100.0, 2, "2024-01-01", "2024-01-03"), // 3 days
            mock_trade(50.0, 1, "2024-01-05", "2024-01-06"),  // 2 days
                                                              // Gap: 01-04 is empty
                                                              // Total period: 2024-01-01 to 2024-01-06 = 6 days
        ];

        let analysis = analyze_time_in_market(&trades).unwrap();

        // Days with positions: 01-01, 01-02, 01-03, 01-05, 01-06 = 5 days
        // Time in market = 5/6 = 83.3%
        assert!((analysis.time_in_market_percent - (5.0 / 6.0 * 100.0)).abs() < 1.0);

        // Total position-days: 3 + 2 = 5
        // Days in market: 5
        // Avg concurrent = 5/5 = 1
        assert!((analysis.avg_concurrent_positions - 1.0).abs() < 0.01);

        assert_eq!(analysis.max_concurrent_positions, 1);
        assert_eq!(analysis.total_calendar_days, 6);
    }
}
