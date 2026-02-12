use analysis_core::{Bar, SignalStrength};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct BacktestEngine {
    initial_capital: f64,
    position_size: f64,           // Percentage of capital per trade (0.0 to 1.0)
    commission_rate: f64,         // Commission as percentage (e.g., 0.001 = 0.1%)
    slippage_rate: f64,           // Slippage as percentage (e.g., 0.0005 = 0.05%)
    stop_loss_pct: Option<f64>,   // Stop loss percentage (e.g., 0.05 = 5%)
    take_profit_pct: Option<f64>, // Take profit percentage (e.g., 0.10 = 10%)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TradeSignal {
    pub timestamp: DateTime<Utc>,
    pub signal: SignalStrength,
    pub confidence: f64,
    pub price: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trade {
    pub entry_date: DateTime<Utc>,
    pub exit_date: DateTime<Utc>,
    pub entry_price: f64,
    pub exit_price: f64,
    pub signal: SignalStrength,
    pub confidence: f64,
    pub shares: f64,
    pub profit_loss: f64,
    pub profit_loss_percent: f64,
    pub holding_period_days: i64,
    pub commission_cost: f64,
    pub slippage_cost: f64,
    pub exit_reason: String, // "signal", "stop_loss", "take_profit"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BacktestResult {
    pub symbol: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub initial_capital: f64,
    pub final_capital: f64,
    pub total_return: f64,
    pub total_return_percent: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub average_win: f64,
    pub average_loss: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub total_commission_paid: f64,
    pub total_slippage_cost: f64,
    pub trades: Vec<Trade>,
    pub equity_curve: Vec<EquityPoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: DateTime<Utc>,
    pub equity: f64,
}

impl BacktestEngine {
    pub fn new(initial_capital: f64, position_size: f64) -> Self {
        Self::with_params(initial_capital, position_size, 0.001, 0.0005, None, None)
    }

    pub fn with_params(
        initial_capital: f64,
        position_size: f64,
        commission_rate: f64,
        slippage_rate: f64,
        stop_loss_pct: Option<f64>,
        take_profit_pct: Option<f64>,
    ) -> Self {
        Self {
            initial_capital,
            position_size: position_size.clamp(0.0, 1.0),
            commission_rate,
            slippage_rate,
            stop_loss_pct,
            take_profit_pct,
        }
    }

    /// Run backtest with historical signals
    /// Note: This assumes signals are generated from historical data at each point in time
    pub fn backtest(
        &self,
        symbol: &str,
        signals: Vec<TradeSignal>,
        bars: &[Bar],
    ) -> Result<BacktestResult> {
        if signals.is_empty() || bars.is_empty() {
            return Err(anyhow::anyhow!("No signals or bars provided"));
        }

        let mut capital = self.initial_capital;
        let mut position: Option<Position> = None;
        let mut trades = Vec::new();
        let mut equity_curve = Vec::new();
        let mut total_commission = 0.0;
        let mut total_slippage = 0.0;

        // Create a map of signals by date for quick lookup
        let mut signal_map = std::collections::HashMap::new();
        for signal in &signals {
            let date_key = signal.timestamp.date_naive();
            signal_map.insert(date_key, signal.clone());
        }

        // Iterate through all bars to check for stop loss/take profit
        for bar in bars {
            let date_key = bar.timestamp.date_naive();
            let current_price = bar.close;

            // Check for stop loss or take profit if we have a position
            if let Some(ref pos) = position {
                let price_change_pct = (current_price - pos.entry_price) / pos.entry_price;
                let mut should_exit = false;
                let mut exit_reason = String::new();

                // Check stop loss
                if let Some(stop_loss) = self.stop_loss_pct {
                    if price_change_pct <= -stop_loss {
                        should_exit = true;
                        exit_reason = "stop_loss".to_string();
                    }
                }

                // Check take profit
                if let Some(take_profit) = self.take_profit_pct {
                    if price_change_pct >= take_profit {
                        should_exit = true;
                        exit_reason = "take_profit".to_string();
                    }
                }

                if should_exit {
                    let pos = position.take().unwrap();
                    let (exit_value, commission, slippage) =
                        self.calculate_exit_value(pos.shares, current_price);

                    capital += exit_value;
                    total_commission += commission;
                    total_slippage += slippage;

                    let entry_cost = pos.shares * pos.entry_price;
                    let profit_loss =
                        exit_value - entry_cost - pos.entry_commission - pos.entry_slippage;
                    let profit_loss_percent = (profit_loss / entry_cost) * 100.0;
                    let holding_period = (bar.timestamp - pos.entry_date).num_days();

                    trades.push(Trade {
                        entry_date: pos.entry_date,
                        exit_date: bar.timestamp,
                        entry_price: pos.entry_price,
                        exit_price: current_price,
                        signal: pos.entry_signal,
                        confidence: pos.confidence,
                        shares: pos.shares,
                        profit_loss,
                        profit_loss_percent,
                        holding_period_days: holding_period,
                        commission_cost: pos.entry_commission + commission,
                        slippage_cost: pos.entry_slippage + slippage,
                        exit_reason,
                    });
                }
            }

            // Check for trade signals
            if let Some(signal) = signal_map.get(&date_key) {
                // Record equity
                let current_equity = if let Some(ref pos) = position {
                    capital + (pos.shares * current_price)
                } else {
                    capital
                };
                equity_curve.push(EquityPoint {
                    timestamp: bar.timestamp,
                    equity: current_equity,
                });

                match &signal.signal {
                    SignalStrength::StrongBuy | SignalStrength::Buy | SignalStrength::WeakBuy => {
                        // Open position if we don't have one and confidence is high enough
                        if position.is_none() && signal.confidence >= 0.5 {
                            let investment = capital * self.position_size;
                            let (shares, commission, slippage) =
                                self.calculate_entry_shares(investment, current_price);

                            total_commission += commission;
                            total_slippage += slippage;

                            let total_cost = (shares * current_price) + commission + slippage;
                            capital -= total_cost;

                            position = Some(Position {
                                entry_date: signal.timestamp,
                                entry_price: current_price,
                                entry_signal: signal.signal,
                                confidence: signal.confidence,
                                shares,
                                entry_commission: commission,
                                entry_slippage: slippage,
                            });
                        }
                    }
                    SignalStrength::StrongSell
                    | SignalStrength::Sell
                    | SignalStrength::WeakSell => {
                        // Close position if we have one
                        if let Some(pos) = position.take() {
                            let (exit_value, commission, slippage) =
                                self.calculate_exit_value(pos.shares, current_price);

                            capital += exit_value;
                            total_commission += commission;
                            total_slippage += slippage;

                            let entry_cost = pos.shares * pos.entry_price;
                            let profit_loss =
                                exit_value - entry_cost - pos.entry_commission - pos.entry_slippage;
                            let profit_loss_percent = (profit_loss / entry_cost) * 100.0;
                            let holding_period = (signal.timestamp - pos.entry_date).num_days();

                            trades.push(Trade {
                                entry_date: pos.entry_date,
                                exit_date: signal.timestamp,
                                entry_price: pos.entry_price,
                                exit_price: current_price,
                                signal: pos.entry_signal,
                                confidence: pos.confidence,
                                shares: pos.shares,
                                profit_loss,
                                profit_loss_percent,
                                holding_period_days: holding_period,
                                commission_cost: pos.entry_commission + commission,
                                slippage_cost: pos.entry_slippage + slippage,
                                exit_reason: "signal".to_string(),
                            });
                        }
                    }
                    SignalStrength::Neutral => {
                        // Hold current position, do nothing
                    }
                }
            }
        }

        // Close any remaining position at the last price
        if let Some(pos) = position {
            let last_bar = bars.last().unwrap();
            let (exit_value, commission, slippage) =
                self.calculate_exit_value(pos.shares, last_bar.close);

            capital += exit_value;
            total_commission += commission;
            total_slippage += slippage;

            let entry_cost = pos.shares * pos.entry_price;
            let profit_loss = exit_value - entry_cost - pos.entry_commission - pos.entry_slippage;
            let profit_loss_percent = (profit_loss / entry_cost) * 100.0;
            let holding_period = (last_bar.timestamp - pos.entry_date).num_days();

            trades.push(Trade {
                entry_date: pos.entry_date,
                exit_date: last_bar.timestamp,
                entry_price: pos.entry_price,
                exit_price: last_bar.close,
                signal: pos.entry_signal,
                confidence: pos.confidence,
                shares: pos.shares,
                profit_loss,
                profit_loss_percent,
                holding_period_days: holding_period,
                commission_cost: pos.entry_commission + commission,
                slippage_cost: pos.entry_slippage + slippage,
                exit_reason: "end_of_period".to_string(),
            });
        }

        // Calculate metrics
        let final_capital = capital;
        let total_return = final_capital - self.initial_capital;
        let total_return_percent = (total_return / self.initial_capital) * 100.0;

        let winning_trades = trades.iter().filter(|t| t.profit_loss > 0.0).count();
        let losing_trades = trades.iter().filter(|t| t.profit_loss < 0.0).count();
        let win_rate = if !trades.is_empty() {
            (winning_trades as f64 / trades.len() as f64) * 100.0
        } else {
            0.0
        };

        let total_wins: f64 = trades
            .iter()
            .filter(|t| t.profit_loss > 0.0)
            .map(|t| t.profit_loss)
            .sum();

        let total_losses: f64 = trades
            .iter()
            .filter(|t| t.profit_loss < 0.0)
            .map(|t| t.profit_loss.abs())
            .sum();

        let average_win = if winning_trades > 0 {
            total_wins / winning_trades as f64
        } else {
            0.0
        };

        let average_loss = if losing_trades > 0 {
            total_losses / losing_trades as f64
        } else {
            0.0
        };

        let largest_win = trades
            .iter()
            .map(|t| t.profit_loss)
            .fold(f64::MIN, f64::max);

        let largest_loss = trades
            .iter()
            .map(|t| t.profit_loss)
            .fold(f64::MAX, f64::min);

        let profit_factor = if total_losses > 0.0 {
            total_wins / total_losses
        } else if total_wins > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        // Calculate max drawdown
        let max_drawdown = self.calculate_max_drawdown(&equity_curve);

        // Calculate Sharpe ratio
        let sharpe_ratio = self.calculate_sharpe_ratio(&trades);

        let start_date = signals.first().unwrap().timestamp;
        let end_date = signals.last().unwrap().timestamp;

        Ok(BacktestResult {
            symbol: symbol.to_string(),
            start_date,
            end_date,
            initial_capital: self.initial_capital,
            final_capital,
            total_return,
            total_return_percent,
            total_trades: trades.len(),
            winning_trades,
            losing_trades,
            win_rate,
            average_win,
            average_loss,
            largest_win,
            largest_loss,
            profit_factor,
            max_drawdown,
            sharpe_ratio,
            total_commission_paid: total_commission,
            total_slippage_cost: total_slippage,
            trades,
            equity_curve,
        })
    }

    /// Calculate shares and costs for entry
    fn calculate_entry_shares(&self, investment: f64, price: f64) -> (f64, f64, f64) {
        let commission = investment * self.commission_rate;
        let effective_price = price * (1.0 + self.slippage_rate); // Slippage increases buy price
        let shares = (investment - commission) / effective_price;
        let slippage = shares * price * self.slippage_rate;
        (shares, commission, slippage)
    }

    /// Calculate exit value and costs for selling
    fn calculate_exit_value(&self, shares: f64, price: f64) -> (f64, f64, f64) {
        let effective_price = price * (1.0 - self.slippage_rate); // Slippage decreases sell price
        let gross_value = shares * effective_price;
        let commission = gross_value * self.commission_rate;
        let slippage = shares * price * self.slippage_rate;
        let net_value = gross_value - commission;
        (net_value, commission, slippage)
    }

    fn calculate_max_drawdown(&self, equity_curve: &[EquityPoint]) -> f64 {
        let mut max_drawdown = 0.0;
        let mut peak = 0.0;

        for point in equity_curve {
            if point.equity > peak {
                peak = point.equity;
            }

            let drawdown = ((peak - point.equity) / peak) * 100.0;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        max_drawdown
    }

    fn calculate_sharpe_ratio(&self, trades: &[Trade]) -> f64 {
        if trades.is_empty() {
            return 0.0;
        }

        // Calculate returns as percentage of initial capital per trade
        let returns: Vec<f64> = trades
            .iter()
            .map(|t| (t.profit_loss / self.initial_capital) * 100.0)
            .collect();

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;

        let variance = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>()
            / returns.len() as f64;

        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return 0.0;
        }

        // Calculate average holding period in days
        let avg_holding_days: f64 = trades
            .iter()
            .map(|t| t.holding_period_days as f64)
            .sum::<f64>()
            / trades.len() as f64;

        // Annualize the Sharpe ratio
        // Sharpe = (Mean Return - Risk Free Rate) / Std Dev
        // Annualized Sharpe = Sharpe * sqrt(periods per year)
        let risk_free_rate_annual = 2.0; // 2% annual risk-free rate
        let periods_per_year = 252.0 / avg_holding_days.max(1.0); // 252 trading days per year

        let excess_return = mean_return - (risk_free_rate_annual / periods_per_year);
        let sharpe = excess_return / std_dev;

        // Annualize
        sharpe * periods_per_year.sqrt()
    }
}

#[derive(Debug, Clone)]
struct Position {
    entry_date: DateTime<Utc>,
    entry_price: f64,
    entry_signal: SignalStrength,
    confidence: f64,
    shares: f64,
    entry_commission: f64,
    entry_slippage: f64,
}

impl Default for BacktestEngine {
    fn default() -> Self {
        Self::new(10000.0, 0.95) // Default: $10k capital, 95% position size
    }
}
