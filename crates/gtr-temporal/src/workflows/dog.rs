use futures_util::StreamExt;
use temporalio_sdk::{WfContext, WfExitValue};

use crate::signals::*;

/// Dog workflow — reusable cross-rig infrastructure worker.
/// Unlike polecats, dogs are persistent and return to idle after completing work.
/// Managed by the Deacon via dispatch/release signals.
pub async fn dog_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let name = if let Some(payload) = args.first() {
        serde_json::from_slice::<String>(&payload.data).unwrap_or("unknown".into())
    } else {
        "unknown".into()
    };

    let mut status = "idle".to_string();
    let mut current_work: Option<String> = None;
    let mut current_rig: Option<String> = None;

    let mut dispatch_ch = ctx.make_signal_channel(SIGNAL_DOG_DISPATCH);
    let mut release_ch = ctx.make_signal_channel(SIGNAL_DOG_RELEASE);
    let mut stop_ch = ctx.make_signal_channel(SIGNAL_DOG_STOP);

    tracing::info!("Dog {name} started — idle");

    loop {
        tokio::select! {
            biased;
            Some(_) = stop_ch.next() => {
                tracing::info!("Dog {name} stopped");
                return Ok(WfExitValue::Normal(serde_json::to_string(&DogState {
                    name,
                    status: "stopped".to_string(),
                    current_work,
                    current_rig,
                })?));
            }
            Some(signal) = dispatch_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<DogDispatchSignal>(&payload.data) {
                        status = "working".to_string();
                        current_work = Some(data.work_item_id.clone());
                        current_rig = Some(data.rig.clone());
                        tracing::info!(
                            "Dog {name}: dispatched to rig {} — work item {}",
                            data.rig, data.work_item_id
                        );
                    }
                }
            }
            Some(_) = release_ch.next() => {
                if status == "working" {
                    tracing::info!(
                        "Dog {name}: released from {}",
                        current_work.as_deref().unwrap_or("?")
                    );
                    status = "idle".to_string();
                    current_work = None;
                    current_rig = None;
                }
            }
        }
    }
}
