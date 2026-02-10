//! Confidence Calibration
//!
//! Implements probability calibration techniques to transform raw model outputs
//! into well-calibrated probability estimates.

use serde::{Deserialize, Serialize};

/// Method used for calibration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalibrationMethod {
    /// Platt scaling - fits a sigmoid to the scores
    PlattScaling,
    /// Isotonic regression - non-parametric monotonic fit
    IsotonicRegression,
    /// Temperature scaling - single parameter scaling
    TemperatureScaling,
    /// No calibration applied
    None,
}

/// A prediction with calibration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibratedPrediction {
    /// Original raw confidence from the model
    pub raw_confidence: f64,
    /// Calibrated confidence (what it actually means)
    pub calibrated_confidence: f64,
    /// Lower bound of confidence interval (e.g., 95%)
    pub lower_bound: f64,
    /// Upper bound of confidence interval
    pub upper_bound: f64,
    /// Model uncertainty (epistemic - reducible with more data)
    pub model_uncertainty: f64,
    /// Data uncertainty (aleatoric - inherent randomness)
    pub data_uncertainty: f64,
    /// Total uncertainty
    pub total_uncertainty: f64,
    /// Calibration method used
    pub method: CalibrationMethod,
    /// Human-readable recommendation
    pub recommendation: String,
    /// Reliability description
    pub reliability_note: String,
}

/// Statistics about the calibration model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationStats {
    /// Expected Calibration Error (lower is better)
    pub ece: f64,
    /// Maximum Calibration Error
    pub mce: f64,
    /// Brier score (mean squared error of probabilities)
    pub brier_score: f64,
    /// Number of predictions used for calibration
    pub sample_size: usize,
    /// Calibration method
    pub method: CalibrationMethod,
    /// Reliability diagram bins
    pub reliability_bins: Vec<ReliabilityBin>,
}

/// A bin in the reliability diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityBin {
    /// Average predicted probability in this bin
    pub avg_predicted: f64,
    /// Actual fraction of positives in this bin
    pub actual_positive_rate: f64,
    /// Number of samples in this bin
    pub count: usize,
}

/// Confidence calibrator using historical prediction data
pub struct ConfidenceCalibrator {
    /// Platt scaling parameters (sigmoid)
    platt_a: f64,
    platt_b: f64,
    /// Temperature for temperature scaling
    temperature: f64,
    /// Isotonic regression lookup table
    isotonic_table: Vec<(f64, f64)>,
    /// Current calibration method
    method: CalibrationMethod,
    /// Whether the calibrator has been fitted
    is_fitted: bool,
    /// Calibration statistics
    stats: Option<CalibrationStats>,
}

impl Default for ConfidenceCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfidenceCalibrator {
    /// Create a new uncalibrated calibrator
    pub fn new() -> Self {
        Self {
            platt_a: 1.0,
            platt_b: 0.0,
            temperature: 1.0,
            isotonic_table: Vec::new(),
            method: CalibrationMethod::None,
            is_fitted: false,
            stats: None,
        }
    }

    /// Fit the calibrator using historical predictions and outcomes
    pub fn fit(
        &mut self,
        predictions: &[(f64, bool)], // (predicted_confidence, was_correct)
        method: CalibrationMethod,
    ) -> anyhow::Result<()> {
        if predictions.len() < 10 {
            anyhow::bail!("Need at least 10 predictions for calibration");
        }

        self.method = method;

        match method {
            CalibrationMethod::PlattScaling => self.fit_platt(predictions)?,
            CalibrationMethod::IsotonicRegression => self.fit_isotonic(predictions)?,
            CalibrationMethod::TemperatureScaling => self.fit_temperature(predictions)?,
            CalibrationMethod::None => {}
        }

        self.is_fitted = true;
        self.stats = Some(self.calculate_stats(predictions));

        Ok(())
    }

    /// Fit Platt scaling (sigmoid calibration)
    fn fit_platt(&mut self, predictions: &[(f64, bool)]) -> anyhow::Result<()> {
        // Simple gradient descent for sigmoid parameters
        // P(y=1|x) = 1 / (1 + exp(Ax + B))
        let mut a = 0.0;
        let mut b = 0.0;
        let learning_rate = 0.01;
        let iterations = 1000;

        for _ in 0..iterations {
            let mut grad_a = 0.0;
            let mut grad_b = 0.0;

            for (pred, outcome) in predictions {
                let y = if *outcome { 1.0 } else { 0.0 };
                let p = 1.0 / (1.0 + (-a * pred - b).exp());
                let error = p - y;
                grad_a += error * pred;
                grad_b += error;
            }

            a -= learning_rate * grad_a / predictions.len() as f64;
            b -= learning_rate * grad_b / predictions.len() as f64;
        }

        self.platt_a = a;
        self.platt_b = b;
        Ok(())
    }

