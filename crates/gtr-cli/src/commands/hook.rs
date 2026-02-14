use clap::Args;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Args)]
pub struct HookCommand {
    /// Agent workflow ID to query
    pub agent: String,
}

pub async fn run(cmd: &HookCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    let resp = client
        .describe_workflow_execution(cmd.agent.clone(), None)
        .await?;

    if let Some(info) = resp.workflow_execution_info {
        let status = match info.status {
            1 => "Running",
            2 => "Completed",
            _ => "Unknown",
        };
        println!("Agent:   {}", cmd.agent);
        println!("Status:  {status}");
    } else {
        println!("No agent workflow found: {}", cmd.agent);
    }

    Ok(())
}
