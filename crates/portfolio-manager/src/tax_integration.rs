use crate::models::*;
use chrono::NaiveDate;
use rust_decimal::prelude::*;
use std::collections::HashMap;
use tax_optimizer::{TaxJurisdiction, TaxLot, TaxRules};

pub struct TaxBridge;

impl TaxBridge {
    /// Build TaxLots from trade history (FIFO).
    #[allow(unused_variables)]
    pub fn build_tax_lots(
        trades: &[Trade],
        prices: &HashMap<String, f64>,
    ) -> Vec<TaxLot> {
        let mut lots = Vec::new();
        // Group trades by symbol, sorted by date ascending
        let mut by_symbol: HashMap<String, Vec<&Trade>> = HashMap::new();
        for t in trades {
            by_symbol.entry(t.symbol.clone()).or_default().push(t);
        }

        for (symbol, mut symbol_trades) in by_symbol {
            symbol_trades.sort_by(|a, b| a.trade_date.cmp(&b.trade_date));

            let mut open_lots: Vec<TaxLot> = Vec::new();
            let mut lot_counter = 0usize;

            for trade in &symbol_trades {
                let shares = trade.shares.to_f64().unwrap_or(0.0);
                let price = trade.price.to_f64().unwrap_or(0.0);
                let date = NaiveDate::parse_from_str(&trade.trade_date, "%Y-%m-%d")
                    .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());

                if trade.action == "buy" {
                    lot_counter += 1;
                    let lot = TaxLot::new(
                        format!("{}-{}", symbol, lot_counter),
                        symbol.clone(),
                        shares,
                        price,
                        date,
                    );
                    open_lots.push(lot);
                } else if trade.action == "sell" {
                    // FIFO: close oldest lots first
                    let mut remaining = shares;
                    while remaining > 0.001 && !open_lots.is_empty() {
                        let lot = &mut open_lots[0];
                        if lot.shares <= remaining + 0.001 {
                            remaining -= lot.shares;
                            lot.sale_date = Some(date);
                            lot.sale_price_per_share = Some(price);
                            lot.is_closed = true;
                            // Move to closed
                            let closed = open_lots.remove(0);
                            lots.push(closed);
                        } else {
                            // Partial close — split the lot
                            let mut closed_lot = lot.clone();
                            closed_lot.shares = remaining;
                            closed_lot.total_cost_basis = remaining * lot.cost_basis_per_share;
                            closed_lot.sale_date = Some(date);
                            closed_lot.sale_price_per_share = Some(price);
                            closed_lot.is_closed = true;
                            closed_lot.id = format!("{}-partial", closed_lot.id);

                            lot.shares -= remaining;
                            lot.total_cost_basis = lot.shares * lot.cost_basis_per_share;

                            lots.push(closed_lot);
                            remaining = 0.0;
                        }
                    }
                }
            }

            // Add remaining open lots
            for lot in open_lots {
                lots.push(lot);
            }
        }

