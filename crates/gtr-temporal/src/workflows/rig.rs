use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::workflow_commands::ContinueAsNewWorkflowExecution;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::discover_session::DiscoverSessionInput;
use crate::activities::spawn_agent::SpawnAgentInput;
use crate::signals::*;

/// Rig workflow — manages a registered git repository's lifecycle.
/// States: operational (active), parked (paused), docked (long-term shutdown), dormant (after stop).
///
/// On `rig_stop`, uses Continue-As-New to preserve state across stop/start cycles.
pub async fn rig_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();

    // Parse input: try RigState (Continue-As-New) first, fall back to (name, git_url) tuple.
    let (mut state, from_continue) = if let Some(payload) = args.first() {
        if let Ok(continued) = serde_json::from_slice::<RigState>(&payload.data) {
            (continued, true)
        } else if let Ok((name, git_url)) =
            serde_json::from_slice::<(String, String)>(&payload.data)
        {
            (
                RigState {
                    name,
                    git_url,
                    status: "operational".to_string(),
                    agents: vec![],
                    polecats: vec![],
                    crew: vec![],
                    has_witness: false,
                    has_refinery: false,
                    witness_session_id: None,
                    refinery_session_id: None,
                },
                false,
            )
        } else {
            (
                RigState {
                    name: "unknown".to_string(),
                    git_url: String::new(),
                    status: "operational".to_string(),
                    agents: vec![],
                    polecats: vec![],
                    crew: vec![],
                    has_witness: false,
                    has_refinery: false,
                    witness_session_id: None,
                    refinery_session_id: None,
                },
                false,
            )
        }
    } else {
        (
            RigState {
                name: "unknown".to_string(),
                git_url: String::new(),
                status: "operational".to_string(),
                agents: vec![],
                polecats: vec![],
                crew: vec![],
                has_witness: false,
                has_refinery: false,
                witness_session_id: None,
                refinery_session_id: None,
            },
            false,
        )
    };

    // If resumed from Continue-As-New, start in dormant state
    if from_continue {
        state.status = "dormant".to_string();
        tracing::info!("Rig {} resumed from Continue-As-New (dormant)", state.name);
    } else {
        tracing::info!(
            "Rig {} started — operational ({})",
            state.name,
            state.git_url
        );
    }

    let mut park_ch = ctx.make_signal_channel(SIGNAL_RIG_PARK);
    let mut unpark_ch = ctx.make_signal_channel(SIGNAL_RIG_UNPARK);
    let mut dock_ch = ctx.make_signal_channel(SIGNAL_RIG_DOCK);
    let mut undock_ch = ctx.make_signal_channel(SIGNAL_RIG_UNDOCK);
    let mut reg_ch = ctx.make_signal_channel(SIGNAL_RIG_REGISTER_AGENT);
    let mut unreg_ch = ctx.make_signal_channel(SIGNAL_RIG_UNREGISTER_AGENT);
    let mut boot_ch = ctx.make_signal_channel(SIGNAL_RIG_BOOT);
    let mut stop_ch = ctx.make_signal_channel(SIGNAL_RIG_STOP);

    loop {
        tokio::select! {
            biased;
            Some(_) = boot_ch.next() => {
                if state.status == "operational" || state.status == "dormant" {
                    state.status = "operational".to_string();
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());

                    // Spawn witness
                    if !state.has_witness {
                        let witness_input = SpawnAgentInput {
                            agent_id: format!("{}-witness", state.name),
                            runtime: "claude".to_string(),
                            work_dir: format!("{home}/.gtr/rigs/{}/witness", state.name),
                            role: "witness".to_string(),
                            rig: Some(state.name.clone()),
                            initial_prompt: Some(format!(
                                "You are the Witness for rig '{}'. Monitor polecats and report issues. \
                                 Use $RGT_BIN instead of rgt (env var has the full path).\n\
                                 1. `$RGT_BIN feed` — watch system status\n\
                                 2. Report stuck polecats to mayor via `$RGT_BIN mail send mayor`",
                                state.name
                            )),
                            env_extra: None,
                            resume_session_id: state.witness_session_id.clone(),
                        };

                        let result = ctx
                            .activity(ActivityOptions {
                                activity_type: "spawn_agent".to_string(),
                                input: witness_input.as_json_payload()?,
                                start_to_close_timeout: Some(Duration::from_secs(30)),
                                ..Default::default()
                            })
                            .await;

                        if result.completed_ok() {
                            state.has_witness = true;
                            tracing::info!("Rig {}: spawned witness", state.name);

                            // Discover session ID
                            let discover_input = DiscoverSessionInput {
                                work_dir: format!("{home}/.gtr/rigs/{}/witness", state.name),
                            };
                            let session_result = ctx
                                .activity(ActivityOptions {
                                    activity_type: "discover_session_id".to_string(),
                                    input: discover_input.as_json_payload()?,
                                    start_to_close_timeout: Some(Duration::from_secs(15)),
                                    ..Default::default()
                                })
                                .await;
                            if let Ok(Some(payload)) = session_result.success_payload_or_error() {
                                if let Ok(output) = serde_json::from_slice::<crate::activities::discover_session::DiscoverSessionOutput>(&payload.data) {
                                    state.witness_session_id = output.session_id;
                                }
                            }
                        }
                    }

                    // Spawn refinery
                    if !state.has_refinery {
                        let refinery_input = SpawnAgentInput {
                            agent_id: format!("{}-refinery", state.name),
                            runtime: "claude".to_string(),
                            work_dir: format!("{home}/.gtr/rigs/{}/refinery", state.name),
                            role: "refinery".to_string(),
                            rig: Some(state.name.clone()),
                            initial_prompt: Some(format!(
                                "You are the Refinery for rig '{}'. Process the merge queue.\n\
                                 Check for enqueued branches and merge them.",
                                state.name
                            )),
                            env_extra: None,
                            resume_session_id: state.refinery_session_id.clone(),
                        };

                        let result = ctx
                            .activity(ActivityOptions {
                                activity_type: "spawn_agent".to_string(),
                                input: refinery_input.as_json_payload()?,
                                start_to_close_timeout: Some(Duration::from_secs(30)),
                                ..Default::default()
                            })
                            .await;

                        if result.completed_ok() {
                            state.has_refinery = true;
                            tracing::info!("Rig {}: spawned refinery", state.name);

                            // Discover session ID
                            let discover_input = DiscoverSessionInput {
                                work_dir: format!("{home}/.gtr/rigs/{}/refinery", state.name),
                            };
                            let session_result = ctx
                                .activity(ActivityOptions {
                                    activity_type: "discover_session_id".to_string(),
                                    input: discover_input.as_json_payload()?,
                                    start_to_close_timeout: Some(Duration::from_secs(15)),
                                    ..Default::default()
                                })
                                .await;
                            if let Ok(Some(payload)) = session_result.success_payload_or_error() {
                                if let Ok(output) = serde_json::from_slice::<crate::activities::discover_session::DiscoverSessionOutput>(&payload.data) {
                                    state.refinery_session_id = output.session_id;
                                }
                            }
                        }
                    }

                    tracing::info!("Rig {}: bootstrap complete", state.name);
                }
            }
            Some(_) = stop_ch.next() => {
                tracing::info!("Rig {} stopping — Continue-As-New (dormant)", state.name);
                state.status = "dormant".to_string();
                // Reset agent presence flags since processes will be killed
                state.has_witness = false;
                state.has_refinery = false;
                state.agents.clear();
                state.polecats.clear();
                state.crew.clear();
                // Session IDs are preserved for --resume on next boot

                let state_payload = state.as_json_payload()?;
                return Ok(WfExitValue::continue_as_new(ContinueAsNewWorkflowExecution {
                    arguments: vec![state_payload],
                    ..Default::default()
                }));
            }
            Some(_) = park_ch.next() => {
                if state.status == "operational" {
                    state.status = "parked".to_string();
                    tracing::info!("Rig {} parked", state.name);
                } else {
                    tracing::warn!("Rig {} ignoring park signal (status: {})", state.name, state.status);
                }
            }
            Some(_) = unpark_ch.next() => {
                if state.status == "parked" {
                    state.status = "operational".to_string();
                    tracing::info!("Rig {} unparked", state.name);
                }
            }
            Some(_) = dock_ch.next() => {
                if state.status != "dormant" {
                    state.status = "docked".to_string();
                    tracing::info!("Rig {} docked", state.name);
                }
            }
            Some(_) = undock_ch.next() => {
                if state.status == "docked" {
                    state.status = "operational".to_string();
                    tracing::info!("Rig {} undocked", state.name);
                }
            }
            Some(signal) = reg_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<RigAgentEntry>(&payload.data) {
                        match data.role.as_str() {
                            "witness" => state.has_witness = true,
                            "refinery" => state.has_refinery = true,
                            "polecat" => state.polecats.push(data.agent_id.clone()),
                            "crew" => state.crew.push(data.agent_id.clone()),
                            _ => {}
                        }
                        tracing::info!("Rig {}: registered {} ({})", state.name, data.agent_id, data.role);
                        state.agents.push(data);
                    }
                }
            }
            Some(signal) = unreg_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(id) = serde_json::from_slice::<String>(&payload.data) {
                        if let Some(pos) = state.agents.iter().position(|a| a.agent_id == id) {
                            let removed = state.agents.remove(pos);
                            match removed.role.as_str() {
                                "witness" => state.has_witness = false,
                                "refinery" => state.has_refinery = false,
                                "polecat" => state.polecats.retain(|p| p != &id),
                                "crew" => state.crew.retain(|c| c != &id),
                                _ => {}
                            }
                            tracing::info!("Rig {}: unregistered {id}", state.name);
                        }
                    }
                }
            }
        }
    }
}
