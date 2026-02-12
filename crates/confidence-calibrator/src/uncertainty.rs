//! Uncertainty Estimation Module
//!
//! Decomposes prediction uncertainty into epistemic (model uncertainty)
//! and aleatoric (data uncertainty) components.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Level of uncertainty classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum UncertaintyLevel {
    /// Very low uncertainty - high confidence in prediction
    VeryLow,
    /// Low uncertainty - reasonable confidence
    Low,
    /// Moderate uncertainty - proceed with caution
    Moderate,
    /// High uncertainty - significant doubt
    High,
    /// Very high uncertainty - prediction unreliable
    VeryHigh,
}

impl UncertaintyLevel {
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s < 0.1 => UncertaintyLevel::VeryLow,
            s if s < 0.2 => UncertaintyLevel::Low,
            s if s < 0.35 => UncertaintyLevel::Moderate,
            s if s < 0.5 => UncertaintyLevel::High,
            _ => UncertaintyLevel::VeryHigh,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            UncertaintyLevel::VeryLow => "Very Low",
            UncertaintyLevel::Low => "Low",
            UncertaintyLevel::Moderate => "Moderate",
            UncertaintyLevel::High => "High",
            UncertaintyLevel::VeryHigh => "Very High",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            UncertaintyLevel::VeryLow => "#00cc88",
            UncertaintyLevel::Low => "#00ccff",
            UncertaintyLevel::Moderate => "#ffcc00",
            UncertaintyLevel::High => "#ff8800",
            UncertaintyLevel::VeryHigh => "#ff4444",
        }
    }
}

/// Decomposition of uncertainty into components
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UncertaintyDecomposition {
    /// Epistemic uncertainty (model uncertainty)
    /// Can be reduced with more training data or better models
    pub epistemic: f64,
    /// Aleatoric uncertainty (data uncertainty)
    /// Inherent randomness that cannot be reduced
    pub aleatoric: f64,
    /// Total uncertainty (combined)
    pub total: f64,
    /// Uncertainty level classification
    pub level: UncertaintyLevel,
}

impl UncertaintyDecomposition {
    pub fn new(epistemic: f64, aleatoric: f64) -> Self {
        let total = (epistemic.powi(2) + aleatoric.powi(2)).sqrt();
        let level = UncertaintyLevel::from_score(total);

        Self {
            epistemic,
            aleatoric,
            total,
            level,
        }
    }

    /// Check if uncertainty is primarily from model (reducible)
    pub fn is_model_dominated(&self) -> bool {
        self.epistemic > self.aleatoric
    }

    /// Check if uncertainty is primarily from data (irreducible)
    pub fn is_data_dominated(&self) -> bool {
        self.aleatoric > self.epistemic
    }

    /// Get a recommendation for reducing uncertainty
    pub fn reduction_recommendation(&self) -> String {
        if self.total < 0.2 {
            "Uncertainty is acceptably low.".to_string()
        } else if self.is_model_dominated() {
            format!(
                "Uncertainty is primarily from the model ({:.0}%). \
                 Could be reduced with more training data or model improvements.",
                self.epistemic / self.total * 100.0
            )
        } else {
            format!(
                "Uncertainty is primarily from inherent data variability ({:.0}%). \
                 This is harder to reduce - consider using ensembles or \
                 waiting for more stable market conditions.",
                self.aleatoric / self.total * 100.0
            )
        }
    }
}

/// Complete uncertainty analysis for a prediction
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UncertaintyAnalysis {
    /// The prediction value
    pub prediction: f64,
    /// Uncertainty decomposition
    pub uncertainty: UncertaintyDecomposition,
    /// Confidence interval lower bound
    pub ci_lower: f64,
    /// Confidence interval upper bound
    pub ci_upper: f64,
    /// Confidence level (e.g., 0.95 for 95%)
    pub ci_level: f64,
    /// Sources contributing to uncertainty
    pub sources: Vec<UncertaintySource>,
    /// Overall reliability assessment
    pub reliability: ReliabilityAssessment,
}

