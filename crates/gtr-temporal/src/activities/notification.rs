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

    // Stub: just log. Real implementation would dispatch to email/SMS/webhook providers.
    match input.channel.as_str() {
        "email" => {
            tracing::info!("Would send email to {}: {}", input.target, input.subject);
        }
        "sms" => {
            tracing::info!("Would send SMS to {}: {}", input.target, input.message);
        }
        "webhook" => {
            tracing::info!("Would POST to webhook {}", input.target);
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
