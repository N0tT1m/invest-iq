//! Preference Learning Module
//!
//! Learns user preferences from their interactions with the system.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, Row, SqlitePool};
use std::collections::HashMap;

use crate::models::{InteractionType, SymbolInteraction, UserPreference};

/// Row returned from interactions query
#[derive(Debug, FromRow)]
struct InteractionRow {
    symbol: String,
    action: String,
    created_at: DateTime<Utc>,
}

/// Row returned from preferences query
#[derive(Debug, FromRow)]
struct PreferenceRow {
    user_id: String,
    preferred_sectors: Option<String>,
    risk_tolerance: Option<f64>,
}

/// Learns and manages user preferences
pub struct PreferenceLearner {
    pool: SqlitePool,
    /// Decay factor for older interactions (per day)
    decay_factor: f64,
}

impl PreferenceLearner {
    /// Create a new preference learner
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            decay_factor: 0.99, // 1% decay per day
        }
    }

    /// Record a user interaction
    pub async fn record_interaction(&self, interaction: &SymbolInteraction) -> Result<i64> {
        let action_str = interaction.action.as_str();

        let result = sqlx::query(
            r#"
            INSERT INTO symbol_interactions (user_id, symbol, action, created_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&interaction.user_id)
        .bind(&interaction.symbol)
        .bind(action_str)
        .bind(interaction.created_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get user preferences
    pub async fn get_preferences(&self, user_id: &str) -> Result<UserPreference> {
        // Try to load from database
        let row: Option<PreferenceRow> = sqlx::query_as(
            r#"
            SELECT user_id, preferred_sectors, risk_tolerance
            FROM user_preferences
            WHERE user_id = ?
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let mut prefs = if let Some(row) = row {
            let sectors: Vec<String> = row
                .preferred_sectors
                .as_ref()
                .map(|s| serde_json::from_str(s).unwrap_or_default())
                .unwrap_or_default();

            UserPreference {
                user_id: row.user_id,
                preferred_sectors: sectors,
                risk_tolerance: row.risk_tolerance.unwrap_or(0.5),
                ..Default::default()
            }
        } else {
            UserPreference {
                user_id: user_id.to_string(),
                ..Default::default()
            }
        };

        // Learn affinities from recent interactions
        let (symbol_affinities, sector_affinities) = self.calculate_affinities(user_id).await?;
        prefs.symbol_affinities = symbol_affinities;
        prefs.sector_affinities = sector_affinities;
        prefs.updated_at = Utc::now();

        Ok(prefs)
    }

    /// Save user preferences
    pub async fn save_preferences(&self, prefs: &UserPreference) -> Result<()> {
        let sectors_json = serde_json::to_string(&prefs.preferred_sectors)?;

        sqlx::query(
            r#"
            INSERT INTO user_preferences (user_id, preferred_sectors, risk_tolerance, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(user_id) DO UPDATE SET
                preferred_sectors = excluded.preferred_sectors,
                risk_tolerance = excluded.risk_tolerance,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&prefs.user_id)
        .bind(&sectors_json)
        .bind(prefs.risk_tolerance)
        .bind(prefs.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Calculate symbol and sector affinities from interactions
    async fn calculate_affinities(
        &self,
        user_id: &str,
    ) -> Result<(HashMap<String, f64>, HashMap<String, f64>)> {
        // Get recent interactions (last 90 days)
        let interactions: Vec<InteractionRow> = sqlx::query_as(
            r#"
            SELECT symbol, action, created_at
            FROM symbol_interactions
            WHERE user_id = ?
            AND created_at > datetime('now', '-90 days')
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut symbol_scores: HashMap<String, f64> = HashMap::new();

        for row in interactions {
            let action = InteractionType::from_str(&row.action);
            let weight = action.preference_weight();

            // Apply time decay
            let duration = Utc::now().signed_duration_since(row.created_at);
            let age_days = duration.num_days().max(0) as u32;
            let decay = self.decay_factor.powi(age_days as i32);

            *symbol_scores.entry(row.symbol).or_insert(0.0) += weight * decay;
        }

        // Normalize to -1 to 1 range
        let max_abs = symbol_scores
            .values()
            .map(|v| v.abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(1.0)
            .max(1.0);

        for score in symbol_scores.values_mut() {
            *score /= max_abs;
        }

        // For now, sector affinities would require looking up symbol sectors
        // This is a placeholder - in production you'd join with a sector table
        let sector_affinities = HashMap::new();

        Ok((symbol_scores, sector_affinities))
    }

    /// Get symbols the user is interested in (positive affinity)
    pub async fn get_interested_symbols(&self, user_id: &str, limit: i32) -> Result<Vec<String>> {
        let prefs = self.get_preferences(user_id).await?;

        let mut sorted: Vec<_> = prefs
            .symbol_affinities
            .into_iter()
            .filter(|(_, score)| *score > 0.0)
            .collect();

        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        sorted.truncate(limit as usize);

        Ok(sorted.into_iter().map(|(s, _)| s).collect())
    }

    /// Get symbols the user has dismissed (negative affinity)
    pub async fn get_dismissed_symbols(&self, user_id: &str) -> Result<Vec<String>> {
        let prefs = self.get_preferences(user_id).await?;

        Ok(prefs
            .symbol_affinities
            .into_iter()
            .filter(|(_, score)| *score < -0.3)
            .map(|(s, _)| s)
            .collect())
    }

    /// Update preferences based on explicit user settings
    pub async fn update_explicit_preferences(
        &self,
        user_id: &str,
        sectors: Option<Vec<String>>,
        risk_tolerance: Option<f64>,
        excluded_symbols: Option<Vec<String>>,
    ) -> Result<UserPreference> {
        let mut prefs = self.get_preferences(user_id).await?;

        if let Some(s) = sectors {
            prefs.preferred_sectors = s;
        }
        if let Some(r) = risk_tolerance {
            prefs.risk_tolerance = r.clamp(0.0, 1.0);
        }
        if let Some(e) = excluded_symbols {
            prefs.excluded_symbols = e;
        }

        prefs.updated_at = Utc::now();
        self.save_preferences(&prefs).await?;

        Ok(prefs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interaction_weights() {
        assert!(InteractionType::Trade.preference_weight() > InteractionType::Click.preference_weight());
        assert!(InteractionType::Dismiss.preference_weight() < 0.0);
    }

    #[test]
    fn test_default_preferences() {
        let prefs = UserPreference::default();
        assert_eq!(prefs.risk_tolerance, 0.5);
        assert!(!prefs.preferred_signals.is_empty());
    }
}
