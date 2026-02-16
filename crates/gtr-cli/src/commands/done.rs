use clap::Parser;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::{PolecatDoneSignal, RefineryEnqueueSignal};

#[derive(Debug, Parser)]
#[command(about = "Mark work done and enqueue branch for merge (defaults to GTR_WORK_ITEM env var)")]
pub struct DoneCommand {
    /// Work item ID to enqueue for merge (defaults to GTR_WORK_ITEM env var)
    pub work_item_id: Option<String>,

    /// Branch name
    #[arg(short, long)]
    pub branch: String,

    /// Priority (0 = highest)
    #[arg(short, long, default_value = "2")]
    pub priority: u8,

    /// Summary of work done (sent to polecat workflow and mayor)
    #[arg(short, long)]
    pub summary: Option<String>,
}

pub async fn run(cmd: &DoneCommand) -> anyhow::Result<()> {
    let work_item_id = cmd
        .work_item_id
        .clone()
        .or_else(|| std::env::var("GTR_WORK_ITEM").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No work item specified. Pass <WORK_ITEM_ID> or set GTR_WORK_ITEM env var"
            )
        })?;

    let client = crate::client::connect().await?;

    // Signal polecat that work is done (if we're running as an agent)
    if let Ok(agent_id) = std::env::var("GTR_AGENT") {
        let done_signal = PolecatDoneSignal {
            branch: cmd.branch.clone(),
            status: "completed".to_string(),
            summary: cmd.summary.clone(),
        };
        let payload = done_signal.as_json_payload()?;
        client
            .signal_workflow_execution(
                agent_id.clone(),
                String::new(),
                "polecat_done".to_string(),
                Some(payload.into()),
                None,
            )
            .await
            .ok(); // Don't fail if polecat signal fails
        println!("Signaled polecat '{agent_id}' — done");
    }

    // Determine refinery workflow ID based on rig context
    let refinery_id = std::env::var("GTR_RIG")
        .map(|rig| format!("{rig}-refinery"))
        .unwrap_or_else(|_| "refinery".to_string());

    let signal = RefineryEnqueueSignal {
        work_item_id: work_item_id.clone(),
        branch: cmd.branch.clone(),
        priority: cmd.priority,
    };

    let payload = signal.as_json_payload()?;
    let enqueue_result = client
        .signal_workflow_execution(
            refinery_id.clone(),
            String::new(),
            "refinery_enqueue".to_string(),
            Some(payload.into()),
            None,
        )
        .await;

    match enqueue_result {
        Ok(_) => {
            println!(
                "Enqueued '{}' (branch: {}, priority: P{}) for merge → {refinery_id}",
                work_item_id, cmd.branch, cmd.priority
            );
        }
        Err(e) => {
            println!(
                "Done signaled, but refinery enqueue failed (branch saved on '{}'): {e}",
                cmd.branch
            );
        }
    }
    Ok(())
}
