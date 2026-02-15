use clap::Subcommand;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Subcommand)]
pub enum AgentsCommand {
    /// List all running agents (Temporal agent_wf workflows)
    List,
    /// Show details for a specific agent by workflow ID
    Show {
        /// Agent workflow ID
        name: String,
    },
    /// Summary of running agents (count and names)
    Status,
}

pub async fn run(cmd: &AgentsCommand) -> anyhow::Result<()> {
    match cmd {
        AgentsCommand::List => handle_list().await,
        AgentsCommand::Show { name } => handle_show(name).await,
        AgentsCommand::Status => handle_status().await,
    }
}

async fn handle_list() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let resp = client
        .list_workflow_executions(
            100,
            vec![],
            "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'".to_string(),
        )
        .await?;

    if resp.executions.is_empty() {
        println!("No running agents.");
        return Ok(());
    }

    println!("{:<30} {:<12} {:<24}", "AGENT", "STATUS", "STARTED");
    println!("{}", "-".repeat(66));

    for exec in &resp.executions {
        let wf_id = exec
            .execution
            .as_ref()
            .map(|e| e.workflow_id.as_str())
            .unwrap_or("unknown");
        let status = crate::commands::convoy::workflow_status_str(exec.status);
        let started = exec
            .start_time
            .as_ref()
            .map(|t| format_timestamp(t))
            .unwrap_or_else(|| "-".to_string());
        println!("{:<30} {:<12} {:<24}", wf_id, status, started);
    }

    println!("\n{} agent(s) running", resp.executions.len());
    Ok(())
}

async fn handle_show(name: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let resp = client
        .describe_workflow_execution(name.to_string(), None)
        .await?;

    if let Some(info) = resp.workflow_execution_info {
        let status = crate::commands::convoy::workflow_status_str(info.status);
        let wf_id = info
            .execution
            .as_ref()
            .map(|e| e.workflow_id.as_str())
            .unwrap_or(name);

        println!("Agent:     {wf_id}");
        println!("Status:    {status}");

        if let Some(ref start) = info.start_time {
            println!("Started:   {}", format_timestamp(start));
        }
        if let Some(ref close) = info.close_time {
            println!("Closed:    {}", format_timestamp(close));
        }
        println!("History:   {} events", info.history_length);
    } else {
        println!("No agent found: {name}");
    }

    Ok(())
}

async fn handle_status() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let resp = client
        .list_workflow_executions(
            100,
            vec![],
            "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'".to_string(),
        )
        .await?;

    if resp.executions.is_empty() {
        println!("No running agents.");
        return Ok(());
    }

    let names: Vec<&str> = resp
        .executions
        .iter()
        .filter_map(|e| e.execution.as_ref().map(|ex| ex.workflow_id.as_str()))
        .collect();

    println!("{} agent(s) running: {}", names.len(), names.join(", "));
    Ok(())
}

fn format_timestamp(ts: &prost_wkt_types::Timestamp) -> String {
    let secs = ts.seconds;
    let nanos = ts.nanos as u64;
    let total_ms = secs as u64 * 1000 + nanos / 1_000_000;
    let dt = chrono::DateTime::from_timestamp_millis(total_ms as i64);
    match dt {
        Some(d) => d.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => format!("{secs}s"),
    }
}
