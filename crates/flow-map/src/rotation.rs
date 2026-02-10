//! Rotation Detection
//!
//! Detects sector rotation patterns in the market.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type of rotation pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationType {
    /// Growth to Value rotation
    GrowthToValue,
    /// Value to Growth rotation
    ValueToGrowth,
    /// Cyclical to Defensive rotation (risk-off)
    CyclicalToDefensive,
    /// Defensive to Cyclical rotation (risk-on)
    DefensiveToCyclical,
    /// Large Cap to Small Cap
    LargeToSmall,
    /// Small Cap to Large Cap
    SmallToLarge,
    /// US to International
    DomesticToInternational,
    /// International to US
    InternationalToDomestic,
    /// No clear rotation
    None,
}

impl RotationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RotationType::GrowthToValue => "Growth → Value",
            RotationType::ValueToGrowth => "Value → Growth",
            RotationType::CyclicalToDefensive => "Risk-Off (Cyclical → Defensive)",
            RotationType::DefensiveToCyclical => "Risk-On (Defensive → Cyclical)",
            RotationType::LargeToSmall => "Large Cap → Small Cap",
            RotationType::SmallToLarge => "Small Cap → Large Cap",
            RotationType::DomesticToInternational => "US → International",
            RotationType::InternationalToDomestic => "International → US",
            RotationType::None => "No Clear Rotation",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            RotationType::GrowthToValue => {
                "Money flowing from high-growth tech stocks to value/dividend stocks"
            }
            RotationType::ValueToGrowth => {
                "Money flowing from value/dividend stocks to high-growth tech"
            }
            RotationType::CyclicalToDefensive => {
                "Risk-off move: investors seeking safety in utilities, staples, healthcare"
            }
            RotationType::DefensiveToCyclical => {
                "Risk-on move: investors seeking growth in financials, industrials, tech"
            }
            RotationType::LargeToSmall => {
                "Rotation from mega-caps to small/mid-cap stocks"
            }
            RotationType::SmallToLarge => {
                "Flight to safety in large, established companies"
            }
            RotationType::DomesticToInternational => {
                "Capital flowing from US markets to international"
            }
            RotationType::InternationalToDomestic => {
                "Capital flowing back to US markets"
            }
            RotationType::None => "No significant rotation pattern detected",
        }
    }

    pub fn is_risk_on(&self) -> bool {
        matches!(
            self,
            RotationType::ValueToGrowth | RotationType::DefensiveToCyclical | RotationType::LargeToSmall
        )
    }

    pub fn is_risk_off(&self) -> bool {
        matches!(
            self,
            RotationType::GrowthToValue | RotationType::CyclicalToDefensive | RotationType::SmallToLarge
        )
    }
}

/// A detected rotation pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationPattern {
    /// Type of rotation
    pub rotation_type: RotationType,
    /// Confidence in the detection (0-1)
    pub confidence: f64,
    /// When the rotation started (approximate)
    pub start_date: Option<DateTime<Utc>>,
    /// Duration in days
    pub duration_days: Option<i32>,
    /// Sectors gaining
    pub gaining_sectors: Vec<String>,
    /// Sectors losing
    pub losing_sectors: Vec<String>,
    /// Strength of the rotation (0-100)
    pub strength: f64,
    /// Is the rotation still active?
    pub is_active: bool,
}

/// Sector classification for rotation analysis
#[derive(Debug, Clone)]
struct SectorClassification {
    growth_sectors: Vec<String>,
    value_sectors: Vec<String>,
    cyclical_sectors: Vec<String>,
    defensive_sectors: Vec<String>,
}

impl Default for SectorClassification {
    fn default() -> Self {
        Self {
            growth_sectors: vec![
                "Technology".to_string(),
                "Communication Services".to_string(),
                "Consumer Discretionary".to_string(),
            ],
            value_sectors: vec![
                "Financials".to_string(),
                "Energy".to_string(),
                "Materials".to_string(),
            ],
            cyclical_sectors: vec![
                "Technology".to_string(),
                "Financials".to_string(),
                "Industrials".to_string(),
                "Consumer Discretionary".to_string(),
                "Energy".to_string(),
                "Materials".to_string(),
            ],
            defensive_sectors: vec![
                "Utilities".to_string(),
                "Consumer Staples".to_string(),
                "Healthcare".to_string(),
                "Real Estate".to_string(),
            ],
        }
    }
}

