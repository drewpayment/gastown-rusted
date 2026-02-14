use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::git_ops::GitOperation;
use crate::activities::heartbeat::HeartbeatInput;
use crate::activities::spawn_agent::SpawnAgentInput;
use crate::signals::*;

/// Polecat workflow — ephemeral worker lifecycle.
/// Lifecycle: create worktree → spawn agent → heartbeat loop → done/kill → cleanup.
pub async fn polecat_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (name, rig, work_item_id, title) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String, String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "unknown".into(), "unknown".into(), "untitled".into()))
    } else {
        ("unknown".into(), "unknown".into(), "unknown".into(), "untitled".into())
    };

    let polecat_id = format!("{rig}-polecat-{name}");
    let branch = format!("polecat/{name}/{work_item_id}");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let worktree_path = format!("{home}/.gtr/rigs/{rig}/polecats/{name}");
    let mut status = "working".to_string();

    tracing::info!("Polecat {name} started on rig {rig}: {title}");

    // Step 1: Create git worktree
    let worktree_op = GitOperation::WorktreeAdd {
        repo_path: format!("{home}/.gtr/rigs/{rig}/.repo.git"),
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

    // Step 2: Spawn Claude Code agent via PTY
    let spawn_input = SpawnAgentInput {
        agent_id: polecat_id.clone(),
        runtime: "claude".to_string(),
        work_dir: worktree_path.clone(),
        role: format!("{rig}/polecats/{name}"),
        rig: Some(rig.clone()),
        initial_prompt: Some(format!(
            "You are polecat '{name}' on rig '{rig}'. Your work item: {work_item_id} — {title}.\n\
             Work in this directory. When done, run: gtr done {work_item_id} --branch {branch}"
        )),
        env_extra: Some({
            let mut m = std::collections::HashMap::new();
            m.insert("GTR_WORK_ITEM".into(), work_item_id.clone());
            m
        }),
    };

    let spawn_result = ctx
        .activity(ActivityOptions {
            activity_type: "spawn_agent".to_string(),
            input: spawn_input.as_json_payload()?,
            start_to_close_timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        })
        .await;

    if !spawn_result.completed_ok() {
        tracing::error!("Polecat {name}: failed to spawn agent");
        return Ok(WfExitValue::Normal(serde_json::to_string(&PolecatState {
            name,
            rig,
            work_item_id,
            status: "spawn_failed".into(),
            branch,
            worktree_path,
        })?));
    }

    tracing::info!("Polecat {name}: agent spawned, entering heartbeat loop");

    // Step 3: Heartbeat loop — check every 60s if agent is alive
    let mut done_ch = ctx.make_signal_channel(SIGNAL_POLECAT_DONE);
    let mut kill_ch = ctx.make_signal_channel(SIGNAL_POLECAT_KILL);
    let mut stuck_ch = ctx.make_signal_channel(SIGNAL_POLECAT_STUCK);

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
                    }
                }
                status = "done".to_string();
                break;
            }
            Some(_) = stuck_ch.next() => {
                status = "stuck".to_string();
                tracing::warn!("Polecat {name} reports stuck");
                // Continue running — witness will handle escalation
            }
            _ = ctx.timer(Duration::from_secs(60)) => {
                // Heartbeat check
                let hb_input = HeartbeatInput {
                    agent_id: polecat_id.clone(),
                };
                let hb_result = ctx
                    .activity(ActivityOptions {
                        activity_type: "check_agent_alive".to_string(),
                        input: hb_input.as_json_payload()?,
                        start_to_close_timeout: Some(Duration::from_secs(10)),
                        ..Default::default()
                    })
                    .await;

                if !hb_result.completed_ok() {
                    status = "stuck".to_string();
                    tracing::warn!("Polecat {name}: agent process died — stuck");
                    // Don't break — allow kill/done signals to clean up
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
