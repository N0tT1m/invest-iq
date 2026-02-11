//! Risk Radar Module
//!
//! Multi-dimensional risk analysis for portfolios and individual positions.
//! Provides a comprehensive view of risk across multiple dimensions.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Multi-dimensional risk profile
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RiskRadar {
    /// Market risk (beta, correlation to SPY) - 0 to 100
    pub market_risk: f64,
    /// Volatility risk (ATR, realized vol) - 0 to 100
    pub volatility_risk: f64,
    /// Liquidity risk (bid-ask spread, volume) - 0 to 100
    pub liquidity_risk: f64,
    /// Event risk (earnings proximity, dividends) - 0 to 100
    pub event_risk: f64,
    /// Concentration risk (position size, sector exposure) - 0 to 100
    pub concentration_risk: f64,
    /// Sentiment risk (news volatility, social heat) - 0 to 100
    pub sentiment_risk: f64,
}

impl RiskRadar {
    /// Create a new risk radar with all dimensions at moderate level
    pub fn moderate() -> Self {
        Self {
            market_risk: 50.0,
            volatility_risk: 50.0,
            liquidity_risk: 30.0,
            event_risk: 30.0,
            concentration_risk: 50.0,
            sentiment_risk: 50.0,
        }
    }

    /// Calculate overall risk score (weighted average)
    pub fn overall_score(&self) -> f64 {
        // Weights for each dimension
        let weights = [0.20, 0.25, 0.10, 0.15, 0.15, 0.15]; // Sum = 1.0
        let values = [
            self.market_risk,
            self.volatility_risk,
            self.liquidity_risk,
            self.event_risk,
            self.concentration_risk,
            self.sentiment_risk,
        ];

        values.iter().zip(weights.iter()).map(|(v, w)| v * w).sum()
    }

    /// Get the risk level classification
    pub fn risk_level(&self) -> RiskLevel {
        let score = self.overall_score();
        RiskLevel::from_score(score)
    }

    /// Get the highest risk dimension
    pub fn highest_risk_dimension(&self) -> (&'static str, f64) {
        let dimensions = [
            ("Market Risk", self.market_risk),
            ("Volatility", self.volatility_risk),
            ("Liquidity", self.liquidity_risk),
            ("Event Risk", self.event_risk),
            ("Concentration", self.concentration_risk),
            ("Sentiment", self.sentiment_risk),
        ];

        dimensions
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(("unknown", 0.0))
    }

    /// Get all dimensions as a vector for charting
    pub fn to_values(&self) -> Vec<f64> {
        vec![
            self.market_risk,
            self.volatility_risk,
            self.liquidity_risk,
            self.event_risk,
            self.concentration_risk,
            self.sentiment_risk,
        ]
    }

    /// Get dimension labels
    pub fn dimension_labels() -> Vec<&'static str> {
        vec![
            "Market Risk",
            "Volatility",
            "Liquidity",
            "Event Risk",
            "Concentration",
            "Sentiment",
        ]
    }
}

/// Risk level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Moderate,
    Elevated,
    High,
    Critical,
}

impl RiskLevel {
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s < 25.0 => RiskLevel::Low,
            s if s < 45.0 => RiskLevel::Moderate,
            s if s < 65.0 => RiskLevel::Elevated,
            s if s < 80.0 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "Low",
            RiskLevel::Moderate => "Moderate",
            RiskLevel::Elevated => "Elevated",
            RiskLevel::High => "High",
            RiskLevel::Critical => "Critical",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            RiskLevel::Low => "#00cc88",
            RiskLevel::Moderate => "#00ccff",
            RiskLevel::Elevated => "#ffaa00",
            RiskLevel::High => "#ff6600",
            RiskLevel::Critical => "#ff0000",
        }
    }
}

/// Target risk profile for comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskTargetProfile {
    pub user_id: String,
    pub target: RiskRadar,
    pub updated_at: DateTime<Utc>,
}

impl Default for RiskTargetProfile {
    fn default() -> Self {
        Self {
            user_id: "default".to_string(),
            target: RiskRadar {
                market_risk: 50.0,
                volatility_risk: 50.0,
                liquidity_risk: 30.0,
                event_risk: 40.0,
                concentration_risk: 40.0,
                sentiment_risk: 50.0,
            },
            updated_at: Utc::now(),
        }
    }
}

/// Risk alert when a dimension exceeds target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    pub dimension: String,
    pub current_value: f64,
    pub target_value: f64,
    pub severity: AlertSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Complete risk profile with current, target, and alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskProfile {
    pub current: RiskRadar,
    pub target: Option<RiskRadar>,
    pub alerts: Vec<RiskAlert>,
    pub overall_score: f64,
    pub risk_level: RiskLevel,
    pub generated_at: DateTime<Utc>,
}

