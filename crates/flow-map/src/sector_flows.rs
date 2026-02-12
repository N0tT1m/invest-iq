//! Sector Flow Calculations
//!
//! Calculates money flow between market sectors.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A flow of capital from one sector to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorFlow {
    /// Source sector
    pub from_sector: String,
    /// Destination sector
    pub to_sector: String,
    /// Estimated flow amount in millions
    pub flow_amount: f64,
    /// Flow as percentage of source sector
    pub flow_percentage: f64,
    /// Confidence in the flow estimate
    pub confidence: f64,
    /// Intensity of the flow (for visualization)
    pub intensity: f64,
}

impl SectorFlow {
    /// Check if this is a significant flow
    pub fn is_significant(&self) -> bool {
        self.flow_percentage.abs() > 1.0 && self.confidence > 0.5
    }
}

/// A sector node in the flow map
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorNode {
    /// Sector name
    pub name: String,
    /// Representative ETF symbol
    pub etf_symbol: String,
    /// Net flow (positive = inflow, negative = outflow)
    pub net_flow: f64,
    /// 1-day performance
    pub performance_1d: f64,
    /// 1-week performance
    pub performance_1w: f64,
    /// 1-month performance
    pub performance_1m: f64,
    /// Current relative strength
    pub relative_strength: f64,
    /// Momentum score
    pub momentum: f64,
    /// Color for visualization
    pub color: String,
}

impl SectorNode {
    /// Get flow direction
    pub fn flow_direction(&self) -> FlowDirection {
        if self.net_flow > 1.0 {
            FlowDirection::Inflow
        } else if self.net_flow < -1.0 {
            FlowDirection::Outflow
        } else {
            FlowDirection::Neutral
        }
    }
}

/// Direction of flow for a sector
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowDirection {
    Inflow,
    Outflow,
    Neutral,
}

/// Complete flow map data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowMapData {
    /// All sector nodes
    pub sectors: Vec<SectorNode>,
    /// All flows between sectors
    pub flows: Vec<SectorFlow>,
    /// Timeframe for the data
    pub timeframe: String,
    /// Dominant rotation pattern
    pub dominant_rotation: Option<String>,
    /// When the data was generated
    pub generated_at: DateTime<Utc>,
    /// Overall market trend
    pub market_trend: MarketTrend,
}

/// Overall market trend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketTrend {
    RiskOn,
    RiskOff,
    SectorRotation,
    Mixed,
}

impl FlowMapData {
    /// Create flow map from sector performance data
    pub fn from_performance(performances: &[(SectorNode, f64)]) -> Self {
        let mut sectors: Vec<SectorNode> = performances.iter().map(|(s, _)| s.clone()).collect();
        let mut flows = Vec::new();

        // Calculate relative strength rankings
        let mut ranked: Vec<_> = performances.iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Estimate flows based on relative performance changes
        for (i, (from_sector, from_perf)) in ranked.iter().enumerate() {
            for (to_sector, to_perf) in ranked.iter().skip(i + 1) {
                let perf_diff = to_perf - from_perf;

                if perf_diff.abs() > 0.5 {
                    // Significant performance difference suggests flow
                    let flow_pct = perf_diff.abs() * 0.1; // Rough estimate
                    let confidence = (perf_diff.abs() / 5.0).min(1.0);

                    // Flow from weaker (lower ranked) to stronger (higher ranked)
                    flows.push(SectorFlow {
                        from_sector: to_sector.name.clone(),
                        to_sector: from_sector.name.clone(),
                        flow_amount: flow_pct * 100.0,
                        flow_percentage: flow_pct,
                        confidence,
                        intensity: (flow_pct / 2.0).min(1.0),
                    });
                }
            }
        }

        // Calculate net flows for each sector
        for sector in &mut sectors {
            let inflows: f64 = flows
                .iter()
                .filter(|f| f.to_sector == sector.name)
                .map(|f| f.flow_percentage)
                .sum();
            let outflows: f64 = flows
                .iter()
                .filter(|f| f.from_sector == sector.name)
                .map(|f| f.flow_percentage)
                .sum();
            sector.net_flow = inflows - outflows;
        }

        // Determine dominant rotation
        let dominant_rotation = Self::detect_dominant_rotation(&sectors, &flows);

        // Determine market trend
        let market_trend = Self::detect_market_trend(&sectors);

        FlowMapData {
            sectors,
            flows,
            timeframe: "1W".to_string(),
            dominant_rotation,
            generated_at: Utc::now(),
            market_trend,
        }
    }

