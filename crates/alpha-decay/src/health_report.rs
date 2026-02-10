//! Strategy Health Reporting
//!
//! Generates comprehensive health reports for trading strategies
//! including decay status, recommendations, and alerts.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::change_detector::{ChangeState, CusumResult};
use crate::monitor::DecayMetrics;

/// Health status classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Strategy is performing well
    Healthy,
    /// Some degradation detected but still viable
    Degrading,
    /// Significant degradation, needs attention
    Critical,
    /// Strategy should be retired
    Retired,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "Healthy",
            HealthStatus::Degrading => "Degrading",
            HealthStatus::Critical => "Critical",
            HealthStatus::Retired => "Retired",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "#00cc88",
            HealthStatus::Degrading => "#ffaa00",
            HealthStatus::Critical => "#ff6600",
            HealthStatus::Retired => "#888888",
        }
    }

    pub fn from_decay_metrics(metrics: &DecayMetrics) -> Self {
        if metrics.current_sharpe < 0.0 {
            HealthStatus::Critical
        } else if metrics.decay_from_peak_pct > 50.0 || metrics.days_to_breakeven.map(|d| d < 30).unwrap_or(false) {
            HealthStatus::Critical
        } else if metrics.is_decaying && metrics.decay_from_peak_pct > 25.0 {
            HealthStatus::Degrading
        } else if metrics.is_decaying {
            HealthStatus::Degrading
        } else {
            HealthStatus::Healthy
        }
    }
}

/// Complete health report for a strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub strategy_name: String,
    pub status: HealthStatus,
    pub health_score: f64,
    pub decay_metrics: DecayMetrics,
    pub change_analysis: ChangeAnalysisSummary,
    pub recommendations: Vec<String>,
    pub alerts: Vec<DecayAlert>,
    pub generated_at: DateTime<Utc>,
}

/// Summary of change detection analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeAnalysisSummary {
    pub current_state: ChangeState,
    pub recent_change_points: usize,
    pub cusum_trending: CusumTrend,
    pub stability_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CusumTrend {
    StrongPositive,
    Positive,
    Neutral,
    Negative,
    StrongNegative,
}

impl CusumTrend {
    pub fn from_cusum(result: &CusumResult) -> Self {
        let n = result.upper_cusum.len();
        if n < 10 {
            return CusumTrend::Neutral;
        }

        // Look at recent CUSUM values
        let recent_start = n - (n / 5).max(5);
        let recent_upper_avg: f64 = result.upper_cusum[recent_start..].iter().sum::<f64>()
            / (n - recent_start) as f64;
        let recent_lower_avg: f64 = result.lower_cusum[recent_start..].iter().sum::<f64>()
            / (n - recent_start) as f64;

        let threshold = result.threshold;

        if recent_lower_avg > threshold * 0.8 {
            CusumTrend::StrongNegative
        } else if recent_lower_avg > threshold * 0.4 {
            CusumTrend::Negative
        } else if recent_upper_avg > threshold * 0.8 {
            CusumTrend::StrongPositive
        } else if recent_upper_avg > threshold * 0.4 {
            CusumTrend::Positive
        } else {
            CusumTrend::Neutral
        }
    }
}

/// An alert about strategy health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayAlert {
    pub severity: AlertSeverity,
    pub category: AlertCategory,
    pub message: String,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold_value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertCategory {
    SharpeDecay,
    WinRateDecay,
    DrawdownIncrease,
    RegimeChange,
    BreakevenRisk,
}

/// Detailed health assessment for a strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyHealth {
    pub strategy_name: String,
    pub current_sharpe: f64,
    pub historical_sharpe: f64,
    pub decay_percentage: f64,
    pub time_to_breakeven: Option<Duration>,
    pub status: HealthStatus,
    pub recommendation: String,
}

/// Builder for generating health reports
pub struct HealthReportBuilder {
    strategy_name: String,
    decay_metrics: Option<DecayMetrics>,
    cusum_result: Option<CusumResult>,
}

impl HealthReportBuilder {
    pub fn new(strategy_name: &str) -> Self {
        Self {
            strategy_name: strategy_name.to_string(),
            decay_metrics: None,
            cusum_result: None,
        }
    }

    pub fn with_decay_metrics(mut self, metrics: DecayMetrics) -> Self {
        self.decay_metrics = Some(metrics);
        self
    }

    pub fn with_cusum_result(mut self, result: CusumResult) -> Self {
        self.cusum_result = Some(result);
        self
    }

