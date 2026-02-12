use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use rust_decimal::prelude::*;

use crate::advanced_risk;
use crate::circuit_breaker::CircuitBreaker;
use crate::commission::compute_tiered_commission;
use crate::data_quality::check_data_quality;
use crate::extended_metrics::{compute_extended_metrics, equity_returns};
use crate::factor_attribution::compute_factor_attribution;
use crate::margin::MarginTracker;
use crate::models::*;
use crate::order_types::LimitOrderManager;
use crate::overfitting;
use crate::regime_risk::{detect_regime, regime_size_multiplier, Regime};
use crate::short_selling;
use crate::statistical::bootstrap_confidence_intervals;
use crate::tear_sheet::generate_tear_sheet;
use crate::trade_analysis;
use crate::trailing_stop::TrailingStopManager;

/// Production-grade backtesting engine with next-bar execution, directional
/// slippage, volume participation limits, portfolio-equity sizing, stop-loss,
/// take-profit, benchmark comparison, and comprehensive risk metrics.
pub struct BacktestEngine {
    config: BacktestConfig,
}

/// An open position being tracked during the backtest.
#[allow(dead_code)]
struct OpenPosition {
    symbol: String,
    entry_date: String,
    /// The actual fill price (includes buy-side slippage).
    entry_price: Decimal,
    shares: Decimal,
    stop_loss_price: Option<Decimal>,
    take_profit_price: Option<Decimal>,
    entry_signal: String,
    entry_confidence: f64,
    entry_commission: Decimal,
    entry_slippage: Decimal,
    /// "long" or "short"
    direction: String,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self { config }
    }

    /// Run the backtest over historical data using pre-generated signals.
    ///
    /// Signals generated on date[i] execute at date[i+1]'s **open** price
    /// (next-bar execution) to eliminate look-ahead bias. Slippage is applied
    /// directionally: buys fill above the open, sells fill below.
    pub fn run(
        &mut self,
        historical_data: HashMap<String, Vec<HistoricalBar>>,
        signals: Vec<Signal>,
    ) -> Result<BacktestResult, String> {
        let commission_rate = self.config.commission_rate.unwrap_or(0.001);
        let slippage_rate = self.config.slippage_rate.unwrap_or(0.0005);
        let max_volume_pct = self.config.max_volume_participation.unwrap_or(0.05);
        let allow_short = self.config.allow_short_selling.unwrap_or(false);
        let allow_fractional = self.config.allow_fractional_shares.unwrap_or(false);
        let cash_sweep_rate = self.config.cash_sweep_rate.unwrap_or(0.0);
        let margin_mult = self.config.margin_multiplier.unwrap_or(1.0);

        // Data quality check
        let data_quality_report = Some(check_data_quality(&historical_data));

        let mut cash = self.config.initial_capital;
        let mut positions: HashMap<String, OpenPosition> = HashMap::new();
        let mut trades: Vec<BacktestTrade> = Vec::new();
        let mut equity_curve: Vec<EquityPoint> = Vec::new();
        let mut total_commission = Decimal::ZERO;
        let mut total_slippage = Decimal::ZERO;
        let mut peak_equity = self.config.initial_capital;
        let mut max_drawdown = 0.0;
        let mut total_bars: usize = 0;
        let mut exposed_bars: usize = 0;
        let mut short_trade_count = 0i32;

        // New feature trackers
        let mut margin_tracker = MarginTracker::new(margin_mult);
        let mut trailing_stop_mgr = self
            .config
            .trailing_stop_percent
            .map(TrailingStopManager::new);
        let mut circuit_breaker = self
            .config
            .max_drawdown_halt_percent
            .map(CircuitBreaker::new);
        let mut limit_order_mgr = LimitOrderManager::new();
        let regime_config = self.config.regime_config.clone().unwrap_or_default();
        let mut current_regime = Regime::Normal;
        let mut daily_returns_for_regime: Vec<f64> = Vec::new();

        // Per-symbol P&L tracking
        let mut symbol_pnl: HashMap<String, (Decimal, i32, i32)> = HashMap::new();

        // Build a unified timeline of all bars across all symbols (O(n) dedup)
        let mut date_set: HashSet<String> = HashSet::new();
        let mut bars_by_symbol_date: HashMap<String, HashMap<String, &HistoricalBar>> =
            HashMap::new();

        for (symbol, bars) in &historical_data {
            let mut date_map = HashMap::new();
            for bar in bars {
                date_set.insert(bar.date.clone());
                date_map.insert(bar.date.clone(), bar);
            }
            bars_by_symbol_date.insert(symbol.clone(), date_map);
        }
        let mut all_dates: Vec<String> = date_set.into_iter().collect();
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

        // Pending signals: collected on date[i], executed on date[i+1]
        let mut pending_signals: Vec<&Signal> = Vec::new();

        let commission_dec = Decimal::from_f64(commission_rate).unwrap_or(Decimal::ZERO);
        let slippage_dec = Decimal::from_f64(slippage_rate).unwrap_or(Decimal::ZERO);

        // Walk through each date chronologically
        for date in &all_dates {
            total_bars += 1;
            bars_since_rebalance += 1;

            // 0. Compute portfolio equity at today's open (for position sizing)
            let mut long_value_at_open = Decimal::ZERO;
            let mut short_value_at_open = Decimal::ZERO;
            for (symbol, pos) in &positions {
                let current_price = bars_by_symbol_date
                    .get(symbol)
                    .and_then(|m| m.get(date))
                    .map(|b| b.open)
                    .unwrap_or(pos.entry_price);

                if pos.direction == "short" {
                    short_value_at_open += short_selling::short_position_mtm(
                        pos.entry_price,
                        current_price,
                        pos.shares,
                    );
                } else {
                    long_value_at_open += current_price * pos.shares;
                }
            }
            let positions_value_at_open = long_value_at_open + short_value_at_open;

            // Cash sweep: accrue interest on idle cash daily
            if cash_sweep_rate > 0.0 {
                let daily_rate =
                    Decimal::from_f64(cash_sweep_rate / 252.0).unwrap_or(Decimal::ZERO);
                cash += cash * daily_rate;
            }

            let total_equity = cash + positions_value_at_open;

            // Margin utilization tracking
            margin_tracker
                .update_utilization(long_value_at_open + short_value_at_open, total_equity);

            // 1. Rebalancing: periodically close all positions at today's open
            if let Some(interval) = rebalance_interval {
                if interval > 0 && bars_since_rebalance >= interval && !positions.is_empty() {
                    let symbols_to_close: Vec<String> = positions.keys().cloned().collect();
                    for sym in symbols_to_close {
                        if let Some(pos) = positions.remove(&sym) {
                            let raw_price = bars_by_symbol_date
                                .get(&sym)
                                .and_then(|m| m.get(date))
                                .map(|b| b.open)
                                .unwrap_or(pos.entry_price);

                            let (trade, exit_comm, exit_slip) = Self::close_position(
                                pos,
                                &sym,
                                date,
                                raw_price,
                                commission_dec,
                                slippage_dec,
                                "rebalance",
                            );
                            if trade.direction.as_deref() == Some("short") {
                                cash -= trade.exit_price * trade.shares + exit_comm;
                            } else {
                                cash += trade.exit_price * trade.shares - exit_comm;
                            }
                            if let Some(ref mut ts) = trailing_stop_mgr {
                                ts.remove(&sym);
                            }
                            total_commission += exit_comm;
                            total_slippage += exit_slip;
                            Self::record_symbol_pnl(&mut symbol_pnl, &trade);
                            trades.push(trade);
                        }
                    }
                    bars_since_rebalance = 0;
                }
            }

            // 1.5 Regime detection (every 20 bars)
            if self.config.regime_config.is_some()
                && total_bars.is_multiple_of(20)
                && !daily_returns_for_regime.is_empty()
            {
                current_regime = detect_regime(&daily_returns_for_regime, &regime_config);
            }

            // 1.6 Circuit breaker check
            let trading_halted = circuit_breaker
                .as_mut()
                .map(|cb| cb.check(total_equity, peak_equity))
                .unwrap_or(false);

            // 1.7 Process pending limit orders
            if !trading_halted {
                let mut limit_triggered = Vec::new();
                // Check all symbols' bars for limit order fills
                for bars_map in bars_by_symbol_date.values() {
                    if let Some(bar) = bars_map.get(date) {
                        let triggered = limit_order_mgr.check_and_expire(bar.low, bar.high);
                        limit_triggered.extend(triggered);
                    }
                }
                // Convert triggered limits into pending signals for execution
                for (mut signal, _direction) in limit_triggered {
                    // Clear order type so triggered limits execute as market orders
                    signal.order_type = None;
                    pending_signals.push(Box::leak(Box::new(signal)));
                }
            }

            // 2. Execute pending signals from previous date at today's OPEN
            if !trading_halted {
                let signals_to_execute: Vec<&Signal> = std::mem::take(&mut pending_signals);
                for signal in signals_to_execute {
                    if signal.confidence < self.config.confidence_threshold {
                        continue;
                    }

                    // Route limit orders to the limit order manager
                    if signal.order_type == Some(OrderType::Limit) && signal.limit_price.is_some() {
                        let action = signal.signal_type.to_lowercase();
                        let direction = if action.contains("buy") {
                            "buy"
                        } else {
                            "sell"
                        };
                        limit_order_mgr.add_order(signal.clone(), direction);
                        continue;
                    }

                    let action = signal.signal_type.to_lowercase();

                    // Get today's bar for this symbol (needed for open price + volume)
                    let bar = match bars_by_symbol_date
                        .get(&signal.symbol)
                        .and_then(|m| m.get(date))
                    {
                        Some(b) => *b,
                        None => continue,
                    };

                    if action.contains("buy") {
                        // Buy signal: if we have a short position, close it first
                        if let Some(pos) = positions.get(&signal.symbol) {
                            if pos.direction == "short" {
                                let pos = positions.remove(&signal.symbol).unwrap();
                                let raw_price = bar.open;
                                let (trade, exit_comm, exit_slip) = Self::close_position(
                                    pos,
                                    &signal.symbol,
                                    date,
                                    raw_price,
                                    commission_dec,
                                    slippage_dec,
                                    "signal_cover",
                                );
                                // Buy-to-cover: pay exit price + commission
                                cash -= trade.exit_price * trade.shares + exit_comm;
                                if let Some(ref mut ts) = trailing_stop_mgr {
                                    ts.remove(&signal.symbol);
                                }
                                total_commission += exit_comm;
                                total_slippage += exit_slip;
                                Self::record_symbol_pnl(&mut symbol_pnl, &trade);
                                trades.push(trade);
                                continue; // Cover only, don't also open long
                            } else {
                                continue; // Already long
                            }
                        }

                        let raw_price = bar.open;
                        let fill_price = raw_price + raw_price * slippage_dec;

                        // Regime-adjusted sizing
                        let regime_mult = if self.config.regime_config.is_some() {
                            regime_size_multiplier(current_regime, &regime_config)
                        } else {
                            1.0
                        };

                        let weight = weights
                            .get(&signal.symbol)
                            .copied()
                            .unwrap_or(self.config.position_size_percent / 100.0);
                        let adjusted_weight = weight * regime_mult;
                        let weight_dec =
                            Decimal::from_f64(adjusted_weight).unwrap_or(Decimal::ZERO);

                        // Margin-adjusted buying power
                        let buying_power =
                            margin_tracker.buying_power(cash) + (total_equity - cash);
                        let position_value = buying_power * weight_dec;

                        if position_value < fill_price {
                            continue;
                        }

                        let mut shares = if allow_fractional {
                            position_value / fill_price
                        } else {
                            (position_value / fill_price).floor()
                        };
                        if !allow_fractional && shares < Decimal::ONE {
                            continue;
                        }
                        if allow_fractional && shares <= Decimal::ZERO {
                            continue;
                        }

                        // Volume participation limit
                        if bar.volume > 0.0 && max_volume_pct > 0.0 {
                            let max_shares = Decimal::from_f64(bar.volume * max_volume_pct)
                                .unwrap_or(Decimal::MAX)
                                .floor();
                            if shares > max_shares {
                                shares = max_shares;
                                if shares < Decimal::ONE {
                                    continue;
                                }
                            }
                        }

                        // If cost exceeds available cash, reduce to affordable size
                        let initial_cost =
                            fill_price * shares + fill_price * shares * commission_dec;
                        let available = margin_tracker.buying_power(cash);
                        if initial_cost > available {
                            let denom = fill_price * (Decimal::ONE + commission_dec);
                            let affordable = if allow_fractional {
                                available / denom
                            } else {
                                (available / denom).floor()
                            };
                            if (!allow_fractional && affordable < Decimal::ONE)
                                || (allow_fractional && affordable <= Decimal::ZERO)
                            {
                                continue;
                            }
                            shares = affordable;
                        }

                        let entry_slippage = (fill_price - raw_price) * shares;
                        let entry_commission = compute_tiered_commission(
                            self.config.commission_model.as_ref(),
                            shares,
                            fill_price,
                            commission_dec,
                            0.0,
                        );
                        let cost = fill_price * shares + entry_commission;

                        if cost > margin_tracker.buying_power(cash) {
                            continue;
                        }

                        total_commission += entry_commission;
                        total_slippage += entry_slippage;
                        cash -= cost;

                        let stop_loss_price = self
                            .config
                            .stop_loss_percent
                            .and_then(|pct| Decimal::from_f64(1.0 - pct).map(|d| fill_price * d));
                        let take_profit_price = self
                            .config
                            .take_profit_percent
                            .and_then(|pct| Decimal::from_f64(1.0 + pct).map(|d| fill_price * d));

                        // Initialize trailing stop
                        if let Some(ref mut ts) = trailing_stop_mgr {
                            ts.init(&signal.symbol, fill_price);
                        }

                        let display_signal = Self::capitalize(&signal.signal_type);

                        positions.insert(
                            signal.symbol.clone(),
                            OpenPosition {
                                symbol: signal.symbol.clone(),
                                entry_date: date.clone(),
                                entry_price: fill_price,
                                shares,
                                stop_loss_price,
                                take_profit_price,
                                entry_signal: display_signal,
                                entry_confidence: signal.confidence,
                                entry_commission,
                                entry_slippage,
                                direction: "long".to_string(),
                            },
                        );
                    } else if action.contains("sell") {
                        if let Some(pos) = positions.get(&signal.symbol) {
                            if pos.direction == "long" {
                                // Close long position
                                let pos = positions.remove(&signal.symbol).unwrap();
                                let raw_price = bar.open;
                                let (trade, exit_comm, exit_slip) = Self::close_position(
                                    pos,
                                    &signal.symbol,
                                    date,
                                    raw_price,
                                    commission_dec,
                                    slippage_dec,
                                    "signal",
                                );
                                cash += trade.exit_price * trade.shares - exit_comm;
                                if let Some(ref mut ts) = trailing_stop_mgr {
                                    ts.remove(&signal.symbol);
                                }
                                total_commission += exit_comm;
                                total_slippage += exit_slip;
                                Self::record_symbol_pnl(&mut symbol_pnl, &trade);
                                trades.push(trade);
                            }
                            // Already short → skip
                        } else if allow_short {
                            // Open short position
                            let raw_price = bar.open;
                            let fill_price =
                                short_selling::short_entry_fill(raw_price, slippage_dec);

                            let regime_mult = if self.config.regime_config.is_some() {
                                regime_size_multiplier(current_regime, &regime_config)
                            } else {
                                1.0
                            };

                            let weight = weights
                                .get(&signal.symbol)
                                .copied()
                                .unwrap_or(self.config.position_size_percent / 100.0);
                            let adjusted_weight = weight * regime_mult;
                            let weight_dec =
                                Decimal::from_f64(adjusted_weight).unwrap_or(Decimal::ZERO);
                            let position_value = total_equity * weight_dec;

                            if position_value < fill_price {
                                continue;
                            }

                            let mut shares = if allow_fractional {
                                position_value / fill_price
                            } else {
                                (position_value / fill_price).floor()
                            };
                            if !allow_fractional && shares < Decimal::ONE {
                                continue;
                            }

                            // Volume participation limit for shorts
                            if bar.volume > 0.0 && max_volume_pct > 0.0 {
                                let max_shares = Decimal::from_f64(bar.volume * max_volume_pct)
                                    .unwrap_or(Decimal::MAX)
                                    .floor();
                                if shares > max_shares {
                                    shares = max_shares;
                                    if shares < Decimal::ONE {
                                        continue;
                                    }
                                }
                            }

                            let entry_slippage = (raw_price - fill_price) * shares;
                            let entry_commission = compute_tiered_commission(
                                self.config.commission_model.as_ref(),
                                shares,
                                fill_price,
                                commission_dec,
                                0.0,
                            );

                            // Short sale: receive cash (fill_price * shares) minus commission
                            cash += fill_price * shares - entry_commission;
                            total_commission += entry_commission;
                            total_slippage += entry_slippage;

                            // SL/TP for shorts (inverted)
                            let stop_loss_price = self.config.stop_loss_percent.and_then(|pct| {
                                Decimal::from_f64(1.0 + pct).map(|d| fill_price * d)
                            });
                            let take_profit_price =
                                self.config.take_profit_percent.and_then(|pct| {
                                    Decimal::from_f64(1.0 - pct).map(|d| fill_price * d)
                                });

                            let display_signal = Self::capitalize(&signal.signal_type);
                            short_trade_count += 1;

                            positions.insert(
                                signal.symbol.clone(),
                                OpenPosition {
                                    symbol: signal.symbol.clone(),
                                    entry_date: date.clone(),
                                    entry_price: fill_price,
                                    shares,
                                    stop_loss_price,
                                    take_profit_price,
                                    entry_signal: display_signal,
                                    entry_confidence: signal.confidence,
                                    entry_commission,
                                    entry_slippage,
                                    direction: "short".to_string(),
                                },
                            );
                        }
                    }
                }
            }

            // 3. Check stop-loss / take-profit on open positions (+ trailing stops)
            let mut to_close: Vec<(String, Decimal, &str)> = Vec::new();
            for (symbol, pos) in &positions {
                if let Some(bars_map) = bars_by_symbol_date.get(symbol) {
                    if let Some(bar) = bars_map.get(date) {
                        if pos.direction == "short" {
                            // Short SL/TP (inverted)
                            if let Some(sl) = pos.stop_loss_price {
                                if short_selling::short_stop_loss_triggered(bar.high, sl) {
                                    let raw_fill = short_selling::short_sl_fill_price(bar.open, sl);
                                    to_close.push((symbol.clone(), raw_fill, "stop_loss"));
                                    continue;
                                }
                            }
                            if let Some(tp) = pos.take_profit_price {
                                if short_selling::short_take_profit_triggered(bar.low, tp) {
                                    let raw_fill = short_selling::short_tp_fill_price(bar.open, tp);
                                    to_close.push((symbol.clone(), raw_fill, "take_profit"));
                                }
                            }
                        } else {
                            // Long SL/TP
                            // Trailing stop: update and use as SL
                            let effective_sl = if let Some(ref mut ts) = trailing_stop_mgr {
                                let ts_price = ts.update(symbol, bar.high);
                                // Use trailing stop if higher than fixed SL
                                match (pos.stop_loss_price, ts_price) {
                                    (Some(fixed), Some(trail)) => Some(fixed.max(trail)),
                                    (None, ts) => ts,
                                    (fixed, None) => fixed,
                                }
                            } else {
                                pos.stop_loss_price
                            };

                            if let Some(sl) = effective_sl {
                                if bar.low <= sl {
                                    let raw_fill = if bar.open <= sl { bar.open } else { sl };
                                    to_close.push((symbol.clone(), raw_fill, "stop_loss"));
                                    continue;
                                }
                            }
                            if let Some(tp) = pos.take_profit_price {
                                if bar.high >= tp {
                                    let raw_fill = if bar.open >= tp { bar.open } else { tp };
                                    to_close.push((symbol.clone(), raw_fill, "take_profit"));
                                }
                            }
                        }
                    }
                }
            }

            for (symbol, raw_exit_price, reason) in to_close {
                if let Some(pos) = positions.remove(&symbol) {
                    let (trade, exit_comm, exit_slip) = Self::close_position(
                        pos,
                        &symbol,
                        date,
                        raw_exit_price,
                        commission_dec,
                        slippage_dec,
                        reason,
                    );
                    // For shorts: close_position handles cash adjustment internally
                    if trade.direction.as_deref() == Some("short") {
                        // Buy-to-cover: pay exit_price * shares + commission
                        cash -= trade.exit_price * trade.shares + exit_comm;
                    } else {
                        cash += trade.exit_price * trade.shares - exit_comm;
                    }
                    if let Some(ref mut ts) = trailing_stop_mgr {
                        ts.remove(&symbol);
                    }
                    total_commission += exit_comm;
                    total_slippage += exit_slip;
                    Self::record_symbol_pnl(&mut symbol_pnl, &trade);
                    trades.push(trade);
                }
            }

            // 4. Collect signals for this date → pending (execute next bar)
            if let Some(day_signals) = signals_by_date.get(date) {
                pending_signals.extend(day_signals.iter());
            }

            // 5. Track exposure
            if !positions.is_empty() {
                exposed_bars += 1;
            }

            // 6. Mark-to-market at close
            let mut positions_value = Decimal::ZERO;
            for (symbol, pos) in &positions {
                let current_price = bars_by_symbol_date
                    .get(symbol)
                    .and_then(|m| m.get(date))
                    .map(|b| b.close)
                    .unwrap_or(pos.entry_price);

                if pos.direction == "short" {
                    positions_value += short_selling::short_position_mtm(
                        pos.entry_price,
                        current_price,
                        pos.shares,
                    );
                } else {
                    positions_value += current_price * pos.shares;
                }
            }

            let equity = cash + positions_value;
            if equity > peak_equity {
                peak_equity = equity;
            }
            let peak_f64 = peak_equity.to_f64().unwrap_or(1.0);
            let equity_f64 = equity.to_f64().unwrap_or(0.0);
            let drawdown_pct = if peak_f64 > 0.0 {
                (peak_f64 - equity_f64) / peak_f64 * 100.0
            } else {
                0.0
            };
            if drawdown_pct > max_drawdown {
                max_drawdown = drawdown_pct;
            }

            // Track daily returns for regime detection
            if !equity_curve.is_empty() {
                let prev_equity = equity_curve.last().unwrap().equity.to_f64().unwrap_or(1.0);
                if prev_equity > 0.0 {
                    daily_returns_for_regime.push(equity_f64 / prev_equity - 1.0);
                }
            }

            equity_curve.push(EquityPoint {
                timestamp: date.clone(),
                equity,
                drawdown_percent: drawdown_pct,
            });
        }

        // 7. Close remaining positions at last available close price
        let last_date = all_dates.last().cloned().unwrap_or_default();
        for (symbol, pos) in positions.drain() {
            let last_price = bars_by_symbol_date
                .get(&symbol)
                .and_then(|m| m.get(&last_date))
                .map(|b| b.close)
                .unwrap_or(pos.entry_price);

            let (trade, exit_comm, exit_slip) = Self::close_position(
                pos,
                &symbol,
                &last_date,
                last_price,
                commission_dec,
                slippage_dec,
                "end_of_backtest",
            );
            if trade.direction.as_deref() == Some("short") {
                cash -= trade.exit_price * trade.shares + exit_comm;
            } else {
                cash += trade.exit_price * trade.shares - exit_comm;
            }
            total_commission += exit_comm;
            total_slippage += exit_slip;
            Self::record_symbol_pnl(&mut symbol_pnl, &trade);
            trades.push(trade);
        }

        // 8. Compute aggregate metrics
        let final_capital = cash;
        let total_return = final_capital - self.config.initial_capital;
        let initial_f64 = self.config.initial_capital.to_f64().unwrap_or(1.0);
        let final_f64 = final_capital.to_f64().unwrap_or(0.0);
        let total_return_percent = (final_f64 / initial_f64 - 1.0) * 100.0;

        let years = total_bars as f64 / 252.0;
        let annualized_return_percent = if years > 0.0 && initial_f64 > 0.0 && final_f64 > 0.0 {
            let ratio = final_f64 / initial_f64;
            if ratio > 0.0 {
                Some((ratio.powf(1.0 / years) - 1.0) * 100.0)
            } else {
                Some(-100.0)
            }
        } else {
            None
        };

        let total_trades = trades.len() as i32;
        let winning_trades = trades
            .iter()
            .filter(|t| t.profit_loss > Decimal::ZERO)
            .count() as i32;
        let losing_trades = total_trades - winning_trades;
        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        let gross_profits: Decimal = trades
            .iter()
            .filter(|t| t.profit_loss > Decimal::ZERO)
            .map(|t| t.profit_loss)
            .sum();
        let gross_losses: Decimal = trades
            .iter()
            .filter(|t| t.profit_loss < Decimal::ZERO)
            .map(|t| t.profit_loss.abs())
            .sum();
        let profit_factor = {
            let gp = gross_profits.to_f64().unwrap_or(0.0);
            let gl = gross_losses.to_f64().unwrap_or(0.0);
            if gl > 0.0 {
                Some(gp / gl)
            } else if gp > 0.0 {
                Some(f64::INFINITY)
            } else {
                None
            }
        };

        let (sharpe, sortino) = Self::compute_risk_ratios(&equity_curve);

        let avg_trade_return = if !trades.is_empty() {
            Some(trades.iter().map(|t| t.profit_loss_percent).sum::<f64>() / trades.len() as f64)
        } else {
            None
        };
        let average_win = if winning_trades > 0 {
            Some(gross_profits / Decimal::from(winning_trades))
        } else {
            None
        };
        let average_loss = if losing_trades > 0 {
            Some(gross_losses / Decimal::from(losing_trades))
        } else {
            None
        };
        let largest_win = trades
            .iter()
            .map(|t| t.profit_loss)
            .filter(|p| *p > Decimal::ZERO)
            .fold(None, |a: Option<Decimal>, p| {
                Some(a.map_or(p, |v| v.max(p)))
            });
        let largest_loss = trades
            .iter()
            .map(|t| t.profit_loss)
            .filter(|p| *p < Decimal::ZERO)
            .fold(None, |a: Option<Decimal>, p| {
                Some(a.map_or(p, |v| v.min(p)))
            });
        let (max_con_wins, max_con_losses) = Self::max_consecutive_streaks(&trades);
        let avg_holding = if !trades.is_empty() {
            Some(
                trades.iter().map(|t| t.holding_period_days).sum::<i64>() as f64
                    / trades.len() as f64,
            )
        } else {
            None
        };
        let exposure = if total_bars > 0 {
            Some(exposed_bars as f64 / total_bars as f64 * 100.0)
        } else {
            None
        };
        let calmar = if max_drawdown > 0.0 {
            annualized_return_percent.map(|cagr| cagr / max_drawdown)
        } else {
            None
        };
        let recovery = if max_drawdown > 0.0 {
            Some(total_return_percent / max_drawdown)
        } else {
            None
        };

        // 9. Benchmark comparison
        let benchmark = self.compute_benchmark(&all_dates, &bars_by_symbol_date, &equity_curve);

        // 10. Per-symbol results
        let per_symbol_results = if self.config.symbols.len() > 1 {
            Some(self.compute_per_symbol_results(&symbol_pnl, &weights))
        } else {
            None
        };

        // 11. Extended analytics (post-processing)
        let benchmark_returns = benchmark.as_ref().and_then(|b| {
            if b.spy_equity_curve.len() >= 2 {
                Some(equity_returns(&b.spy_equity_curve))
            } else {
                None
            }
        });

        let extended_metrics = if equity_curve.len() >= 5 {
            Some(compute_extended_metrics(
                &equity_curve,
                &trades,
                benchmark_returns.as_deref(),
                total_return_percent,
                annualized_return_percent,
            ))
        } else {
            None
        };

        let factor_attribution_result = benchmark_returns.as_ref().and_then(|br| {
            let strategy_rets = equity_returns(&equity_curve);
            compute_factor_attribution(&strategy_rets, br)
        });

        let confidence_intervals = if trades.len() >= 10 {
            bootstrap_confidence_intervals(&trades, 1000)
        } else {
            None
        };

        let short_trades_opt = if allow_short {
            Some(short_trade_count)
        } else {
            None
        };

        let margin_used_peak = if margin_mult > 1.0 {
            Some(margin_tracker.peak_utilization())
        } else {
            None
        };

        // 12. Advanced analytics (institutional-grade)
        let advanced_analytics = self.compute_advanced_analytics(
            &equity_curve,
            &trades,
            sharpe,
            extended_metrics.as_ref(),
        );

        let mut result = BacktestResult {
            id: None,
            strategy_name: self.config.strategy_name.clone(),
            symbols: self.config.symbols.clone(),
            start_date: self.config.start_date.clone(),
            end_date: self.config.end_date.clone(),
            initial_capital: self.config.initial_capital,
            final_capital,
            total_return,
            total_return_percent,
            annualized_return_percent,
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
            short_trades: short_trades_opt,
            margin_used_peak,
            data_quality_report,
            confidence_intervals,
            extended_metrics,
            factor_attribution: factor_attribution_result,
            tear_sheet: None,
            advanced_analytics,
        };

        // 13. Generate tear sheet from all collected analytics
        result.tear_sheet = Some(generate_tear_sheet(&result));

        Ok(result)
    }

    // --- Helpers ---

    /// Close a position with directional slippage.
    ///
    /// `raw_exit_price` is the price before slippage (e.g. bar open, SL/TP trigger).
    /// For longs: sell-side slippage fills below; for shorts: buy-side slippage fills above.
    /// Returns (trade_record, exit_commission, exit_slippage).
    fn close_position(
        pos: OpenPosition,
        symbol: &str,
        date: &str,
        raw_exit_price: Decimal,
        commission_dec: Decimal,
        slippage_dec: Decimal,
        reason: &str,
    ) -> (BacktestTrade, Decimal, Decimal) {
        let is_short = pos.direction == "short";

        let fill_price = if is_short {
            // Buy-to-cover: slippage fills ABOVE the raw price
            raw_exit_price + raw_exit_price * slippage_dec
        } else {
            // Sell: slippage fills BELOW the raw price
            raw_exit_price - raw_exit_price * slippage_dec
        };

        let exit_slippage = (raw_exit_price - fill_price).abs() * pos.shares;
        let exit_commission = fill_price * pos.shares * commission_dec;

        // P&L: long = (exit - entry), short = (entry - exit)
        let gross_pnl = if is_short {
            (pos.entry_price - fill_price) * pos.shares
        } else {
            (fill_price - pos.entry_price) * pos.shares
        };
        let total_costs = pos.entry_commission + exit_commission;
        let net_pnl = gross_pnl - total_costs;

        let entry_f64 = pos.entry_price.to_f64().unwrap_or(1.0);
        let exit_f64 = fill_price.to_f64().unwrap_or(0.0);
        let return_pct = if entry_f64 > 0.0 {
            if is_short {
                // Short return: profit when price drops
                ((entry_f64 - exit_f64) / entry_f64) * 100.0
            } else {
                (exit_f64 / entry_f64 - 1.0) * 100.0
            }
        } else {
            0.0
        };
        let holding_days = Self::date_diff(&pos.entry_date, date);

        let direction_str = pos.direction.clone();

        let trade = BacktestTrade {
            id: None,
            backtest_id: None,
            symbol: symbol.to_string(),
            signal: pos.entry_signal,
            confidence: pos.entry_confidence,
            entry_date: pos.entry_date,
            exit_date: date.to_string(),
            entry_price: pos.entry_price,
            exit_price: fill_price,
            shares: pos.shares,
            profit_loss: net_pnl,
            profit_loss_percent: return_pct,
            holding_period_days: holding_days,
            commission_cost: pos.entry_commission + exit_commission,
            slippage_cost: pos.entry_slippage + exit_slippage,
            exit_reason: reason.to_string(),
            direction: Some(direction_str),
        };
        (trade, exit_commission, exit_slippage)
    }

    fn record_symbol_pnl(map: &mut HashMap<String, (Decimal, i32, i32)>, trade: &BacktestTrade) {
        let entry = map
            .entry(trade.symbol.clone())
            .or_insert((Decimal::ZERO, 0, 0));
        entry.0 += trade.profit_loss;
        if trade.profit_loss > Decimal::ZERO {
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

        let primary_symbol = self.config.symbols.first()?;
        let primary_bars = bars_by_symbol_date.get(primary_symbol)?;

        let first_price = primary_bars.get(&all_dates[0]).map(|b| b.close)?;
        let bh_shares = self.config.initial_capital / first_price;

        let mut bh_curve = Vec::new();
        let mut bh_peak = self.config.initial_capital;
        for date in all_dates {
            let price = primary_bars
                .get(date)
                .map(|b| b.close)
                .unwrap_or(first_price);
            let equity = bh_shares * price;
            if equity > bh_peak {
                bh_peak = equity;
            }
            let peak_f64 = bh_peak.to_f64().unwrap_or(1.0);
            let equity_f64 = equity.to_f64().unwrap_or(0.0);
            let dd = if peak_f64 > 0.0 {
                (peak_f64 - equity_f64) / peak_f64 * 100.0
            } else {
                0.0
            };
            bh_curve.push(EquityPoint {
                timestamp: date.clone(),
                equity,
                drawdown_percent: dd,
            });
        }

        let bh_final = bh_curve
            .last()
            .map(|p| p.equity)
            .unwrap_or(self.config.initial_capital);
        let initial_f64 = self.config.initial_capital.to_f64().unwrap_or(1.0);
        let bh_final_f64 = bh_final.to_f64().unwrap_or(0.0);
        let buy_hold_return_percent = (bh_final_f64 / initial_f64 - 1.0) * 100.0;
        let strategy_final = strategy_equity
            .last()
            .map(|p| p.equity)
            .unwrap_or(self.config.initial_capital);
        let strategy_final_f64 = strategy_final.to_f64().unwrap_or(0.0);
        let strategy_return = (strategy_final_f64 / initial_f64 - 1.0) * 100.0;
        let alpha = strategy_return - buy_hold_return_percent;

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
            let price = spy_date_map
                .get(date.as_str())
                .map(|b| b.close)
                .unwrap_or(first_spy_price);
            let equity = spy_shares * price;
            if equity > spy_peak {
                spy_peak = equity;
            }
            let peak_f64 = spy_peak.to_f64().unwrap_or(1.0);
            let equity_f64 = equity.to_f64().unwrap_or(0.0);
            let dd = if peak_f64 > 0.0 {
                (peak_f64 - equity_f64) / peak_f64 * 100.0
            } else {
                0.0
            };
            spy_curve.push(EquityPoint {
                timestamp: date.clone(),
                equity,
                drawdown_percent: dd,
            });
        }

        let spy_final = spy_curve
            .last()
            .map(|p| p.equity)
            .unwrap_or(self.config.initial_capital);
        let initial_f64 = self.config.initial_capital.to_f64().unwrap_or(1.0);
        let spy_final_f64 = spy_final.to_f64().unwrap_or(0.0);
        let spy_return = (spy_final_f64 / initial_f64 - 1.0) * 100.0;
        let strategy_final = strategy_equity
            .last()
            .map(|p| p.equity)
            .unwrap_or(self.config.initial_capital);
        let strategy_final_f64 = strategy_final.to_f64().unwrap_or(0.0);
        let strategy_return = (strategy_final_f64 / initial_f64 - 1.0) * 100.0;
        let spy_alpha = strategy_return - spy_return;

        // Information ratio: alpha / tracking error
        let information_ratio = if strategy_equity.len() > 1
            && spy_curve.len() == strategy_equity.len()
        {
            let diffs: Vec<f64> = strategy_equity
                .windows(2)
                .zip(spy_curve.windows(2))
                .map(|(s, b)| {
                    let s0 = s[0].equity.to_f64().unwrap_or(1.0);
                    let s1 = s[1].equity.to_f64().unwrap_or(1.0);
                    let b0 = b[0].equity.to_f64().unwrap_or(1.0);
                    let b1 = b[1].equity.to_f64().unwrap_or(1.0);
                    let s_ret = (s1 / s0) - 1.0;
                    let b_ret = (b1 / b0) - 1.0;
                    s_ret - b_ret
                })
                .collect();
            if diffs.len() > 1 {
                let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
                let var = diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>()
                    / (diffs.len() - 1) as f64;
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

        (
            Some(spy_return),
            Some(spy_alpha),
            spy_curve,
            information_ratio,
        )
    }

    // --- Per-Symbol Results ---

    fn compute_per_symbol_results(
        &self,
        symbol_pnl: &HashMap<String, (Decimal, i32, i32)>,
        weights: &HashMap<String, f64>,
    ) -> Vec<SymbolResult> {
        self.config
            .symbols
            .iter()
            .map(|sym| {
                let (pnl, wins, total) =
                    symbol_pnl
                        .get(sym)
                        .copied()
                        .unwrap_or((Decimal::ZERO, 0, 0));
                let w = weights.get(sym).copied().unwrap_or(0.0);
                let w_dec = Decimal::from_f64(w).unwrap_or(Decimal::ZERO);
                let invested = self.config.initial_capital * w_dec;
                let invested_f64 = invested.to_f64().unwrap_or(1.0);
                let pnl_f64 = pnl.to_f64().unwrap_or(0.0);
                SymbolResult {
                    symbol: sym.clone(),
                    total_trades: total,
                    winning_trades: wins,
                    win_rate: if total > 0 {
                        (wins as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    },
                    total_return: pnl,
                    total_return_percent: if invested_f64 > 0.0 {
                        (pnl_f64 / invested_f64) * 100.0
                    } else {
                        0.0
                    },
                    weight: w,
                }
            })
            .collect()
    }

    // --- Risk Ratios ---

    fn compute_risk_ratios(equity_curve: &[EquityPoint]) -> (Option<f64>, Option<f64>) {
        if equity_curve.len() < 3 {
            return (None, None);
        }
        let returns: Vec<f64> = equity_curve
            .windows(2)
            .map(|w| {
                let e0 = w[0].equity.to_f64().unwrap_or(1.0);
                let e1 = w[1].equity.to_f64().unwrap_or(1.0);
                (e1 / e0) - 1.0
            })
            .collect();
        if returns.len() < 2 {
            return (None, None);
        }
        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        // Sample standard deviation (Bessel's correction: n-1)
        let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
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
        } else if mean > rf_daily {
            // Zero downside deviation with positive excess return → excellent
            Some(99.99)
        } else {
            // Zero downside, zero or negative excess → undefined
            None
        };

        (sharpe, sortino)
    }

    fn max_consecutive_streaks(trades: &[BacktestTrade]) -> (i32, i32) {
        let mut max_w = 0;
        let mut max_l = 0;
        let mut w = 0;
        let mut l = 0;
        for t in trades {
            if t.profit_loss > Decimal::ZERO {
                w += 1;
                l = 0;
                max_w = max_w.max(w);
            } else if t.profit_loss < Decimal::ZERO {
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

    /// Compute advanced analytics (institutional-grade metrics).
    fn compute_advanced_analytics(
        &self,
        equity_curve: &[EquityPoint],
        trades: &[BacktestTrade],
        sharpe: Option<f64>,
        extended_metrics: Option<&ExtendedMetrics>,
    ) -> Option<AdvancedAnalytics> {
        if trades.len() < 5 {
            return None;
        }

        // Trade expectancy analysis
        let expectancy = trade_analysis::compute_expectancy(trades).map(|e| ExpectancyAnalysis {
            expectancy: e.expectancy,
            expectancy_percent: e.expectancy_percent,
            kelly_fraction: e.kelly_fraction,
            payoff_ratio: e.payoff_ratio,
            sqn: e.sqn,
        });

        // Win/loss streak distribution
        let streaks = trade_analysis::analyze_streaks(trades).map(|s| StreakDistribution {
            max_win_streak: s.max_win_streak,
            max_loss_streak: s.max_loss_streak,
            avg_win_streak: s.avg_win_streak,
            avg_loss_streak: s.avg_loss_streak,
            prob_win_after_win: s.prob_win_after_win,
            prob_win_after_loss: s.prob_win_after_loss,
        });

        // Regime-based payoff analysis (by holding period)
        let regime_payoffs_raw = trade_analysis::analyze_payoff_by_holding_period(trades);
        let regime_payoffs = if regime_payoffs_raw.is_empty() {
            None
        } else {
            Some(
                regime_payoffs_raw
                    .into_iter()
                    .map(|rp| RegimePayoff {
                        regime_name: rp.regime_name,
                        num_trades: rp.num_trades,
                        win_rate: rp.win_rate,
                        avg_win: rp.avg_win,
                        avg_loss: rp.avg_loss,
                        payoff_ratio: rp.payoff_ratio,
                        expectancy: rp.expectancy,
                    })
                    .collect(),
            )
        };

        // Time-in-market analysis
        let time_in_market =
            trade_analysis::analyze_time_in_market(trades).map(|t| TimeInMarketAnalysis {
                time_in_market_percent: t.time_in_market_percent,
                avg_concurrent_positions: t.avg_concurrent_positions,
                max_concurrent_positions: t.max_concurrent_positions,
                active_trading_days: t.active_trading_days,
                total_calendar_days: t.total_calendar_days,
            });

        // Drawdown recovery analysis
        let drawdown_recovery = advanced_risk::drawdown_recovery_analysis(equity_curve).map(|r| {
            DrawdownRecoveryStats {
                avg_recovery_days: r.avg_recovery_days,
                max_recovery_days: r.max_recovery_days,
                num_recovered: r.num_recovered,
                num_ongoing: r.num_ongoing,
                time_in_drawdown_percent: r.time_in_drawdown_percent,
            }
        });

        // Overfitting detection
        let overfitting_analysis = sharpe.map(|sr| {
            // Use skewness and kurtosis if available
            let (skew, kurt) = extended_metrics
                .and_then(|e| e.skewness.zip(e.kurtosis))
                .unwrap_or((0.0, 0.0));

            let num_observations = equity_curve.len().saturating_sub(1).max(1) as i32;
            let num_trials = 1; // Single strategy (for multi-strategy, pass strategy count)

            let dsr =
                overfitting::deflated_sharpe_ratio(sr, num_trials, num_observations, skew, kurt);

            // Minimum backtest length recommendation
            let min_length = overfitting::minimum_backtest_length(sr.max(0.1), 0.95, 0.80);

            OverfittingAnalysis {
                deflated_sharpe: dsr.deflated_sharpe,
                sharpe_p_value: dsr.p_value,
                expected_max_sharpe_null: dsr.expected_max_sharpe_null,
                min_backtest_length: min_length,
            }
        });

        Some(AdvancedAnalytics {
            expectancy,
            streaks,
            regime_payoffs,
            time_in_market,
            drawdown_recovery,
            overfitting_analysis,
        })
    }
}

// --- Walk-Forward Validation ---

/// Runs walk-forward validation across pre-prepared folds.
pub struct WalkForwardRunner;

impl WalkForwardRunner {
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
        let avg_oos = fold_results
            .iter()
            .map(|f| f.out_of_sample_return)
            .sum::<f64>()
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
