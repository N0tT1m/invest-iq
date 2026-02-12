use anyhow::Result;

use crate::models::{SignalQuality, SignalQualityReport};

pub struct SignalAnalyzer {
    pool: sqlx::AnyPool,
}

impl SignalAnalyzer {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    /// Record a trade outcome for signal quality tracking
    pub async fn record_trade_outcome(
        &self,
        signal_type: &str,
        confidence: f64,
        is_win: bool,
        return_pct: f64,
    ) -> Result<()> {
        // Determine confidence range
        let confidence_range = self.get_confidence_range(confidence);

        // Get existing signal quality or create new
        let existing: Option<SignalQuality> = sqlx::query_as(
            "SELECT * FROM signal_quality WHERE signal_type = ? AND confidence_range = ?",
        )
        .bind(signal_type)
        .bind(&confidence_range)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(mut qual) = existing {
            // Update existing
            qual.signals_taken += 1;
            if is_win {
                qual.winning_trades += 1;
            } else {
                qual.losing_trades += 1;
            }

            qual.actual_win_rate = qual.winning_trades as f64 / qual.signals_taken as f64;
            qual.avg_return = ((qual.avg_return * (qual.signals_taken - 1) as f64) + return_pct)
                / qual.signals_taken as f64;

            // Calculate calibration error (difference between predicted and actual win rate)
            let predicted_win_rate = confidence;
            qual.calibration_error = (predicted_win_rate - qual.actual_win_rate).abs();

            sqlx::query(
                r#"
                UPDATE signal_quality
                SET signals_taken = ?, winning_trades = ?, losing_trades = ?,
                    actual_win_rate = ?, avg_return = ?, calibration_error = ?,
                    last_updated = CURRENT_TIMESTAMP
                WHERE signal_type = ? AND confidence_range = ?
                "#,
            )
            .bind(qual.signals_taken)
            .bind(qual.winning_trades)
            .bind(qual.losing_trades)
            .bind(qual.actual_win_rate)
            .bind(qual.avg_return)
            .bind(qual.calibration_error)
            .bind(signal_type)
            .bind(&confidence_range)
            .execute(&self.pool)
            .await?;
        } else {
            // Create new
            let actual_win_rate = if is_win { 1.0 } else { 0.0 };
            let winning_trades = if is_win { 1 } else { 0 };
            let losing_trades = if is_win { 0 } else { 1 };
            let calibration_error = (confidence - actual_win_rate).abs();

            sqlx::query(
                r#"
                INSERT INTO signal_quality
                (signal_type, confidence_range, total_signals, signals_taken,
                 winning_trades, losing_trades, actual_win_rate, avg_return, calibration_error)
                VALUES (?, ?, 1, 1, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(signal_type)
            .bind(&confidence_range)
            .bind(winning_trades)
            .bind(losing_trades)
            .bind(actual_win_rate)
            .bind(return_pct)
            .bind(calibration_error)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get signal quality for a specific signal type
    pub async fn get_signal_quality(&self, signal_type: &str) -> Result<Vec<SignalQuality>> {
        let qualities: Vec<SignalQuality> = sqlx::query_as(
            "SELECT * FROM signal_quality WHERE signal_type = ? ORDER BY confidence_range DESC",
        )
        .bind(signal_type)
        .fetch_all(&self.pool)
        .await?;

        Ok(qualities)
    }

    /// Get all signal qualities
    pub async fn get_all_signal_qualities(&self) -> Result<Vec<SignalQuality>> {
        let qualities: Vec<SignalQuality> =
            sqlx::query_as("SELECT * FROM signal_quality ORDER BY actual_win_rate DESC")
                .fetch_all(&self.pool)
                .await?;

        Ok(qualities)
    }

    /// Get signal quality report
    pub async fn get_quality_report(&self) -> Result<SignalQualityReport> {
        let all_signals = self.get_all_signal_qualities().await?;

        let total_signal_types = all_signals.len() as i32;

        let avg_calibration_error = if !all_signals.is_empty() {
            all_signals.iter().map(|s| s.calibration_error).sum::<f64>() / all_signals.len() as f64
        } else {
            0.0
        };

        // Get best signals (high win rate, low calibration error)
        let mut best_signals: Vec<SignalQuality> = all_signals
            .iter()
            .filter(|s| s.signals_taken >= 10) // Minimum sample size
            .filter(|s| s.actual_win_rate >= 0.60) // 60%+ win rate
            .cloned()
            .collect();
        best_signals.sort_by(|a, b| b.actual_win_rate.partial_cmp(&a.actual_win_rate).unwrap());
        best_signals.truncate(10);

        // Get worst signals (low win rate)
        let mut worst_signals: Vec<SignalQuality> = all_signals
            .iter()
            .filter(|s| s.signals_taken >= 10)
            .cloned()
            .collect();
        worst_signals.sort_by(|a, b| a.actual_win_rate.partial_cmp(&b.actual_win_rate).unwrap());
        worst_signals.truncate(10);

        Ok(SignalQualityReport {
            total_signal_types,
            avg_calibration_error,
            best_signals,
            worst_signals,
            all_signals,
        })
    }

    /// Check if a signal should be filtered out based on quality
    pub async fn should_filter_signal(
        &self,
        signal_type: &str,
        confidence: f64,
        min_win_rate: f64,
    ) -> Result<bool> {
        let confidence_range = self.get_confidence_range(confidence);

        let quality: Option<SignalQuality> = sqlx::query_as(
            "SELECT * FROM signal_quality WHERE signal_type = ? AND confidence_range = ?",
        )
        .bind(signal_type)
        .bind(&confidence_range)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(qual) = quality {
            // Filter if: not enough data OR win rate below threshold
            if qual.signals_taken < 10 {
                return Ok(false); // Not enough data, allow signal
            }
            if qual.actual_win_rate < min_win_rate {
                return Ok(true); // Filter out - historical performance poor
            }
        }

        Ok(false) // Allow signal
    }

    /// Get calibrated confidence (adjust based on historical performance)
    pub async fn get_calibrated_confidence(
        &self,
        signal_type: &str,
        predicted_confidence: f64,
    ) -> Result<f64> {
        let confidence_range = self.get_confidence_range(predicted_confidence);

        let quality: Option<SignalQuality> = sqlx::query_as(
            "SELECT * FROM signal_quality WHERE signal_type = ? AND confidence_range = ?",
        )
        .bind(signal_type)
        .bind(&confidence_range)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(qual) = quality {
            if qual.signals_taken >= 10 {
                // Return actual win rate if we have enough data
                return Ok(qual.actual_win_rate);
            }
        }

        // Not enough data, return predicted
        Ok(predicted_confidence)
    }

    /// Helper: Get confidence range bucket
    fn get_confidence_range(&self, confidence: f64) -> String {
        let conf_pct = (confidence * 100.0) as i32;
        match conf_pct {
            90..=100 => "90-100%".to_string(),
            80..=89 => "80-89%".to_string(),
            70..=79 => "70-79%".to_string(),
            60..=69 => "60-69%".to_string(),
            50..=59 => "50-59%".to_string(),
            _ => "0-49%".to_string(),
        }
    }
}
