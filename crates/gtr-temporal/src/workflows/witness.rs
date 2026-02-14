use std::collections::HashMap;
use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::heartbeat::HeartbeatInput;
use crate::activities::notification::NotificationInput;
use crate::signals::SIGNAL_AGENT_STOP;

/// Witness workflow — real polecat heartbeat-based health monitoring and escalation.
/// On each cycle:
/// 1. Checks each tracked polecat via `check_agent_alive` heartbeat activity
/// 2. If dead (heartbeat fails), sends escalation to Mayor
/// 3. Tracks alert count per polecat to avoid spam
/// 4. Sends periodic health reports
pub async fn witness_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (rig, interval_secs) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, u64)>(&payload.data)
            .unwrap_or(("default".into(), 300))
    } else {
        ("default".into(), 300)
    };

    let mut stop_ch = ctx.make_signal_channel(SIGNAL_AGENT_STOP);
    let mut checks: u64 = 0;
    let mut alerts_sent: u64 = 0;
    // Track last known alive state per polecat
    let mut last_alive: HashMap<String, bool> = HashMap::new();
    // Track how many consecutive dead checks per polecat (avoid spam)
    let mut dead_counts: HashMap<String, u32> = HashMap::new();
    // Known polecats on this rig (populated as they appear)
    let mut tracked_polecats: Vec<String> = vec![];

    tracing::info!("Witness started for rig {rig} — check interval {interval_secs}s");

    loop {
        tokio::select! {
            biased;
            Some(_) = stop_ch.next() => {
                tracing::info!("Witness stopped after {checks} checks, {alerts_sent} alerts");
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&serde_json::json!({
                        "rig": rig,
                        "checks": checks,
                        "alerts_sent": alerts_sent,
                        "tracked_polecats": tracked_polecats.len(),
                    }))?
                ));
            }
            _ = ctx.timer(Duration::from_secs(interval_secs)) => {
                checks += 1;
                tracing::info!("Witness check #{checks} for rig {rig}");

                // Build polecat IDs to check — convention: {rig}-polecat-{n}
                // Start with any previously tracked, plus standard naming pattern
                if tracked_polecats.is_empty() {
                    // Seed with conventional polecat names for this rig
                    for i in 0..4 {
                        tracked_polecats.push(format!("{rig}-polecat-{i}"));
                    }
                }

                let mut dead_polecats: Vec<String> = vec![];

                // Heartbeat check each tracked polecat
                for polecat_id in &tracked_polecats {
                    let input = HeartbeatInput {
                        agent_id: polecat_id.clone(),
                    };

                    let result = ctx
                        .activity(ActivityOptions {
                            activity_type: "check_agent_alive".to_string(),
                            input: input.as_json_payload()?,
                            start_to_close_timeout: Some(Duration::from_secs(10)),
                            ..Default::default()
                        })
                        .await;

                    let alive = result.completed_ok();
                    let was_alive = last_alive.insert(polecat_id.clone(), alive);

                    if !alive {
                        let dead_count = dead_counts.entry(polecat_id.clone()).or_insert(0);
                        *dead_count += 1;

                        // Alert if dead for 3+ consecutive checks (15 min at default interval)
                        // but only alert once every 6 checks (30 min) to avoid spam
                        if *dead_count >= 3 && *dead_count % 6 == 3 {
                            dead_polecats.push(polecat_id.clone());
                        }

                        // Log transition from alive to dead
                        if was_alive == Some(true) {
                            tracing::warn!(
                                "Witness: {polecat_id} went from alive to dead on rig {rig}"
                            );
                        }
                    } else {
                        // Reset dead count if alive
                        dead_counts.insert(polecat_id.clone(), 0);
                    }
                }

                if !dead_polecats.is_empty() {
                    let message = format!(
                        "Witness alert: {} dead polecats on rig {}: {}",
                        dead_polecats.len(),
                        rig,
                        dead_polecats.join(", ")
                    );
                    tracing::warn!("{message}");

                    let input = NotificationInput {
                        channel: "signal".to_string(),
                        target: "mayor".to_string(),
                        subject: format!("Witness: dead polecats on {rig}"),
                        message,
                    };

                    let _ = ctx
                        .activity(ActivityOptions {
                            activity_type: "send_notification".to_string(),
                            input: input.as_json_payload()?,
                            start_to_close_timeout: Some(Duration::from_secs(30)),
                            ..Default::default()
                        })
                        .await;

                    alerts_sent += dead_polecats.len() as u64;
                } else if checks % 12 == 0 {
                    // Periodic health report every ~1 hour
                    let alive_count = last_alive.values().filter(|&&v| v).count();
                    let input = NotificationInput {
                        channel: "signal".to_string(),
                        target: "mayor".to_string(),
                        subject: format!("Witness health: rig {rig}"),
                        message: format!(
                            "Check #{checks}, tracking {} polecats ({alive_count} alive), {alerts_sent} alerts total",
                            tracked_polecats.len()
                        ),
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
            }
        }
    }
}
