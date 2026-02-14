use clap::Parser;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::RefineryEnqueueSignal;

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

    let signal = RefineryEnqueueSignal {
        work_item_id: cmd.work_item_id.clone(),
        branch: cmd.branch.clone(),
        priority: cmd.priority,
    };

    let payload = signal.as_json_payload()?;
    client
        .signal_workflow_execution(
            "refinery".to_string(),
            String::new(),
            "refinery_enqueue".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;

    println!(
        "Enqueued '{}' (branch: {}, priority: P{}) for merge",
        cmd.work_item_id, cmd.branch, cmd.priority
    );
    Ok(())
}
