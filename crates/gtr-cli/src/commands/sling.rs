use clap::Args;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::AgentAssignSignal;

#[derive(Debug, Args)]
pub struct SlingCommand {
    /// Work item ID to assign
    pub work_id: String,
    /// Agent workflow ID to assign to
    pub agent: String,
}

pub async fn run(cmd: &SlingCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    let signal_data = AgentAssignSignal {
        work_item_id: cmd.work_id.clone(),
        title: cmd.work_id.clone(),
    };
    let payload = signal_data.as_json_payload()?;

    client
        .signal_workflow_execution(
            cmd.agent.clone(),
            String::new(),
            "agent_assign".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;

    println!("Slung {} → {}", cmd.work_id, cmd.agent);
    Ok(())
}