/// Detects rotation patterns
pub struct RotationDetector {
    classification: SectorClassification,
    /// Minimum performance difference to trigger detection
    threshold: f64,
}

impl Default for RotationDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RotationDetector {
    pub fn new() -> Self {
        Self {
            classification: SectorClassification::default(),
            threshold: 2.0, // 2% difference
        }
    }

    /// Detect rotation from sector performance data
    pub fn detect(&self, sector_performance: &[(String, f64)]) -> Vec<RotationPattern> {
        let mut patterns = Vec::new();

        // Check Growth vs Value
        if let Some(pattern) = self.check_growth_value(sector_performance) {
            patterns.push(pattern);
        }

        // Check Cyclical vs Defensive
        if let Some(pattern) = self.check_cyclical_defensive(sector_performance) {
            patterns.push(pattern);
        }

        patterns
    }

    fn check_growth_value(&self, perf: &[(String, f64)]) -> Option<RotationPattern> {
        let growth_avg = self.average_performance(perf, &self.classification.growth_sectors);
        let value_avg = self.average_performance(perf, &self.classification.value_sectors);

        let diff = growth_avg - value_avg;

        if diff.abs() < self.threshold {
            return None;
        }

        let rotation_type = if diff > 0.0 {
            RotationType::ValueToGrowth
        } else {
            RotationType::GrowthToValue
        };

        let (gaining, losing) = if diff > 0.0 {
            (&self.classification.growth_sectors, &self.classification.value_sectors)
        } else {
            (&self.classification.value_sectors, &self.classification.growth_sectors)
        };

        Some(RotationPattern {
            rotation_type,
            confidence: (diff.abs() / 10.0).min(1.0),
            start_date: None,
            duration_days: None,
            gaining_sectors: gaining.clone(),
            losing_sectors: losing.clone(),
            strength: diff.abs().min(100.0),
            is_active: true,
        })
    }

    fn check_cyclical_defensive(&self, perf: &[(String, f64)]) -> Option<RotationPattern> {
        let cyclical_avg = self.average_performance(perf, &self.classification.cyclical_sectors);
        let defensive_avg = self.average_performance(perf, &self.classification.defensive_sectors);

        let diff = cyclical_avg - defensive_avg;

        if diff.abs() < self.threshold {
            return None;
        }

        let rotation_type = if diff > 0.0 {
            RotationType::DefensiveToCyclical
        } else {
            RotationType::CyclicalToDefensive
        };

        let (gaining, losing) = if diff > 0.0 {
            (&self.classification.cyclical_sectors, &self.classification.defensive_sectors)
        } else {
            (&self.classification.defensive_sectors, &self.classification.cyclical_sectors)
        };

        Some(RotationPattern {
            rotation_type,
            confidence: (diff.abs() / 10.0).min(1.0),
            start_date: None,
            duration_days: None,
            gaining_sectors: gaining.clone(),
            losing_sectors: losing.clone(),
            strength: diff.abs().min(100.0),
            is_active: true,
        })
    }

    fn average_performance(&self, perf: &[(String, f64)], sectors: &[String]) -> f64 {
        let relevant: Vec<f64> = perf
            .iter()
            .filter(|(s, _)| sectors.contains(s))
            .map(|(_, p)| *p)
            .collect();

        if relevant.is_empty() {
            0.0
        } else {
            relevant.iter().sum::<f64>() / relevant.len() as f64
        }
    }

    /// Get the primary rotation pattern
    pub fn primary_rotation<'a>(&self, patterns: &'a [RotationPattern]) -> Option<&'a RotationPattern> {
        patterns.iter().max_by(|a, b| a.strength.partial_cmp(&b.strength).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotation_type_properties() {
        assert!(RotationType::ValueToGrowth.is_risk_on());
        assert!(RotationType::CyclicalToDefensive.is_risk_off());
        assert!(!RotationType::None.is_risk_on());
    }

    #[test]
    fn test_rotation_detection() {
        let detector = RotationDetector::new();

        let perf = vec![
            ("Technology".to_string(), 5.0),
            ("Communication Services".to_string(), 4.0),
            ("Financials".to_string(), -2.0),
            ("Energy".to_string(), -1.5),
            ("Utilities".to_string(), 0.5),
        ];

        let patterns = detector.detect(&perf);
        assert!(!patterns.is_empty());

        // Should detect growth outperforming
        let growth_pattern = patterns
            .iter()
            .find(|p| p.rotation_type == RotationType::ValueToGrowth);
        assert!(growth_pattern.is_some());
    }
}
