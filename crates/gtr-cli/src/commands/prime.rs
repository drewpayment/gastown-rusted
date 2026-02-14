use clap::Args;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Args)]
pub struct PrimeCommand {
    /// Agent workflow ID (overrides GTR_AGENT env var)
    #[arg(long)]
    pub agent: Option<String>,
}

pub async fn run(cmd: &PrimeCommand) -> anyhow::Result<()> {
    let agent_id = cmd
        .agent
        .clone()
        .or_else(|| std::env::var("GTR_AGENT").ok())
        .ok_or_else(|| anyhow::anyhow!("No agent specified. Set GTR_AGENT or use --agent"))?;

    let role = std::env::var("GTR_ROLE").unwrap_or_else(|_| "unknown".into());
    let rig = std::env::var("GTR_RIG").unwrap_or_else(|_| "unknown".into());

    let client = crate::client::connect().await?;

    let resp = client
        .describe_workflow_execution(agent_id.clone(), None)
        .await?;

    let status = resp
        .workflow_execution_info
        .as_ref()
        .map(|i| crate::commands::convoy::workflow_status_str(i.status))
        .unwrap_or("Unknown");

    println!("# GTR Context — Agent Prime");
    println!();
    println!("Agent: {agent_id}");
    println!("Role:  {role}");
    println!("Rig:   {rig}");
    println!("Status: {status}");
    println!();

    // In a full implementation, we'd use Temporal queries to get the agent's
    // current hook, inbox, and handoff content from the running workflow.
    // For now, we output what we can from describe + env vars.
    println!("## Instructions");
    println!();
    println!("You are a {role} agent on rig {rig}.");
    println!("Use `gtr hook {agent_id}` to check your current work assignment.");
    println!("Use `gtr mail read` to check your inbox.");
    println!("Use `gtr handoff` before ending your session to preserve context.");

    Ok(())
}
