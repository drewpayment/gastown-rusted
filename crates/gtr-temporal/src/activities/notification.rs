use serde::{Deserialize, Serialize};
use temporalio_sdk::{ActContext, ActivityError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationInput {
    pub channel: String, // "email", "sms", "webhook", "signal"
    pub target: String,
    pub subject: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationResult {
    pub channel: String,
    pub target: String,
    pub sent: bool,
}

pub async fn send_notification(
    _ctx: ActContext,
    input: NotificationInput,
) -> Result<NotificationResult, ActivityError> {
    tracing::info!(
        "Notification [{}] to {}: {} — {}",
        input.channel,
        input.target,
        input.subject,
        input.message,
    );

    match input.channel.as_str() {
        "email" => {
            // Stub: log only. Real implementation would use an email provider (SES, SendGrid, etc.)
            tracing::info!("Would send email to {}: {}", input.target, input.subject);
        }
        "sms" => {
            // Stub: log only. Real implementation would use Twilio or similar.
            tracing::info!("Would send SMS to {}: {}", input.target, input.message);
        }
        "webhook" => {
            let url = &input.target;
            let body = serde_json::json!({
                "subject": input.subject,
                "message": input.message,
                "channel": input.channel,
            });
            let client = reqwest::Client::new();
            let resp = client
                .post(url)
                .json(&body)
                .send()
                .await
                .map_err(|e| ActivityError::Retryable {
                    source: anyhow::anyhow!("webhook failed: {e}"),
                    explicit_delay: None,
                })?;
            tracing::info!("Notification webhook to {url}: {}", resp.status());
        }
        "signal" => {
            tracing::info!("Would signal workflow {}", input.target);
        }
        other => {
            tracing::warn!("Unknown notification channel: {other}");
        }
    }

    Ok(NotificationResult {
        channel: input.channel,
        target: input.target,
        sent: true,
    })
}
