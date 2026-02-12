use anyhow::Result;
use reqwest::Client;
use serde_json::json;

pub struct DiscordNotifier {
    client: Client,
    webhook_url: String,
}

impl DiscordNotifier {
    pub fn new(webhook_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self {
            client,
            webhook_url,
        })
    }

    pub async fn send_message(&self, content: &str) -> Result<()> {
        if self.webhook_url.is_empty() {
            tracing::debug!("Discord webhook not configured, skipping notification");
            return Ok(());
        }

        let payload = json!({
            "content": content,
            "username": "InvestIQ Trading Agent",
        });

        match self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::warn!("Discord webhook returned {}", resp.status());
                } else {
                    tracing::debug!("Discord notification sent");
                }
            }
            Err(e) => {
                tracing::warn!("Discord notification failed (non-fatal): {}", e);
            }
        }

        Ok(())
    }

    pub async fn send_daily_report(&self, report: &DailyReport) -> Result<()> {
        let ml_total = report.signals_ml_approved + report.signals_ml_rejected;
        let ml_rate = if ml_total > 0 {
            (report.signals_ml_approved as f64 / ml_total as f64) * 100.0
        } else {
            0.0
        };

        let mut message = format!(
            r#"**Daily Trading Report**

**P/L**: ${:.2} ({:+.2}%)
**Trades**: {}
**Win Rate**: {:.1}%
**Best Trade**: {} (${:.2})
**Worst Trade**: {} (${:.2})

**Signal Funnel**: Generated: {} -> Filtered: {} -> ML Approved: {} ({:.0}% rate)
**Conviction**: HIGH: {} | MODERATE: {} | LOW: {}
"#,
            report.pnl,
            report.pnl_percent,
            report.trade_count,
            report.win_rate * 100.0,
            report.best_trade_symbol,
            report.best_trade_pnl,
            report.worst_trade_symbol,
            report.worst_trade_pnl,
            report.signals_generated,
            report.signals_filtered,
            report.signals_ml_approved,
            ml_rate,
            report.conviction_high,
            report.conviction_moderate,
            report.conviction_low,
        );

        // Supplementary signal summary (only if any counts > 0)
        let supp_total = report.insider_signals
            + report.smart_money_boosts
            + report.iv_penalties
            + report.gap_boosts;
        if supp_total > 0 {
            message.push_str(&format!(
                "**Supplementary**: Insider: {} | Smart$: {} | IV Pen: {} | Gap: {}\n",
                report.insider_signals,
                report.smart_money_boosts,
                report.iv_penalties,
                report.gap_boosts,
            ));
        }

        message.push_str(&format!(
            r#"
**Account Balance**: ${:.2}
**Positions Held**: {}
**Largest Position**: {}
**Exposure**: {:.1}%
**Market Regime**: {}
"#,
            report.account_balance,
            report.positions_held,
            report.largest_position,
            report.exposure_percent,
            report.regime,
        ));

        self.send_message(&message).await
    }
}

#[derive(Debug)]
pub struct DailyReport {
    pub pnl: f64,
    pub pnl_percent: f64,
    pub trade_count: usize,
    pub win_rate: f64,
    pub best_trade_symbol: String,
    pub best_trade_pnl: f64,
    pub worst_trade_symbol: String,
    pub worst_trade_pnl: f64,
    pub account_balance: f64,
    pub positions_held: usize,
    pub largest_position: String,
    pub exposure_percent: f64,
    pub regime: String,
    // Signal funnel
    pub signals_generated: u64,
    pub signals_filtered: u64,
    pub signals_ml_approved: u64,
    pub signals_ml_rejected: u64,
    // Conviction breakdown
    pub conviction_high: usize,
    pub conviction_moderate: usize,
    pub conviction_low: usize,
    // Supplementary signal counts
    pub insider_signals: usize,
    pub smart_money_boosts: usize,
    pub iv_penalties: usize,
    pub gap_boosts: usize,
}