    pub fn build(mut self) -> HealthReport {
        let decay_metrics = self.decay_metrics.take().unwrap_or(DecayMetrics {
            strategy_name: self.strategy_name.clone(),
            current_sharpe: 0.0,
            peak_sharpe: 0.0,
            avg_sharpe: 0.0,
            decay_from_peak_pct: 0.0,
            decay_from_avg_pct: 0.0,
            trend_per_day: 0.0,
            days_to_breakeven: None,
            is_decaying: false,
        });

        let status = HealthStatus::from_decay_metrics(&decay_metrics);
        let health_score = self.calculate_health_score(&decay_metrics, &status);
        let alerts = self.generate_alerts(&decay_metrics);
        let recommendations = self.generate_recommendations(&decay_metrics, &status);

        let change_analysis = if let Some(ref cusum) = self.cusum_result {
            ChangeAnalysisSummary {
                current_state: cusum.current_state,
                recent_change_points: cusum.change_points.len(),
                cusum_trending: CusumTrend::from_cusum(cusum),
                stability_score: self.calculate_stability_score(cusum),
            }
        } else {
            ChangeAnalysisSummary {
                current_state: ChangeState::Stable,
                recent_change_points: 0,
                cusum_trending: CusumTrend::Neutral,
                stability_score: 50.0,
            }
        };

        HealthReport {
            strategy_name: self.strategy_name,
            status,
            health_score,
            decay_metrics,
            change_analysis,
            recommendations,
            alerts,
            generated_at: Utc::now(),
        }
    }

    fn calculate_health_score(&self, metrics: &DecayMetrics, status: &HealthStatus) -> f64 {
        let base_score = match status {
            HealthStatus::Healthy => 80.0,
            HealthStatus::Degrading => 50.0,
            HealthStatus::Critical => 25.0,
            HealthStatus::Retired => 0.0,
        };

        // Adjust based on Sharpe
        let sharpe_bonus = (metrics.current_sharpe * 10.0).clamp(-20.0, 20.0);

        // Penalty for decay
        let decay_penalty = metrics.decay_from_peak_pct.min(30.0) * 0.5;

        // Penalty for imminent breakeven
        let breakeven_penalty = metrics
            .days_to_breakeven
            .map(|d| if d < 30 { 20.0 } else if d < 90 { 10.0 } else { 0.0 })
            .unwrap_or(0.0);

        (base_score + sharpe_bonus - decay_penalty - breakeven_penalty).clamp(0.0, 100.0)
    }

    fn calculate_stability_score(&self, cusum: &CusumResult) -> f64 {
        // Fewer change points = more stable
        let change_point_penalty = (cusum.change_points.len() as f64 * 10.0).min(50.0);

        // Lower recent CUSUM = more stable
        let n = cusum.upper_cusum.len();
        if n == 0 {
            return 50.0;
        }

        let recent = &cusum.lower_cusum[n.saturating_sub(10)..];
        let recent_avg = recent.iter().sum::<f64>() / recent.len().max(1) as f64;
        let cusum_penalty = (recent_avg / cusum.threshold * 30.0).min(30.0);

        (100.0 - change_point_penalty - cusum_penalty).max(0.0)
    }

    fn generate_alerts(&self, metrics: &DecayMetrics) -> Vec<DecayAlert> {
        let mut alerts = Vec::new();

        // Sharpe decay alert
        if metrics.decay_from_peak_pct > 25.0 {
            alerts.push(DecayAlert {
                severity: if metrics.decay_from_peak_pct > 50.0 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                },
                category: AlertCategory::SharpeDecay,
                message: format!(
                    "Sharpe ratio has declined {:.1}% from peak",
                    metrics.decay_from_peak_pct
                ),
                metric_name: "sharpe_decay".to_string(),
                current_value: metrics.current_sharpe,
                threshold_value: metrics.peak_sharpe * 0.75,
            });
        }

        // Breakeven risk alert
        if let Some(days) = metrics.days_to_breakeven {
            if days < 90 {
                alerts.push(DecayAlert {
                    severity: if days < 30 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    category: AlertCategory::BreakevenRisk,
                    message: format!(
                        "At current decay rate, strategy will reach breakeven in {} days",
                        days
                    ),
                    metric_name: "days_to_breakeven".to_string(),
                    current_value: days as f64,
                    threshold_value: 90.0,
                });
            }
        }

