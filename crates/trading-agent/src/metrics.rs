use std::collections::VecDeque;
use std::time::Instant;

/// Structured telemetry for the trading agent (P7).
/// Tracks per-cycle timing, aggregate stats, and rolling performance windows.
pub struct AgentMetrics {
    pub cycles_run: u64,
    pub signals_generated: u64,
    pub signals_filtered: u64,
    pub signals_ml_approved: u64,
    pub signals_ml_rejected: u64,
    pub trades_executed: u64,
    pub trades_failed: u64,
    pub total_pnl: f64,
    pub winning_trades: u64,
    pub losing_trades: u64,

    // Per-cycle timing (last cycle)
    pub last_scan_duration_ms: u64,
    pub last_analysis_duration_ms: u64,
    pub last_gate_duration_ms: u64,
    pub last_execution_duration_ms: u64,
    pub last_total_duration_ms: u64,

    // Rolling 20-trade window
    recent_trades: VecDeque<TradeRecord>,
    log_interval_cycles: u64,
}

struct TradeRecord {
    pnl: f64,
    won: bool,
}

impl AgentMetrics {
    pub fn new(log_interval_cycles: u64) -> Self {
        Self {
            cycles_run: 0,
            signals_generated: 0,
            signals_filtered: 0,
            signals_ml_approved: 0,
            signals_ml_rejected: 0,
            trades_executed: 0,
            trades_failed: 0,
            total_pnl: 0.0,
            winning_trades: 0,
            losing_trades: 0,
            last_scan_duration_ms: 0,
            last_analysis_duration_ms: 0,
            last_gate_duration_ms: 0,
            last_execution_duration_ms: 0,
            last_total_duration_ms: 0,
            recent_trades: VecDeque::with_capacity(20),
            log_interval_cycles,
        }
    }

    pub fn start_timer() -> Instant {
        Instant::now()
    }

    pub fn record_scan_duration(&mut self, start: Instant) {
        self.last_scan_duration_ms = start.elapsed().as_millis() as u64;
    }

    pub fn record_analysis_duration(&mut self, start: Instant) {
        self.last_analysis_duration_ms = start.elapsed().as_millis() as u64;
    }

    pub fn record_gate_duration(&mut self, start: Instant) {
        self.last_gate_duration_ms = start.elapsed().as_millis() as u64;
    }

    pub fn record_execution_duration(&mut self, start: Instant) {
        self.last_execution_duration_ms = start.elapsed().as_millis() as u64;
    }

    pub fn record_trade_result(&mut self, pnl: f64) {
        let won = pnl > 0.0;
        self.total_pnl += pnl;
        if won {
            self.winning_trades += 1;
        } else {
            self.losing_trades += 1;
        }

        self.recent_trades.push_back(TradeRecord { pnl, won });
        if self.recent_trades.len() > 20 {
            self.recent_trades.pop_front();
        }
    }

    pub fn finish_cycle(&mut self, cycle_start: Instant) {
        self.last_total_duration_ms = cycle_start.elapsed().as_millis() as u64;
        self.cycles_run += 1;

        // Emit structured metrics periodically
        if self.log_interval_cycles > 0 && self.cycles_run.is_multiple_of(self.log_interval_cycles)
        {
            self.log_metrics();
        }
    }

    /// Rolling win rate from last 20 trades (0-100%)
    pub fn recent_win_rate(&self) -> f64 {
        if self.recent_trades.is_empty() {
            return 0.0;
        }
        let wins = self.recent_trades.iter().filter(|t| t.won).count() as f64;
        (wins / self.recent_trades.len() as f64) * 100.0
    }

    /// Rolling average P&L from last 20 trades
    pub fn recent_avg_pnl(&self) -> f64 {
        if self.recent_trades.is_empty() {
            return 0.0;
        }
        self.recent_trades.iter().map(|t| t.pnl).sum::<f64>() / self.recent_trades.len() as f64
    }

    /// Overall win rate (0-100%)
    pub fn overall_win_rate(&self) -> f64 {
        let total = self.winning_trades + self.losing_trades;
        if total == 0 {
            return 0.0;
        }
        (self.winning_trades as f64 / total as f64) * 100.0
    }

    /// Emit structured telemetry via tracing
    pub fn log_metrics(&self) {
        tracing::info!(
            cycles = self.cycles_run,
            signals_generated = self.signals_generated,
            signals_filtered = self.signals_filtered,
            signals_ml_approved = self.signals_ml_approved,
            signals_ml_rejected = self.signals_ml_rejected,
            trades_executed = self.trades_executed,
            trades_failed = self.trades_failed,
            total_pnl = format!("{:.2}", self.total_pnl),
            overall_win_rate = format!("{:.1}%", self.overall_win_rate()),
            recent_win_rate = format!("{:.1}%", self.recent_win_rate()),
            recent_avg_pnl = format!("{:.2}", self.recent_avg_pnl()),
            last_cycle_ms = self.last_total_duration_ms,
            last_scan_ms = self.last_scan_duration_ms,
            last_analysis_ms = self.last_analysis_duration_ms,
            "Agent metrics summary"
        );
    }

    /// Serialize metrics to JSON for state persistence
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "cycles_run": self.cycles_run,
            "signals_generated": self.signals_generated,
            "signals_filtered": self.signals_filtered,
            "signals_ml_approved": self.signals_ml_approved,
            "signals_ml_rejected": self.signals_ml_rejected,
            "trades_executed": self.trades_executed,
            "trades_failed": self.trades_failed,
            "total_pnl": self.total_pnl,
            "winning_trades": self.winning_trades,
            "losing_trades": self.losing_trades,
        })
    }

    /// Restore counters from persisted JSON
    pub fn restore_from_json(&mut self, json: &serde_json::Value) {
        if let Some(v) = json.get("cycles_run").and_then(|v| v.as_u64()) {
            self.cycles_run = v;
        }
        if let Some(v) = json.get("signals_generated").and_then(|v| v.as_u64()) {
            self.signals_generated = v;
        }
        if let Some(v) = json.get("signals_filtered").and_then(|v| v.as_u64()) {
            self.signals_filtered = v;
        }
        if let Some(v) = json.get("signals_ml_approved").and_then(|v| v.as_u64()) {
            self.signals_ml_approved = v;
        }
        if let Some(v) = json.get("signals_ml_rejected").and_then(|v| v.as_u64()) {
            self.signals_ml_rejected = v;
        }
        if let Some(v) = json.get("trades_executed").and_then(|v| v.as_u64()) {
            self.trades_executed = v;
        }
        if let Some(v) = json.get("trades_failed").and_then(|v| v.as_u64()) {
            self.trades_failed = v;
        }
        if let Some(v) = json.get("total_pnl").and_then(|v| v.as_f64()) {
            self.total_pnl = v;
        }
        if let Some(v) = json.get("winning_trades").and_then(|v| v.as_u64()) {
            self.winning_trades = v;
        }
        if let Some(v) = json.get("losing_trades").and_then(|v| v.as_u64()) {
            self.losing_trades = v;
        }
        tracing::info!(
            "Restored metrics from persisted state (cycles={})",
            self.cycles_run
        );
    }
}
