use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::run_plugin::RunPluginInput;
use crate::signals::SIGNAL_AGENT_STOP;

/// Patrol workflow — runs on a timer, executes registered plugins.
/// Used by Deacon agent for periodic maintenance tasks.
pub async fn patrol_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let interval_secs = if let Some(payload) = args.first() {
        serde_json::from_slice::<u64>(&payload.data).unwrap_or(60)
    } else {
        60
    };

    let mut stop_ch = ctx.make_signal_channel(SIGNAL_AGENT_STOP);
    let mut cycles: u64 = 0;

    tracing::info!("Patrol started — interval {interval_secs}s");

    loop {
        // Use tokio::select to race between timer and stop signal
        tokio::select! {
            _ = ctx.timer(Duration::from_secs(interval_secs)) => {
                cycles += 1;
                tracing::info!("Patrol cycle {cycles}");

                // In a full implementation, we'd discover plugins and check gates here.
                // For now, run a heartbeat plugin as a placeholder.
                let input = RunPluginInput {
                    plugin_name: "patrol-heartbeat".to_string(),
                    command: "echo".to_string(),
                    args: vec![format!("patrol cycle {cycles}")],
                    work_dir: None,
                };

                let _result = ctx
                    .activity(ActivityOptions {
                        activity_type: "run_plugin".to_string(),
                        input: input.as_json_payload()?,
                        start_to_close_timeout: Some(Duration::from_secs(30)),
                        ..Default::default()
                    })
                    .await;
            }
            Some(_) = stop_ch.next() => {
                tracing::info!("Patrol stopped after {cycles} cycles");
                return Ok(WfExitValue::Normal(
                    format!("{{\"cycles\":{cycles}}}")
                ));
            }
        }
    }
}
