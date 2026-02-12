use async_trait::async_trait;
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use crate::templates::EmailTemplate;
use crate::{Alert, NotificationChannel, NotificationConfig, NotificationError, SmtpTls};

pub struct SmtpNotifier {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    to: Vec<Mailbox>,
}

impl SmtpNotifier {
    pub fn new(config: &NotificationConfig) -> Result<Self, NotificationError> {
        let host = config
            .smtp_host
            .as_deref()
            .ok_or_else(|| NotificationError::Config("SMTP_HOST not set".into()))?;
        let from_addr = config
            .smtp_from
            .as_deref()
            .ok_or_else(|| NotificationError::Config("SMTP_FROM_ADDRESS not set".into()))?;

        let from: Mailbox = from_addr
            .parse()
            .map_err(|e| NotificationError::Config(format!("Invalid from address: {}", e)))?;

        let to: Vec<Mailbox> = config
            .smtp_to
            .iter()
            .filter_map(|addr| addr.parse().ok())
            .collect();

        if to.is_empty() {
            return Err(NotificationError::Config(
                "No valid NOTIFICATION_EMAIL_TO addresses".into(),
            ));
        }

        let mut builder = match config.smtp_tls {
            SmtpTls::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(host),
            SmtpTls::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host),
            SmtpTls::None => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
                host,
            )),
        }
        .map_err(|e| NotificationError::Smtp(format!("SMTP transport error: {}", e)))?;

        builder = builder.port(config.smtp_port);

        if let (Some(user), Some(pass)) = (&config.smtp_username, &config.smtp_password) {
            builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
        }

        let transport = builder.build();

        Ok(Self {
            transport,
            from,
            to,
        })
    }
}

#[async_trait]
impl NotificationChannel for SmtpNotifier {
    async fn send(&self, alert: &Alert) -> Result<(), NotificationError> {
        let html_body = EmailTemplate::render(alert);

        for recipient in &self.to {
            let email = Message::builder()
                .from(self.from.clone())
                .to(recipient.clone())
                .subject(&alert.title)
                .header(ContentType::TEXT_HTML)
                .body(html_body.clone())
                .map_err(|e| NotificationError::Smtp(format!("Failed to build email: {}", e)))?;

            self.transport
                .send(email)
                .await
                .map_err(|e| NotificationError::Smtp(format!("Failed to send email: {}", e)))?;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "smtp"
    }
}
