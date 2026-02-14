use std::time::Duration;

use futures_util::StreamExt;
use temporalio_sdk::{WfContext, WfExitValue};

use crate::signals::*;

/// Gate workflow — async wait primitive.
/// Timer gates auto-close after duration.
/// Human gates wait for approval signal.
/// Mail gates wait for close signal.
pub async fn gate_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (id, gate_type, parked_work) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, GateType, Option<String>)>(&payload.data)
            .unwrap_or(("unknown".into(), GateType::Human { description: "unknown".into() }, None))
    } else {
        ("unknown".into(), GateType::Human { description: "unknown".into() }, None)
    };

    let mut status = "waiting".to_string();

    tracing::info!("Gate {id} created — waiting ({:?})", gate_type);

    let mut close_ch = ctx.make_signal_channel(SIGNAL_GATE_CLOSE);
    let mut approve_ch = ctx.make_signal_channel(SIGNAL_GATE_APPROVE);

    match &gate_type {
        GateType::Timer { duration_secs } => {
            let duration = Duration::from_secs(*duration_secs);
            tokio::select! {
                biased;
                Some(_) = close_ch.next() => {
                    status = "closed".to_string();
                    tracing::info!("Gate {id} manually closed");
                }
                _ = ctx.timer(duration) => {
                    status = "closed".to_string();
                    tracing::info!("Gate {id} timer expired after {duration_secs}s");
                }
            }
        }
        GateType::Human { description } => {
            tracing::info!("Gate {id} waiting for human approval: {description}");
            tokio::select! {
                biased;
                Some(_) = close_ch.next() => {
                    status = "closed".to_string();
                    tracing::info!("Gate {id} closed (denied)");
                }
                Some(_) = approve_ch.next() => {
                    status = "approved".to_string();
                    tracing::info!("Gate {id} approved");
                }
            }
        }
        GateType::Mail { from } => {
            tracing::info!("Gate {id} waiting for mail from {from}");
            tokio::select! {
                biased;
                Some(_) = close_ch.next() => {
                    status = "closed".to_string();
                    tracing::info!("Gate {id} closed by mail");
                }
                Some(_) = approve_ch.next() => {
                    status = "approved".to_string();
                    tracing::info!("Gate {id} approved by mail");
                }
            }
        }
    }

    Ok(WfExitValue::Normal(serde_json::to_string(&GateState {
        id,
        gate_type,
        status,
        parked_work,
    })?))
}
