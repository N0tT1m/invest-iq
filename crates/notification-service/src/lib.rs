mod smtp;
mod templates;

pub use smtp::SmtpNotifier;
pub use templates::EmailTemplate;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Alert types that trigger notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    TradeExecuted {
        symbol: String,
        action: String,
        shares: f64,
        price: f64,
        confidence: Option<f64>,
    },
    CircuitBreakerTripped {
        reason: String,
    },
    StopLossHit {
        symbol: String,
        entry_price: f64,
        stop_price: f64,
        loss_percent: f64,
    },
    AgentTradeProposal {
        symbol: String,
        action: String,
        confidence: f64,
        reason: String,
    },
    DailyReport {
        date: String,
        pnl: f64,
        trades_count: i32,
        positions_count: i32,
    },
}

/// A notification alert to be dispatched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub alert_type: AlertType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub title: String,
    pub message: String,
}

impl Alert {
    pub fn new(
        alert_type: AlertType,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            alert_type,
            timestamp: chrono::Utc::now(),
            title: title.into(),
            message: message.into(),
        }
    }
}

/// Trait for notification channels.
#[async_trait]
pub trait NotificationChannel: Send + Sync {
    async fn send(&self, alert: &Alert) -> Result<(), NotificationError>;
    fn name(&self) -> &str;
}

/// Errors from the notification system.
#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("SMTP error: {0}")]
    Smtp(String),
    #[error("Discord webhook error: {0}")]
    Discord(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Configuration for the notification service.
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: Option<String>,
    pub smtp_to: Vec<String>,
    pub smtp_tls: SmtpTls,
    pub discord_webhook_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub enum SmtpTls {
    #[default]
    StartTls,
    Tls,
    None,
}

impl NotificationConfig {
    /// Load from environment variables.
    pub fn from_env() -> Self {
        let smtp_to = std::env::var("NOTIFICATION_EMAIL_TO")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let smtp_tls = match std::env::var("SMTP_TLS").unwrap_or_default().as_str() {
            "tls" => SmtpTls::Tls,
            "none" => SmtpTls::None,
            _ => SmtpTls::StartTls,
        };

        Self {
            smtp_host: std::env::var("SMTP_HOST").ok().filter(|s| !s.is_empty()),
            smtp_port: std::env::var("SMTP_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(587),
            smtp_username: std::env::var("SMTP_USERNAME")
                .ok()
                .filter(|s| !s.is_empty()),
            smtp_password: std::env::var("SMTP_PASSWORD")
                .ok()
                .filter(|s| !s.is_empty()),
            smtp_from: std::env::var("SMTP_FROM_ADDRESS")
                .ok()
                .filter(|s| !s.is_empty()),
            smtp_to,
            smtp_tls,
            discord_webhook_url: std::env::var("DISCORD_WEBHOOK_URL")
                .ok()
                .filter(|s| !s.is_empty()),
        }
    }
}

/// The main notification service — dispatches alerts to all configured channels.
pub struct NotificationService {
    channels: std::sync::Arc<Vec<Box<dyn NotificationChannel>>>,
}

impl NotificationService {
    pub fn new(config: &NotificationConfig) -> Self {
        let mut channels: Vec<Box<dyn NotificationChannel>> = Vec::new();

        // Add SMTP channel if configured
        if config.smtp_host.is_some() && config.smtp_from.is_some() && !config.smtp_to.is_empty() {
            match SmtpNotifier::new(config) {
                Ok(notifier) => {
                    tracing::info!(
                        "Email notifications enabled (SMTP -> {} recipients)",
                        config.smtp_to.len()
                    );
                    channels.push(Box::new(notifier));
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize SMTP notifier: {}", e);
                }
            }
        }

        // Add Discord webhook channel if configured
        if let Some(ref webhook_url) = config.discord_webhook_url {
            channels.push(Box::new(DiscordWebhookNotifier {
                webhook_url: webhook_url.clone(),
                client: reqwest::Client::new(),
            }));
            tracing::info!("Discord webhook notifications enabled");
        }

        if channels.is_empty() {
            tracing::info!(
                "No notification channels configured (set SMTP_HOST or DISCORD_WEBHOOK_URL)"
            );
        }

        Self {
            channels: std::sync::Arc::new(channels),
        }
    }

    /// Send an alert to all configured channels (fire-and-forget via tokio::spawn).
    pub fn send_alert(&self, alert: Alert) {
        let channels = self.channels.clone();
        tokio::spawn(async move {
            for channel in channels.iter() {
                match channel.send(&alert).await {
                    Ok(()) => tracing::debug!("Sent notification via {}", channel.name()),
                    Err(e) => {
                        tracing::warn!("Failed to send notification via {}: {}", channel.name(), e)
                    }
                }
            }
        });
    }

    /// Send alert to all channels, awaiting completion.
    pub async fn send_alert_async(&self, alert: &Alert) {
        for channel in self.channels.iter() {
            match channel.send(alert).await {
                Ok(()) => tracing::debug!("Sent notification via {}", channel.name()),
                Err(e) => {
                    tracing::warn!("Failed to send notification via {}: {}", channel.name(), e)
                }
            }
        }
    }
}

/// Discord webhook notifier.
struct DiscordWebhookNotifier {
    webhook_url: String,
    client: reqwest::Client,
}

#[async_trait]
impl NotificationChannel for DiscordWebhookNotifier {
    async fn send(&self, alert: &Alert) -> Result<(), NotificationError> {
        let color = match &alert.alert_type {
            AlertType::TradeExecuted { action, .. } => {
                if action == "buy" {
                    0x00ff00
                } else {
                    0xff0000
                }
            }
            AlertType::CircuitBreakerTripped { .. } => 0xff0000,
            AlertType::StopLossHit { .. } => 0xff6600,
            AlertType::AgentTradeProposal { .. } => 0x0099ff,
            AlertType::DailyReport { pnl, .. } => {
                if *pnl >= 0.0 {
                    0x00ff00
                } else {
                    0xff0000
                }
            }
        };

        let payload = serde_json::json!({
            "embeds": [{
                "title": alert.title,
                "description": alert.message,
                "color": color,
                "timestamp": alert.timestamp.to_rfc3339(),
            }]
        });

        self.client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| NotificationError::Discord(e.to_string()))?;

        Ok(())
    }

    fn name(&self) -> &str {
        "discord-webhook"
    }
}
