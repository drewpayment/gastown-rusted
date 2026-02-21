use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::heartbeat::HeartbeatInput;
use crate::activities::spawn_agent::SpawnAgentInput;
use crate::signals::SIGNAL_AGENT_STOP;

/// Boot workflow — spawns mayor agent, then monitors health of all spawned agents.
pub async fn boot_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let interval_secs = if let Some(payload) = args.first() {
        serde_json::from_slice::<u64>(&payload.data).unwrap_or(120)
    } else {
        120
    };

    let mut stop_ch = ctx.make_signal_channel(SIGNAL_AGENT_STOP);
    let mut checks: u64 = 0;
    let mut spawned: Vec<String> = vec![];

    tracing::info!("Boot started — health check interval {interval_secs}s");

    // Initial spawn: mayor agent
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let mayor_input = SpawnAgentInput {
        agent_id: "mayor".to_string(),
        runtime: "claude".to_string(),
        work_dir: format!("{home}/.gtr"),
        role: "mayor".to_string(),
        rig: None,
        initial_prompt: Some(
            "You are the Mayor of Gas Town. The RGT_BIN env var has the full path to the rgt binary. \
             Use $RGT_BIN instead of rgt in all commands. Check your hook and mail, then act accordingly:\n\
             1. `$RGT_BIN hook` - shows hooked work (if any)\n\
             2. `$RGT_BIN mail inbox` - check for messages\n\
             3. If work is hooked -> execute it immediately\n\
             4. If nothing hooked -> wait for instructions".to_string()
        ),
        env_extra: None,
    };

    let result = ctx
        .activity(ActivityOptions {
            activity_type: "spawn_agent".to_string(),
            input: mayor_input.as_json_payload()?,
            start_to_close_timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        })
        .await;

    if result.completed_ok() {
        spawned.push("mayor".to_string());
        tracing::info!("Boot: spawned mayor agent");
    } else {
        tracing::warn!("Boot: failed to spawn mayor agent");
    }

    // Health check loop
    loop {
        tokio::select! {
            biased;
            Some(_) = stop_ch.next() => {
                tracing::info!("Boot stopped after {checks} checks");
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&serde_json::json!({
                        "checks": checks,
                        "spawned": spawned,
                    }))?
                ));
            }
            _ = ctx.timer(Duration::from_secs(interval_secs)) => {
                checks += 1;
                tracing::info!("Boot health check #{checks}");

                let mut dead_agents: Vec<String> = vec![];

                for agent_id in &spawned {
                    let input = HeartbeatInput {
                        agent_id: agent_id.clone(),
                    };

                    let result = ctx
                        .activity(ActivityOptions {
                            activity_type: "check_agent_alive".to_string(),
                            input: input.as_json_payload()?,
                            start_to_close_timeout: Some(Duration::from_secs(10)),
                            ..Default::default()
                        })
                        .await;

                    if !result.completed_ok() {
                        tracing::warn!("Boot: {agent_id} appears dead — scheduling respawn");
                        dead_agents.push(agent_id.clone());
                    }
                }

                // Respawn dead agents
                for agent_id in &dead_agents {
                    tracing::info!("Boot: respawning {agent_id}");
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                    let respawn_input = SpawnAgentInput {
                        agent_id: agent_id.clone(),
                        runtime: "claude".to_string(),
                        work_dir: format!("{home}/.gtr"),
                        role: "mayor".to_string(), // TODO: track original role per agent
                        rig: None,
                        initial_prompt: Some(
                            "You are being respawned after a crash. Run `$RGT_BIN prime` to restore context. (RGT_BIN env var has the full path.)".to_string()
                        ),
                        env_extra: None,
                    };

                    let result = ctx
                        .activity(ActivityOptions {
                            activity_type: "spawn_agent".to_string(),
                            input: respawn_input.as_json_payload()?,
                            start_to_close_timeout: Some(Duration::from_secs(30)),
                            ..Default::default()
                        })
                        .await;

                    if result.completed_ok() {
                        tracing::info!("Boot: respawned {agent_id}");
                    } else {
                        tracing::error!("Boot: failed to respawn {agent_id}");
                    }
                }
            }
        }
    }
}
