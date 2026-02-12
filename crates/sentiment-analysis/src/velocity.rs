//! Sentiment Velocity Module
//!
//! Tracks the rate of change in sentiment over time, providing insights into
//! momentum shifts in market sentiment before they fully materialize in price.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the dynamics of sentiment change over time
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SentimentDynamics {
    /// Current absolute sentiment score (-100 to 100)
    pub current_sentiment: f64,
    /// First derivative: rate of change in sentiment
    pub velocity: f64,
    /// Second derivative: acceleration of sentiment change
    pub acceleration: f64,
    /// Detected narrative shift, if any
    pub narrative_shift: Option<NarrativeShift>,
    /// Signal based on velocity analysis
    pub signal: VelocitySignal,
    /// Human-readable interpretation
    pub interpretation: String,
    /// Confidence in the velocity measurement (0-1)
    pub confidence: f64,
}

/// Signal types based on sentiment velocity analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum VelocitySignal {
    /// Sentiment is rapidly improving
    AcceleratingPositive,
    /// Sentiment is rapidly declining
    AcceleratingNegative,
    /// Sentiment change is slowing down
    Decelerating,
    /// Potential reversal point detected
    TurningPoint,
    /// Sentiment is stable with minimal change
    Stable,
}

impl VelocitySignal {
    /// Convert to display string
    pub fn as_str(&self) -> &'static str {
        match self {
            VelocitySignal::AcceleratingPositive => "Accelerating Positive",
            VelocitySignal::AcceleratingNegative => "Accelerating Negative",
            VelocitySignal::Decelerating => "Decelerating",
            VelocitySignal::TurningPoint => "Turning Point",
            VelocitySignal::Stable => "Stable",
        }
    }

    /// Get trading implication
    pub fn trading_implication(&self) -> &'static str {
        match self {
            VelocitySignal::AcceleratingPositive => "Strong buy signal - sentiment momentum building",
            VelocitySignal::AcceleratingNegative => "Strong sell signal - negative momentum building",
            VelocitySignal::Decelerating => "Momentum fading - consider taking profits or waiting",
            VelocitySignal::TurningPoint => "Potential reversal - watch for confirmation",
            VelocitySignal::Stable => "No clear direction - maintain current position",
        }
    }
}

/// Represents a shift in the dominant narrative
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NarrativeShift {
    /// Previous dominant theme
    pub from_theme: String,
    /// New dominant theme
    pub to_theme: String,
    /// Confidence in the detected shift (0-1)
    pub confidence: f64,
    /// When the shift was detected
    pub detected_at: DateTime<Utc>,
}

/// Historical sentiment data point for velocity calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentDataPoint {
    /// Timestamp of the measurement
    pub timestamp: DateTime<Utc>,
    /// Sentiment score at this point
    pub sentiment_score: f64,
    /// Number of articles analyzed
    pub article_count: i32,
    /// Symbol this data is for
    pub symbol: String,
}

/// Calculator for sentiment velocity and acceleration
pub struct SentimentVelocityCalculator {
    /// Minimum data points required for velocity calculation
    min_data_points: usize,
    /// Time window for velocity calculation (in hours)
    velocity_window_hours: i64,
    /// Threshold for considering velocity significant
    velocity_threshold: f64,
    /// Threshold for considering acceleration significant
    acceleration_threshold: f64,
}

impl Default for SentimentVelocityCalculator {
    fn default() -> Self {
        Self {
            min_data_points: 3,
            velocity_window_hours: 72, // 3 days
            velocity_threshold: 0.5,
            acceleration_threshold: 0.3,
        }
    }
}

impl SentimentVelocityCalculator {
    /// Create a new calculator with custom parameters
    pub fn new(
        min_data_points: usize,
        velocity_window_hours: i64,
        velocity_threshold: f64,
        acceleration_threshold: f64,
    ) -> Self {
        Self {
            min_data_points,
            velocity_window_hours,
            velocity_threshold,
            acceleration_threshold,
        }
    }

