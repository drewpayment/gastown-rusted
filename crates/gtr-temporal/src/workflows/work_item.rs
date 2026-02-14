use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::notification::NotificationInput;
use crate::signals::*;

const STALE_TIMEOUT: Duration = Duration::from_secs(4 * 60 * 60); // 4 hours

pub async fn work_item_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    // Parse input: (id, title)
    let args = ctx.get_args();
    let (id, title) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "untitled".into()))
    } else {
        ("unknown".into(), "untitled".into())
    };

    let mut status = "pending".to_string();
    let mut assigned_to: Option<String> = None;
    let mut escalation_level: u32 = 0;

    // Set up signal channels
    let mut assign_ch = ctx.make_signal_channel(SIGNAL_ASSIGN);
    let mut start_ch = ctx.make_signal_channel(SIGNAL_START);
    let mut complete_ch = ctx.make_signal_channel(SIGNAL_COMPLETE);
    let mut fail_ch = ctx.make_signal_channel(SIGNAL_FAIL);
    let mut close_ch = ctx.make_signal_channel(SIGNAL_CLOSE);
    let mut release_ch = ctx.make_signal_channel(SIGNAL_RELEASE);
    let mut heartbeat_ch = ctx.make_signal_channel(SIGNAL_HEARTBEAT);
    let mut escalate_ch = ctx.make_signal_channel(SIGNAL_ESCALATE);

    tracing::info!("WorkItem {id} started: {title}");

    // Main signal loop — wait for signals and transition state
    loop {
        // Use a staleness timer when work is in progress
        let use_timer = status == "in_progress" || status == "assigned";

        if use_timer {
            tokio::select! {
                biased;
                Some(signal) = assign_ch.next() => {
                    handle_assign(&id, &mut status, &mut assigned_to, signal);
                }
                Some(_) = start_ch.next() => {
                    if status == "assigned" {
                        status = "in_progress".to_string();
                        escalation_level = 0;
                        tracing::info!("WorkItem {id} in progress");
                    }
                }
                Some(_) = complete_ch.next() => {
                    if status == "in_progress" || status == "assigned" {
                        status = "done".to_string();
                        tracing::info!("WorkItem {id} completed");
                        return Ok(WfExitValue::Normal(
                            serde_json::to_string(&WorkItemState {
                                id: id.clone(),
                                title: title.clone(),
                                status: status.clone(),
                                assigned_to: assigned_to.clone(),
                            })?
                        ));
                    }
                }
                Some(signal) = fail_ch.next() => {
                    if let Some(payload) = signal.input.first() {
                        if let Ok(data) = serde_json::from_slice::<FailSignal>(&payload.data) {
                            status = "failed".to_string();
                            tracing::warn!("WorkItem {id} failed: {}", data.reason);
                            return Ok(WfExitValue::Normal(
                                serde_json::to_string(&WorkItemState {
                                    id, title, status, assigned_to,
                                })?
                            ));
                        }
                    }
                }
                Some(_) = close_ch.next() => {
                    status = "closed".to_string();
                    tracing::info!("WorkItem {id} closed");
                    return Ok(WfExitValue::Normal(
                        serde_json::to_string(&WorkItemState {
                            id, title, status, assigned_to,
                        })?
                    ));
                }
                Some(_) = release_ch.next() => {
                    if status == "assigned" || status == "in_progress" {
                        assigned_to = None;
                        status = "pending".to_string();
                        escalation_level = 0;
                        tracing::info!("WorkItem {id} released back to pending");
                    }
                }
                Some(signal) = heartbeat_ch.next() => {
                    if let Some(payload) = signal.input.first() {
                        if let Ok(hb) = serde_json::from_slice::<HeartbeatSignal>(&payload.data) {
                            tracing::debug!("WorkItem {id} heartbeat: {:?}", hb.progress);
                        }
                    }
                    // Heartbeat resets the timer by continuing the loop
                }
                Some(_) = escalate_ch.next() => {
                    escalation_level += 1;
                    tracing::warn!("WorkItem {id} escalated (level {escalation_level})");
                    send_escalation_notification(&ctx, &id, &title, escalation_level).await?;
                }
                _ = ctx.timer(STALE_TIMEOUT) => {
                    // Staleness timeout — auto-escalate
                    escalation_level += 1;
                    tracing::warn!("WorkItem {id} stale — auto-escalating (level {escalation_level})");
                    send_escalation_notification(&ctx, &id, &title, escalation_level).await?;
                }
            }
        } else {
            // No timer needed when pending — just wait for signals
            tokio::select! {
                biased;
                Some(signal) = assign_ch.next() => {
                    handle_assign(&id, &mut status, &mut assigned_to, signal);
                }
                Some(_) = start_ch.next() => {
                    if status == "assigned" {
                        status = "in_progress".to_string();
                        tracing::info!("WorkItem {id} in progress");
                    }
                }
                Some(_) = complete_ch.next() => {
                    if status == "in_progress" || status == "assigned" {
                        status = "done".to_string();
                        tracing::info!("WorkItem {id} completed");
                        return Ok(WfExitValue::Normal(
                            serde_json::to_string(&WorkItemState {
                                id: id.clone(),
                                title: title.clone(),
                                status: status.clone(),
                                assigned_to: assigned_to.clone(),
                            })?
                        ));
                    }
                }
                Some(signal) = fail_ch.next() => {
                    if let Some(payload) = signal.input.first() {
                        if let Ok(data) = serde_json::from_slice::<FailSignal>(&payload.data) {
                            status = "failed".to_string();
                            tracing::warn!("WorkItem {id} failed: {}", data.reason);
                            return Ok(WfExitValue::Normal(
                                serde_json::to_string(&WorkItemState {
                                    id, title, status, assigned_to,
                                })?
                            ));
                        }
                    }
                }
                Some(_) = close_ch.next() => {
                    status = "closed".to_string();
                    tracing::info!("WorkItem {id} closed");
                    return Ok(WfExitValue::Normal(
                        serde_json::to_string(&WorkItemState {
                            id, title, status, assigned_to,
                        })?
                    ));
                }
                Some(_) = release_ch.next() => {
                    if status == "assigned" || status == "in_progress" {
                        assigned_to = None;
                        status = "pending".to_string();
                        tracing::info!("WorkItem {id} released back to pending");
                    }
                }
                Some(_) = heartbeat_ch.next() => {
                    // Heartbeat while pending — ignore
                }
                Some(_) = escalate_ch.next() => {
                    escalation_level += 1;
                    tracing::warn!("WorkItem {id} manually escalated (level {escalation_level})");
                    send_escalation_notification(&ctx, &id, &title, escalation_level).await?;
                }
            }
        }
    }
}

fn handle_assign(
    id: &str,
    status: &mut String,
    assigned_to: &mut Option<String>,
    signal: temporalio_sdk::SignalData,
) {
    if *status == "pending" {
        if let Some(payload) = signal.input.first() {
            if let Ok(data) = serde_json::from_slice::<AssignSignal>(&payload.data) {
                *assigned_to = Some(data.agent_id.clone());
                *status = "assigned".to_string();
                tracing::info!("WorkItem {id} assigned to {}", data.agent_id);
            }
        }
    }
}

async fn send_escalation_notification(
    ctx: &WfContext,
    id: &str,
    title: &str,
    level: u32,
) -> anyhow::Result<()> {
    let input = NotificationInput {
        channel: "signal".to_string(),
        target: "mayor".to_string(),
        subject: format!("Escalation L{level}: {id}"),
        message: format!("Work item '{title}' ({id}) escalated to level {level}"),
    };

    let _ = ctx
        .activity(ActivityOptions {
            activity_type: "send_notification".to_string(),
            input: input.as_json_payload()?,
            start_to_close_timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        })
        .await;

    Ok(())
}
