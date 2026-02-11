use crate::models::*;
use crate::db::PortfolioDb;
use anyhow::Result;

pub struct AlertManager {
    db: PortfolioDb,
}

impl AlertManager {
    pub fn new(db: PortfolioDb) -> Self {
        Self { db }
    }

    /// Create a new alert
    pub async fn create_alert(&self, alert: AlertInput) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO alerts
            (symbol, alert_type, signal, confidence, current_price, target_price, stop_loss_price, reason, expires_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&alert.symbol)
        .bind(&alert.alert_type)
        .bind(&alert.signal)
        .bind(alert.confidence)
        .bind(alert.current_price)
        .bind(alert.target_price)
        .bind(alert.stop_loss_price)
        .bind(&alert.reason)
        .bind(&alert.expires_at)
        .execute(self.db.pool())
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get all active alerts
    pub async fn get_active_alerts(&self) -> Result<Vec<Alert>> {
        let alerts = sqlx::query_as::<_, Alert>(
            r#"
            SELECT * FROM alerts
            WHERE status = 'active'
            AND (expires_at IS NULL OR expires_at > datetime('now'))
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(self.db.pool())
        .await?;

        Ok(alerts)
    }

    /// Get alerts for a specific symbol
    pub async fn get_alerts_for_symbol(&self, symbol: &str) -> Result<Vec<Alert>> {
        let alerts = sqlx::query_as::<_, Alert>(
            r#"
            SELECT * FROM alerts
            WHERE symbol = ? AND status = 'active'
            ORDER BY created_at DESC
            "#
        )
        .bind(symbol)
        .fetch_all(self.db.pool())
        .await?;

        Ok(alerts)
    }

    /// Get alert by ID
    pub async fn get_alert(&self, id: i64) -> Result<Option<Alert>> {
        let alert = sqlx::query_as::<_, Alert>(
            "SELECT * FROM alerts WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(alert)
    }

    /// Update alert status
    pub async fn update_alert_status(&self, id: i64, status: &str) -> Result<()> {
        if status == "completed" {
            sqlx::query(
                "UPDATE alerts SET status = ?, completed_at = datetime('now') WHERE id = ?"
            )
            .bind(status)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        } else {
            sqlx::query("UPDATE alerts SET status = ? WHERE id = ?")
                .bind(status)
                .bind(id)
                .execute(self.db.pool())
                .await?;
        }

        Ok(())
    }

    /// Mark alert as completed
    pub async fn complete_alert(&self, id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE alerts SET status = 'completed', completed_at = datetime('now') WHERE id = ?"
        )
        .bind(id)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// Mark alert as ignored
    pub async fn ignore_alert(&self, id: i64) -> Result<()> {
        self.update_alert_status(id, "ignored").await
    }

    /// Delete alert
    pub async fn delete_alert(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM alerts WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }

    /// Clean up expired alerts
    pub async fn cleanup_expired_alerts(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE alerts SET status = 'expired' WHERE status = 'active' AND expires_at <= datetime('now')"
        )
        .execute(self.db.pool())
        .await?;

        Ok(result.rows_affected())
    }

    /// Get all alerts (including inactive)
    pub async fn get_all_alerts(&self, limit: Option<i64>) -> Result<Vec<Alert>> {
        let alerts = if let Some(lim) = limit {
            sqlx::query_as::<_, Alert>(
                "SELECT * FROM alerts ORDER BY created_at DESC LIMIT ?"
            )
            .bind(lim)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, Alert>(
                "SELECT * FROM alerts ORDER BY created_at DESC"
            )
            .fetch_all(self.db.pool())
            .await?
        };

        Ok(alerts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> PortfolioDb {
        PortfolioDb::new("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get_alert() {
        let db = setup_test_db().await;
        let manager = AlertManager::new(db);

        let alert = AlertInput {
            symbol: "AAPL".to_string(),
            alert_type: "buy".to_string(),
            signal: "StrongBuy".to_string(),
            confidence: 0.85,
            current_price: Some(150.0),
            target_price: Some(180.0),
            stop_loss_price: Some(140.0),
            reason: Some("Bullish signals".to_string()),
            expires_at: None,
        };

        let id = manager.create_alert(alert).await.unwrap();
        assert!(id > 0);

        let active = manager.get_active_alerts().await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].symbol, "AAPL");
    }

    #[tokio::test]
    async fn test_update_alert_status() {
        let db = setup_test_db().await;
        let manager = AlertManager::new(db);

        let alert = AlertInput {
            symbol: "AAPL".to_string(),
            alert_type: "buy".to_string(),
            signal: "Buy".to_string(),
            confidence: 0.75,
            current_price: Some(150.0),
            target_price: None,
            stop_loss_price: None,
            reason: None,
            expires_at: None,
        };

        let id = manager.create_alert(alert).await.unwrap();
        manager.complete_alert(id).await.unwrap();

        let active = manager.get_active_alerts().await.unwrap();
        assert_eq!(active.len(), 0);

        let retrieved = manager.get_alert(id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, "completed");
    }
}
