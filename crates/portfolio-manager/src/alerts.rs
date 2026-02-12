use crate::models::*;
use crate::db::PortfolioDb;
use anyhow::Result;
use rust_decimal::prelude::*;

pub struct AlertManager {
    db: PortfolioDb,
}

impl AlertManager {
    pub fn new(db: PortfolioDb) -> Self {
        Self { db }
    }

    /// Create a new alert
    pub async fn create_alert(&self, alert: AlertInput) -> Result<i64> {
        let (id,): (i64,) = sqlx::query_as(
            r#"
            INSERT INTO alerts
            (symbol, alert_type, signal, confidence, current_price, target_price, stop_loss_price, reason, expires_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id
            "#
        )
        .bind(&alert.symbol)
        .bind(&alert.alert_type)
        .bind(&alert.signal)
        .bind(alert.confidence)
        .bind(alert.current_price.map(|p| p.to_f64().unwrap_or(0.0)))
        .bind(alert.target_price.map(|p| p.to_f64().unwrap_or(0.0)))
        .bind(alert.stop_loss_price.map(|p| p.to_f64().unwrap_or(0.0)))
        .bind(&alert.reason)
        .bind(&alert.expires_at)
        .fetch_one(self.db.pool())
        .await?;

        Ok(id)
    }

    /// Get all active alerts
    pub async fn get_active_alerts(&self) -> Result<Vec<Alert>> {
        let alerts = sqlx::query_as::<_, Alert>(
            r#"
            SELECT * FROM alerts
            WHERE status = 'active'
            AND (expires_at IS NULL OR expires_at > ?)
            ORDER BY created_at DESC
            "#
        )
        .bind(chrono::Utc::now().to_rfc3339())
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
                "UPDATE alerts SET status = ?, completed_at = ? WHERE id = ?"
            )
            .bind(status)
            .bind(chrono::Utc::now().to_rfc3339())
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
            "UPDATE alerts SET status = 'completed', completed_at = ? WHERE id = ?"
        )
        .bind(chrono::Utc::now().to_rfc3339())
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
            "UPDATE alerts SET status = 'expired' WHERE status = 'active' AND expires_at <= ?"
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(self.db.pool())
        .await?;

        Ok(result.rows_affected())
    }

    // ======== Alert Execution Tracking ========

    /// Record that an alert was acted on (trade executed).
    pub async fn record_execution(
        &self,
        alert_id: i64,
        trade_id: i64,
        execution_price: f64,
    ) -> Result<i64> {
        // Get alert details
        let alert = self.get_alert(alert_id).await?
            .ok_or_else(|| anyhow::anyhow!("Alert not found: {}", alert_id))?;

        let (id,): (i64,) = sqlx::query_as(
            r#"
            INSERT INTO alert_executions
            (alert_id, trade_id, symbol, alert_signal, alert_confidence, alert_price, execution_price, executed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id
            "#
        )
        .bind(alert_id)
        .bind(trade_id)
        .bind(&alert.symbol)
        .bind(&alert.signal)
        .bind(alert.confidence)
        .bind(alert.current_price)
        .bind(execution_price)
        .bind(chrono::Utc::now().to_rfc3339())
        .fetch_one(self.db.pool())
        .await?;

        Ok(id)
    }

    /// Update execution outcome when position is closed.
    pub async fn update_execution_outcome(
        &self,
        trade_id: i64,
        pnl: f64,
        pnl_percent: f64,
    ) -> Result<()> {
        let outcome = if pnl > 0.0 {
            "profit"
        } else if pnl < 0.0 {
            "loss"
        } else {
            "breakeven"
        };

        sqlx::query(
            "UPDATE alert_executions SET outcome = ?, outcome_pnl = ?, outcome_pnl_percent = ?, closed_at = ? WHERE trade_id = ?"
        )
        .bind(outcome)
        .bind(pnl)
        .bind(pnl_percent)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(trade_id)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// Get accuracy report for alert executions.
    pub async fn get_accuracy_report(&self, days: Option<i64>) -> Result<AlertAccuracyReport> {
        let executions = if let Some(d) = days {
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(d)).to_rfc3339();
            sqlx::query_as::<_, AlertExecution>(
                "SELECT * FROM alert_executions WHERE created_at >= ? ORDER BY created_at DESC"
            )
            .bind(&cutoff)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, AlertExecution>(
                "SELECT * FROM alert_executions ORDER BY created_at DESC"
            )
            .fetch_all(self.db.pool())
            .await?
        };

        let total = executions.len();
        let profitable = executions.iter().filter(|e| e.outcome == "profit").count();
        let unprofitable = executions.iter().filter(|e| e.outcome == "loss").count();
        let still_open = executions.iter().filter(|e| e.outcome == "open").count();

        let closed = total - still_open;
        let accuracy = if closed > 0 {
            profitable as f64 / closed as f64 * 100.0
        } else {
            0.0
        };

        let closed_execs: Vec<&AlertExecution> = executions.iter().filter(|e| e.outcome != "open").collect();
        let avg_pnl = if !closed_execs.is_empty() {
            closed_execs.iter().filter_map(|e| e.outcome_pnl).sum::<f64>() / closed_execs.len() as f64
        } else {
            0.0
        };
        let avg_pnl_pct = if !closed_execs.is_empty() {
            closed_execs.iter().filter_map(|e| e.outcome_pnl_percent).sum::<f64>() / closed_execs.len() as f64
        } else {
            0.0
        };

        // By signal
        let mut by_signal: std::collections::HashMap<String, SignalAccuracy> = std::collections::HashMap::new();
        for exec in &executions {
            if exec.outcome == "open" {
                continue;
            }
            let entry = by_signal
                .entry(exec.alert_signal.clone())
                .or_insert_with(|| SignalAccuracy {
                    total: 0,
                    profitable: 0,
                    accuracy_percent: 0.0,
                    avg_pnl: 0.0,
                });
            entry.total += 1;
            if exec.outcome == "profit" {
                entry.profitable += 1;
            }
            entry.avg_pnl += exec.outcome_pnl.unwrap_or(0.0);
        }
        for entry in by_signal.values_mut() {
            if entry.total > 0 {
                entry.accuracy_percent = entry.profitable as f64 / entry.total as f64 * 100.0;
                entry.avg_pnl /= entry.total as f64;
            }
        }

        Ok(AlertAccuracyReport {
            total_executions: total,
            profitable,
            unprofitable,
            still_open,
            accuracy_percent: accuracy,
            avg_pnl,
            avg_pnl_percent: avg_pnl_pct,
            by_signal,
        })
    }

    /// Get executions for a specific alert.
    pub async fn get_executions_for_alert(&self, alert_id: i64) -> Result<Vec<AlertExecution>> {
        let executions = sqlx::query_as::<_, AlertExecution>(
            "SELECT * FROM alert_executions WHERE alert_id = ? ORDER BY created_at DESC"
        )
        .bind(alert_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(executions)
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
            current_price: Some(Decimal::from(150)),
            target_price: Some(Decimal::from(180)),
            stop_loss_price: Some(Decimal::from(140)),
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
            current_price: Some(Decimal::from(150)),
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
