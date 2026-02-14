use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::git_ops::GitOperation;
use crate::signals::*;

/// Polecat workflow — ephemeral worker lifecycle.
/// States: working → done | stuck | zombie (no idle state).
/// Lifecycle: spawn worktree → work → done/kill → cleanup.
pub async fn polecat_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (name, rig, work_item_id, title) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String, String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "unknown".into(), "unknown".into(), "untitled".into()))
    } else {
        ("unknown".into(), "unknown".into(), "unknown".into(), "untitled".into())
    };

    let branch = format!("polecat/{name}/{work_item_id}");
    let worktree_path = format!("{rig}/polecats/{name}/{rig}");
    let mut status = "working".to_string();

    tracing::info!("Polecat {name} started on rig {rig}: {title}");

    // Step 1: Create git worktree
    let worktree_op = GitOperation::WorktreeAdd {
        repo_path: format!("{rig}/.repo.git"),
        path: worktree_path.clone(),
        branch: branch.clone(),
    };
    let worktree_result = ctx
        .activity(ActivityOptions {
            activity_type: "git_operation".to_string(),
            input: worktree_op.as_json_payload()?,
            start_to_close_timeout: Some(Duration::from_secs(120)),
            ..Default::default()
        })
        .await;

    if !worktree_result.completed_ok() {
        tracing::error!("Polecat {name}: failed to create worktree");
        return Ok(WfExitValue::Normal(serde_json::to_string(&PolecatState {
            name,
            rig,
            work_item_id,
            status: "failed".into(),
            branch,
            worktree_path,
        })?));
    }

    // Step 2: Listen for lifecycle signals
    let mut heartbeat_ch = ctx.make_signal_channel(SIGNAL_POLECAT_HEARTBEAT);
    let mut done_ch = ctx.make_signal_channel(SIGNAL_POLECAT_DONE);
    let mut stuck_ch = ctx.make_signal_channel(SIGNAL_POLECAT_STUCK);
    let mut kill_ch = ctx.make_signal_channel(SIGNAL_POLECAT_KILL);

    loop {
        tokio::select! {
            biased;
            Some(_) = kill_ch.next() => {
                tracing::info!("Polecat {name} killed");
                status = "zombie".to_string();
                break;
            }
            Some(signal) = done_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<PolecatDoneSignal>(&payload.data) {
                        tracing::info!("Polecat {name} done: {}", data.status);
                        status = "done".to_string();
                    }
                }
                break;
            }
            Some(_) = stuck_ch.next() => {
                status = "stuck".to_string();
                tracing::warn!("Polecat {name} reports stuck");
                // Continue running — witness will handle escalation
            }
            Some(_) = heartbeat_ch.next() => {
                tracing::debug!("Polecat {name} heartbeat");
            }
            _ = ctx.timer(Duration::from_secs(1800)) => {
                if status == "working" {
                    status = "stuck".to_string();
                    tracing::warn!("Polecat {name} stale — no heartbeat in 30m");
                }
            }
        }
    }

    Ok(WfExitValue::Normal(serde_json::to_string(&PolecatState {
        name,
        rig,
        work_item_id,
        status,
        branch,
        worktree_path,
    })?))
}