    /// Calculate sentiment dynamics from historical data
    pub fn calculate(&self, history: &[SentimentDataPoint]) -> SentimentDynamics {
        if history.len() < self.min_data_points {
            return SentimentDynamics {
                current_sentiment: history.last().map(|p| p.sentiment_score).unwrap_or(0.0),
                velocity: 0.0,
                acceleration: 0.0,
                narrative_shift: None,
                signal: VelocitySignal::Stable,
                interpretation: "Insufficient data for velocity analysis".to_string(),
                confidence: 0.0,
            };
        }

        // Sort by timestamp (oldest first)
        let mut sorted: Vec<_> = history.to_vec();
        sorted.sort_by_key(|p| p.timestamp);

        // Get current sentiment (most recent)
        let current_sentiment = sorted.last().map(|p| p.sentiment_score).unwrap_or(0.0);

        // Calculate velocity (first derivative) using linear regression slope
        let velocity = self.calculate_velocity(&sorted);

        // Calculate acceleration (second derivative)
        let acceleration = self.calculate_acceleration(&sorted);

        // Detect narrative shift
        let narrative_shift = self.detect_narrative_shift(&sorted);

        // Determine signal
        let signal = self.determine_signal(velocity, acceleration, &sorted);

        // Calculate confidence based on data quality
        let confidence = self.calculate_confidence(&sorted);

        // Generate interpretation
        let interpretation = self.generate_interpretation(
            current_sentiment,
            velocity,
            acceleration,
            &signal,
        );

        SentimentDynamics {
            current_sentiment,
            velocity,
            acceleration,
            narrative_shift,
            signal,
            interpretation,
            confidence,
        }
    }

    /// Calculate velocity using simple finite difference
    fn calculate_velocity(&self, sorted: &[SentimentDataPoint]) -> f64 {
        if sorted.len() < 2 {
            return 0.0;
        }

        // Use weighted recent changes
        let n = sorted.len();
        let recent_count = (n / 2).max(2).min(5);

        let recent_slice = &sorted[n.saturating_sub(recent_count)..];
        if recent_slice.len() < 2 {
            return 0.0;
        }

        // Calculate average velocity over recent period
        let first = &recent_slice[0];
        let last = &recent_slice[recent_slice.len() - 1];

        let time_diff_hours = (last.timestamp - first.timestamp).num_hours() as f64;
        if time_diff_hours <= 0.0 {
            return 0.0;
        }

        let sentiment_diff = last.sentiment_score - first.sentiment_score;

        // Normalize to per-day velocity
        (sentiment_diff / time_diff_hours) * 24.0
    }

    /// Calculate acceleration (rate of change of velocity)
    fn calculate_acceleration(&self, sorted: &[SentimentDataPoint]) -> f64 {
        if sorted.len() < 3 {
            return 0.0;
        }

        let n = sorted.len();
        let mid = n / 2;

        // Calculate velocity for first half
        let first_half = &sorted[..mid.max(2)];
        let velocity_first = self.calculate_velocity(first_half);

        // Calculate velocity for second half
        let second_half = &sorted[mid.saturating_sub(1)..];
        let velocity_second = self.calculate_velocity(second_half);

        // Acceleration is change in velocity
        let time_span_hours = (sorted[n - 1].timestamp - sorted[0].timestamp).num_hours() as f64;
        if time_span_hours <= 0.0 {
            return 0.0;
        }

        // Normalize acceleration to per-day^2
        ((velocity_second - velocity_first) / time_span_hours) * 24.0
    }