    fn detect_dominant_rotation(sectors: &[SectorNode], _flows: &[SectorFlow]) -> Option<String> {
        // Look for patterns
        let tech_inflow = sectors
            .iter()
            .any(|s| s.name == "Technology" && s.net_flow > 2.0);
        let value_inflow = sectors
            .iter()
            .any(|s| (s.name == "Financials" || s.name == "Energy") && s.net_flow > 2.0);
        let defensive_inflow = sectors
            .iter()
            .any(|s| (s.name == "Utilities" || s.name == "Consumer Staples") && s.net_flow > 2.0);

        if tech_inflow && !value_inflow {
            Some("Value → Growth".to_string())
        } else if value_inflow && !tech_inflow {
            Some("Growth → Value".to_string())
        } else if defensive_inflow {
            Some("Risk-Off Rotation".to_string())
        } else {
            None
        }
    }

    fn detect_market_trend(sectors: &[SectorNode]) -> MarketTrend {
        let defensive_sectors = ["Utilities", "Consumer Staples", "Healthcare"];
        let cyclical_sectors = ["Technology", "Consumer Discretionary", "Financials"];

        let defensive_avg: f64 = sectors
            .iter()
            .filter(|s| defensive_sectors.contains(&s.name.as_str()))
            .map(|s| s.performance_1w)
            .sum::<f64>()
            / 3.0;

        let cyclical_avg: f64 = sectors
            .iter()
            .filter(|s| cyclical_sectors.contains(&s.name.as_str()))
            .map(|s| s.performance_1w)
            .sum::<f64>()
            / 3.0;

        if cyclical_avg > defensive_avg + 1.0 {
            MarketTrend::RiskOn
        } else if defensive_avg > cyclical_avg + 1.0 {
            MarketTrend::RiskOff
        } else {
            MarketTrend::Mixed
        }
    }

    /// Get the strongest inflow sectors
    pub fn strongest_inflows(&self, n: usize) -> Vec<&SectorNode> {
        let mut sorted: Vec<_> = self.sectors.iter().collect();
        sorted.sort_by(|a, b| b.net_flow.partial_cmp(&a.net_flow).unwrap());
        sorted.into_iter().take(n).collect()
    }

    /// Get the strongest outflow sectors
    pub fn strongest_outflows(&self, n: usize) -> Vec<&SectorNode> {
        let mut sorted: Vec<_> = self.sectors.iter().collect();
        sorted.sort_by(|a, b| a.net_flow.partial_cmp(&b.net_flow).unwrap());
        sorted.into_iter().take(n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_sector(name: &str, perf_1w: f64) -> SectorNode {
        SectorNode {
            name: name.to_string(),
            etf_symbol: format!("X{}", &name[..2].to_uppercase()),
            net_flow: 0.0,
            performance_1d: perf_1w / 5.0,
            performance_1w: perf_1w,
            performance_1m: perf_1w * 2.0,
            relative_strength: 50.0 + perf_1w * 5.0,
            momentum: perf_1w,
            color: "#00cc88".to_string(),
        }
    }

    #[test]
    fn test_flow_map_creation() {
        let performances = vec![
            (create_test_sector("Technology", 3.0), 3.0),
            (create_test_sector("Financials", -2.0), -2.0),
            (create_test_sector("Healthcare", 0.5), 0.5),
        ];

        let flow_map = FlowMapData::from_performance(&performances);

        assert_eq!(flow_map.sectors.len(), 3);
        assert!(!flow_map.flows.is_empty());
    }

    #[test]
    fn test_flow_significance() {
        let flow = SectorFlow {
            from_sector: "Financials".to_string(),
            to_sector: "Technology".to_string(),
            flow_amount: 50.0,
            flow_percentage: 2.0,
            confidence: 0.8,
            intensity: 0.5,
        };

        assert!(flow.is_significant());
    }
}
