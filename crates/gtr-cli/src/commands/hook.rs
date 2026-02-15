use clap::Args;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Args)]
#[command(about = "Query agent's current work assignment (defaults to GTR_AGENT env var)")]
pub struct HookCommand {
    /// Agent workflow ID to query (defaults to GTR_AGENT env var)
    pub agent: Option<String>,
}

pub async fn run(cmd: &HookCommand) -> anyhow::Result<()> {
    let agent_id = cmd
        .agent
        .clone()
        .or_else(|| std::env::var("GTR_AGENT").ok())
        .ok_or_else(|| anyhow::anyhow!("No agent specified. Set GTR_AGENT or pass <AGENT> argument"))?;

    let client = crate::client::connect().await?;

    let resp = client
        .describe_workflow_execution(agent_id.clone(), None)
        .await?;

    if let Some(info) = resp.workflow_execution_info {
        let status = crate::commands::convoy::workflow_status_str(info.status);
        println!("Agent:  {agent_id}");
        println!("Status: {status}");

        if info.status == 1 {
            println!("Hook:   (query agent workflow for current hook state)");
            println!("        Use: gtr hook {agent_id} — agent must expose query handler");
        } else if info.status == 2 {
            println!("Hook:   (agent stopped — check workflow result for final state)");
        }
    } else {
        println!("No agent workflow found: {agent_id}");
    }

    Ok(())
}
