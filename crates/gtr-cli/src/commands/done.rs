use clap::Parser;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::{PolecatDoneSignal, RefineryEnqueueSignal};

#[derive(Debug, Parser)]
pub struct DoneCommand {
    /// Work item ID to enqueue for merge
    pub work_item_id: String,

    /// Branch name
    #[arg(short, long)]
    pub branch: String,

    /// Priority (0 = highest)
    #[arg(short, long, default_value = "2")]
    pub priority: u8,
}

pub async fn run(cmd: &DoneCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    // Signal polecat that work is done (if we're running as an agent)
    if let Ok(agent_id) = std::env::var("GTR_AGENT") {
        let done_signal = PolecatDoneSignal {
            branch: cmd.branch.clone(),
            status: "completed".to_string(),
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
        work_item_id: cmd.work_item_id.clone(),
        branch: cmd.branch.clone(),
        priority: cmd.priority,
    };

    let payload = signal.as_json_payload()?;
    client
        .signal_workflow_execution(
            refinery_id.clone(),
            String::new(),
            "refinery_enqueue".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;

    println!(
        "Enqueued '{}' (branch: {}, priority: P{}) for merge → {refinery_id}",
        cmd.work_item_id, cmd.branch, cmd.priority
    );
    Ok(())
}
