use crate::models::*;
use crate::shared_math;
use rust_decimal::prelude::*;
use std::collections::HashMap;

pub struct RiskCalculator;

impl RiskCalculator {
    /// Compute portfolio-level risk metrics from snapshots and current positions.
    pub fn compute(
        snapshots: &[PortfolioSnapshot],
        positions: &[PositionWithPnL],
        sector_map: &HashMap<String, String>,
    ) -> PortfolioRiskMetrics {
        let values: Vec<f64> = snapshots
            .iter()
            .map(|s| s.total_value.to_f64().unwrap_or(0.0))
            .collect();

        let returns = shared_math::daily_returns(&values);
        let (max_dd, current_dd) = shared_math::max_drawdown(&values);

        // Concentration from positions
        let total_value: f64 = positions
            .iter()
            .map(|p| p.market_value.to_f64().unwrap_or(0.0))
            .sum();

        let mut holding_weights: Vec<HoldingWeight> = positions
            .iter()
            .map(|p| {
                let mv = p.market_value.to_f64().unwrap_or(0.0);
                HoldingWeight {
                    symbol: p.position.symbol.clone(),
                    weight_percent: if total_value > 0.0 {
                        mv / total_value * 100.0
                    } else {
                        0.0
                    },
                    market_value: mv,
                }
            })
            .collect();
        holding_weights.sort_by(|a, b| {
            b.weight_percent
                .partial_cmp(&a.weight_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let weights_frac: Vec<f64> = holding_weights
            .iter()
            .map(|h| h.weight_percent / 100.0)
            .collect();

        // Sector weights
        let mut sector_weights: HashMap<String, f64> = HashMap::new();
        for p in positions {
            let sector = sector_map
                .get(&p.position.symbol)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            let mv = p.market_value.to_f64().unwrap_or(0.0);
            *sector_weights.entry(sector).or_insert(0.0) += mv;
        }
        if total_value > 0.0 {
            for v in sector_weights.values_mut() {
                *v = *v / total_value * 100.0;
            }
        }

        PortfolioRiskMetrics {
            sharpe_ratio: shared_math::sharpe_ratio(&returns, 0.02),
            sortino_ratio: shared_math::sortino_ratio(&returns, 0.02),
            max_drawdown_percent: max_dd * 100.0,
            current_drawdown_percent: current_dd * 100.0,
            rolling_volatility_20d: shared_math::rolling_volatility(&returns, 20),
            rolling_volatility_63d: shared_math::rolling_volatility(&returns, 63),
            var_95: shared_math::var_historical(&returns, 0.95),
            cvar_95: shared_math::cvar_historical(&returns, 0.95),
            herfindahl_index: shared_math::herfindahl_index(&weights_frac),
            top_holdings: holding_weights.into_iter().take(10).collect(),
            sector_weights,
            data_points: snapshots.len(),
        }
    }

    /// Compute performance analytics from snapshots, positions, and trades.
    pub fn compute_performance(
        snapshots: &[PortfolioSnapshot],
        positions: &[PositionWithPnL],
        _trades: &[Trade],
    ) -> PerformanceAnalytics {
        if snapshots.is_empty() {
            return PerformanceAnalytics {
                total_return_percent: 0.0,
                twr_percent: None,
                rolling_30d_return: None,
                rolling_90d_return: None,
                ytd_return: None,
                rolling_1y_return: None,
                monthly_returns: Vec::new(),
                symbol_attribution: Vec::new(),
                data_points: 0,
            };
        }

        let values: Vec<f64> = snapshots
            .iter()
            .map(|s| s.total_value.to_f64().unwrap_or(0.0))
            .collect();
        let dates: Vec<String> = snapshots.iter().map(|s| s.snapshot_date.clone()).collect();

        let first = values[0];
        let last = values[values.len() - 1];
        let total_return = if first > 0.0 {
            (last - first) / first * 100.0
        } else {
            0.0
        };

        // TWR: geometric chain-linking of daily returns
        let daily = shared_math::daily_returns(&values);
        let twr = if !daily.is_empty() {
            let product: f64 = daily.iter().map(|r| 1.0 + r).product();
            Some((product - 1.0) * 100.0)
        } else {
            None
        };

        // Rolling returns
        let rolling_30d = rolling_return(&values, 30);
        let rolling_90d = rolling_return(&values, 90);
        let rolling_1y = rolling_return(&values, 252);

        // YTD
        let ytd = ytd_return(&dates, &values);

        // Monthly returns
        let monthly_raw = shared_math::monthly_returns(&dates, &values);
        let monthly: Vec<MonthlyReturn> = monthly_raw
            .into_iter()
            .map(|(y, m, r)| MonthlyReturn {
                year: y,
                month: m,
                return_percent: r,
            })
            .collect();

        // Symbol attribution
        let total_value: f64 = positions
            .iter()
            .map(|p| p.market_value.to_f64().unwrap_or(0.0))
            .sum();

        let attribution: Vec<SymbolAttribution> = positions
            .iter()
            .map(|p| {
                let mv = p.market_value.to_f64().unwrap_or(0.0);
                let cost = p.cost_basis.to_f64().unwrap_or(0.0);
                let weight = if total_value > 0.0 {
                    mv / total_value * 100.0
                } else {
                    0.0
                };
                let symbol_return = if cost > 0.0 {
                    (mv - cost) / cost * 100.0
                } else {
                    0.0
                };
                SymbolAttribution {
                    symbol: p.position.symbol.clone(),
                    weight_percent: weight,
                    return_percent: symbol_return,
                    contribution_percent: weight * symbol_return / 100.0,
                }
            })
            .collect();

        PerformanceAnalytics {
            total_return_percent: total_return,
            twr_percent: twr,
            rolling_30d_return: rolling_30d,
            rolling_90d_return: rolling_90d,
            ytd_return: ytd,
            rolling_1y_return: rolling_1y,
            monthly_returns: monthly,
            symbol_attribution: attribution,
            data_points: snapshots.len(),
        }
    }
}

fn rolling_return(values: &[f64], days: usize) -> Option<f64> {
    if values.len() < days + 1 {
        return None;
    }
    let start = values[values.len() - days - 1];
    let end = values[values.len() - 1];
    if start > 0.0 {
        Some((end - start) / start * 100.0)
    } else {
        None
    }
}

fn ytd_return(dates: &[String], values: &[f64]) -> Option<f64> {
    if dates.is_empty() || values.is_empty() {
        return None;
    }
    let current_year = dates
        .last()?
        .split(['-', 'T'])
        .next()?
        .parse::<i32>()
        .ok()?;

    // Find first value of the current year
    for (i, date) in dates.iter().enumerate() {
        let year: i32 = date
            .split(['-', 'T'])
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if year == current_year {
            let start = values[i];
            let end = values[values.len() - 1];
            return if start > 0.0 {
                Some((end - start) / start * 100.0)
            } else {
                None
            };
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn make_snapshot(value: f64, date: &str) -> PortfolioSnapshot {
        PortfolioSnapshot {
            id: None,
            total_value: Decimal::from_f64(value).unwrap_or_default(),
            total_cost: Decimal::from_f64(value * 0.9).unwrap_or_default(),
            total_pnl: Decimal::from_f64(value * 0.1).unwrap_or_default(),
            total_pnl_percent: 10.0,
            snapshot_date: date.to_string(),
        }
    }

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
    fn test_risk_metrics_basic() {
        let snapshots: Vec<PortfolioSnapshot> = (0..30)
            .map(|i| {
                make_snapshot(
                    10000.0 + (i as f64 * 50.0),
                    &format!("2025-01-{:02}", i + 1),
                )
            })
            .collect();
        let positions = vec![
            make_position("AAPL", 10.0, 150.0, 160.0),
            make_position("MSFT", 5.0, 300.0, 320.0),
        ];
        let sector_map: HashMap<String, String> = vec![
            ("AAPL".to_string(), "Technology".to_string()),
            ("MSFT".to_string(), "Technology".to_string()),
        ]
        .into_iter()
        .collect();

        let metrics = RiskCalculator::compute(&snapshots, &positions, &sector_map);
        assert!(metrics.max_drawdown_percent >= 0.0);
        assert_eq!(metrics.top_holdings.len(), 2);
        assert!(metrics.sector_weights.contains_key("Technology"));
    }

    #[test]
    fn test_performance_analytics() {
        let snapshots: Vec<PortfolioSnapshot> = (0..60)
            .map(|i| {
                make_snapshot(
                    10000.0 + (i as f64 * 30.0),
                    &format!("2025-{:02}-{:02}", i / 28 + 1, (i % 28) + 1),
                )
            })
            .collect();
        let positions = vec![make_position("AAPL", 10.0, 150.0, 160.0)];

        let analytics = RiskCalculator::compute_performance(&snapshots, &positions, &[]);
        assert!(analytics.total_return_percent > 0.0);
        assert!(analytics.twr_percent.is_some());
    }
}
