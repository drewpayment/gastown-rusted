use futures_util::StreamExt;
use temporalio_sdk::{WfContext, WfExitValue};

use crate::signals::*;

/// Molecule workflow — an instantiated formula with step-by-step tracking.
/// Tracks which steps are complete, in-progress, and what's next.
/// Advances on `mol_step_done` signals, pauses/resumes/cancels on signals.
pub async fn molecule_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (id, formula_name, step_names) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String, Vec<String>)>(&payload.data)
            .unwrap_or(("unknown".into(), "unknown".into(), vec![]))
    } else {
        ("unknown".into(), "unknown".into(), vec![])
    };

    let mut steps: Vec<MolStepState> = step_names
        .iter()
        .map(|name| MolStepState {
            ref_id: name.clone(),
            title: name.clone(),
            status: "pending".to_string(),
            output: None,
        })
        .collect();

    let mut status = "running".to_string();
    let mut current_step: Option<String> = steps.first().map(|s| s.ref_id.clone());

    // Mark first step as in_progress
    if let Some(first) = steps.first_mut() {
        first.status = "in_progress".to_string();
    }

    tracing::info!("Molecule {id} started — formula {formula_name} ({} steps)", steps.len());

    let mut step_done_ch = ctx.make_signal_channel(SIGNAL_MOL_STEP_DONE);
    let mut step_fail_ch = ctx.make_signal_channel(SIGNAL_MOL_STEP_FAIL);
    let mut pause_ch = ctx.make_signal_channel(SIGNAL_MOL_PAUSE);
    let mut resume_ch = ctx.make_signal_channel(SIGNAL_MOL_RESUME);
    let mut cancel_ch = ctx.make_signal_channel(SIGNAL_MOL_CANCEL);

    loop {
        // Check if all steps are done
        if steps.iter().all(|s| s.status == "done" || s.status == "failed") {
            let all_done = steps.iter().all(|s| s.status == "done");
            status = if all_done {
                "completed".to_string()
            } else {
                "failed".to_string()
            };
            tracing::info!("Molecule {id} {status}");
            break;
        }

        tokio::select! {
            biased;
            Some(_) = cancel_ch.next() => {
                status = "cancelled".to_string();
                tracing::info!("Molecule {id} cancelled");
                break;
            }
            Some(_) = pause_ch.next() => {
                if status == "running" {
                    status = "paused".to_string();
                    tracing::info!("Molecule {id} paused");
                }
            }
            Some(_) = resume_ch.next() => {
                if status == "paused" {
                    status = "running".to_string();
                    tracing::info!("Molecule {id} resumed");
                }
            }
            Some(signal) = step_done_ch.next() => {
                if status != "running" {
                    continue;
                }
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<MolStepDoneSignal>(&payload.data) {
                        if let Some(step) = steps.iter_mut().find(|s| s.ref_id == data.step_ref) {
                            step.status = "done".to_string();
                            step.output = data.output;
                            tracing::info!("Molecule {id}: step {} done", data.step_ref);
                        }
                        // Advance to next pending step
                        current_step = None;
                        for step in steps.iter_mut() {
                            if step.status == "pending" {
                                step.status = "in_progress".to_string();
                                current_step = Some(step.ref_id.clone());
                                tracing::info!("Molecule {id}: advancing to step {}", step.ref_id);
                                break;
                            }
                        }
                    }
                }
            }
            Some(signal) = step_fail_ch.next() => {
                if status != "running" {
                    continue;
                }
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<MolStepFailSignal>(&payload.data) {
                        if let Some(step) = steps.iter_mut().find(|s| s.ref_id == data.step_ref) {
                            step.status = "failed".to_string();
                            step.output = Some(data.reason.clone());
                            tracing::warn!("Molecule {id}: step {} failed — {}", data.step_ref, data.reason);
                        }
                        // On step failure, mark remaining as blocked
                        status = "failed".to_string();
                        break;
                    }
                }
            }
        }
    }

    Ok(WfExitValue::Normal(serde_json::to_string(&MoleculeState {
        id,
        formula_name,
        status,
        steps,
        current_step,
    })?))
}
