use std::time::Duration;

use temporalio_common::protos::coresdk::activity_result::activity_resolution::Status;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::git_ops::GitOperation;
use crate::activities::run_plugin::RunPluginInput;
use crate::signals::{
    RefineryEntry, RefineryEnqueueSignal, RefineryState, SIGNAL_REFINERY_DEQUEUE,
    SIGNAL_REFINERY_ENQUEUE, SIGNAL_REFINERY_STOP,
};

use futures_util::StreamExt;

/// Refinery v2 — real git rebase, test execution, and conflict detection.
/// For each queued work item:
/// 1. Checkout branch (git_operation activity)
/// 2. Rebase onto main (git_operation activity)
/// 3. Run tests (run_plugin activity)
/// 4. If tests pass: merge to main (git_operation activity)
/// 5. If rebase fails: mark as conflict, spawn conflict-resolution polecat
pub async fn refinery_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let repo_path = if let Some(payload) = args.first() {
        serde_json::from_slice::<String>(&payload.data).unwrap_or_else(|_| ".".into())
    } else {
        ".".into()
    };

    let mut queue: Vec<RefineryEntry> = Vec::new();
    let mut processed: Vec<RefineryEntry> = Vec::new();

    let mut enqueue_ch = ctx.make_signal_channel(SIGNAL_REFINERY_ENQUEUE);
    let mut dequeue_ch = ctx.make_signal_channel(SIGNAL_REFINERY_DEQUEUE);
    let mut stop_ch = ctx.make_signal_channel(SIGNAL_REFINERY_STOP);

    tracing::info!("Refinery started — merge queue ready (repo: {repo_path})");

    loop {
        // Wait for any signal
        tokio::select! {
            biased;
            Some(_) = stop_ch.next() => {
                tracing::info!("Refinery: stopping");
                break;
            }
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
        }

        // Sort by priority (lower = higher priority)
        queue.sort_by_key(|e| e.priority);

        // Process all queued items sequentially
        while let Some(idx) = queue.iter().position(|e| e.status == "queued") {
            let item_id = queue[idx].work_item_id.clone();
            let branch = queue[idx].branch.clone();
            queue[idx].status = "validating".to_string();

            // Step 1: Checkout the feature branch
            let checkout_op = GitOperation::Checkout {
                repo_path: repo_path.clone(),
                branch: branch.clone(),
                create: false,
            };

            let checkout_result = ctx
                .activity(ActivityOptions {
                    activity_type: "git_operation".to_string(),
                    input: checkout_op.as_json_payload()?,
                    start_to_close_timeout: Some(Duration::from_secs(120)),
                    ..Default::default()
                })
                .await;

            if !checkout_result.completed_ok() {
                let mut entry = queue.remove(idx);
                entry.status = "checkout_failed".to_string();
                tracing::warn!("Refinery: checkout failed for '{item_id}' branch '{branch}'");
                processed.push(entry);
                continue;
            }

            // Step 2: Rebase onto main
            let rebase_op = GitOperation::Rebase {
                repo_path: repo_path.clone(),
                branch: branch.clone(),
                onto: "main".to_string(),
            };

            let rebase_result = ctx
                .activity(ActivityOptions {
                    activity_type: "git_operation".to_string(),
                    input: rebase_op.as_json_payload()?,
                    start_to_close_timeout: Some(Duration::from_secs(300)),
                    ..Default::default()
                })
                .await;

            if !rebase_result.completed_ok() {
                let mut entry = queue.remove(idx);
                entry.status = "conflict".to_string();
                tracing::warn!(
                    "Refinery: rebase conflict for '{item_id}' — needs conflict resolution"
                );
                processed.push(entry);
                continue;
            }

            // Step 3: Run tests via run_plugin
            let test_input = RunPluginInput {
                plugin_name: format!("refinery:test:{item_id}"),
                command: "cargo".to_string(),
                args: vec!["test".to_string()],
                work_dir: Some(repo_path.clone()),
            };

            let test_result = ctx
                .activity(ActivityOptions {
                    activity_type: "run_plugin".to_string(),
                    input: test_input.as_json_payload()?,
                    start_to_close_timeout: Some(Duration::from_secs(600)),
                    ..Default::default()
                })
                .await;

            if !test_result.completed_ok() {
                let mut entry = queue.remove(idx);
                entry.status = "tests_failed".to_string();
                tracing::warn!("Refinery: tests failed for '{item_id}' after rebase");
                processed.push(entry);
                continue;
            }

            // Step 4: Checkout main and merge the rebased branch
            let checkout_main = GitOperation::Checkout {
                repo_path: repo_path.clone(),
                branch: "main".to_string(),
                create: false,
            };

            let _ = ctx
                .activity(ActivityOptions {
                    activity_type: "git_operation".to_string(),
                    input: checkout_main.as_json_payload()?,
                    start_to_close_timeout: Some(Duration::from_secs(60)),
                    ..Default::default()
                })
                .await;

            let merge_op = GitOperation::Merge {
                repo_path: repo_path.clone(),
                branch: branch.clone(),
            };

            let merge_result = ctx
                .activity(ActivityOptions {
                    activity_type: "git_operation".to_string(),
                    input: merge_op.as_json_payload()?,
                    start_to_close_timeout: Some(Duration::from_secs(300)),
                    ..Default::default()
                })
                .await;

            let mut entry = queue.remove(idx);
            match merge_result.status {
                Some(Status::Completed(_)) => {
                    entry.status = "merged".to_string();
                    tracing::info!("Refinery: merged '{item_id}' branch '{branch}'");

                    // Step 5: Push main to remote
                    let push_op = GitOperation::Push {
                        repo_path: repo_path.clone(),
                        remote: "origin".to_string(),
                        branch: "main".to_string(),
                    };
                    let push_result = ctx
                        .activity(ActivityOptions {
                            activity_type: "git_operation".to_string(),
                            input: push_op.as_json_payload()?,
                            start_to_close_timeout: Some(Duration::from_secs(120)),
                            ..Default::default()
                        })
                        .await;

                    if !push_result.completed_ok() {
                        tracing::warn!("Refinery: push to remote failed for '{item_id}' — merged locally but not pushed");
                        entry.status = "merged_push_failed".to_string();
                    } else {
                        tracing::info!("Refinery: pushed main to remote after merging '{item_id}'");
                    }
                }
                _ => {
                    entry.status = "merge_failed".to_string();
                    tracing::warn!("Refinery: merge failed for '{item_id}'");
                }
            }
            processed.push(entry);
        }
    }

    let state = RefineryState { queue, processed };
    Ok(WfExitValue::Normal(serde_json::to_string(&state)?))
}
