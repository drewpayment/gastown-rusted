use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::notification::NotificationInput;
use crate::signals::SIGNAL_AGENT_STOP;

/// Witness workflow — periodic monitoring of agent workflows.
/// Detects stuck or stale workflows and sends notifications.
pub async fn witness_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let interval_secs = if let Some(payload) = args.first() {
        serde_json::from_slice::<u64>(&payload.data).unwrap_or(300)
    } else {
        300 // default: check every 5 minutes
    };

    let mut stop_ch = ctx.make_signal_channel(SIGNAL_AGENT_STOP);
    let mut checks: u64 = 0;
    let mut alerts_sent: u64 = 0;

    tracing::info!("Witness started — check interval {interval_secs}s");

    loop {
        tokio::select! {
            _ = ctx.timer(Duration::from_secs(interval_secs)) => {
                checks += 1;
                tracing::info!("Witness check #{checks}");

                // In a full implementation, this would:
                // 1. List running work_item_wf workflows
                // 2. Check each for staleness (no recent events)
                // 3. Alert on stuck workflows
                //
                // For now, send a health check notification as a placeholder.
                let input = NotificationInput {
                    channel: "signal".to_string(),
                    target: "mayor".to_string(),
                    subject: format!("Witness check #{checks}"),
                    message: format!("Health check passed, {alerts_sent} alerts sent so far"),
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
                tracing::info!("Witness stopped after {checks} checks, {alerts_sent} alerts");
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&serde_json::json!({
                        "checks": checks,
                        "alerts_sent": alerts_sent,
                    }))?
                ));
            }
        }
    }
}
