//! Change Detection Algorithms
//!
//! Implements CUSUM and other algorithms for detecting
//! performance regime changes in trading strategies.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Result of CUSUM analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CusumResult {
    /// Upper CUSUM values (detecting upward shifts)
    pub upper_cusum: Vec<f64>,
    /// Lower CUSUM values (detecting downward shifts)
    pub lower_cusum: Vec<f64>,
    /// Threshold for triggering alert
    pub threshold: f64,
    /// Detected change points
    pub change_points: Vec<ChangePoint>,
    /// Current state
    pub current_state: ChangeState,
}

/// A detected change point in the time series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePoint {
    pub index: usize,
    pub date: Option<NaiveDate>,
    pub direction: ChangeDirection,
    pub cusum_value: f64,
    pub confidence: f64,
}

/// Direction of the detected change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeDirection {
    Increase,
    Decrease,
}

/// Current state of the change detector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeState {
    /// No significant change detected
    Stable,
    /// Performance is improving
    Improving,
    /// Performance is degrading (alpha decay detected)
    Degrading,
    /// Recent change point detected, still confirming
    Transitioning,
}

/// Change detector using various algorithms
pub struct ChangeDetector {
    /// Allowable slack before triggering (k parameter)
    slack: f64,
    /// Threshold for detecting change (h parameter)
    threshold: f64,
    /// Window size for local mean estimation
    window_size: usize,
}

impl Default for ChangeDetector {
    fn default() -> Self {
        Self::new(0.5, 5.0, 20)
    }
}

impl ChangeDetector {
    /// Create a new change detector
    ///
    /// # Arguments
    /// * `slack` - Allowable slack (k), typically 0.5 for normal detection
    /// * `threshold` - Decision threshold (h), typically 4-5 for ARL of ~400-1000
    /// * `window_size` - Window for estimating mean/std
    pub fn new(slack: f64, threshold: f64, window_size: usize) -> Self {
        Self {
            slack,
            threshold,
            window_size,
        }
    }

    /// Run CUSUM analysis on a time series
    pub fn cusum_analysis(&self, values: &[f64], dates: Option<&[NaiveDate]>) -> CusumResult {
        if values.len() < self.window_size {
            return CusumResult {
                upper_cusum: vec![0.0; values.len()],
                lower_cusum: vec![0.0; values.len()],
                threshold: self.threshold,
                change_points: Vec::new(),
                current_state: ChangeState::Stable,
            };
        }

        // Calculate baseline mean and std from initial window
        let baseline: &[f64] = &values[..self.window_size];
        let mean = baseline.iter().sum::<f64>() / baseline.len() as f64;
        let variance = baseline.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
            / (baseline.len() - 1) as f64;
        let std = variance.sqrt().max(0.001); // Avoid division by zero

        // Standardize values
        let standardized: Vec<f64> = values.iter().map(|x| (x - mean) / std).collect();

        // Calculate CUSUM
        let mut upper = vec![0.0; values.len()];
        let mut lower = vec![0.0; values.len()];
        let mut change_points = Vec::new();

        for i in 1..values.len() {
            // Upper CUSUM (detecting increase)
            upper[i] = (upper[i - 1] + standardized[i] - self.slack).max(0.0);

            // Lower CUSUM (detecting decrease)
            lower[i] = (lower[i - 1] - standardized[i] - self.slack).max(0.0);

            // Check for change points
            if upper[i] > self.threshold {
                let date = dates.map(|d| d.get(i).copied()).flatten();
                change_points.push(ChangePoint {
                    index: i,
                    date,
                    direction: ChangeDirection::Increase,
                    cusum_value: upper[i],
                    confidence: self.calculate_confidence(upper[i]),
                });
                // Reset after detection
                upper[i] = 0.0;
            }

            if lower[i] > self.threshold {
                let date = dates.map(|d| d.get(i).copied()).flatten();
                change_points.push(ChangePoint {
                    index: i,
                    date,
                    direction: ChangeDirection::Decrease,
                    cusum_value: lower[i],
                    confidence: self.calculate_confidence(lower[i]),
                });
                // Reset after detection
                lower[i] = 0.0;
            }
        }

        // Determine current state
        let current_state = self.determine_state(&upper, &lower, &change_points);

        CusumResult {
            upper_cusum: upper,
            lower_cusum: lower,
            threshold: self.threshold,
            change_points,
            current_state,
        }
    }

    /// Calculate confidence based on how much CUSUM exceeds threshold
    fn calculate_confidence(&self, cusum_value: f64) -> f64 {
        let excess = (cusum_value - self.threshold) / self.threshold;
        (0.5 + 0.5 * (1.0 - (-excess).exp())).min(0.99)
    }

    /// Determine the current state based on CUSUM values
    fn determine_state(
        &self,
        upper: &[f64],
        lower: &[f64],
        change_points: &[ChangePoint],
    ) -> ChangeState {
        let n = upper.len();
        if n == 0 {
            return ChangeState::Stable;
        }

        // Look at recent values (last 10%)
        let recent_start = (n as f64 * 0.9) as usize;
        let recent_upper_avg: f64 = upper[recent_start..].iter().sum::<f64>()
            / (n - recent_start) as f64;
        let recent_lower_avg: f64 = lower[recent_start..].iter().sum::<f64>()
            / (n - recent_start) as f64;

        // Check for recent change points (last 20%)
        let recent_threshold = (n as f64 * 0.8) as usize;
        let recent_changes: Vec<_> = change_points
            .iter()
            .filter(|cp| cp.index >= recent_threshold)
            .collect();

        if !recent_changes.is_empty() {
            return ChangeState::Transitioning;
        }

        // Determine based on CUSUM trend
        let high_threshold = self.threshold * 0.5;

        if recent_lower_avg > high_threshold {
            ChangeState::Degrading
        } else if recent_upper_avg > high_threshold {
            ChangeState::Improving
        } else {
            ChangeState::Stable
        }
    }

