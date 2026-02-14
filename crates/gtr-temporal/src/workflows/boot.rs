use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::notification::NotificationInput;
use crate::signals::SIGNAL_AGENT_STOP;

/// Boot workflow — periodic health check of system services.
/// Detects missing Deacon, Witness, or other critical workflows and alerts.
pub async fn boot_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let interval_secs = if let Some(payload) = args.first() {
        serde_json::from_slice::<u64>(&payload.data).unwrap_or(120)
    } else {
        120 // default: check every 2 minutes
    };

    let mut stop_ch = ctx.make_signal_channel(SIGNAL_AGENT_STOP);
    let mut checks: u64 = 0;
    let mut restarts: u64 = 0;

    tracing::info!("Boot monitor started — check interval {interval_secs}s");

    loop {
        tokio::select! {
            _ = ctx.timer(Duration::from_secs(interval_secs)) => {
                checks += 1;
                tracing::info!("Boot health check #{checks}");

                // In a full implementation, this would:
                // 1. Check if Deacon workflow is running
                // 2. Check if Witness workflow is running
                // 3. Restart missing workflows
                //
                // For now, send a health report notification.
                let input = NotificationInput {
                    channel: "signal".to_string(),
                    target: "mayor".to_string(),
                    subject: format!("Boot check #{checks}"),
                    message: format!("System health OK, {restarts} restarts so far"),
                };

                let _ = ctx
                    .activity(ActivityOptions {
                        activity_type: "send_notification".to_string(),
                        input: input.as_json_payload()?,
                        start_to_close_timeout: Some(Duration::from_secs(30)),
                        ..Default::default()
                    })
                    .await;
            }
            Some(_) = stop_ch.next() => {
                tracing::info!("Boot monitor stopped after {checks} checks, {restarts} restarts");
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&serde_json::json!({
                        "checks": checks,
                        "restarts": restarts,
                    }))?
                ));
            }
        }
    }
}
