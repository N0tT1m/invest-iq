use analysis_core::{AnalysisError, AnalysisResult, Bar, QuantAnalyzer, SignalStrength};
use async_trait::async_trait;
use chrono::Utc;
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
        if sharpe > 1.0 {
            signals.push(("Good Risk-Adjusted Return", 3, true));
        } else if sharpe < 0.0 {
            signals.push(("Poor Risk-Adjusted Return", 2, false));
        }

        // Sortino Ratio
        let sortino = self.calculate_sortino_ratio(&returns, risk_free_rate);
        if sortino > 1.5 {
            signals.push(("Strong Downside Protection", 2, true));
        } else if sortino < 0.0 {
            signals.push(("Poor Downside Profile", 1, false));
        }

        // Volatility
        let volatility = self.calculate_volatility(&returns);
        if volatility < 20.0 {
            signals.push(("Low Volatility", 1, true));
        } else if volatility > 40.0 {
            signals.push(("High Volatility", 2, false));
        }

        // Max Drawdown
        let max_dd = self.calculate_max_drawdown(&prices);
        if max_dd < 10.0 {
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
        if var > 10.0 {
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
        if recent_return > 0.20 {
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
            "recent_return": recent_return * 100.0,
            "risk_free_rate": risk_free_rate,
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
