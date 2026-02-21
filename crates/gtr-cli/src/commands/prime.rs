use clap::Args;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Args)]
pub struct PrimeCommand {
    /// Agent workflow ID (overrides GTR_AGENT env var)
    #[arg(long)]
    pub agent: Option<String>,

    /// Hook mode — output context for Claude Code SessionStart hook
    #[arg(long)]
    pub hook: bool,
}

pub async fn run(cmd: &PrimeCommand) -> anyhow::Result<()> {
    let agent_id = cmd
        .agent
        .clone()
        .or_else(|| std::env::var("GTR_AGENT").ok())
        .ok_or_else(|| anyhow::anyhow!("No agent specified. Set GTR_AGENT or use --agent"))?;

    let role = std::env::var("GTR_ROLE").unwrap_or_else(|_| "unknown".into());
    let rig = std::env::var("GTR_RIG").unwrap_or_else(|_| "none".into());
    let root = std::env::var("GTR_ROOT").unwrap_or_else(|_| "~/.gtr".into());

    let client = crate::client::connect().await?;

    // Query agent workflow state
    let agent_resp = client
        .describe_workflow_execution(agent_id.clone(), None)
        .await;

    let agent_status = agent_resp
        .as_ref()
        .ok()
        .and_then(|r| r.workflow_execution_info.as_ref())
        .map(|i| crate::commands::convoy::workflow_status_str(i.status))
        .unwrap_or("Unknown");

    // Query for hooked work items assigned to this agent
    let hook_query =
        "WorkflowType = 'work_item_wf' AND ExecutionStatus = 'Running'".to_string();
    let work_items = client
        .list_workflow_executions(50, vec![], hook_query)
        .await
        .unwrap_or_default();

    let work_count = work_items.executions.len();

    // Query for unread mail (check agent workflow for mail signals)
    // We can't easily query signal history, but we can note it as available

    // Output context
    println!("# GTR Agent Context");
    println!();
    println!("- **Agent:** {agent_id}");
    println!("- **Role:** {role}");
    println!("- **Rig:** {rig}");
    println!("- **Root:** {root}");
    println!("- **Status:** {agent_status}");
    println!("- **Active work items:** {work_count}");
    println!();
    println!("## Instructions");
    println!();

    match role.as_str() {
        "mayor" => {
            println!("You are the **Mayor** of Gas Town.");
            println!("- `rgt hook` — check your current work assignment");
            println!("- `rgt mail inbox` — check for messages from agents");
            println!("- `rgt feed` — monitor system activity");
            println!("- `rgt sling <work-id> --target <rig>` — assign work to polecats");
            println!("- `rgt status` — system overview");
        }
        "witness" => {
            println!("You are the **Witness** for rig '{rig}'.");
            println!("- `rgt feed` — monitor rig activity");
            println!("- `rgt mail send mayor <message>` — escalate issues");
            println!("- Watch for stuck polecats and report to mayor");
        }
        "refinery" => {
            println!("You are the **Refinery** for rig '{rig}'.");
            println!("- `rgt mq list` — check merge queue");
            println!("- Process branches: rebase, test, merge");
            println!("- Report conflicts to mayor");
        }
        _ if role.contains("polecats") => {
            println!("You are a **Polecat** on rig '{rig}'.");
            println!("- Work on your assigned task in this directory");
            println!("- `rgt hook` — check your work assignment");
            println!("- When done: `rgt done <work-id> --branch <branch>`");
        }
        _ => {
            println!("You are agent '{agent_id}' ({role}).");
            println!("- `rgt hook` — check your current work");
            println!("- `rgt mail inbox` — check for messages");
            println!("- `rgt handoff` — save context before ending session");
        }
    }

    if cmd.hook {
        println!();
        println!("---");
        println!("*Context injected by `rgt prime --hook` (SessionStart)*");
    }

    Ok(())
}
