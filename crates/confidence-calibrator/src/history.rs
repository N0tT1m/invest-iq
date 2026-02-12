//! Calibration History Module
//!
//! Stores and manages prediction history for ongoing calibration.
//! Tracks predicted confidence vs actual outcomes over time.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A recorded prediction outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionOutcome {
    /// Unique prediction ID
    pub id: Option<i64>,
    /// Symbol that was predicted
    pub symbol: String,
    /// Type of prediction (buy, sell, hold)
    pub prediction_type: String,
    /// Predicted confidence (0-1)
    pub predicted_confidence: f64,
    /// Actual outcome (true if prediction was correct)
    pub actual_outcome: Option<bool>,
    /// Actual return (if measurable)
    pub actual_return: Option<f64>,
    /// Analysis source (technical, sentiment, quant, etc.)
    pub source: String,
    /// When the prediction was made
    pub prediction_date: DateTime<Utc>,
    /// When the outcome was determined
    pub outcome_date: Option<DateTime<Utc>>,
    /// Timeframe for the prediction (e.g., "1d", "1w")
    pub timeframe: String,
}

/// Complete calibration history entry (matching DB schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationHistory {
    pub id: Option<i64>,
    pub symbol: String,
    pub prediction_type: String,
    pub predicted_confidence: f64,
    pub actual_outcome: Option<bool>,
    pub actual_return: Option<f64>,
    pub source: String,
    pub prediction_date: DateTime<Utc>,
    pub outcome_date: Option<DateTime<Utc>>,
    pub timeframe: String,
    pub created_at: Option<DateTime<Utc>>,
}

/// Internal DB row type with String dates (compatible with sqlx Any backend)
#[derive(Debug, FromRow)]
struct CalibrationRow {
    id: Option<i64>,
    symbol: String,
    prediction_type: String,
    predicted_confidence: f64,
    actual_outcome: Option<bool>,
    actual_return: Option<f64>,
    source: String,
    prediction_date: String,
    outcome_date: Option<String>,
    timeframe: String,
    created_at: Option<String>,
}

impl CalibrationRow {
    fn into_history(self) -> CalibrationHistory {
        CalibrationHistory {
            id: self.id,
            symbol: self.symbol,
            prediction_type: self.prediction_type,
            predicted_confidence: self.predicted_confidence,
            actual_outcome: self.actual_outcome,
            actual_return: self.actual_return,
            source: self.source,
            prediction_date: self
                .prediction_date
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now()),
            outcome_date: self
                .outcome_date
                .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
            timeframe: self.timeframe,
            created_at: self
                .created_at
                .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
        }
    }
}

impl From<PredictionOutcome> for CalibrationHistory {
    fn from(outcome: PredictionOutcome) -> Self {
        CalibrationHistory {
            id: outcome.id,
            symbol: outcome.symbol,
            prediction_type: outcome.prediction_type,
            predicted_confidence: outcome.predicted_confidence,
            actual_outcome: outcome.actual_outcome,
            actual_return: outcome.actual_return,
            source: outcome.source,
            prediction_date: outcome.prediction_date,
            outcome_date: outcome.outcome_date,
            timeframe: outcome.timeframe,
            created_at: None,
        }
    }
}

/// Store for calibration history data
pub struct CalibrationHistoryStore {
    pool: sqlx::AnyPool,
}

