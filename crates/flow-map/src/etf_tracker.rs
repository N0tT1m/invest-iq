//! Sector ETF Tracking
//!
//! Tracks sector ETFs to estimate money flows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Standard sector ETFs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorETF {
    pub symbol: String,
    pub name: String,
    pub sector: String,
    pub expense_ratio: f64,
}

impl SectorETF {
    /// Get the standard SPDR sector ETFs
    pub fn standard_sectors() -> Vec<SectorETF> {
        vec![
            SectorETF {
                symbol: "XLK".to_string(),
                name: "Technology Select Sector".to_string(),
                sector: "Technology".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLF".to_string(),
                name: "Financial Select Sector".to_string(),
                sector: "Financials".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLV".to_string(),
                name: "Health Care Select Sector".to_string(),
                sector: "Healthcare".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLE".to_string(),
                name: "Energy Select Sector".to_string(),
                sector: "Energy".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLY".to_string(),
                name: "Consumer Discretionary Select Sector".to_string(),
                sector: "Consumer Discretionary".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLP".to_string(),
                name: "Consumer Staples Select Sector".to_string(),
                sector: "Consumer Staples".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLI".to_string(),
                name: "Industrial Select Sector".to_string(),
                sector: "Industrials".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLB".to_string(),
                name: "Materials Select Sector".to_string(),
                sector: "Materials".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLU".to_string(),
                name: "Utilities Select Sector".to_string(),
                sector: "Utilities".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLRE".to_string(),
                name: "Real Estate Select Sector".to_string(),
                sector: "Real Estate".to_string(),
                expense_ratio: 0.10,
            },
            SectorETF {
                symbol: "XLC".to_string(),
                name: "Communication Services Select Sector".to_string(),
                sector: "Communication Services".to_string(),
                expense_ratio: 0.10,
            },
        ]
    }

    /// Get color for sector
    pub fn sector_color(sector: &str) -> &'static str {
        match sector {
            "Technology" => "#00ccff",
            "Financials" => "#00cc88",
            "Healthcare" => "#ff6699",
            "Energy" => "#ff9933",
            "Consumer Discretionary" => "#9966ff",
            "Consumer Staples" => "#66cc99",
            "Industrials" => "#cc9933",
            "Materials" => "#999999",
            "Utilities" => "#ffcc00",
            "Real Estate" => "#cc6666",
            "Communication Services" => "#6699ff",
            _ => "#888888",
        }
    }
}

/// Performance data for an ETF
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ETFPerformance {
    pub symbol: String,
    pub sector: String,
    pub current_price: f64,
    pub change_1d: f64,
    pub change_1w: f64,
    pub change_1m: f64,
    pub change_3m: f64,
    pub volume: f64,
    pub avg_volume: f64,
    pub relative_volume: f64,
    pub updated_at: DateTime<Utc>,
}

impl ETFPerformance {
    /// Calculate momentum score
    pub fn momentum_score(&self) -> f64 {
        // Weighted average of timeframes
        self.change_1d * 0.1 + self.change_1w * 0.3 + self.change_1m * 0.4 + self.change_3m * 0.2
    }

    /// Check if volume is elevated
    pub fn has_elevated_volume(&self) -> bool {
        self.relative_volume > 1.5
    }
}

/// Tracks sector ETF performance
pub struct SectorETFTracker {
    etfs: Vec<SectorETF>,
    performance_cache: HashMap<String, ETFPerformance>,
}

impl Default for SectorETFTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SectorETFTracker {
    /// Create a new tracker with standard sector ETFs
    pub fn new() -> Self {
        Self {
            etfs: SectorETF::standard_sectors(),
            performance_cache: HashMap::new(),
        }
    }

    /// Get ETF symbols to track
    pub fn symbols(&self) -> Vec<String> {
        self.etfs.iter().map(|e| e.symbol.clone()).collect()
    }

    /// Update performance data
    pub fn update_performance(&mut self, symbol: &str, performance: ETFPerformance) {
        self.performance_cache
            .insert(symbol.to_string(), performance);
    }

    /// Get sector performance ranking
    pub fn get_ranking(&self) -> Vec<(String, f64)> {
        let mut ranking: Vec<_> = self
            .performance_cache
            .values()
            .map(|p| (p.sector.clone(), p.momentum_score()))
            .collect();

        ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        ranking
    }

    /// Get relative strength for a sector
    pub fn relative_strength(&self, sector: &str) -> f64 {
        let ranking = self.get_ranking();
        if ranking.is_empty() {
            return 50.0;
        }

        let position = ranking.iter().position(|(s, _)| s == sector);
        match position {
            Some(pos) => 100.0 - (pos as f64 / ranking.len() as f64) * 100.0,
            None => 50.0,
        }
    }

    /// Identify sectors with momentum
    pub fn sectors_with_momentum(&self, threshold: f64) -> Vec<String> {
        self.performance_cache
            .values()
            .filter(|p| p.momentum_score() > threshold)
            .map(|p| p.sector.clone())
            .collect()
    }

    /// Identify sectors losing momentum
    pub fn sectors_losing_momentum(&self, threshold: f64) -> Vec<String> {
        self.performance_cache
            .values()
            .filter(|p| p.momentum_score() < threshold)
            .map(|p| p.sector.clone())
            .collect()
    }

    /// Get sector for a symbol
    pub fn get_sector(&self, symbol: &str) -> Option<String> {
        self.etfs
            .iter()
            .find(|e| e.symbol == symbol)
            .map(|e| e.sector.clone())
    }

    /// Get all ETF info
    pub fn get_etfs(&self) -> &[SectorETF] {
        &self.etfs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_sectors() {
        let etfs = SectorETF::standard_sectors();
        assert_eq!(etfs.len(), 11); // 11 GICS sectors
        assert!(etfs.iter().any(|e| e.symbol == "XLK"));
    }

    #[test]
    fn test_momentum_score() {
        let perf = ETFPerformance {
            symbol: "XLK".to_string(),
            sector: "Technology".to_string(),
            current_price: 200.0,
            change_1d: 1.0,
            change_1w: 3.0,
            change_1m: 5.0,
            change_3m: 10.0,
            volume: 1_000_000.0,
            avg_volume: 800_000.0,
            relative_volume: 1.25,
            updated_at: Utc::now(),
        };

        let score = perf.momentum_score();
        assert!(score > 0.0);
    }

    #[test]
    fn test_tracker() {
        let tracker = SectorETFTracker::new();
        let symbols = tracker.symbols();

        assert!(!symbols.is_empty());
        assert!(symbols.contains(&"XLK".to_string()));
    }
}
