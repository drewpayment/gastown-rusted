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
        let status = crate::commands::convoy::workflow_status_str(info.status);
        println!("Agent:  {}", cmd.agent);
        println!("Status: {status}");

        // Display search attributes if available for hook info
        // In a full implementation, we'd use a Temporal query to get the hook state.
        // For now, the hook is part of the agent workflow's internal state,
        // visible when the workflow completes (AgentState JSON).
        if info.status == 1 {
            println!("Hook:   (query agent workflow for current hook state)");
            println!("        Use: gtr hook {} — agent must expose query handler", cmd.agent);
        } else if info.status == 2 {
            println!("Hook:   (agent stopped — check workflow result for final state)");
        }
    } else {
        println!("No agent workflow found: {}", cmd.agent);
    }

    Ok(())
}
