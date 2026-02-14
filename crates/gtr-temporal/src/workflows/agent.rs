use futures_util::StreamExt;
use temporalio_sdk::{WfContext, WfExitValue};

use crate::signals::*;

pub async fn agent_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (id, role) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "polecat".into()))
    } else {
        ("unknown".into(), "polecat".into())
    };

    let mut status = "idle".to_string();
    let mut current_work: Option<String> = None;
    let mut inbox: Vec<MailEntry> = vec![];

    let mut assign_ch = ctx.make_signal_channel(SIGNAL_AGENT_ASSIGN);
    let mut unassign_ch = ctx.make_signal_channel(SIGNAL_AGENT_UNASSIGN);
    let mut mail_ch = ctx.make_signal_channel(SIGNAL_AGENT_MAIL);
    let mut nudge_ch = ctx.make_signal_channel(SIGNAL_AGENT_NUDGE);
    let mut stop_ch = ctx.make_signal_channel(SIGNAL_AGENT_STOP);

    tracing::info!("Agent {id} ({role}) started — idle");

    loop {
        tokio::select! {
            Some(signal) = assign_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<AgentAssignSignal>(&payload.data) {
                        current_work = Some(data.work_item_id.clone());
                        status = "working".to_string();
                        tracing::info!("Agent {id}: assigned work item {} — {}", data.work_item_id, data.title);
                    }
                }
            }
            Some(_) = unassign_ch.next() => {
                if current_work.is_some() {
                    tracing::info!("Agent {id}: unassigned from {}", current_work.as_deref().unwrap_or("?"));
                    current_work = None;
                    status = "idle".to_string();
                }
            }
            Some(signal) = mail_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<AgentMailSignal>(&payload.data) {
                        tracing::info!("Agent {id}: mail from {} — {}", data.from, data.message);
                        inbox.push(MailEntry {
                            from: data.from,
                            message: data.message,
                        });
                    }
                }
            }
            Some(signal) = nudge_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<AgentNudgeSignal>(&payload.data) {
                        tracing::info!("Agent {id}: nudge from {} — {}", data.from, data.message);
                        inbox.push(MailEntry {
                            from: data.from,
                            message: format!("[nudge] {}", data.message),
                        });
                    }
                }
            }
            Some(_) = stop_ch.next() => {
                status = "stopped".to_string();
                tracing::info!("Agent {id} stopped");
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&AgentState {
                        id, role, status, current_work, inbox,
                    })?
                ));
            }
        }
    }
}