impl RiskProfile {
    /// Create a new risk profile
    pub fn new(current: RiskRadar, target: Option<RiskRadar>) -> Self {
        let alerts = if let Some(ref t) = target {
            Self::generate_alerts(&current, t)
        } else {
            Vec::new()
        };

        let overall_score = current.overall_score();
        let risk_level = current.risk_level();

        Self {
            current,
            target,
            alerts,
            overall_score,
            risk_level,
            generated_at: Utc::now(),
        }
    }

    fn generate_alerts(current: &RiskRadar, target: &RiskRadar) -> Vec<RiskAlert> {
        let mut alerts = Vec::new();

        let checks = [
            ("Market Risk", current.market_risk, target.market_risk),
            ("Volatility", current.volatility_risk, target.volatility_risk),
            ("Liquidity", current.liquidity_risk, target.liquidity_risk),
            ("Event Risk", current.event_risk, target.event_risk),
            ("Concentration", current.concentration_risk, target.concentration_risk),
            ("Sentiment", current.sentiment_risk, target.sentiment_risk),
        ];

        for (name, current_val, target_val) in checks {
            let excess = current_val - target_val;

            if excess > 20.0 {
                alerts.push(RiskAlert {
                    dimension: name.to_string(),
                    current_value: current_val,
                    target_value: target_val,
                    severity: AlertSeverity::Critical,
                    message: format!(
                        "{} is {:.0} points above target. Consider reducing exposure.",
                        name, excess
                    ),
                });
            } else if excess > 10.0 {
                alerts.push(RiskAlert {
                    dimension: name.to_string(),
                    current_value: current_val,
                    target_value: target_val,
                    severity: AlertSeverity::Warning,
                    message: format!(
                        "{} is {:.0} points above target. Monitor closely.",
                        name, excess
                    ),
                });
            }
        }

        alerts
    }
}

/// Calculator for risk radar from market data
pub struct RiskRadarCalculator;

impl RiskRadarCalculator {
    /// Calculate market risk from beta and correlation
    pub fn calculate_market_risk(beta: f64, spy_correlation: f64) -> f64 {
        // Beta contribution (50% weight)
        // Beta > 2 = 100, Beta < 0.5 = 0, Linear between
        let beta_score = ((beta - 0.5) / 1.5 * 100.0).clamp(0.0, 100.0);

        // Correlation contribution (50% weight)
        // Higher correlation = higher market risk
        let corr_score = spy_correlation.abs() * 100.0;

        beta_score * 0.5 + corr_score * 0.5
    }

    /// Calculate volatility risk from realized volatility and ATR
    pub fn calculate_volatility_risk(annualized_vol: f64, atr_percent: f64) -> f64 {
        // Annualized vol contribution
        // > 60% = 100, < 10% = 0
        let vol_score = ((annualized_vol - 0.10) / 0.50 * 100.0).clamp(0.0, 100.0);

        // ATR percent contribution (daily range as % of price)
        // > 5% = 100, < 0.5% = 0
        let atr_score = ((atr_percent - 0.005) / 0.045 * 100.0).clamp(0.0, 100.0);

        vol_score * 0.6 + atr_score * 0.4
    }

    /// Calculate liquidity risk from volume and spread
    pub fn calculate_liquidity_risk(avg_volume: f64, spread_percent: f64) -> f64 {
        // Volume contribution (inverted - low volume = high risk)
        // < 100k = 100, > 10M = 0
        let vol_log = (avg_volume.max(1.0)).log10();
        let volume_score = ((7.0 - vol_log) / 2.0 * 100.0).clamp(0.0, 100.0);

        // Spread contribution
        // > 1% = 100, < 0.01% = 0
        let spread_score = ((spread_percent - 0.0001) / 0.01 * 100.0).clamp(0.0, 100.0);

        volume_score * 0.5 + spread_score * 0.5
    }

    /// Calculate event risk based on upcoming events
    pub fn calculate_event_risk(
        days_to_earnings: Option<i32>,
        days_to_dividend: Option<i32>,
        has_pending_fda: bool,
    ) -> f64 {
        let mut score: f64 = 20.0; // Base event risk

        // Earnings proximity
        if let Some(days) = days_to_earnings {
            if days <= 3 {
                score += 40.0;
            } else if days <= 7 {
                score += 25.0;
            } else if days <= 14 {
                score += 10.0;
            }
        }

        // Dividend proximity
        if let Some(days) = days_to_dividend {
            if days <= 3 {
                score += 15.0;
            } else if days <= 7 {
                score += 8.0;
            }
        }

        // FDA/major pending events
        if has_pending_fda {
            score += 30.0;
        }

        score.min(100.0)
    }

