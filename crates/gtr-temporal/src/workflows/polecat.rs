use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, SignalWorkflowOptions, WfContext, WfExitValue};

use crate::activities::heartbeat::{CapturePaneInput, CapturePaneOutput, HeartbeatInput};
use crate::activities::spawn_agent::SpawnAgentInput;
use crate::activities::git_ops::GitOperation;
use crate::signals::*;

/// Polecat workflow — ephemeral worker lifecycle.
/// Lifecycle: create worktree → spawn agent → heartbeat loop → report to mayor → cleanup.
///
/// GUARANTEE: Every exit path sends a PolecatReportSignal to the mayor workflow
/// before returning, so the mayor always has visibility into polecat outcomes.
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

    // Tracking state — accumulated through all paths, used in the final report.
    let mut status = "working".to_string();
    let mut exit_reason = "unknown".to_string();
    let mut agent_summary: Option<String> = None;
    let mut agent_spawned = false;

    tracing::info!("Polecat {name} started on rig {rig}: {title}");

    // ─── Step 1: Create git worktree ───
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
        status = "failed".to_string();
        exit_reason = "worktree_failed".to_string();
    }

    // ─── Step 2: Spawn Claude Code agent (only if worktree succeeded) ───
    if status == "working" {
        let spawn_input = SpawnAgentInput {
            agent_id: polecat_id.clone(),
            runtime: "claude".to_string(),
            work_dir: worktree_path.clone(),
            role: format!("{rig}/polecats/{name}"),
            rig: Some(rig.clone()),
            initial_prompt: Some(format!(
                "You are polecat '{name}' on rig '{rig}'. Your work item: {work_item_id} — {title}.\n\
                 Work in this directory.\n\n\
                 IMPORTANT: You MUST run this command when your work is complete:\n\
                 $RGT_BIN done {work_item_id} --branch $GTR_BRANCH --summary \"<what you did>\"\n\n\
                 This is NOT optional. The system cannot merge your work without this signal.\n\
                 Do NOT exit or stop without running this command first."
            )),
            env_extra: Some({
                let mut m = std::collections::HashMap::new();
                m.insert("GTR_WORK_ITEM".into(), work_item_id.clone());
                m.insert("GTR_BRANCH".into(), branch.clone());
                m
            }),
            resume_session_id: None,
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
            status = "spawn_failed".to_string();
            exit_reason = "spawn_failed".to_string();
        } else {
            agent_spawned = true;
            tracing::info!("Polecat {name}: agent spawned, entering heartbeat loop");
        }
    }

    // ─── Step 3: Heartbeat loop (only if agent spawned) ───
    if agent_spawned {
        let mut done_ch = ctx.make_signal_channel(SIGNAL_POLECAT_DONE);
        let mut kill_ch = ctx.make_signal_channel(SIGNAL_POLECAT_KILL);
        let mut stuck_ch = ctx.make_signal_channel(SIGNAL_POLECAT_STUCK);

        loop {
            tokio::select! {
                biased;
                Some(_) = kill_ch.next() => {
                    tracing::info!("Polecat {name} killed");
                    status = "zombie".to_string();
                    exit_reason = "killed".to_string();
                    break;
                }
                Some(signal) = done_ch.next() => {
                    if let Some(payload) = signal.input.first() {
                        if let Ok(data) = serde_json::from_slice::<PolecatDoneSignal>(&payload.data) {
                            tracing::info!("Polecat {name} done: {}", data.status);
                            agent_summary = data.summary;
                        }
                    }
                    status = "done".to_string();
                    exit_reason = "completed".to_string();
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
                        tracing::warn!("Polecat {name}: agent process died");
                        status = "dead".to_string();
                        exit_reason = "agent_died".to_string();
                        break;
                    }
                }
            }
        }
    }

    // ─── Step 4: Capture tmux pane output (best effort, only if agent was spawned) ───
    let mut captured_output: Option<String> = None;
    if agent_spawned {
        let cap_input = CapturePaneInput {
            agent_id: polecat_id.clone(),
            lines: 100,
        };
        let cap_result = ctx
            .activity(ActivityOptions {
                activity_type: "capture_pane".to_string(),
                input: cap_input.as_json_payload()?,
                start_to_close_timeout: Some(Duration::from_secs(10)),
                ..Default::default()
            })
            .await;

        if let Ok(Some(payload)) = cap_result.success_payload_or_error() {
            if let Ok(output) = serde_json::from_slice::<CapturePaneOutput>(&payload.data) {
                captured_output = output.captured;
            }
        }
    }

    // Choose summary: agent-provided takes priority, then tmux capture
    let summary = agent_summary.or(captured_output);

    // ─── Step 5: Kill the tmux session ───
    if agent_spawned {
        let cleanup_input = HeartbeatInput {
            agent_id: polecat_id.clone(),
        };
        let _ = ctx
            .activity(ActivityOptions {
                activity_type: "kill_agent".to_string(),
                input: cleanup_input.as_json_payload()?,
                start_to_close_timeout: Some(Duration::from_secs(10)),
                ..Default::default()
            })
            .await;
    }

    // ─── Step 6: GUARANTEED — Report to mayor via signal_workflow ───
    let report = PolecatReportSignal {
        polecat_id: polecat_id.clone(),
        name: name.clone(),
        rig: rig.clone(),
        work_item_id: work_item_id.clone(),
        branch: branch.clone(),
        status: status.clone(),
        summary: summary.clone(),
        exit_reason: exit_reason.clone(),
    };

    tracing::info!(
        "Polecat {name}: sending report to mayor — status={status} exit={exit_reason}{}",
        summary.as_ref().map(|s| format!(" summary={}", &s[..s.len().min(80)])).unwrap_or_default()
    );

    let report_payload = report.as_json_payload()?;
    let sig_opts = SignalWorkflowOptions::new(
        "mayor",
        "",
        SIGNAL_POLECAT_REPORT,
        vec![report_payload],
    );
    // Await the signal but ignore errors — mayor may not be running
    let _ = ctx.signal_workflow(sig_opts).await;

    // ─── Return final state ───
    Ok(WfExitValue::Normal(serde_json::to_string(&PolecatState {
        name,
        rig,
        work_item_id,
        status,
        branch,
        worktree_path,
        summary,
    })?))
}