    /// Run exponentially weighted moving average (EWMA) control chart
    pub fn ewma_analysis(&self, values: &[f64], lambda: f64) -> EwmaResult {
        if values.is_empty() {
            return EwmaResult {
                ewma: Vec::new(),
                upper_control_limit: Vec::new(),
                lower_control_limit: Vec::new(),
                out_of_control: Vec::new(),
            };
        }

        // Calculate baseline statistics
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
            / (values.len() - 1) as f64;
        let std = variance.sqrt().max(0.001);

        // Calculate EWMA and control limits
        let mut ewma = vec![mean; values.len()];
        let mut ucl = Vec::with_capacity(values.len());
        let mut lcl = Vec::with_capacity(values.len());
        let mut ooc = Vec::new();

        let l = 3.0; // Number of standard deviations for limits

        for i in 0..values.len() {
            if i == 0 {
                ewma[i] = values[i];
            } else {
                ewma[i] = lambda * values[i] + (1.0 - lambda) * ewma[i - 1];
            }

            // Control limits widen with i, then stabilize
            let limit_factor = (lambda / (2.0 - lambda) * (1.0 - (1.0 - lambda).powi(2 * (i + 1) as i32))).sqrt();
            let limit = l * std * limit_factor;

            ucl.push(mean + limit);
            lcl.push(mean - limit);

            // Check for out of control
            if ewma[i] > ucl[i] || ewma[i] < lcl[i] {
                ooc.push(i);
            }
        }

        EwmaResult {
            ewma,
            upper_control_limit: ucl,
            lower_control_limit: lcl,
            out_of_control: ooc,
        }
    }

    /// Detect structural breaks using Chow test approximation
    pub fn detect_structural_breaks(&self, values: &[f64], min_segment: usize) -> Vec<usize> {
        if values.len() < min_segment * 2 {
            return Vec::new();
        }

        let mut breaks = Vec::new();
        let total_rss = self.calculate_rss(values);

        for i in min_segment..(values.len() - min_segment) {
            let left = &values[..i];
            let right = &values[i..];

            let left_rss = self.calculate_rss(left);
            let right_rss = self.calculate_rss(right);
            let split_rss = left_rss + right_rss;

            // F-statistic approximation
            let f_stat = (total_rss - split_rss) / 2.0 / (split_rss / (values.len() - 4) as f64);

            // Simple threshold (would use F-distribution in production)
            if f_stat > 10.0 {
                breaks.push(i);
            }
        }

        // Remove nearby breaks (keep strongest)
        self.filter_nearby_breaks(&breaks, min_segment)
    }

    fn calculate_rss(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        values.iter().map(|x| (x - mean).powi(2)).sum()
    }

    fn filter_nearby_breaks(&self, breaks: &[usize], min_distance: usize) -> Vec<usize> {
        if breaks.is_empty() {
            return Vec::new();
        }

        let mut filtered = vec![breaks[0]];
        for &b in &breaks[1..] {
            if b - filtered.last().unwrap() >= min_distance {
                filtered.push(b);
            }
        }
        filtered
    }
}

/// Result of EWMA control chart analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EwmaResult {
    pub ewma: Vec<f64>,
    pub upper_control_limit: Vec<f64>,
    pub lower_control_limit: Vec<f64>,
    pub out_of_control: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cusum_stable() {
        let detector = ChangeDetector::default();
        let values: Vec<f64> = (0..100).map(|_| 0.5).collect();
        let result = detector.cusum_analysis(&values, None);

        assert_eq!(result.current_state, ChangeState::Stable);
        assert!(result.change_points.is_empty());
    }

    #[test]
    fn test_cusum_detects_decrease() {
        let detector = ChangeDetector::new(0.5, 4.0, 20);

        // Stable then decreasing
        let mut values: Vec<f64> = (0..50).map(|_| 1.0).collect();
        values.extend((0..50).map(|_| -1.0));

        let result = detector.cusum_analysis(&values, None);

        // Should detect at least one change point
        assert!(
            !result.change_points.is_empty()
                || result.current_state == ChangeState::Degrading
        );
    }

    #[test]
    fn test_ewma() {
        let detector = ChangeDetector::default();
        let values: Vec<f64> = (0..50).map(|i| (i as f64 * 0.1).sin()).collect();
        let result = detector.ewma_analysis(&values, 0.2);

        assert_eq!(result.ewma.len(), values.len());
        assert_eq!(result.upper_control_limit.len(), values.len());
    }

    #[test]
    fn test_structural_breaks() {
        let detector = ChangeDetector::default();

        // Two distinct regimes
        let mut values: Vec<f64> = (0..30).map(|_| 0.0).collect();
        values.extend((0..30).map(|_| 10.0));

        let breaks = detector.detect_structural_breaks(&values, 10);
        assert!(!breaks.is_empty());
    }
}
