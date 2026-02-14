use std::collections::HashSet;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ChildWorkflowOptions, WfContext, WfExitValue};

use crate::signals::*;

pub async fn convoy_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (id, title) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "untitled".into()))
    } else {
        ("unknown".into(), "untitled".into())
    };

    let mut work_items: Vec<String> = vec![];
    let mut completed: HashSet<String> = HashSet::new();
    let mut status = "open".to_string();

    let mut add_item_ch = ctx.make_signal_channel(SIGNAL_ADD_WORK_ITEM);
    let mut item_done_ch = ctx.make_signal_channel(SIGNAL_ITEM_DONE);
    let mut cancel_ch = ctx.make_signal_channel(SIGNAL_CANCEL_CONVOY);
    let mut close_ch = ctx.make_signal_channel(SIGNAL_CLOSE);

    tracing::info!("Convoy {id} started: {title}");

    loop {
        tokio::select! {
            Some(signal) = add_item_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<AddWorkItemSignal>(&payload.data) {
                        work_items.push(data.work_item_id.clone());

                        // Start child WorkItem workflow
                        let input_payload = (data.work_item_id.as_str(), data.title.as_str())
                            .as_json_payload()?;
                        let child = ctx.child_workflow(ChildWorkflowOptions {
                            workflow_id: data.work_item_id.clone(),
                            workflow_type: "work_item_wf".to_string(),
                            input: vec![input_payload],
                            ..Default::default()
                        });
                        let pending = child.start(&ctx).await;
                        if pending.into_started().is_some() {
                            tracing::info!("Convoy {id}: started child work item {}", data.work_item_id);
                        } else {
                            tracing::warn!("Convoy {id}: failed to start child {}", data.work_item_id);
                        }
                    }
                }
            }
            Some(signal) = item_done_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<ItemDoneSignal>(&payload.data) {
                        completed.insert(data.work_item_id.clone());
                        tracing::info!(
                            "Convoy {id}: item {} done ({}/{})",
                            data.work_item_id, completed.len(), work_items.len()
                        );

                        if !work_items.is_empty() && completed.len() == work_items.len() {
                            status = "closed".to_string();
                            tracing::info!("Convoy {id} complete — all items done");
                            return Ok(WfExitValue::Normal(
                                serde_json::to_string(&ConvoyState {
                                    id, title, status,
                                    work_items,
                                    completed_items: completed.into_iter().collect(),
                                })?
                            ));
                        }
                    }
                }
            }
            Some(_) = close_ch.next() => {
                status = "closed".to_string();
                tracing::info!("Convoy {id} closed");
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&ConvoyState {
                        id, title, status,
                        work_items,
                        completed_items: completed.into_iter().collect(),
                    })?
                ));
            }
            Some(_) = cancel_ch.next() => {
                status = "cancelled".to_string();
                tracing::info!("Convoy {id} cancelled");
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&ConvoyState {
                        id, title, status,
                        work_items,
                        completed_items: completed.into_iter().collect(),
                    })?
                ));
            }
        }
    }
}