    /// Calculate concentration risk
    pub fn calculate_concentration_risk(
        position_weight: f64,
        sector_weight: f64,
        num_positions: usize,
    ) -> f64 {
        // Position weight contribution
        // > 25% = 100, < 2% = 0
        let position_score = ((position_weight - 0.02) / 0.23 * 100.0).clamp(0.0, 100.0);

        // Sector weight contribution
        // > 50% = 100, < 10% = 0
        let sector_score = ((sector_weight - 0.10) / 0.40 * 100.0).clamp(0.0, 100.0);

        // Diversification (inverted - fewer positions = higher risk)
        // < 5 = 100, > 20 = 0
        let diversity_score = ((20 - num_positions as i32).max(0) as f64 / 15.0 * 100.0)
            .clamp(0.0, 100.0);

        position_score * 0.4 + sector_score * 0.3 + diversity_score * 0.3
    }

    /// Calculate sentiment risk
    pub fn calculate_sentiment_risk(
        sentiment_volatility: f64,
        confidence: f64,
        article_count: i32,
    ) -> f64 {
        // Sentiment volatility (how much sentiment changes)
        let volatility_score = (sentiment_volatility * 100.0).clamp(0.0, 100.0);

        // Confidence (inverted - low confidence = high risk)
        let confidence_score = ((1.0 - confidence) * 100.0).clamp(0.0, 100.0);

        // Article count (few articles = higher uncertainty)
        // < 5 = 100, > 50 = 0
        let coverage_score = ((50 - article_count).max(0) as f64 / 45.0 * 100.0).clamp(0.0, 100.0);

        volatility_score * 0.4 + confidence_score * 0.4 + coverage_score * 0.2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_radar_overall_score() {
        let radar = RiskRadar {
            market_risk: 60.0,
            volatility_risk: 70.0,
            liquidity_risk: 20.0,
            event_risk: 40.0,
            concentration_risk: 50.0,
            sentiment_risk: 45.0,
        };

        let score = radar.overall_score();
        assert!(score > 40.0 && score < 60.0, "Score should be moderate range");
    }

    #[test]
    fn test_risk_level_classification() {
        assert_eq!(RiskLevel::from_score(20.0), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(40.0), RiskLevel::Moderate);
        assert_eq!(RiskLevel::from_score(60.0), RiskLevel::Elevated);
        assert_eq!(RiskLevel::from_score(75.0), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(90.0), RiskLevel::Critical);
    }

    #[test]
    fn test_highest_risk_dimension() {
        let radar = RiskRadar {
            market_risk: 30.0,
            volatility_risk: 80.0, // Highest
            liquidity_risk: 20.0,
            event_risk: 40.0,
            concentration_risk: 50.0,
            sentiment_risk: 45.0,
        };

        let (name, value) = radar.highest_risk_dimension();
        assert_eq!(name, "Volatility");
        assert_eq!(value, 80.0);
    }

    #[test]
    fn test_market_risk_calculation() {
        // High beta, high correlation
        let risk = RiskRadarCalculator::calculate_market_risk(1.8, 0.9);
        assert!(risk > 70.0);

        // Low beta, low correlation
        let risk = RiskRadarCalculator::calculate_market_risk(0.6, 0.2);
        assert!(risk < 30.0);
    }

    #[test]
    fn test_event_risk_with_earnings() {
        // Earnings tomorrow
        let risk = RiskRadarCalculator::calculate_event_risk(Some(1), None, false);
        assert!(risk > 50.0);

        // No upcoming events
        let risk = RiskRadarCalculator::calculate_event_risk(None, None, false);
        assert!(risk < 30.0);

        // FDA pending
        let risk = RiskRadarCalculator::calculate_event_risk(None, None, true);
        assert!(risk > 40.0);
    }

    #[test]
    fn test_alert_generation() {
        let current = RiskRadar {
            market_risk: 80.0, // Way above target
            volatility_risk: 50.0,
            liquidity_risk: 30.0,
            event_risk: 40.0,
            concentration_risk: 50.0,
            sentiment_risk: 50.0,
        };

        let target = RiskRadar::moderate();
        let profile = RiskProfile::new(current, Some(target));

        assert!(!profile.alerts.is_empty());
        assert!(profile.alerts.iter().any(|a| a.dimension == "Market Risk"));
    }
}