/// A source contributing to uncertainty
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UncertaintySource {
    pub name: String,
    pub contribution: f64,
    pub uncertainty_type: UncertaintyType,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum UncertaintyType {
    Epistemic,
    Aleatoric,
}

/// Overall reliability assessment
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReliabilityAssessment {
    pub score: f64,
    pub grade: String,
    pub summary: String,
    pub actionable: bool,
}

/// Uncertainty estimator for predictions
pub struct UncertaintyEstimator {
    /// Base model variance from training
    base_model_variance: f64,
    /// Historical prediction accuracy by confidence bucket
    accuracy_by_confidence: Vec<(f64, f64, usize)>, // (confidence, accuracy, count)
}

impl Default for UncertaintyEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl UncertaintyEstimator {
    pub fn new() -> Self {
        Self {
            base_model_variance: 0.1,
            accuracy_by_confidence: Vec::new(),
        }
    }

    /// Update with historical accuracy data
    pub fn update_from_history(&mut self, history: &[(f64, bool)]) {
        // Bucket by confidence and calculate accuracy
        let n_buckets = 10;
        let mut buckets: Vec<Vec<bool>> = vec![Vec::new(); n_buckets];

        for (conf, outcome) in history {
            let bucket = ((*conf * n_buckets as f64) as usize).min(n_buckets - 1);
            buckets[bucket].push(*outcome);
        }

        self.accuracy_by_confidence.clear();
        for (i, bucket) in buckets.iter().enumerate() {
            if !bucket.is_empty() {
                let mid_conf = (i as f64 + 0.5) / n_buckets as f64;
                let accuracy = bucket.iter().filter(|&&x| x).count() as f64 / bucket.len() as f64;
                self.accuracy_by_confidence
                    .push((mid_conf, accuracy, bucket.len()));
            }
        }

        // Update base model variance from calibration gap
        if !self.accuracy_by_confidence.is_empty() {
            let total_gap: f64 = self
                .accuracy_by_confidence
                .iter()
                .map(|(conf, acc, _)| (conf - acc).powi(2))
                .sum();
            self.base_model_variance =
                (total_gap / self.accuracy_by_confidence.len() as f64).sqrt();
        }
    }

    /// Estimate uncertainty for a prediction
    pub fn estimate(&self, confidence: f64, context: &PredictionContext) -> UncertaintyAnalysis {
        let mut sources = Vec::new();

        // 1. Model uncertainty (epistemic)
        let model_uncertainty = self.estimate_epistemic(confidence, context, &mut sources);

        // 2. Data uncertainty (aleatoric)
        let data_uncertainty = self.estimate_aleatoric(confidence, context, &mut sources);

        let decomposition = UncertaintyDecomposition::new(model_uncertainty, data_uncertainty);

        // Calculate confidence interval
        let z = 1.96; // 95% CI
        let margin = z * decomposition.total;
        let ci_lower = (confidence - margin).max(0.0);
        let ci_upper = (confidence + margin).min(1.0);

        // Reliability assessment
        let reliability = self.assess_reliability(confidence, &decomposition, context);

        UncertaintyAnalysis {
            prediction: confidence,
            uncertainty: decomposition,
            ci_lower,
            ci_upper,
            ci_level: 0.95,
            sources,
            reliability,
        }
    }

    fn estimate_epistemic(
        &self,
        confidence: f64,
        context: &PredictionContext,
        sources: &mut Vec<UncertaintySource>,
    ) -> f64 {
        let mut epistemic = self.base_model_variance;

        // Historical accuracy gap
        if let Some((_, acc, count)) = self.find_closest_bucket(confidence) {
            let gap = (confidence - acc).abs();
            if gap > 0.1 {
                sources.push(UncertaintySource {
                    name: "Historical Calibration Gap".to_string(),
                    contribution: gap * 0.5,
                    uncertainty_type: UncertaintyType::Epistemic,
                    description: format!(
                        "Model predictions at {:.0}% confidence historically achieve {:.0}%",
                        confidence * 100.0,
                        acc * 100.0
                    ),
                });
                epistemic += gap * 0.5;
            }

            // Sample size effect
            if count < 50 {
                let sample_contribution = 0.1 * (1.0 - count as f64 / 50.0);
                sources.push(UncertaintySource {
                    name: "Limited Historical Data".to_string(),
                    contribution: sample_contribution,
                    uncertainty_type: UncertaintyType::Epistemic,
                    description: format!("Only {} similar predictions in history", count),
                });
                epistemic += sample_contribution;
            }
        } else {
            sources.push(UncertaintySource {
                name: "No Historical Data".to_string(),
                contribution: 0.15,
                uncertainty_type: UncertaintyType::Epistemic,
                description: "No historical predictions at this confidence level".to_string(),
            });
            epistemic += 0.15;
        }

        // Regime uncertainty
        if context.regime_change_probability > 0.3 {
            let regime_contribution = context.regime_change_probability * 0.2;
            sources.push(UncertaintySource {
                name: "Regime Change Risk".to_string(),
                contribution: regime_contribution,
                uncertainty_type: UncertaintyType::Epistemic,
                description: "Market conditions may be shifting".to_string(),
            });
            epistemic += regime_contribution;
        }

        // Model disagreement
        if context.model_disagreement > 0.2 {
            let disagreement_contribution = context.model_disagreement * 0.25;
            sources.push(UncertaintySource {
                name: "Model Disagreement".to_string(),
                contribution: disagreement_contribution,
                uncertainty_type: UncertaintyType::Epistemic,
                description: "Different analysis methods give conflicting signals".to_string(),
            });
            epistemic += disagreement_contribution;
        }

        epistemic.min(0.5)
    }

    fn estimate_aleatoric(
        &self,
        confidence: f64,
        context: &PredictionContext,
        sources: &mut Vec<UncertaintySource>,
    ) -> f64 {
        let mut aleatoric = 0.0;

        // Base aleatoric from probability (highest at 0.5)
        let base_aleatoric = 2.0 * confidence * (1.0 - confidence);
        aleatoric += base_aleatoric;

        // Market volatility
        if context.market_volatility > 0.02 {
            let vol_contribution = (context.market_volatility - 0.02) / 0.08; // Normalize
            sources.push(UncertaintySource {
                name: "Market Volatility".to_string(),
                contribution: vol_contribution * 0.15,
                uncertainty_type: UncertaintyType::Aleatoric,
                description: format!(
                    "Current volatility: {:.1}%",
                    context.market_volatility * 100.0
                ),
            });
            aleatoric += vol_contribution * 0.15;
        }

        // Event risk
        if context.days_to_earnings.map(|d| d <= 7).unwrap_or(false) {
            sources.push(UncertaintySource {
                name: "Upcoming Earnings".to_string(),
                contribution: 0.1,
                uncertainty_type: UncertaintyType::Aleatoric,
                description: "Earnings announcement increases unpredictability".to_string(),
            });
            aleatoric += 0.1;
        }

        // News flow uncertainty
        if context.news_sentiment_variance > 0.3 {
            let news_contribution = (context.news_sentiment_variance - 0.3) * 0.2;
            sources.push(UncertaintySource {
                name: "Mixed News Sentiment".to_string(),
                contribution: news_contribution,
                uncertainty_type: UncertaintyType::Aleatoric,
                description: "Conflicting news creates unpredictability".to_string(),
            });
            aleatoric += news_contribution;
        }

        aleatoric.min(0.5)
    }

    fn find_closest_bucket(&self, confidence: f64) -> Option<(f64, f64, usize)> {
        self.accuracy_by_confidence
            .iter()
            .min_by(|a, b| {
                (a.0 - confidence)
                    .abs()
                    .partial_cmp(&(b.0 - confidence).abs())
                    .unwrap()
            })
            .copied()
    }

    fn assess_reliability(
        &self,
        confidence: f64,
        uncertainty: &UncertaintyDecomposition,
        context: &PredictionContext,
    ) -> ReliabilityAssessment {
        // Calculate reliability score (0-100)
        let base_reliability = confidence * (1.0 - uncertainty.total);

        // Adjust for context
        let mut reliability = base_reliability;
        if context.model_disagreement > 0.3 {
            reliability *= 0.8;
        }
        if context.days_to_earnings.map(|d| d <= 3).unwrap_or(false) {
            reliability *= 0.7;
        }

        let score = (reliability * 100.0).clamp(0.0, 100.0);

        let (grade, summary, actionable) = if score >= 80.0 {
            ("A", "Highly reliable prediction", true)
        } else if score >= 65.0 {
            ("B", "Reliable prediction with some uncertainty", true)
        } else if score >= 50.0 {
            ("C", "Moderate reliability - proceed with caution", true)
        } else if score >= 35.0 {
            (
                "D",
                "Low reliability - consider waiting for better signal",
                false,
            )
        } else {
            ("F", "Unreliable - insufficient confidence to act", false)
        };

        ReliabilityAssessment {
            score,
            grade: grade.to_string(),
            summary: summary.to_string(),
            actionable,
        }
    }
}

/// Context for uncertainty estimation
#[derive(Debug, Clone, Default)]
pub struct PredictionContext {
    /// Probability of regime change (0-1)
    pub regime_change_probability: f64,
    /// Disagreement between models (0-1)
    pub model_disagreement: f64,
    /// Current market volatility (daily %)
    pub market_volatility: f64,
    /// Days until earnings (if applicable)
    pub days_to_earnings: Option<i32>,
    /// Variance in news sentiment
    pub news_sentiment_variance: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uncertainty_level_classification() {
        assert_eq!(
            UncertaintyLevel::from_score(0.05),
            UncertaintyLevel::VeryLow
        );
        assert_eq!(UncertaintyLevel::from_score(0.15), UncertaintyLevel::Low);
        assert_eq!(
            UncertaintyLevel::from_score(0.25),
            UncertaintyLevel::Moderate
        );
        assert_eq!(UncertaintyLevel::from_score(0.45), UncertaintyLevel::High);
        assert_eq!(
            UncertaintyLevel::from_score(0.6),
            UncertaintyLevel::VeryHigh
        );
    }

    #[test]
    fn test_uncertainty_decomposition() {
        let decomp = UncertaintyDecomposition::new(0.2, 0.15);

        assert!(decomp.total > 0.2 && decomp.total < 0.35);
        assert!(decomp.is_model_dominated());
        assert!(!decomp.is_data_dominated());
    }

    #[test]
    fn test_uncertainty_estimator() {
        let mut estimator = UncertaintyEstimator::new();

        // Add some history
        let history: Vec<(f64, bool)> = (0..100)
            .map(|i| {
                let conf = (i as f64) / 100.0;
                let outcome = conf > 0.5;
                (conf, outcome)
            })
            .collect();

        estimator.update_from_history(&history);

        let context = PredictionContext::default();
        let analysis = estimator.estimate(0.7, &context);

        assert!(analysis.ci_lower < 0.7);
        assert!(analysis.ci_upper > 0.7);
        assert!(analysis.reliability.score > 0.0);
    }

    #[test]
    fn test_high_uncertainty_context() {
        let estimator = UncertaintyEstimator::new();

        let context = PredictionContext {
            regime_change_probability: 0.5,
            model_disagreement: 0.4,
            market_volatility: 0.05,
            days_to_earnings: Some(2),
            news_sentiment_variance: 0.5,
        };

        let analysis = estimator.estimate(0.6, &context);

        // Should have high uncertainty with this context
        assert!(analysis.uncertainty.total > 0.2);
        assert!(!analysis.sources.is_empty());
    }
}