    /// Detect if there's been a narrative shift
    fn detect_narrative_shift(&self, sorted: &[SentimentDataPoint]) -> Option<NarrativeShift> {
        if sorted.len() < 5 {
            return None;
        }

        let n = sorted.len();
        let mid = n / 2;

        // Calculate average sentiment for each half
        let first_half_avg: f64 = sorted[..mid].iter()
            .map(|p| p.sentiment_score)
            .sum::<f64>() / mid as f64;

        let second_half_avg: f64 = sorted[mid..].iter()
            .map(|p| p.sentiment_score)
            .sum::<f64>() / (n - mid) as f64;

        let shift_magnitude = (second_half_avg - first_half_avg).abs();

        // Only report significant shifts (more than 30 points on -100 to 100 scale)
        if shift_magnitude > 30.0 {
            let from_theme = if first_half_avg > 20.0 {
                "Bullish"
            } else if first_half_avg < -20.0 {
                "Bearish"
            } else {
                "Neutral"
            };

            let to_theme = if second_half_avg > 20.0 {
                "Bullish"
            } else if second_half_avg < -20.0 {
                "Bearish"
            } else {
                "Neutral"
            };

            if from_theme != to_theme {
                return Some(NarrativeShift {
                    from_theme: from_theme.to_string(),
                    to_theme: to_theme.to_string(),
                    confidence: (shift_magnitude / 100.0).min(1.0),
                    detected_at: Utc::now(),
                });
            }
        }

        None
    }

    /// Determine the velocity signal
    fn determine_signal(
        &self,
        velocity: f64,
        acceleration: f64,
        sorted: &[SentimentDataPoint],
    ) -> VelocitySignal {
        let abs_velocity = velocity.abs();
        let abs_acceleration = acceleration.abs();

        // Check for turning point (velocity near zero but high acceleration)
        if abs_velocity < self.velocity_threshold && abs_acceleration > self.acceleration_threshold {
            return VelocitySignal::TurningPoint;
        }

        // Check for acceleration
        if velocity > self.velocity_threshold {
            if acceleration > 0.0 {
                return VelocitySignal::AcceleratingPositive;
            } else if acceleration < -self.acceleration_threshold {
                return VelocitySignal::Decelerating;
            }
        }

        if velocity < -self.velocity_threshold {
            if acceleration < 0.0 {
                return VelocitySignal::AcceleratingNegative;
            } else if acceleration > self.acceleration_threshold {
                return VelocitySignal::Decelerating;
            }
        }

        // Check for reversal patterns
        if sorted.len() >= 3 {
            let n = sorted.len();
            let recent_trend = sorted[n - 1].sentiment_score - sorted[n - 2].sentiment_score;
            let previous_trend = sorted[n - 2].sentiment_score - sorted[n - 3].sentiment_score;

            // Sign change indicates potential turning point
            if (recent_trend > 0.0 && previous_trend < 0.0) ||
               (recent_trend < 0.0 && previous_trend > 0.0) {
                if abs_acceleration > self.acceleration_threshold {
                    return VelocitySignal::TurningPoint;
                }
            }
        }

        VelocitySignal::Stable
    }

    /// Calculate confidence in the velocity measurement
    fn calculate_confidence(&self, sorted: &[SentimentDataPoint]) -> f64 {
        let n = sorted.len();

        // Base confidence on number of data points
        let data_confidence = (n as f64 / 10.0).min(1.0);

        // Confidence based on article counts (more articles = more reliable)
        let avg_articles: f64 = sorted.iter()
            .map(|p| p.article_count as f64)
            .sum::<f64>() / n as f64;
        let article_confidence = (avg_articles / 20.0).min(1.0);

        // Confidence based on time span coverage
        if n < 2 {
            return 0.0;
        }
        let time_span_hours = (sorted[n - 1].timestamp - sorted[0].timestamp).num_hours() as f64;
        let time_confidence = (time_span_hours / self.velocity_window_hours as f64).min(1.0);

        // Weight the components
        data_confidence * 0.4 + article_confidence * 0.3 + time_confidence * 0.3
    }

