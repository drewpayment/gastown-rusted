use clap::Args;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Args)]
pub struct UnslingCommand {
    /// Agent workflow ID to unassign
    pub agent: String,
}

pub async fn run(cmd: &UnslingCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    client
        .signal_workflow_execution(
            cmd.agent.clone(),
            String::new(),
            "agent_unassign".to_string(),
            None,
            None,
        )
        .await?;

    println!("Unslung {}", cmd.agent);
    Ok(())
}