        lots
    }

    /// Compute tax summary from trades and current prices.
    pub fn compute_tax_summary(
        trades: &[Trade],
        prices: &HashMap<String, f64>,
        jurisdiction: TaxJurisdiction,
    ) -> TaxSummary {
        let lots = Self::build_tax_lots(trades, prices);
        let rules = TaxRules::for_jurisdiction(jurisdiction);
        let today = chrono::Utc::now().date_naive();

        let mut short_term_gains = 0.0;
        let mut short_term_losses = 0.0;
        let mut long_term_gains = 0.0;
        let mut long_term_losses = 0.0;
        let mut lot_summaries = Vec::new();

        for lot in &lots {
            let current_price = if lot.is_closed {
                lot.sale_price_per_share.unwrap_or(0.0)
            } else {
                prices.get(&lot.symbol).copied().unwrap_or(lot.cost_basis_per_share)
            };

            let current_value = lot.shares * current_price;
            let gain_loss = current_value - lot.total_cost_basis;
            let days_held = lot.days_held(today);
            let is_long_term = days_held >= rules.long_term_threshold_days as i64;

            if lot.is_closed {
                if gain_loss >= 0.0 {
                    if is_long_term {
                        long_term_gains += gain_loss;
                    } else {
                        short_term_gains += gain_loss;
                    }
                } else if is_long_term {
                    long_term_losses += gain_loss.abs();
                } else {
                    short_term_losses += gain_loss.abs();
                }
            }

            lot_summaries.push(TaxLotSummary {
                symbol: lot.symbol.clone(),
                shares: lot.shares,
                cost_basis: lot.total_cost_basis,
                current_value,
                gain_loss,
                holding_period: if is_long_term {
                    "long_term".to_string()
                } else {
                    "short_term".to_string()
                },
                days_held,
            });
        }

        let net_short = short_term_gains - short_term_losses;
        let net_long = long_term_gains - long_term_losses;
        let estimated_tax =
            (net_short.max(0.0) * rules.short_term_rate) + (net_long.max(0.0) * rules.long_term_rate);

        TaxSummary {
            jurisdiction: jurisdiction.to_string(),
            short_term_gains,
            short_term_losses,
            long_term_gains,
            long_term_losses,
            net_short_term: net_short,
            net_long_term: net_long,
            estimated_tax,
            lots: lot_summaries,
        }
    }

    /// Estimate tax impact of selling shares at a given price.
    pub fn estimate_tax_impact(
        trades: &[Trade],
        symbol: &str,
        shares: f64,
        price: f64,
        jurisdiction: TaxJurisdiction,
    ) -> TaxImpactEstimate {
        let rules = TaxRules::for_jurisdiction(jurisdiction);
        let today = chrono::Utc::now().date_naive();

        // Build open lots for the symbol
        let lots = Self::build_tax_lots(trades, &HashMap::new());
        let open_lots: Vec<&TaxLot> = lots
            .iter()
            .filter(|l| l.symbol == symbol && !l.is_closed)
            .collect();

        let mut total_cost = 0.0;
        let mut remaining = shares;
        let mut min_days = i64::MAX;
        let mut max_days = 0i64;

        for lot in &open_lots {
            if remaining <= 0.001 {
                break;
            }
            let take = remaining.min(lot.shares);
            total_cost += take * lot.cost_basis_per_share;
            let days = lot.days_held(today);
            min_days = min_days.min(days);
            max_days = max_days.max(days);
            remaining -= take;
        }

        let revenue = shares * price;
        let gain_loss = revenue - total_cost;
        let is_long_term = min_days >= rules.long_term_threshold_days as i64;
        let rate = if is_long_term {
            rules.long_term_rate
        } else {
            rules.short_term_rate
        };
        let tax = if gain_loss > 0.0 {
            gain_loss * rate
        } else {
            gain_loss * rate // Negative = tax savings
        };

        TaxImpactEstimate {
            symbol: symbol.to_string(),
            shares,
            estimated_gain_loss: gain_loss,
            gain_type: if is_long_term {
                "long_term".to_string()
            } else {
                "short_term".to_string()
            },
            estimated_tax: tax,
            effective_rate: rate * 100.0,
            wash_sale_risk: Self::check_wash_sale_risk(trades, symbol, &today.to_string()),
        }
    }

    /// Check if buying a symbol within 30 days of a loss sale would trigger wash sale.
    pub fn check_wash_sale_risk(
        trades: &[Trade],
        symbol: &str,
        proposed_buy_date: &str,
    ) -> bool {
        let proposed = NaiveDate::parse_from_str(proposed_buy_date, "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Utc::now().date_naive());

        for trade in trades {
            if trade.symbol != symbol || trade.action != "sell" {
                continue;
            }
            if let Ok(sell_date) =
                NaiveDate::parse_from_str(&trade.trade_date, "%Y-%m-%d")
            {
                let days_diff = (proposed - sell_date).num_days().abs();
                if days_diff <= 30 {
                    // Check if it was a loss sale
                    if let Some(pnl) = trade.profit_loss {
                        if pnl < 0.0 {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn make_trade(symbol: &str, action: &str, shares: f64, price: f64, date: &str) -> Trade {
        Trade {
            id: Some(1),
            symbol: symbol.to_string(),
            action: action.to_string(),
            shares: Decimal::from_f64(shares).unwrap_or_default(),
            price: Decimal::from_f64(price).unwrap_or_default(),
            trade_date: date.to_string(),
            commission: Decimal::ZERO,
            notes: None,
            profit_loss: None,
            profit_loss_percent: None,
            created_at: None,
        }
    }

    #[test]
    fn test_build_tax_lots() {
        let trades = vec![
            make_trade("AAPL", "buy", 10.0, 150.0, "2024-01-01"),
            make_trade("AAPL", "buy", 5.0, 160.0, "2024-06-01"),
            make_trade("AAPL", "sell", 8.0, 170.0, "2025-03-01"),
        ];
        let prices: HashMap<String, f64> = vec![("AAPL".to_string(), 180.0)].into_iter().collect();
        let lots = TaxBridge::build_tax_lots(&trades, &prices);
        assert!(!lots.is_empty());
        // Should have some closed lots from the sell
        let closed: Vec<_> = lots.iter().filter(|l| l.is_closed).collect();
        assert!(!closed.is_empty());
    }

    #[test]
    fn test_tax_summary() {
        let trades = vec![
            make_trade("AAPL", "buy", 10.0, 100.0, "2024-01-01"),
            make_trade("AAPL", "sell", 10.0, 120.0, "2024-06-01"),
        ];
        let prices: HashMap<String, f64> = vec![("AAPL".to_string(), 130.0)].into_iter().collect();
        let summary = TaxBridge::compute_tax_summary(&trades, &prices, TaxJurisdiction::US);
        assert!(summary.short_term_gains > 0.0 || summary.long_term_gains > 0.0);
    }

    #[test]
    fn test_wash_sale_check() {
        let mut trades = vec![make_trade("AAPL", "sell", 10.0, 90.0, "2025-01-15")];
        trades[0].profit_loss = Some(-100.0); // Loss sale

        assert!(TaxBridge::check_wash_sale_risk(&trades, "AAPL", "2025-01-20"));
        assert!(!TaxBridge::check_wash_sale_risk(&trades, "AAPL", "2025-03-01"));
    }
}