    /// Fit isotonic regression (pool adjacent violators algorithm)
    fn fit_isotonic(&mut self, predictions: &[(f64, bool)]) -> anyhow::Result<()> {
        // Sort by predicted probability
        let mut sorted: Vec<_> = predictions.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Pool adjacent violators algorithm (PAVA)
        let mut values: Vec<f64> = sorted.iter().map(|(_, o)| if *o { 1.0 } else { 0.0 }).collect();
        let mut weights: Vec<f64> = vec![1.0; values.len()];

        let mut i = 0;
        while i < values.len() - 1 {
            if values[i] > values[i + 1] {
                // Pool violating pair
                let total_weight = weights[i] + weights[i + 1];
                let pooled_value = (values[i] * weights[i] + values[i + 1] * weights[i + 1]) / total_weight;
                values[i] = pooled_value;
                weights[i] = total_weight;
                values.remove(i + 1);
                weights.remove(i + 1);

                // Check for further violations going backwards
                if i > 0 {
                    i -= 1;
                }
            } else {
                i += 1;
            }
        }

        // Build lookup table
        self.isotonic_table.clear();
        let mut idx = 0;
        for (j, &(pred, _)) in sorted.iter().enumerate() {
            while idx < values.len() - 1 && j >= (weights[..=idx].iter().sum::<f64>() as usize) {
                idx += 1;
            }
            if self.isotonic_table.is_empty() || self.isotonic_table.last().unwrap().0 != pred {
                self.isotonic_table.push((pred, values[idx]));
            }
        }

        Ok(())
    }

    /// Fit temperature scaling
    fn fit_temperature(&mut self, predictions: &[(f64, bool)]) -> anyhow::Result<()> {
        // Grid search for best temperature
        let mut best_temp = 1.0;
        let mut best_loss = f64::MAX;

        for t in (1..=100).map(|i| i as f64 * 0.1) {
            let loss: f64 = predictions
                .iter()
                .map(|(pred, outcome)| {
                    let scaled = self.apply_temperature(*pred, t);
                    let y = if *outcome { 1.0 } else { 0.0 };
                    // Negative log likelihood
                    if y > 0.5 {
                        -scaled.max(1e-10).ln()
                    } else {
                        -(1.0 - scaled).max(1e-10).ln()
                    }
                })
                .sum();

            if loss < best_loss {
                best_loss = loss;
                best_temp = t;
            }
        }

        self.temperature = best_temp;
        Ok(())
    }

    fn apply_temperature(&self, confidence: f64, temp: f64) -> f64 {
        // Convert probability to logit, scale, convert back
        let logit = (confidence / (1.0 - confidence + 1e-10)).ln();
        let scaled_logit = logit / temp;
        1.0 / (1.0 + (-scaled_logit).exp())
    }

    /// Calibrate a single prediction
    pub fn calibrate(&self, raw_confidence: f64) -> CalibratedPrediction {
        let calibrated = if self.is_fitted {
            match self.method {
                CalibrationMethod::PlattScaling => {
                    1.0 / (1.0 + (-self.platt_a * raw_confidence - self.platt_b).exp())
                }
                CalibrationMethod::IsotonicRegression => {
                    self.isotonic_lookup(raw_confidence)
                }
                CalibrationMethod::TemperatureScaling => {
                    self.apply_temperature(raw_confidence, self.temperature)
                }
                CalibrationMethod::None => raw_confidence,
            }
        } else {
            raw_confidence
        };

        // Estimate uncertainty based on calibration and sample size
        let sample_size = self.stats.as_ref().map(|s| s.sample_size).unwrap_or(0);
        let (model_uncertainty, data_uncertainty) = self.estimate_uncertainty(
            raw_confidence,
            calibrated,
            sample_size,
        );

        let total_uncertainty = (model_uncertainty.powi(2) + data_uncertainty.powi(2)).sqrt();

        // Calculate confidence interval (using Wilson score interval)
        let z = 1.96; // 95% CI
        let n = sample_size.max(100) as f64;
        let p = calibrated;
        let denominator = 1.0 + z * z / n;
        let center = (p + z * z / (2.0 * n)) / denominator;
        let margin = (z / denominator) * ((p * (1.0 - p) / n) + z * z / (4.0 * n * n)).sqrt();

        let lower_bound = (center - margin).max(0.0);
        let upper_bound = (center + margin).min(1.0);

        let recommendation = self.generate_recommendation(calibrated, total_uncertainty);
        let reliability_note = self.generate_reliability_note(raw_confidence, calibrated);

        CalibratedPrediction {
            raw_confidence,
            calibrated_confidence: calibrated,
            lower_bound,
            upper_bound,
            model_uncertainty,
            data_uncertainty,
            total_uncertainty,
            method: self.method,
            recommendation,
            reliability_note,
        }
    }

