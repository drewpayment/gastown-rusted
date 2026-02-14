use std::time::Duration;

use temporalio_common::protos::coresdk::activity_result::activity_resolution::Status;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::run_plugin::RunPluginInput;
use crate::signals::{
    RefineryEntry, RefineryEnqueueSignal, RefineryState, SIGNAL_REFINERY_DEQUEUE,
    SIGNAL_REFINERY_ENQUEUE, SIGNAL_REFINERY_STOP,
};

use futures_util::StreamExt;

pub async fn refinery_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let mut queue: Vec<RefineryEntry> = Vec::new();
    let mut processed: Vec<RefineryEntry> = Vec::new();

    let mut enqueue_ch = ctx.make_signal_channel(SIGNAL_REFINERY_ENQUEUE);
    let mut dequeue_ch = ctx.make_signal_channel(SIGNAL_REFINERY_DEQUEUE);
    let mut stop_ch = ctx.make_signal_channel(SIGNAL_REFINERY_STOP);

    tracing::info!("Refinery started — merge queue ready");

    loop {
        // Wait for any signal
        tokio::select! {
            biased;
            Some(signal) = enqueue_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(enq) = serde_json::from_slice::<RefineryEnqueueSignal>(&payload.data) {
                        tracing::info!("Refinery: enqueue '{}' branch '{}'", enq.work_item_id, enq.branch);
                        queue.push(RefineryEntry {
                            work_item_id: enq.work_item_id,
                            branch: enq.branch,
                            priority: enq.priority,
                            status: "queued".to_string(),
                        });
                    }
                }
            }
            Some(signal) = dequeue_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(deq) = serde_json::from_slice::<crate::signals::RefineryDequeueSignal>(&payload.data) {
                        tracing::info!("Refinery: dequeue '{}'", deq.work_item_id);
                        queue.retain(|e| e.work_item_id != deq.work_item_id);
                    }
                }
            }
            Some(_) = stop_ch.next() => {
                tracing::info!("Refinery: stopping");
                break;
            }
        }

        // Sort by priority (lower = higher priority)
        queue.sort_by_key(|e| e.priority);

        // Process all queued items sequentially
        while let Some(idx) = queue.iter().position(|e| e.status == "queued") {
            let item_id = queue[idx].work_item_id.clone();
            let branch = queue[idx].branch.clone();
            queue[idx].status = "validating".to_string();

            // Run validation
            let validate_input = RunPluginInput {
                plugin_name: format!("refinery:validate:{item_id}"),
                command: "echo".to_string(),
                args: vec![format!("validating {branch}")],
                work_dir: None,
            };

            let result = ctx
                .activity(ActivityOptions {
                    activity_type: "run_plugin".to_string(),
                    input: validate_input.as_json_payload()?,
                    start_to_close_timeout: Some(Duration::from_secs(600)),
                    ..Default::default()
                })
                .await;

            match result.status {
                Some(Status::Completed(_)) => {
                    // Run merge
                    let merge_input = RunPluginInput {
                        plugin_name: format!("refinery:merge:{item_id}"),
                        command: "echo".to_string(),
                        args: vec![format!("merged {branch}")],
                        work_dir: None,
                    };

                    let merge_result = ctx
                        .activity(ActivityOptions {
                            activity_type: "run_plugin".to_string(),
                            input: merge_input.as_json_payload()?,
                            start_to_close_timeout: Some(Duration::from_secs(300)),
                            ..Default::default()
                        })
                        .await;

                    let mut entry = queue.remove(idx);
                    match merge_result.status {
                        Some(Status::Completed(_)) => {
                            entry.status = "merged".to_string();
                            tracing::info!("Refinery: merged '{}'", entry.work_item_id);
                        }
                        _ => {
                            entry.status = "merge_failed".to_string();
                            tracing::warn!(
                                "Refinery: merge failed for '{}'",
                                entry.work_item_id
                            );
                        }
                    }
                    processed.push(entry);
                }
                _ => {
                    let mut entry = queue.remove(idx);
                    entry.status = "validation_failed".to_string();
                    tracing::warn!(
                        "Refinery: validation failed for '{}'",
                        entry.work_item_id
                    );
                    processed.push(entry);
                }
            }
        }
    }

    let state = RefineryState { queue, processed };
    Ok(WfExitValue::Normal(serde_json::to_string(&state)?))
}
