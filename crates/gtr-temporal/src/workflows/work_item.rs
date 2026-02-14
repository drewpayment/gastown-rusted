use futures_util::StreamExt;
use temporalio_sdk::{WfContext, WfExitValue};

use crate::signals::*;

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

    // Set up signal channels
    let mut assign_ch = ctx.make_signal_channel(SIGNAL_ASSIGN);
    let mut start_ch = ctx.make_signal_channel(SIGNAL_START);
    let mut complete_ch = ctx.make_signal_channel(SIGNAL_COMPLETE);
    let mut fail_ch = ctx.make_signal_channel(SIGNAL_FAIL);
    let mut close_ch = ctx.make_signal_channel(SIGNAL_CLOSE);
    let mut release_ch = ctx.make_signal_channel(SIGNAL_RELEASE);

    tracing::info!("WorkItem {id} started: {title}");

    // Main signal loop — wait for signals and transition state
    loop {
        tokio::select! {
            Some(signal) = assign_ch.next() => {
                if status == "pending" {
                    if let Some(payload) = signal.input.first() {
                        if let Ok(data) = serde_json::from_slice::<AssignSignal>(&payload.data) {
                            assigned_to = Some(data.agent_id.clone());
                            status = "assigned".to_string();
                            tracing::info!("WorkItem {id} assigned to {}", data.agent_id);
                        }
                    }
                }
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
        }
    }
}