    /// Generate human-readable interpretation
    fn generate_interpretation(
        &self,
        current: f64,
        velocity: f64,
        acceleration: f64,
        signal: &VelocitySignal,
    ) -> String {
        let sentiment_desc = if current > 50.0 {
            "very positive"
        } else if current > 20.0 {
            "positive"
        } else if current > -20.0 {
            "neutral"
        } else if current > -50.0 {
            "negative"
        } else {
            "very negative"
        };

        let velocity_desc = if velocity > 5.0 {
            "rapidly improving"
        } else if velocity > 1.0 {
            "improving"
        } else if velocity > -1.0 {
            "stable"
        } else if velocity > -5.0 {
            "declining"
        } else {
            "rapidly declining"
        };

        let accel_desc = if acceleration > 1.0 {
            " and accelerating"
        } else if acceleration < -1.0 {
            " but decelerating"
        } else {
            ""
        };

        format!(
            "Sentiment is currently {} ({:.1}) and is {}{}. {}",
            sentiment_desc,
            current,
            velocity_desc,
            accel_desc,
            signal.trading_implication()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn create_test_data(scores: &[f64]) -> Vec<SentimentDataPoint> {
        let now = Utc::now();
        scores
            .iter()
            .enumerate()
            .map(|(i, &score)| SentimentDataPoint {
                timestamp: now - Duration::hours((scores.len() - i - 1) as i64 * 12),
                sentiment_score: score,
                article_count: 10,
                symbol: "TEST".to_string(),
            })
            .collect()
    }

    #[test]
    fn test_accelerating_positive() {
        let calculator = SentimentVelocityCalculator::default();
        // Sentiment improving and accelerating: -20, 0, 30, 70
        let data = create_test_data(&[-20.0, 0.0, 30.0, 70.0]);
        let result = calculator.calculate(&data);

        assert!(result.velocity > 0.0, "Velocity should be positive");
        assert!(result.acceleration > 0.0, "Acceleration should be positive");
    }

    #[test]
    fn test_accelerating_negative() {
        let calculator = SentimentVelocityCalculator::default();
        // Sentiment declining and accelerating: 70, 30, 0, -40
        let data = create_test_data(&[70.0, 30.0, 0.0, -40.0]);
        let result = calculator.calculate(&data);

        assert!(result.velocity < 0.0, "Velocity should be negative");
    }

    #[test]
    fn test_turning_point() {
        let calculator = SentimentVelocityCalculator::default();
        // Sentiment bottoming out: -50, -60, -55, -40, -20
        let data = create_test_data(&[-50.0, -60.0, -55.0, -40.0, -20.0]);
        let result = calculator.calculate(&data);

        // Should detect improvement after a bottom
        assert!(result.velocity > 0.0 || result.signal == VelocitySignal::TurningPoint);
    }

    #[test]
    fn test_stable() {
        let calculator = SentimentVelocityCalculator::default();
        // Stable sentiment: 10, 12, 8, 11, 9
        let data = create_test_data(&[10.0, 12.0, 8.0, 11.0, 9.0]);
        let result = calculator.calculate(&data);

        assert!(result.velocity.abs() < 5.0, "Velocity should be near zero for stable sentiment");
    }

    #[test]
    fn test_insufficient_data() {
        let calculator = SentimentVelocityCalculator::default();
        let data = create_test_data(&[10.0, 20.0]); // Only 2 points
        let result = calculator.calculate(&data);

        assert_eq!(result.confidence, 0.0);
        assert!(result.interpretation.contains("Insufficient"));
    }

    #[test]
    fn test_narrative_shift_detection() {
        let calculator = SentimentVelocityCalculator::default();
        // Clear shift from negative to positive
        let data = create_test_data(&[-60.0, -50.0, -40.0, 20.0, 40.0, 60.0]);
        let result = calculator.calculate(&data);

        assert!(result.narrative_shift.is_some(), "Should detect narrative shift");
        if let Some(shift) = result.narrative_shift {
            assert_eq!(shift.from_theme, "Bearish");
            assert_eq!(shift.to_theme, "Bullish");
        }
    }
}