    fn isotonic_lookup(&self, value: f64) -> f64 {
        if self.isotonic_table.is_empty() {
            return value;
        }

        // Binary search for closest value
        match self.isotonic_table.binary_search_by(|probe| {
            probe.0.partial_cmp(&value).unwrap()
        }) {
            Ok(idx) => self.isotonic_table[idx].1,
            Err(idx) => {
                if idx == 0 {
                    self.isotonic_table[0].1
                } else if idx >= self.isotonic_table.len() {
                    self.isotonic_table.last().unwrap().1
                } else {
                    // Linear interpolation
                    let (x0, y0) = self.isotonic_table[idx - 1];
                    let (x1, y1) = self.isotonic_table[idx];
                    let t = (value - x0) / (x1 - x0);
                    y0 + t * (y1 - y0)
                }
            }
        }
    }

    fn estimate_uncertainty(
        &self,
        raw: f64,
        calibrated: f64,
        sample_size: usize,
    ) -> (f64, f64) {
        // Model uncertainty (epistemic) - decreases with more calibration data
        let model_uncertainty = if sample_size > 0 {
            0.5 / (sample_size as f64).sqrt()
        } else {
            0.3 // High uncertainty when not calibrated
        };

        // Data uncertainty (aleatoric) - based on distance from extremes
        // Highest uncertainty at 0.5, lowest at 0 and 1
        let data_uncertainty = 2.0 * calibrated * (1.0 - calibrated);

        // Additional uncertainty from calibration adjustment
        let calibration_gap = (raw - calibrated).abs();
        let adjusted_model = model_uncertainty + calibration_gap * 0.1;

        (adjusted_model.min(0.5), data_uncertainty)
    }

    fn generate_recommendation(&self, confidence: f64, uncertainty: f64) -> String {
        let certainty = 1.0 - uncertainty;

        if confidence >= 0.8 && certainty >= 0.7 {
            "Strong signal with high reliability. Consider full position.".to_string()
        } else if confidence >= 0.7 && certainty >= 0.6 {
            "Good signal with reasonable reliability. Consider moderate position.".to_string()
        } else if confidence >= 0.6 && certainty >= 0.5 {
            "Moderate signal. Consider smaller position with tight stops.".to_string()
        } else if uncertainty > 0.4 {
            "High uncertainty. Wait for clearer signal or reduce position size.".to_string()
        } else if confidence < 0.5 {
            "Weak or opposing signal. Avoid or consider contrarian position.".to_string()
        } else {
            "Mixed signals. Proceed with caution.".to_string()
        }
    }

    fn generate_reliability_note(&self, raw: f64, calibrated: f64) -> String {
        let diff = raw - calibrated;

        if !self.is_fitted {
            "Calibration pending. Confidence may not reflect true probability.".to_string()
        } else if diff.abs() < 0.05 {
            "Model is well-calibrated for this confidence level.".to_string()
        } else if diff > 0.15 {
            format!(
                "Model tends to be overconfident. Raw {:.0}% historically means {:.0}%.",
                raw * 100.0,
                calibrated * 100.0
            )
        } else if diff < -0.15 {
            format!(
                "Model tends to be underconfident. Raw {:.0}% historically means {:.0}%.",
                raw * 100.0,
                calibrated * 100.0
            )
        } else if diff > 0.0 {
            "Model is slightly overconfident at this level.".to_string()
        } else {
            "Model is slightly underconfident at this level.".to_string()
        }
    }

