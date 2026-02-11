use std::collections::HashMap;

use chrono::NaiveDate;

use crate::models::*;

/// Production-grade backtesting engine with commission, slippage, stop-loss,
/// take-profit, benchmark comparison, and comprehensive risk metrics.
pub struct BacktestEngine {
    config: BacktestConfig,
}

/// An open position being tracked during the backtest.
#[allow(dead_code)]
struct OpenPosition {
    symbol: String,
    entry_date: String,
    entry_price: f64,
    shares: f64,
    stop_loss_price: Option<f64>,
    take_profit_price: Option<f64>,
    entry_signal: String,
    entry_commission: f64,
    entry_slippage: f64,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self { config }
    }

    /// Run the backtest over historical data using pre-generated signals.
    pub fn run(
        &mut self,
        historical_data: HashMap<String, Vec<HistoricalBar>>,
        signals: Vec<Signal>,
    ) -> Result<BacktestResult, String> {
        let commission_rate = self.config.commission_rate.unwrap_or(0.001);
        let slippage_rate = self.config.slippage_rate.unwrap_or(0.0005);

        let mut cash = self.config.initial_capital;
        let mut positions: HashMap<String, OpenPosition> = HashMap::new();
        let mut trades: Vec<BacktestTrade> = Vec::new();
        let mut equity_curve: Vec<EquityPoint> = Vec::new();
        let mut total_commission = 0.0;
        let mut total_slippage = 0.0;
        let mut peak_equity = self.config.initial_capital;
        let mut max_drawdown = 0.0;
        let mut total_bars: usize = 0;
        let mut exposed_bars: usize = 0;

        // Per-symbol P&L tracking
        let mut symbol_pnl: HashMap<String, (f64, i32, i32)> = HashMap::new(); // (pnl, wins, total)

        // Build a unified timeline of all bars across all symbols
        let mut all_dates: Vec<String> = Vec::new();
        let mut bars_by_symbol_date: HashMap<String, HashMap<String, &HistoricalBar>> =
            HashMap::new();

        for (symbol, bars) in &historical_data {
            let mut date_map = HashMap::new();
            for bar in bars {
                if !all_dates.contains(&bar.date) {
                    all_dates.push(bar.date.clone());
                }
                date_map.insert(bar.date.clone(), bar);
            }
            bars_by_symbol_date.insert(symbol.clone(), date_map);
        }
        all_dates.sort();

        // Index signals by date for fast lookup
        let mut signals_by_date: HashMap<String, Vec<&Signal>> = HashMap::new();
        for signal in &signals {
            signals_by_date
                .entry(signal.date.clone())
                .or_default()
                .push(signal);
        }

        // Determine position sizing based on allocation strategy
        let weights = self.compute_weights();

        // Rebalancing tracking
        let rebalance_interval = self.config.rebalance_interval_days;
        let mut bars_since_rebalance = 0i32;

        // Walk through each date chronologically
        for date in &all_dates {
            total_bars += 1;
            bars_since_rebalance += 1;

            // 0. Rebalancing: periodically close all positions and re-enter
            if let Some(interval) = rebalance_interval {
                if interval > 0 && bars_since_rebalance >= interval && !positions.is_empty() {
                    // Close all positions for rebalancing
                    let symbols_to_close: Vec<String> = positions.keys().cloned().collect();
                    for sym in symbols_to_close {
                        if let Some(pos) = positions.remove(&sym) {
                            let close_price = bars_by_symbol_date
                                .get(&sym)
                                .and_then(|m| m.get(date))
                                .map(|b| b.close)
                                .unwrap_or(pos.entry_price);

                            let (trade, exit_comm, exit_slip) = self.close_position(
                                pos, &sym, date, close_price, commission_rate, slippage_rate,
                                "rebalance",
                            );
                            cash += close_price * trade.shares - exit_comm - exit_slip;
                            total_commission += exit_comm;
                            total_slippage += exit_slip;
                            Self::record_symbol_pnl(&mut symbol_pnl, &trade);
                            trades.push(trade);
                        }
                    }
                    bars_since_rebalance = 0;
                }
            }

            // 1. Check stop-loss / take-profit on open positions
            let mut to_close: Vec<(String, f64, &str)> = Vec::new();
            for (symbol, pos) in &positions {
                if let Some(bars_map) = bars_by_symbol_date.get(symbol) {
                    if let Some(bar) = bars_map.get(date) {
                        if let Some(sl) = pos.stop_loss_price {
                            if bar.low <= sl {
                                to_close.push((symbol.clone(), sl, "stop_loss"));
                                continue;
                            }
                        }
                        if let Some(tp) = pos.take_profit_price {
                            if bar.high >= tp {
                                to_close.push((symbol.clone(), tp, "take_profit"));
                            }
                        }
                    }
                }
            }

            for (symbol, exit_price, reason) in to_close {
                if let Some(pos) = positions.remove(&symbol) {
                    let (trade, exit_comm, exit_slip) = self.close_position(
                        pos, &symbol, date, exit_price, commission_rate, slippage_rate, reason,
                    );
                    cash += exit_price * trade.shares - exit_comm - exit_slip;
                    total_commission += exit_comm;
                    total_slippage += exit_slip;
                    Self::record_symbol_pnl(&mut symbol_pnl, &trade);
                    trades.push(trade);
                }
            }

            // 2. Process signals for this date
            if let Some(day_signals) = signals_by_date.get(date) {
                for signal in day_signals {
                    if signal.confidence < self.config.confidence_threshold {
                        continue;
                    }

                    let action = signal.signal_type.to_lowercase();

                    if action.contains("buy") {
                        if positions.contains_key(&signal.symbol) {
                            continue;
                        }

                        // Position sizing: use weight-adjusted allocation
                        let weight = weights
                            .get(&signal.symbol)
                            .copied()
                            .unwrap_or(self.config.position_size_percent / 100.0);
                        let position_value = cash * weight;
                        if position_value < signal.price {
                            continue;
                        }

                        let shares = (position_value / signal.price).floor();
                        if shares < 1.0 {
                            continue;
                        }

                        let entry_commission = signal.price * shares * commission_rate;
                        let entry_slippage = signal.price * shares * slippage_rate;
                        total_commission += entry_commission;
                        total_slippage += entry_slippage;

                        let cost = signal.price * shares + entry_commission + entry_slippage;
                        if cost > cash {
                            continue;
                        }
                        cash -= cost;

                        let stop_loss_price = self
                            .config
                            .stop_loss_percent
                            .map(|pct| signal.price * (1.0 - pct));
                        let take_profit_price = self
                            .config
                            .take_profit_percent
                            .map(|pct| signal.price * (1.0 + pct));

                        let display_signal = Self::capitalize(&signal.signal_type);

                        positions.insert(
                            signal.symbol.clone(),
                            OpenPosition {
                                symbol: signal.symbol.clone(),
                                entry_date: date.clone(),
                                entry_price: signal.price,
                                shares,
                                stop_loss_price,
                                take_profit_price,
                                entry_signal: display_signal,
                                entry_commission,
                                entry_slippage,
                            },
                        );
                    } else if action.contains("sell") {
                        if let Some(pos) = positions.remove(&signal.symbol) {
                            let (trade, exit_comm, exit_slip) = self.close_position(
                                pos,
                                &signal.symbol,
                                date,
                                signal.price,
                                commission_rate,
                                slippage_rate,
                                "signal",
                            );
                            cash += signal.price * trade.shares - exit_comm - exit_slip;
                            total_commission += exit_comm;
                            total_slippage += exit_slip;
                            Self::record_symbol_pnl(&mut symbol_pnl, &trade);
                            trades.push(trade);
                        }
                    }
                }
            }

            // Track exposure
            if !positions.is_empty() {
                exposed_bars += 1;
            }

            // 3. Mark-to-market
            let mut positions_value = 0.0;
            for (symbol, pos) in &positions {
                if let Some(bars_map) = bars_by_symbol_date.get(symbol) {
                    if let Some(bar) = bars_map.get(date) {
                        positions_value += bar.close * pos.shares;
                    } else {
                        positions_value += pos.entry_price * pos.shares;
                    }
                }
            }

            let equity = cash + positions_value;
            if equity > peak_equity {
                peak_equity = equity;
            }
            let drawdown_pct = if peak_equity > 0.0 {
                (peak_equity - equity) / peak_equity * 100.0
            } else {
                0.0
            };
            if drawdown_pct > max_drawdown {
                max_drawdown = drawdown_pct;
            }

            equity_curve.push(EquityPoint {
                timestamp: date.clone(),
                equity,
                drawdown_percent: drawdown_pct,
            });
        }

        // 4. Close remaining positions at last available price
        let last_date = all_dates.last().cloned().unwrap_or_default();
        for (symbol, pos) in positions.drain() {
            let last_price = bars_by_symbol_date
                .get(&symbol)
                .and_then(|m| m.get(&last_date))
                .map(|b| b.close)
                .unwrap_or(pos.entry_price);

            let (trade, exit_comm, exit_slip) = self.close_position(
                pos,
                &symbol,
                &last_date,
                last_price,
                commission_rate,
                slippage_rate,
                "end_of_backtest",
            );
            cash += last_price * trade.shares - exit_comm - exit_slip;
            total_commission += exit_comm;
            total_slippage += exit_slip;
            Self::record_symbol_pnl(&mut symbol_pnl, &trade);
            trades.push(trade);
        }

        // 5. Compute aggregate metrics
        let final_capital = cash;
        let total_return = final_capital - self.config.initial_capital;
        let total_return_percent =
            (final_capital / self.config.initial_capital - 1.0) * 100.0;

        let total_trades = trades.len() as i32;
        let winning_trades = trades.iter().filter(|t| t.profit_loss > 0.0).count() as i32;
        let losing_trades = total_trades - winning_trades;
        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        let gross_profits: f64 = trades.iter().filter(|t| t.profit_loss > 0.0).map(|t| t.profit_loss).sum();
        let gross_losses: f64 = trades.iter().filter(|t| t.profit_loss < 0.0).map(|t| t.profit_loss.abs()).sum();
        let profit_factor = if gross_losses > 0.0 {
            Some(gross_profits / gross_losses)
        } else if gross_profits > 0.0 {
            Some(f64::INFINITY)
        } else {
            None
        };

        let (sharpe, sortino) = Self::compute_risk_ratios(&equity_curve);

        let avg_trade_return = if !trades.is_empty() {
            Some(trades.iter().map(|t| t.profit_loss_percent).sum::<f64>() / trades.len() as f64)
        } else {
            None
        };
        let average_win = if winning_trades > 0 { Some(gross_profits / winning_trades as f64) } else { None };
        let average_loss = if losing_trades > 0 { Some(gross_losses / losing_trades as f64) } else { None };
        let largest_win = trades.iter().map(|t| t.profit_loss).filter(|p| *p > 0.0).fold(None, |a: Option<f64>, p| Some(a.map_or(p, |v| v.max(p))));
        let largest_loss = trades.iter().map(|t| t.profit_loss).filter(|p| *p < 0.0).fold(None, |a: Option<f64>, p| Some(a.map_or(p, |v| v.min(p))));
        let (max_con_wins, max_con_losses) = Self::max_consecutive_streaks(&trades);
        let avg_holding = if !trades.is_empty() {
            Some(trades.iter().map(|t| t.holding_period_days).sum::<i64>() as f64 / trades.len() as f64)
        } else {
            None
        };
        let exposure = if total_bars > 0 { Some(exposed_bars as f64 / total_bars as f64 * 100.0) } else { None };
        let calmar = if max_drawdown > 0.0 && total_bars > 0 {
            Some(total_return_percent * (252.0 / total_bars as f64) / max_drawdown)
        } else {
            None
        };
        let recovery = if max_drawdown > 0.0 { Some(total_return_percent / max_drawdown) } else { None };

        // 6. Benchmark comparison
        let benchmark = self.compute_benchmark(&all_dates, &bars_by_symbol_date, &equity_curve);

        // 7. Per-symbol results
        let per_symbol_results = if self.config.symbols.len() > 1 {
            Some(self.compute_per_symbol_results(&symbol_pnl, &weights))
        } else {
            None
        };

        Ok(BacktestResult {
            id: None,
            strategy_name: self.config.strategy_name.clone(),
            symbols: self.config.symbols.clone(),
            start_date: self.config.start_date.clone(),
            end_date: self.config.end_date.clone(),
            initial_capital: self.config.initial_capital,
            final_capital,
            total_return,
            total_return_percent,
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            profit_factor,
            sharpe_ratio: sharpe,
            sortino_ratio: sortino,
            max_drawdown: Some(max_drawdown),
            calmar_ratio: calmar,
            max_consecutive_wins: max_con_wins,
            max_consecutive_losses: max_con_losses,
            avg_holding_period_days: avg_holding,
            exposure_time_percent: exposure,
            recovery_factor: recovery,
            average_win,
            average_loss,
            largest_win,
            largest_loss,
            avg_trade_return_percent: avg_trade_return,
            total_commission_paid: total_commission,
            total_slippage_cost: total_slippage,
            equity_curve,
            trades,
            created_at: None,
            benchmark,
            per_symbol_results,
        })
    }

    // --- Helpers ---

    /// Close a position and return (trade_record, exit_commission, exit_slippage).
    fn close_position(
        &self,
        pos: OpenPosition,
        symbol: &str,
        date: &str,
        exit_price: f64,
        commission_rate: f64,
        slippage_rate: f64,
        reason: &str,
    ) -> (BacktestTrade, f64, f64) {
        let exit_commission = exit_price * pos.shares * commission_rate;
        let exit_slippage = exit_price * pos.shares * slippage_rate;
        let gross_pnl = (exit_price - pos.entry_price) * pos.shares;
        let total_costs = pos.entry_commission + pos.entry_slippage + exit_commission + exit_slippage;
        let net_pnl = gross_pnl - total_costs;
        let return_pct = (exit_price / pos.entry_price - 1.0) * 100.0;
        let holding_days = Self::date_diff(&pos.entry_date, date);

        let trade = BacktestTrade {
            id: None,
            backtest_id: None,
            symbol: symbol.to_string(),
            signal: pos.entry_signal,
            entry_date: pos.entry_date,
            exit_date: date.to_string(),
            entry_price: pos.entry_price,
            exit_price,
            shares: pos.shares,
            profit_loss: net_pnl,
            profit_loss_percent: return_pct,
            holding_period_days: holding_days,
            commission_cost: pos.entry_commission + exit_commission,
            slippage_cost: pos.entry_slippage + exit_slippage,
            exit_reason: reason.to_string(),
        };
        (trade, exit_commission, exit_slippage)
    }

    fn record_symbol_pnl(
        map: &mut HashMap<String, (f64, i32, i32)>,
        trade: &BacktestTrade,
    ) {
        let entry = map.entry(trade.symbol.clone()).or_insert((0.0, 0, 0));
        entry.0 += trade.profit_loss;
        if trade.profit_loss > 0.0 {
            entry.1 += 1;
        }
        entry.2 += 1;
    }

    /// Compute allocation weights based on config.
    fn compute_weights(&self) -> HashMap<String, f64> {
        let mut weights = HashMap::new();
        let n = self.config.symbols.len().max(1) as f64;

        match self.config.allocation_strategy.as_deref() {
            Some("custom") => {
                if let Some(ref w) = self.config.symbol_weights {
                    for sym in &self.config.symbols {
                        weights.insert(sym.clone(), *w.get(sym).unwrap_or(&(1.0 / n)));
                    }
                }
            }
            Some("equal_weight") => {
                let w = 1.0 / n;
                for sym in &self.config.symbols {
                    weights.insert(sym.clone(), w);
                }
            }
            _ => {
                // Default: use position_size_percent for single-symbol,
                // equal weight for multi-symbol
                if self.config.symbols.len() > 1 {
                    let w = 1.0 / n;
                    for sym in &self.config.symbols {
                        weights.insert(sym.clone(), w);
                    }
                } else {
                    for sym in &self.config.symbols {
                        weights.insert(sym.clone(), self.config.position_size_percent / 100.0);
                    }
                }
            }
        }
        weights
    }

    // --- Benchmark Comparison ---

    fn compute_benchmark(
        &self,
        all_dates: &[String],
        bars_by_symbol_date: &HashMap<String, HashMap<String, &HistoricalBar>>,
        strategy_equity: &[EquityPoint],
    ) -> Option<BenchmarkComparison> {
        if all_dates.is_empty() || strategy_equity.is_empty() {
            return None;
        }

        // Buy-and-hold: invest all capital in the first symbol at the first price
        let primary_symbol = self.config.symbols.first()?;
        let primary_bars = bars_by_symbol_date.get(primary_symbol)?;

        let first_price = primary_bars.get(&all_dates[0]).map(|b| b.close)?;
        let bh_shares = self.config.initial_capital / first_price;

        let mut bh_curve = Vec::new();
        let mut bh_peak = self.config.initial_capital;
        for date in all_dates {
            let price = primary_bars.get(date).map(|b| b.close).unwrap_or(first_price);
            let equity = bh_shares * price;
            if equity > bh_peak {
                bh_peak = equity;
            }
            let dd = if bh_peak > 0.0 { (bh_peak - equity) / bh_peak * 100.0 } else { 0.0 };
            bh_curve.push(EquityPoint {
                timestamp: date.clone(),
                equity,
                drawdown_percent: dd,
            });
        }

        let bh_final = bh_curve.last().map(|p| p.equity).unwrap_or(self.config.initial_capital);
        let buy_hold_return_percent = (bh_final / self.config.initial_capital - 1.0) * 100.0;
        let strategy_return = (strategy_equity.last().map(|p| p.equity).unwrap_or(self.config.initial_capital)
            / self.config.initial_capital
            - 1.0)
            * 100.0;
        let alpha = strategy_return - buy_hold_return_percent;

        // SPY benchmark (if benchmark_bars provided)
        let (spy_return_percent, spy_alpha, spy_curve, information_ratio) =
            if let Some(ref spy_bars) = self.config.benchmark_bars {
                self.compute_spy_benchmark(spy_bars, all_dates, strategy_equity)
            } else {
                (None, None, Vec::new(), None)
            };

        Some(BenchmarkComparison {
            buy_hold_return_percent,
            spy_return_percent,
            alpha,
            spy_alpha,
            information_ratio,
            buy_hold_equity_curve: bh_curve,
            spy_equity_curve: spy_curve,
        })
    }

    fn compute_spy_benchmark(
        &self,
        spy_bars: &[HistoricalBar],
        all_dates: &[String],
        strategy_equity: &[EquityPoint],
    ) -> (Option<f64>, Option<f64>, Vec<EquityPoint>, Option<f64>) {
        if spy_bars.is_empty() {
            return (None, None, Vec::new(), None);
        }

        let spy_date_map: HashMap<&str, &HistoricalBar> =
            spy_bars.iter().map(|b| (b.date.as_str(), b)).collect();

        // Find the first SPY price that aligns with our backtest dates
        let first_spy_price = all_dates
            .iter()
            .find_map(|d| spy_date_map.get(d.as_str()).map(|b| b.close));
        let first_spy_price = match first_spy_price {
            Some(p) => p,
            None => return (None, None, Vec::new(), None),
        };

        let spy_shares = self.config.initial_capital / first_spy_price;
        let mut spy_curve = Vec::new();
        let mut spy_peak = self.config.initial_capital;

        for date in all_dates {
            let price = spy_date_map.get(date.as_str()).map(|b| b.close).unwrap_or(first_spy_price);
            let equity = spy_shares * price;
            if equity > spy_peak {
                spy_peak = equity;
            }
            let dd = if spy_peak > 0.0 { (spy_peak - equity) / spy_peak * 100.0 } else { 0.0 };
            spy_curve.push(EquityPoint {
                timestamp: date.clone(),
                equity,
                drawdown_percent: dd,
            });
        }

        let spy_final = spy_curve.last().map(|p| p.equity).unwrap_or(self.config.initial_capital);
        let spy_return = (spy_final / self.config.initial_capital - 1.0) * 100.0;
        let strategy_return = (strategy_equity.last().map(|p| p.equity).unwrap_or(self.config.initial_capital)
            / self.config.initial_capital
            - 1.0)
            * 100.0;
        let spy_alpha = strategy_return - spy_return;

        // Information ratio: alpha / tracking error (std dev of daily return differences)
        let information_ratio = if strategy_equity.len() > 1 && spy_curve.len() == strategy_equity.len() {
            let diffs: Vec<f64> = strategy_equity
                .windows(2)
                .zip(spy_curve.windows(2))
                .map(|(s, b)| {
                    let s_ret = (s[1].equity / s[0].equity) - 1.0;
                    let b_ret = (b[1].equity / b[0].equity) - 1.0;
                    s_ret - b_ret
                })
                .collect();
            if !diffs.is_empty() {
                let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
                let var = diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / diffs.len() as f64;
                let tracking_error = var.sqrt() * 252.0_f64.sqrt();
                if tracking_error > 0.0 {
                    Some((spy_alpha / 100.0 * 252.0 / all_dates.len() as f64) / (tracking_error))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        (Some(spy_return), Some(spy_alpha), spy_curve, information_ratio)
    }

    // --- Per-Symbol Results ---

    fn compute_per_symbol_results(
        &self,
        symbol_pnl: &HashMap<String, (f64, i32, i32)>,
        weights: &HashMap<String, f64>,
    ) -> Vec<SymbolResult> {
        self.config
            .symbols
            .iter()
            .map(|sym| {
                let (pnl, wins, total) = symbol_pnl.get(sym).copied().unwrap_or((0.0, 0, 0));
                let w = weights.get(sym).copied().unwrap_or(0.0);
                let invested = self.config.initial_capital * w;
                SymbolResult {
                    symbol: sym.clone(),
                    total_trades: total,
                    winning_trades: wins,
                    win_rate: if total > 0 { (wins as f64 / total as f64) * 100.0 } else { 0.0 },
                    total_return: pnl,
                    total_return_percent: if invested > 0.0 { (pnl / invested) * 100.0 } else { 0.0 },
                    weight: w,
                }
            })
            .collect()
    }

    // --- Risk Ratios ---

    fn compute_risk_ratios(equity_curve: &[EquityPoint]) -> (Option<f64>, Option<f64>) {
        if equity_curve.len() < 2 {
            return (None, None);
        }
        let returns: Vec<f64> = equity_curve
            .windows(2)
            .map(|w| (w[1].equity / w[0].equity) - 1.0)
            .collect();
        if returns.is_empty() {
            return (None, None);
        }
        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();
        let rf_daily = 0.02 / 252.0;

        let sharpe = if std_dev > 0.0 {
            Some(((mean - rf_daily) / std_dev) * 252.0_f64.sqrt())
        } else {
            None
        };

        let downside: Vec<f64> = returns
            .iter()
            .filter(|r| **r < rf_daily)
            .map(|r| (r - rf_daily).powi(2))
            .collect();
        let downside_dev = if !downside.is_empty() {
            (downside.iter().sum::<f64>() / downside.len() as f64).sqrt()
        } else {
            0.0
        };
        let sortino = if downside_dev > 0.0 {
            Some(((mean - rf_daily) / downside_dev) * 252.0_f64.sqrt())
        } else {
            sharpe
        };

        (sharpe, sortino)
    }

    fn max_consecutive_streaks(trades: &[BacktestTrade]) -> (i32, i32) {
        let mut max_w = 0;
        let mut max_l = 0;
        let mut w = 0;
        let mut l = 0;
        for t in trades {
            if t.profit_loss > 0.0 {
                w += 1;
                l = 0;
                max_w = max_w.max(w);
            } else if t.profit_loss < 0.0 {
                l += 1;
                w = 0;
                max_l = max_l.max(l);
            } else {
                w = 0;
                l = 0;
            }
        }
        (max_w, max_l)
    }

    fn date_diff(from: &str, to: &str) -> i64 {
        let parse = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
        match (parse(from), parse(to)) {
            (Some(a), Some(b)) => (b - a).num_days(),
            _ => 0,
        }
    }

    fn capitalize(s: &str) -> String {
        let mut c = s.chars();
        match c.next() {
            None => String::new(),
            Some(ch) => ch.to_uppercase().collect::<String>() + c.as_str(),
        }
    }
}

// --- Walk-Forward Validation ---

/// Runs walk-forward validation across pre-prepared folds.
pub struct WalkForwardRunner;

impl WalkForwardRunner {
    /// Run walk-forward validation. The caller prepares fold data (splitting bars
    /// and generating PIT signals for each train/test window).
    pub fn run(
        config: &BacktestConfig,
        folds: Vec<WalkForwardFoldData>,
    ) -> Result<WalkForwardResult, String> {
        if folds.is_empty() {
            return Err("No folds provided for walk-forward validation".to_string());
        }

        let mut fold_results = Vec::new();
        let mut all_oos_equity: Vec<EquityPoint> = Vec::new();
        let mut total_oos_trades = 0i32;
        let mut total_oos_wins = 0i32;
        let mut cumulative_capital = config.initial_capital;

        for (i, fold) in folds.into_iter().enumerate() {
            // In-sample backtest
            let mut is_config = config.clone();
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

            let mut is_engine = BacktestEngine::new(is_config.clone());
            let is_result = is_engine.run(fold.train_data, fold.train_signals)?;

            // Out-of-sample backtest (use cumulative capital from previous folds)
            let mut oos_config = config.clone();
            oos_config.initial_capital = cumulative_capital;
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

        let avg_is = fold_results.iter().map(|f| f.in_sample_return).sum::<f64>()
            / fold_results.len() as f64;
        let avg_oos = fold_results.iter().map(|f| f.out_of_sample_return).sum::<f64>()
            / fold_results.len() as f64;
        let overfitting_ratio = if avg_oos.abs() > 0.001 {
            avg_is / avg_oos
        } else {
            f64::INFINITY
        };
        let oos_win_rate = if total_oos_trades > 0 {
            (total_oos_wins as f64 / total_oos_trades as f64) * 100.0
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

        Ok(WalkForwardResult {
            folds: fold_results,
            avg_in_sample_return: avg_is,
            avg_out_of_sample_return: avg_oos,
            overfitting_ratio,
            out_of_sample_win_rate: oos_win_rate,
            out_of_sample_sharpe: oos_sharpe,
            combined_equity_curve: all_oos_equity,
            total_oos_trades,
        })
    }
}