        // Negative Sharpe alert
        if metrics.current_sharpe < 0.0 {
            alerts.push(DecayAlert {
                severity: AlertSeverity::Critical,
                category: AlertCategory::SharpeDecay,
                message: "Strategy has negative Sharpe ratio".to_string(),
                metric_name: "current_sharpe".to_string(),
                current_value: metrics.current_sharpe,
                threshold_value: 0.0,
            });
        }

        // Active decay alert
        if metrics.is_decaying && metrics.trend_per_day < -0.01 {
            alerts.push(DecayAlert {
                severity: AlertSeverity::Warning,
                category: AlertCategory::SharpeDecay,
                message: format!(
                    "Strategy is actively decaying at {:.3} Sharpe per day",
                    metrics.trend_per_day.abs()
                ),
                metric_name: "trend_per_day".to_string(),
                current_value: metrics.trend_per_day,
                threshold_value: -0.01,
            });
        }

        alerts
    }

    fn generate_recommendations(&self, metrics: &DecayMetrics, status: &HealthStatus) -> Vec<String> {
        let mut recs = Vec::new();

        match status {
            HealthStatus::Healthy => {
                recs.push("Strategy is performing well. Continue monitoring.".to_string());
                if metrics.current_sharpe > metrics.avg_sharpe * 1.2 {
                    recs.push("Performance is above historical average - consider increasing allocation.".to_string());
                }
            }
            HealthStatus::Degrading => {
                recs.push("Performance is declining. Monitor closely for further degradation.".to_string());
                recs.push("Consider reducing position sizes to limit exposure.".to_string());
                if metrics.decay_from_peak_pct > 30.0 {
                    recs.push("Review strategy parameters for potential optimization.".to_string());
                }
            }
            HealthStatus::Critical => {
                recs.push("Strategy requires immediate attention.".to_string());
                recs.push("Consider pausing new trades until performance stabilizes.".to_string());
                recs.push("Evaluate whether market conditions have changed fundamentally.".to_string());
                if metrics.days_to_breakeven.map(|d| d < 30).unwrap_or(false) {
                    recs.push("URGENT: Strategy may become unprofitable within 30 days.".to_string());
                }
            }
            HealthStatus::Retired => {
                recs.push("Strategy should be retired or completely overhauled.".to_string());
                recs.push("Close remaining positions in an orderly manner.".to_string());
                recs.push("Analyze what changed to learn for future strategies.".to_string());
            }
        }

        recs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_from_metrics() {
        let healthy = DecayMetrics {
            strategy_name: "test".to_string(),
            current_sharpe: 1.5,
            peak_sharpe: 1.6,
            avg_sharpe: 1.4,
            decay_from_peak_pct: 6.25,
            decay_from_avg_pct: 0.0,
            trend_per_day: 0.001,
            days_to_breakeven: None,
            is_decaying: false,
        };
        assert_eq!(HealthStatus::from_decay_metrics(&healthy), HealthStatus::Healthy);

        let degrading = DecayMetrics {
            strategy_name: "test".to_string(),
            current_sharpe: 0.8,
            peak_sharpe: 1.6,
            avg_sharpe: 1.2,
            decay_from_peak_pct: 50.0,
            decay_from_avg_pct: 33.0,
            trend_per_day: -0.01,
            days_to_breakeven: Some(80),
            is_decaying: true,
        };
        assert_eq!(HealthStatus::from_decay_metrics(&degrading), HealthStatus::Critical);

        let critical = DecayMetrics {
            strategy_name: "test".to_string(),
            current_sharpe: -0.2,
            peak_sharpe: 1.5,
            avg_sharpe: 1.0,
            decay_from_peak_pct: 113.0,
            decay_from_avg_pct: 120.0,
            trend_per_day: -0.02,
            days_to_breakeven: None,
            is_decaying: true,
        };
        assert_eq!(HealthStatus::from_decay_metrics(&critical), HealthStatus::Critical);
    }

    #[test]
    fn test_health_report_builder() {
        let metrics = DecayMetrics {
            strategy_name: "momentum".to_string(),
            current_sharpe: 0.9,
            peak_sharpe: 1.5,
            avg_sharpe: 1.2,
            decay_from_peak_pct: 40.0,
            decay_from_avg_pct: 25.0,
            trend_per_day: -0.005,
            days_to_breakeven: Some(180),
            is_decaying: true,
        };

        let report = HealthReportBuilder::new("momentum")
            .with_decay_metrics(metrics)
            .build();

        assert_eq!(report.strategy_name, "momentum");
        assert!(!report.alerts.is_empty());
        assert!(!report.recommendations.is_empty());
    }
}