    fn calculate_stats(&self, predictions: &[(f64, bool)]) -> CalibrationStats {
        let n_bins = 10;
        let mut bins: Vec<Vec<(f64, bool)>> = vec![Vec::new(); n_bins];

        // Bin predictions
        for &(pred, outcome) in predictions {
            let bin_idx = ((pred * n_bins as f64) as usize).min(n_bins - 1);
            bins[bin_idx].push((pred, outcome));
        }

        // Calculate reliability bins and ECE
        let mut reliability_bins = Vec::new();
        let mut ece = 0.0;
        let mut mce: f64 = 0.0;
        let n = predictions.len() as f64;

        for bin in &bins {
            if bin.is_empty() {
                continue;
            }

            let avg_predicted: f64 = bin.iter().map(|(p, _)| p).sum::<f64>() / bin.len() as f64;
            let actual_positive_rate = bin.iter().filter(|(_, o)| *o).count() as f64 / bin.len() as f64;
            let calibration_error = (avg_predicted - actual_positive_rate).abs();

            ece += calibration_error * bin.len() as f64 / n;
            mce = mce.max(calibration_error);

            reliability_bins.push(ReliabilityBin {
                avg_predicted,
                actual_positive_rate,
                count: bin.len(),
            });
        }

        // Calculate Brier score
        let brier_score: f64 = predictions
            .iter()
            .map(|(pred, outcome)| {
                let y = if *outcome { 1.0 } else { 0.0 };
                (pred - y).powi(2)
            })
            .sum::<f64>()
            / n;

        CalibrationStats {
            ece,
            mce,
            brier_score,
            sample_size: predictions.len(),
            method: self.method,
            reliability_bins,
        }
    }

    /// Get calibration statistics
    pub fn stats(&self) -> Option<&CalibrationStats> {
        self.stats.as_ref()
    }

    /// Check if calibrator is fitted
    pub fn is_fitted(&self) -> bool {
        self.is_fitted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calibrator_uncalibrated() {
        let calibrator = ConfidenceCalibrator::new();
        let result = calibrator.calibrate(0.7);

        assert_eq!(result.raw_confidence, 0.7);
        assert_eq!(result.calibrated_confidence, 0.7);
        assert!(!result.reliability_note.contains("historically"));
    }

    #[test]
    fn test_platt_scaling() {
        let mut calibrator = ConfidenceCalibrator::new();

        // Simulate overconfident model
        let predictions: Vec<(f64, bool)> = (0..100)
            .map(|i| {
                let pred = (i as f64) / 100.0;
                let actual = pred > 0.6; // Model is overconfident
                (pred, actual)
            })
            .collect();

        calibrator.fit(&predictions, CalibrationMethod::PlattScaling).unwrap();

        let result = calibrator.calibrate(0.5);
        assert!(calibrator.is_fitted());
        assert!(result.calibrated_confidence != result.raw_confidence);
    }

    #[test]
    fn test_temperature_scaling() {
        let mut calibrator = ConfidenceCalibrator::new();

        let predictions: Vec<(f64, bool)> = (0..100)
            .map(|i| {
                let pred = (i as f64) / 100.0;
                let actual = rand_like(i) > 0.5;
                (pred, actual)
            })
            .collect();

        calibrator.fit(&predictions, CalibrationMethod::TemperatureScaling).unwrap();
        assert!(calibrator.is_fitted());
    }

    fn rand_like(seed: i32) -> f64 {
        ((seed * 1103515245 + 12345) % 100) as f64 / 100.0
    }

    #[test]
    fn test_calibration_stats() {
        let mut calibrator = ConfidenceCalibrator::new();

        // Well-calibrated predictions
        let predictions: Vec<(f64, bool)> = vec![
            (0.1, false), (0.1, false), (0.1, true), (0.1, false), (0.1, false),
            (0.5, true), (0.5, false), (0.5, true), (0.5, false), (0.5, true),
            (0.9, true), (0.9, true), (0.9, true), (0.9, true), (0.9, false),
        ];

        calibrator.fit(&predictions, CalibrationMethod::None).unwrap();

        let stats = calibrator.stats().unwrap();
        assert!(stats.ece < 0.3); // Reasonably calibrated
        assert!(stats.brier_score < 0.5);
    }
}
