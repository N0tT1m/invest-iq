use std::collections::HashMap;

use crate::metrics::AgentMetrics;
use crate::types::{GateDecision, TradingSignal};
use analysis_core::UnifiedAnalysis;
use anyhow::Result;

/// Persistent state manager for the trading agent (P8).
/// Stores key-value state and trade entry context for post-trade analysis.
pub struct StateManager {
    pub(crate) db_pool: sqlx::AnyPool,
}

impl StateManager {
    pub fn new(db_pool: sqlx::AnyPool) -> Self {
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

        // v2 context table (belt-and-suspenders alongside migration)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agent_trade_context_v2 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pending_trade_id INTEGER,
                symbol TEXT NOT NULL,
                action TEXT NOT NULL,
                entry_price REAL,
                stop_loss REAL,
                take_profit REAL,
                entry_regime TEXT,
                conviction_tier TEXT,
                entry_confidence REAL,
                entry_atr REAL,
                ml_probability REAL,
                ml_reasoning TEXT,
                ml_features_json TEXT,
                technical_reason TEXT,
                fundamental_reason TEXT,
                sentiment_score REAL,
                signal_adjustments TEXT,
                supplementary_signals TEXT,
                engine_signals_json TEXT,
                time_horizon_signals TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                exit_regime TEXT,
                exit_reason TEXT,
                exit_price REAL,
                exit_date TEXT,
                pnl REAL,
                pnl_percent REAL,
                outcome TEXT
            )",
        )
        .execute(&self.db_pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agent_daily_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snapshot_date TEXT NOT NULL UNIQUE,
                cycles_run INTEGER DEFAULT 0,
                signals_generated INTEGER DEFAULT 0,
                signals_filtered INTEGER DEFAULT 0,
                signals_ml_approved INTEGER DEFAULT 0,
                signals_ml_rejected INTEGER DEFAULT 0,
                trades_proposed INTEGER DEFAULT 0,
                winning_trades INTEGER DEFAULT 0,
                losing_trades INTEGER DEFAULT 0,
                total_pnl REAL DEFAULT 0,
                regime TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&self.db_pool)
        .await?;

        // Create indexes if missing
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_atc2_pending ON agent_trade_context_v2(pending_trade_id)")
            .execute(&self.db_pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_atc2_symbol ON agent_trade_context_v2(symbol)")
            .execute(&self.db_pool)
            .await
            .ok();
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_atc2_outcome ON agent_trade_context_v2(outcome)",
        )
        .execute(&self.db_pool)
        .await
        .ok();
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_atc2_created ON agent_trade_context_v2(created_at)",
        )
        .execute(&self.db_pool)
        .await
        .ok();

        Ok(())
    }

    /// Save a state key-value pair.
    pub async fn save_state(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO agent_state (key, value, updated_at) VALUES (?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }

    /// Load a state value by key.
    pub async fn load_state(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM agent_state WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.db_pool)
            .await?;
        Ok(row.map(|(v,)| v))
    }

    /// Save rich trade context (v2) for post-trade analysis.
    /// Returns the context row id.
    pub async fn save_trade_context_v2(
        &self,
        pending_trade_id: i64,
        signal: &TradingSignal,
        analysis: &UnifiedAnalysis,
        decision: &GateDecision,
    ) -> Result<i64> {
        // Build engine signals JSON
        let engine_signals = build_engine_signals_json(analysis);
        let engine_signals_str = serde_json::to_string(&engine_signals).unwrap_or_default();

        let ml_features_json = decision
            .features
            .as_ref()
            .map(|f| serde_json::to_string(f).unwrap_or_default());

        let adjustments_json =
            serde_json::to_string(&signal.signal_adjustments).unwrap_or_default();

        let supp_json = analysis
            .supplementary_signals
            .as_ref()
            .map(|s| serde_json::to_string(s).unwrap_or_default());

        let time_horizon_json = analysis
            .time_horizon_signals
            .as_ref()
            .map(|t| serde_json::to_string(t).unwrap_or_default());

        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO agent_trade_context_v2
             (pending_trade_id, symbol, action, entry_price, stop_loss, take_profit,
              entry_regime, conviction_tier, entry_confidence, entry_atr,
              ml_probability, ml_reasoning, ml_features_json,
              technical_reason, fundamental_reason, sentiment_score,
              signal_adjustments, supplementary_signals, engine_signals_json, time_horizon_signals)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id",
        )
        .bind(pending_trade_id)
        .bind(&signal.symbol)
        .bind(&signal.action)
        .bind(signal.entry_price)
        .bind(signal.stop_loss)
        .bind(signal.take_profit)
        .bind(signal.regime.as_deref())
        .bind(signal.conviction_tier.as_deref())
        .bind(signal.confidence)
        .bind(signal.atr)
        .bind(decision.probability)
        .bind(&decision.reasoning)
        .bind(ml_features_json.as_deref())
        .bind(&signal.technical_reason)
        .bind(signal.fundamental_reason.as_deref())
        .bind(signal.sentiment_score)
        .bind(&adjustments_json)
        .bind(supp_json.as_deref())
        .bind(&engine_signals_str)
        .bind(time_horizon_json.as_deref())
        .fetch_one(&self.db_pool)
        .await?;

        Ok(id)
    }

    /// Record exit information for a trade context row.
    pub async fn record_trade_exit(
        &self,
        pending_trade_id: i64,
        exit_reason: &str,
        exit_price: f64,
        pnl: f64,
        pnl_percent: f64,
        exit_regime: Option<&str>,
    ) -> Result<()> {
        let outcome = if pnl > 0.01 {
            "win"
        } else if pnl < -0.01 {
            "loss"
        } else {
            "breakeven"
        };

        sqlx::query(
            "UPDATE agent_trade_context_v2
             SET exit_reason = ?, exit_price = ?, exit_date = ?,
                 pnl = ?, pnl_percent = ?, exit_regime = ?, outcome = ?
             WHERE pending_trade_id = ?",
        )
        .bind(exit_reason)
        .bind(exit_price)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(pnl)
        .bind(pnl_percent)
        .bind(exit_regime)
        .bind(outcome)
        .bind(pending_trade_id)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }

    /// Record a trade rejection in context.
    #[allow(dead_code)]
    pub async fn record_trade_rejected(&self, pending_trade_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE agent_trade_context_v2
             SET exit_reason = 'REJECTED', outcome = 'rejected', exit_date = ?
             WHERE pending_trade_id = ?",
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(pending_trade_id)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }

    /// Save a daily snapshot of agent metrics.
    pub async fn save_daily_snapshot(
        &self,
        date: &str,
        metrics: &AgentMetrics,
        regime: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO agent_daily_snapshots
             (snapshot_date, cycles_run, signals_generated, signals_filtered,
              signals_ml_approved, signals_ml_rejected, trades_proposed,
              winning_trades, losing_trades, total_pnl, regime)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(snapshot_date) DO UPDATE SET
              cycles_run = excluded.cycles_run,
              signals_generated = excluded.signals_generated,
              signals_filtered = excluded.signals_filtered,
              signals_ml_approved = excluded.signals_ml_approved,
              signals_ml_rejected = excluded.signals_ml_rejected,
              trades_proposed = excluded.trades_proposed,
              winning_trades = excluded.winning_trades,
              losing_trades = excluded.losing_trades,
              total_pnl = excluded.total_pnl,
              regime = excluded.regime",
        )
        .bind(date)
        .bind(metrics.cycles_run as i64)
        .bind(metrics.signals_generated as i64)
        .bind(metrics.signals_filtered as i64)
        .bind(metrics.signals_ml_approved as i64)
        .bind(metrics.signals_ml_rejected as i64)
        .bind(metrics.trades_executed as i64)
        .bind(metrics.winning_trades as i64)
        .bind(metrics.losing_trades as i64)
        .bind(metrics.total_pnl)
        .bind(regime)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }

    /// Get best and worst trades today (by P&L).
    pub async fn get_best_worst_trades_today(
        &self,
    ) -> Result<(Option<(String, f64)>, Option<(String, f64)>)> {
        let best: Option<(String, f64)> = sqlx::query_as(
            "SELECT symbol, pnl FROM agent_trade_context_v2
             WHERE date(exit_date) = date('now') AND outcome IS NOT NULL AND outcome != 'rejected'
             ORDER BY pnl DESC LIMIT 1",
        )
        .fetch_optional(&self.db_pool)
        .await?;

        let worst: Option<(String, f64)> = sqlx::query_as(
            "SELECT symbol, pnl FROM agent_trade_context_v2
             WHERE date(exit_date) = date('now') AND outcome IS NOT NULL AND outcome != 'rejected'
             ORDER BY pnl ASC LIMIT 1",
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok((best, worst))
    }

    /// Get conviction tier breakdown for today's signals.
    pub async fn get_conviction_breakdown_today(&self) -> Result<HashMap<String, i64>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT COALESCE(conviction_tier, 'UNKNOWN'), COUNT(*)
             FROM agent_trade_context_v2
             WHERE date(created_at) = date('now')
             GROUP BY conviction_tier",
        )
        .fetch_all(&self.db_pool)
        .await?;

        Ok(rows.into_iter().collect())
    }

    /// Get signal adjustment type counts for today.
    pub async fn get_adjustment_summary_today(&self) -> Result<HashMap<String, usize>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT signal_adjustments FROM agent_trade_context_v2
             WHERE date(created_at) = date('now') AND signal_adjustments IS NOT NULL",
        )
        .fetch_all(&self.db_pool)
        .await?;

        let mut counts: HashMap<String, usize> = HashMap::new();
        for (json_str,) in rows {
            if let Ok(adjustments) = serde_json::from_str::<Vec<String>>(&json_str) {
                for adj in adjustments {
                    // Extract the adjustment type (before the parenthesis)
                    let adj_type = adj.split('(').next().unwrap_or(&adj).trim().to_string();
                    *counts.entry(adj_type).or_insert(0) += 1;
                }
            }
        }

        Ok(counts)
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

/// Build a JSON object summarizing each engine's signal, confidence, and reason.
fn build_engine_signals_json(analysis: &UnifiedAnalysis) -> serde_json::Value {
    let mut engines = serde_json::Map::new();

    if let Some(t) = &analysis.technical {
        engines.insert(
            "technical".to_string(),
            serde_json::json!({
                "signal": t.signal.to_label(),
                "confidence": t.confidence,
                "reason": t.reason,
            }),
        );
    }
    if let Some(f) = &analysis.fundamental {
        engines.insert(
            "fundamental".to_string(),
            serde_json::json!({
                "signal": f.signal.to_label(),
                "confidence": f.confidence,
                "reason": f.reason,
            }),
        );
    }
    if let Some(q) = &analysis.quantitative {
        engines.insert(
            "quantitative".to_string(),
            serde_json::json!({
                "signal": q.signal.to_label(),
                "confidence": q.confidence,
                "reason": q.reason,
            }),
        );
    }
    if let Some(s) = &analysis.sentiment {
        engines.insert(
            "sentiment".to_string(),
            serde_json::json!({
                "signal": s.signal.to_label(),
                "confidence": s.confidence,
                "reason": s.reason,
            }),
        );
    }

    serde_json::Value::Object(engines)
}
