use crate::models::*;
use rust_decimal::prelude::*;
use std::collections::HashMap;

pub struct RebalanceCalculator;

impl RebalanceCalculator {
    /// Calculate trades needed to rebalance to target allocations.
    pub fn calculate(
        positions: &[PositionWithPnL],
        targets: &[TargetAllocation],
        total_value: Decimal,
        prices: &HashMap<String, f64>,
        _sector_map: &HashMap<String, String>,
    ) -> RebalanceProposal {
        let total_f64 = total_value.to_f64().unwrap_or(0.0);
        if total_f64 <= 0.0 {
            return RebalanceProposal {
                total_portfolio_value: 0.0,
                trades: Vec::new(),
                estimated_turnover_percent: 0.0,
            };
        }

        let mut trades = Vec::new();
        let mut turnover = 0.0;

        // Build current weights by symbol
        let mut current_values: HashMap<String, f64> = HashMap::new();
        for p in positions {
            let mv = p.market_value.to_f64().unwrap_or(0.0);
            *current_values.entry(p.position.symbol.clone()).or_insert(0.0) += mv;
        }

        // Process symbol-level targets
        for target in targets.iter().filter(|t| t.symbol.is_some()) {
            let symbol = target.symbol.as_ref().unwrap();
            let current_value = current_values.get(symbol).copied().unwrap_or(0.0);
            let current_weight = current_value / total_f64 * 100.0;
            let target_value = total_f64 * target.target_weight_percent / 100.0;
            let diff_value = target_value - current_value;

            if diff_value.abs() < 1.0 {
                continue;
            }

            let price = prices.get(symbol).copied().unwrap_or(0.0);
            if price <= 0.0 {
                continue;
            }

            let shares = (diff_value / price).abs();
            let action = if diff_value > 0.0 { "buy" } else { "sell" };

            turnover += diff_value.abs();

            trades.push(RebalanceTrade {
                symbol: symbol.clone(),
                action: action.to_string(),
                shares,
                current_weight_percent: current_weight,
                target_weight_percent: target.target_weight_percent,
                estimated_value: diff_value.abs(),
            });
        }

        let turnover_pct = turnover / total_f64 * 100.0;

        RebalanceProposal {
            total_portfolio_value: total_f64,
            trades,
            estimated_turnover_percent: turnover_pct,
        }
    }

    /// Compute current drift from target allocations.
    pub fn compute_drift(
        positions: &[PositionWithPnL],
        targets: &[TargetAllocation],
        total_value: Decimal,
        sector_map: &HashMap<String, String>,
    ) -> Vec<DriftEntry> {
        let total_f64 = total_value.to_f64().unwrap_or(0.0);
        if total_f64 <= 0.0 {
            return Vec::new();
        }

        let mut current_symbol_weights: HashMap<String, f64> = HashMap::new();
        let mut current_sector_weights: HashMap<String, f64> = HashMap::new();

        for p in positions {
            let mv = p.market_value.to_f64().unwrap_or(0.0);
            let weight = mv / total_f64 * 100.0;
            *current_symbol_weights
                .entry(p.position.symbol.clone())
                .or_insert(0.0) += weight;
            let sector = sector_map
                .get(&p.position.symbol)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            *current_sector_weights.entry(sector).or_insert(0.0) += weight;
        }

        let mut entries = Vec::new();

        for target in targets {
            let (current_weight, is_symbol) = if let Some(sym) = &target.symbol {
                (
                    current_symbol_weights.get(sym).copied().unwrap_or(0.0),
                    true,
                )
            } else if let Some(sec) = &target.sector {
                (
                    current_sector_weights.get(sec).copied().unwrap_or(0.0),
                    false,
                )
            } else {
                continue;
            };

            let drift = current_weight - target.target_weight_percent;
            let needs_rebalance = drift.abs() > target.drift_tolerance_percent;

            entries.push(DriftEntry {
                symbol: if is_symbol {
                    target.symbol.clone()
                } else {
                    None
                },
                sector: target.sector.clone(),
                target_weight_percent: target.target_weight_percent,
                current_weight_percent: current_weight,
                drift_percent: drift,
                tolerance_percent: target.drift_tolerance_percent,
                needs_rebalance,
            });
        }

        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn make_position(symbol: &str, shares: f64, entry: f64, current: f64) -> PositionWithPnL {
        let mv = shares * current;
        let cost = shares * entry;
        PositionWithPnL {
            position: Position {
                id: Some(1),
                symbol: symbol.to_string(),
                shares: Decimal::from_f64(shares).unwrap_or_default(),
                entry_price: Decimal::from_f64(entry).unwrap_or_default(),
                entry_date: "2025-01-01".to_string(),
                notes: None,
                created_at: None,
            },
            current_price: Decimal::from_f64(current).unwrap_or_default(),
            market_value: Decimal::from_f64(mv).unwrap_or_default(),
            cost_basis: Decimal::from_f64(cost).unwrap_or_default(),
            unrealized_pnl: Decimal::from_f64(mv - cost).unwrap_or_default(),
            unrealized_pnl_percent: if cost > 0.0 {
                (mv - cost) / cost * 100.0
            } else {
                0.0
            },
        }
    }

    #[test]
    fn test_rebalance_calculation() {
        let positions = vec![
            make_position("AAPL", 10.0, 150.0, 160.0), // 1600
            make_position("MSFT", 5.0, 300.0, 320.0),   // 1600
        ];
        let total = Decimal::from(3200);

        let targets = vec![
            TargetAllocation {
                id: None,
                symbol: Some("AAPL".to_string()),
                sector: None,
                target_weight_percent: 60.0,
                drift_tolerance_percent: 5.0,
                updated_at: None,
            },
            TargetAllocation {
                id: None,
                symbol: Some("MSFT".to_string()),
                sector: None,
                target_weight_percent: 40.0,
                drift_tolerance_percent: 5.0,
                updated_at: None,
            },
        ];

        let prices: HashMap<String, f64> =
            vec![("AAPL".to_string(), 160.0), ("MSFT".to_string(), 320.0)]
                .into_iter()
                .collect();
        let sector_map = HashMap::new();

        let proposal =
            RebalanceCalculator::calculate(&positions, &targets, total, &prices, &sector_map);
        assert_eq!(proposal.total_portfolio_value, 3200.0);
        // AAPL is 50% but target is 60%, MSFT is 50% but target is 40%
        // Both exceed 5% tolerance
        assert!(!proposal.trades.is_empty());
    }

    #[test]
    fn test_drift_computation() {
        let positions = vec![
            make_position("AAPL", 10.0, 150.0, 160.0),
            make_position("MSFT", 5.0, 300.0, 320.0),
        ];
        let total = Decimal::from(3200);

        let targets = vec![TargetAllocation {
            id: None,
            symbol: Some("AAPL".to_string()),
            sector: None,
            target_weight_percent: 60.0,
            drift_tolerance_percent: 5.0,
            updated_at: None,
        }];

        let sector_map = HashMap::new();
        let drift =
            RebalanceCalculator::compute_drift(&positions, &targets, total, &sector_map);
        assert_eq!(drift.len(), 1);
        // AAPL is at 50%, target 60% → drift = -10%
        assert!(drift[0].drift_percent < 0.0);
        assert!(drift[0].needs_rebalance);
    }
}
