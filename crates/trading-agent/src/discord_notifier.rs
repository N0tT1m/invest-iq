use anyhow::Result;
use reqwest::Client;
use serde_json::json;

pub struct DiscordNotifier {
    client: Client,
    webhook_url: String,
}

impl DiscordNotifier {
    pub fn new(webhook_url: String) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
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

        self.client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        tracing::debug!("Discord notification sent");
        Ok(())
    }

    pub async fn send_daily_report(&self, report: &DailyReport) -> Result<()> {
        let message = format!(
            r#"**Daily Trading Report**

**P/L**: ${:.2} ({:+.2}%)
**Trades**: {}
**Win Rate**: {:.1}%
**Best Trade**: {} (${:.2})
**Worst Trade**: {} (${:.2})

**Account Balance**: ${:.2}
"#,
            report.pnl,
            report.pnl_percent,
            report.trade_count,
            report.win_rate * 100.0,
            report.best_trade_symbol,
            report.best_trade_pnl,
            report.worst_trade_symbol,
            report.worst_trade_pnl,
            report.account_balance
        );

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
}