impl CalibrationHistoryStore {
    /// Create a new history store
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    /// Record a new prediction
    pub async fn record_prediction(&self, prediction: &PredictionOutcome) -> Result<i64> {
        let (id,): (i64,) = sqlx::query_as(
            r#"
            INSERT INTO calibration_history (
                symbol, prediction_type, predicted_confidence, source,
                prediction_date, timeframe
            )
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(&prediction.symbol)
        .bind(&prediction.prediction_type)
        .bind(prediction.predicted_confidence)
        .bind(&prediction.source)
        .bind(prediction.prediction_date.to_rfc3339())
        .bind(&prediction.timeframe)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Record the outcome of a prediction
    pub async fn record_outcome(
        &self,
        prediction_id: i64,
        outcome: bool,
        actual_return: Option<f64>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE calibration_history
            SET actual_outcome = ?, actual_return = ?, outcome_date = ?
            WHERE id = ?
            "#,
        )
        .bind(outcome)
        .bind(actual_return)
        .bind(&now)
        .bind(prediction_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get predictions with outcomes for calibration
    pub async fn get_completed_predictions(
        &self,
        source: Option<&str>,
        limit: i64,
    ) -> Result<Vec<CalibrationHistory>> {
        let rows: Vec<CalibrationRow> = if let Some(source) = source {
            sqlx::query_as(
                r#"
                SELECT
                    id, symbol, prediction_type, predicted_confidence,
                    actual_outcome, actual_return, source,
                    prediction_date, outcome_date, timeframe, created_at
                FROM calibration_history
                WHERE actual_outcome IS NOT NULL AND source = ?
                ORDER BY prediction_date DESC
                LIMIT ?
                "#,
            )
            .bind(source)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    id, symbol, prediction_type, predicted_confidence,
                    actual_outcome, actual_return, source,
                    prediction_date, outcome_date, timeframe, created_at
                FROM calibration_history
                WHERE actual_outcome IS NOT NULL
                ORDER BY prediction_date DESC
                LIMIT ?
                "#,
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.into_iter().map(|r| r.into_history()).collect())
    }

    /// Get pending predictions (no outcome yet)
    pub async fn get_pending_predictions(&self) -> Result<Vec<CalibrationHistory>> {
        let rows: Vec<CalibrationRow> = sqlx::query_as(
            r#"
            SELECT
                id, symbol, prediction_type, predicted_confidence,
                actual_outcome, actual_return, source,
                prediction_date, outcome_date, timeframe, created_at
            FROM calibration_history
            WHERE actual_outcome IS NULL
            ORDER BY prediction_date DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_history()).collect())
    }

    /// Get predictions for a specific symbol
    pub async fn get_symbol_predictions(
        &self,
        symbol: &str,
        limit: i64,
    ) -> Result<Vec<CalibrationHistory>> {
        let rows: Vec<CalibrationRow> = sqlx::query_as(
            r#"
            SELECT
                id, symbol, prediction_type, predicted_confidence,
                actual_outcome, actual_return, source,
                prediction_date, outcome_date, timeframe, created_at
            FROM calibration_history
            WHERE symbol = ?
            ORDER BY prediction_date DESC
            LIMIT ?
            "#,
        )
        .bind(symbol)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_history()).collect())
    }

    /// Get calibration data as (confidence, outcome) pairs for fitting
    pub async fn get_calibration_data(&self, source: Option<&str>) -> Result<Vec<(f64, bool)>> {
        let history = self.get_completed_predictions(source, 1000).await?;

        Ok(history
            .into_iter()
            .filter_map(|h| h.actual_outcome.map(|o| (h.predicted_confidence, o)))
            .collect())
    }

    /// Get accuracy statistics by confidence bucket
    pub async fn get_accuracy_by_bucket(&self, source: Option<&str>) -> Result<Vec<BucketStats>> {
        let data = self.get_calibration_data(source).await?;

        if data.is_empty() {
            return Ok(Vec::new());
        }

        let n_buckets = 10;
        let mut buckets: Vec<Vec<bool>> = vec![Vec::new(); n_buckets];

        for (conf, outcome) in &data {
            let bucket = ((*conf * n_buckets as f64) as usize).min(n_buckets - 1);
            buckets[bucket].push(*outcome);
        }

        let stats: Vec<BucketStats> = buckets
            .into_iter()
            .enumerate()
            .filter(|(_, b)| !b.is_empty())
            .map(|(i, bucket)| {
                let mid = (i as f64 + 0.5) / n_buckets as f64;
                let accuracy = bucket.iter().filter(|&&x| x).count() as f64 / bucket.len() as f64;
                BucketStats {
                    bucket_start: i as f64 / n_buckets as f64,
                    bucket_end: (i + 1) as f64 / n_buckets as f64,
                    mid_confidence: mid,
                    actual_accuracy: accuracy,
                    sample_count: bucket.len(),
                    calibration_gap: mid - accuracy,
                }
            })
            .collect();

        Ok(stats)
    }

