use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::spawn_agent::SpawnAgentInput;
use crate::signals::*;

/// Rig workflow — manages a registered git repository's lifecycle.
/// States: operational (active), parked (paused), docked (long-term shutdown).
pub async fn rig_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (name, git_url) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "".into()))
    } else {
        ("unknown".into(), "".into())
    };

    let mut status = "operational".to_string();
    let mut agents: Vec<RigAgentEntry> = vec![];
    let mut polecats: Vec<String> = vec![];
    let mut crew: Vec<String> = vec![];
    let mut has_witness = false;
    let mut has_refinery = false;

    let mut park_ch = ctx.make_signal_channel(SIGNAL_RIG_PARK);
    let mut unpark_ch = ctx.make_signal_channel(SIGNAL_RIG_UNPARK);
    let mut dock_ch = ctx.make_signal_channel(SIGNAL_RIG_DOCK);
    let mut undock_ch = ctx.make_signal_channel(SIGNAL_RIG_UNDOCK);
    let mut reg_ch = ctx.make_signal_channel(SIGNAL_RIG_REGISTER_AGENT);
    let mut unreg_ch = ctx.make_signal_channel(SIGNAL_RIG_UNREGISTER_AGENT);
    let mut boot_ch = ctx.make_signal_channel(SIGNAL_RIG_BOOT);
    let mut stop_ch = ctx.make_signal_channel(SIGNAL_RIG_STOP);

    tracing::info!("Rig {name} started — operational ({git_url})");

    loop {
        tokio::select! {
            biased;
            Some(_) = boot_ch.next() => {
                if status == "operational" {
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());

                    // Spawn witness
                    if !has_witness {
                        let witness_input = SpawnAgentInput {
                            agent_id: format!("{name}-witness"),
                            runtime: "claude".to_string(),
                            work_dir: format!("{home}/.gtr/rigs/{name}/witness"),
                            role: "witness".to_string(),
                            rig: Some(name.clone()),
                            initial_prompt: Some(format!(
                                "You are the Witness for rig '{name}'. Monitor polecats and report issues. \
                                 Use $RGT_BIN instead of rgt (env var has the full path).\n\
                                 1. `$RGT_BIN feed` — watch system status\n\
                                 2. Report stuck polecats to mayor via `$RGT_BIN mail send mayor`"
                            )),
                            env_extra: None,
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
                            has_witness = true;
                            tracing::info!("Rig {name}: spawned witness");
                        }
                    }

                    // Spawn refinery
                    if !has_refinery {
                        let refinery_input = SpawnAgentInput {
                            agent_id: format!("{name}-refinery"),
                            runtime: "claude".to_string(),
                            work_dir: format!("{home}/.gtr/rigs/{name}/refinery"),
                            role: "refinery".to_string(),
                            rig: Some(name.clone()),
                            initial_prompt: Some(format!(
                                "You are the Refinery for rig '{name}'. Process the merge queue.\n\
                                 Check for enqueued branches and merge them."
                            )),
                            env_extra: None,
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
                            has_refinery = true;
                            tracing::info!("Rig {name}: spawned refinery");
                        }
                    }

                    tracing::info!("Rig {name}: bootstrap complete");
                }
            }
            Some(_) = stop_ch.next() => {
                tracing::info!("Rig {name} stopped");
                return Ok(WfExitValue::Normal(serde_json::to_string(&RigState {
                    name, git_url, status, agents, polecats, crew, has_witness, has_refinery,
                })?));
            }
            Some(_) = park_ch.next() => {
                if status == "operational" {
                    status = "parked".to_string();
                    tracing::info!("Rig {name} parked");
                }
            }
            Some(_) = unpark_ch.next() => {
                if status == "parked" {
                    status = "operational".to_string();
                    tracing::info!("Rig {name} unparked");
                }
            }
            Some(_) = dock_ch.next() => {
                status = "docked".to_string();
                tracing::info!("Rig {name} docked");
            }
            Some(_) = undock_ch.next() => {
                if status == "docked" {
                    status = "operational".to_string();
                    tracing::info!("Rig {name} undocked");
                }
            }
            Some(signal) = reg_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<RigAgentEntry>(&payload.data) {
                        match data.role.as_str() {
                            "witness" => has_witness = true,
                            "refinery" => has_refinery = true,
                            "polecat" => polecats.push(data.agent_id.clone()),
                            "crew" => crew.push(data.agent_id.clone()),
                            _ => {}
                        }
                        tracing::info!("Rig {name}: registered {} ({})", data.agent_id, data.role);
                        agents.push(data);
                    }
                }
            }
            Some(signal) = unreg_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(id) = serde_json::from_slice::<String>(&payload.data) {
                        if let Some(pos) = agents.iter().position(|a| a.agent_id == id) {
                            let removed = agents.remove(pos);
                            match removed.role.as_str() {
                                "witness" => has_witness = false,
                                "refinery" => has_refinery = false,
                                "polecat" => polecats.retain(|p| p != &id),
                                "crew" => crew.retain(|c| c != &id),
                                _ => {}
                            }
                            tracing::info!("Rig {name}: unregistered {id}");
                        }
                    }
                }
            }
        }
    }
}
