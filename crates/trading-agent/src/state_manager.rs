use anyhow::Result;
use sqlx::SqlitePool;

/// Persistent state manager for the trading agent (P8).
/// Stores key-value state and trade entry context for post-trade analysis.
pub struct StateManager {
    db_pool: SqlitePool,
}

impl StateManager {
    pub fn new(db_pool: SqlitePool) -> Self {
        Self { db_pool }
    }

    /// Initialize agent state tables.
    pub async fn init_tables(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agent_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&self.db_pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agent_trade_context (
                order_id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                entry_regime TEXT,
                entry_signals TEXT,
                entry_supplementary TEXT,
                entry_confidence REAL,
                entry_atr REAL,
                signal_adjustments TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Save a state key-value pair.
    pub async fn save_state(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO agent_state (key, value, updated_at) VALUES (?, ?, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }

    /// Load a state value by key.
    pub async fn load_state(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM agent_state WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.db_pool)
                .await?;
        Ok(row.map(|(v,)| v))
    }

    /// Save trade entry context for post-trade analysis.
    #[allow(dead_code)]
    pub async fn save_trade_context(
        &self,
        order_id: &str,
        symbol: &str,
        regime: Option<&str>,
        confidence: f64,
        atr: Option<f64>,
        signal_adjustments: &[String],
        supplementary: Option<&serde_json::Value>,
    ) -> Result<()> {
        let adjustments_json = serde_json::to_string(signal_adjustments).unwrap_or_default();
        let supp_json = supplementary
            .map(|s| serde_json::to_string(s).unwrap_or_default())
            .unwrap_or_default();

        sqlx::query(
            "INSERT OR REPLACE INTO agent_trade_context
             (order_id, symbol, entry_regime, entry_confidence, entry_atr, signal_adjustments, entry_supplementary, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))",
        )
        .bind(order_id)
        .bind(symbol)
        .bind(regime)
        .bind(confidence)
        .bind(atr)
        .bind(&adjustments_json)
        .bind(&supp_json)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }

    /// Persist metrics to DB for recovery after restart.
    pub async fn save_metrics(&self, metrics_json: &serde_json::Value) -> Result<()> {
        let json_str = serde_json::to_string(metrics_json)?;
        self.save_state("agent_metrics", &json_str).await
    }

    /// Load persisted metrics.
    pub async fn load_metrics(&self) -> Result<Option<serde_json::Value>> {
        match self.load_state("agent_metrics").await? {
            Some(s) => Ok(serde_json::from_str(&s).ok()),
            None => Ok(None),
        }
    }

    /// Save last report date to avoid duplicate daily reports.
    pub async fn save_last_report_date(&self, date: &str) -> Result<()> {
        self.save_state("last_report_date", date).await
    }

    /// Load last report date.
    pub async fn load_last_report_date(&self) -> Result<Option<String>> {
        self.load_state("last_report_date").await
    }
}