    /// Calculate overall calibration metrics
    pub async fn calculate_metrics(&self, source: Option<&str>) -> Result<CalibrationMetrics> {
        let buckets = self.get_accuracy_by_bucket(source).await?;
        let data = self.get_calibration_data(source).await?;

        if data.is_empty() {
            return Ok(CalibrationMetrics::default());
        }

        let n = data.len() as f64;

        // Expected Calibration Error
        let ece: f64 = buckets
            .iter()
            .map(|b| b.calibration_gap.abs() * b.sample_count as f64 / n)
            .sum();

        // Maximum Calibration Error
        let mce = buckets
            .iter()
            .map(|b| b.calibration_gap.abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        // Brier Score
        let brier: f64 = data
            .iter()
            .map(|(conf, outcome)| {
                let y = if *outcome { 1.0 } else { 0.0 };
                (conf - y).powi(2)
            })
            .sum::<f64>()
            / n;

        // Accuracy
        let correct = data
            .iter()
            .filter(|(conf, outcome)| (*conf >= 0.5 && *outcome) || (*conf < 0.5 && !*outcome))
            .count();
        let accuracy = correct as f64 / n;

        Ok(CalibrationMetrics {
            ece,
            mce,
            brier_score: brier,
            accuracy,
            sample_size: data.len(),
            bucket_stats: buckets,
        })
    }
}

/// Statistics for a confidence bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketStats {
    pub bucket_start: f64,
    pub bucket_end: f64,
    pub mid_confidence: f64,
    pub actual_accuracy: f64,
    pub sample_count: usize,
    pub calibration_gap: f64,
}

/// Overall calibration metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CalibrationMetrics {
    /// Expected Calibration Error (lower is better, 0 is perfect)
    pub ece: f64,
    /// Maximum Calibration Error
    pub mce: f64,
    /// Brier Score (lower is better)
    pub brier_score: f64,
    /// Overall accuracy
    pub accuracy: f64,
    /// Number of predictions in the dataset
    pub sample_size: usize,
    /// Per-bucket statistics
    pub bucket_stats: Vec<BucketStats>,
}

impl CalibrationMetrics {
    /// Get a human-readable assessment
    pub fn assessment(&self) -> String {
        if self.sample_size < 30 {
            "Insufficient data for reliable calibration assessment".to_string()
        } else if self.ece < 0.05 {
            "Excellent calibration - predictions are highly reliable".to_string()
        } else if self.ece < 0.1 {
            "Good calibration - predictions are reasonably reliable".to_string()
        } else if self.ece < 0.2 {
            "Moderate calibration - some adjustment may be needed".to_string()
        } else {
            "Poor calibration - predictions need significant adjustment".to_string()
        }
    }

    /// Check if the model is overconfident
    pub fn is_overconfident(&self) -> bool {
        self.bucket_stats
            .iter()
            .filter(|b| b.mid_confidence > 0.5)
            .map(|b| b.calibration_gap)
            .sum::<f64>()
            > 0.0
    }

    /// Check if the model is underconfident
    pub fn is_underconfident(&self) -> bool {
        self.bucket_stats
            .iter()
            .filter(|b| b.mid_confidence > 0.5)
            .map(|b| b.calibration_gap)
            .sum::<f64>()
            < 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calibration_metrics_assessment() {
        let good_metrics = CalibrationMetrics {
            ece: 0.03,
            mce: 0.1,
            brier_score: 0.15,
            accuracy: 0.75,
            sample_size: 100,
            bucket_stats: Vec::new(),
        };

        assert!(good_metrics.assessment().contains("Excellent"));

        let poor_metrics = CalibrationMetrics {
            ece: 0.25,
            mce: 0.4,
            brier_score: 0.35,
            accuracy: 0.55,
            sample_size: 100,
            bucket_stats: Vec::new(),
        };

        assert!(poor_metrics.assessment().contains("Poor"));
    }

    #[test]
    fn test_bucket_stats() {
        let bucket = BucketStats {
            bucket_start: 0.6,
            bucket_end: 0.7,
            mid_confidence: 0.65,
            actual_accuracy: 0.55,
            sample_count: 50,
            calibration_gap: 0.1, // Overconfident by 10%
        };

        assert!(bucket.calibration_gap > 0.0); // Overconfident
    }
}
