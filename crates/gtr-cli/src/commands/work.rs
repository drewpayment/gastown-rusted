use clap::Subcommand;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Subcommand)]
pub enum WorkCommand {
    /// Show a work item
    Show {
        /// Work item ID
        id: String,
    },
    /// List work items
    List,
    /// Close a work item
    Close {
        /// Work item ID
        id: String,
    },
}

pub async fn run(cmd: &WorkCommand) -> anyhow::Result<()> {
    match cmd {
        WorkCommand::Show { id } => handle_show(id).await,
        WorkCommand::List => {
            println!("work list: not yet implemented");
            Ok(())
        }
        WorkCommand::Close { id } => handle_close(id).await,
    }
}

async fn handle_show(id: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let resp = client
        .describe_workflow_execution(id.to_string(), None)
        .await?;

    if let Some(info) = resp.workflow_execution_info {
        let status = workflow_status_str(info.status);
        let wf_type = info
            .r#type
            .as_ref()
            .map(|t| t.name.as_str())
            .unwrap_or("unknown");
        let wf_id = info
            .execution
            .as_ref()
            .map(|e| e.workflow_id.as_str())
            .unwrap_or(id);

        println!("Workflow:  {wf_id}");
        println!("Type:      {wf_type}");
        println!("Status:    {status}");

        if let Some(ref start) = info.start_time {
            println!("Started:   {}", format_timestamp(start));
        }
        if let Some(ref close) = info.close_time {
            println!("Closed:    {}", format_timestamp(close));
        }
        println!("History:   {} events", info.history_length);
    } else {
        println!("No execution info returned for {id}");
    }

    Ok(())
}

async fn handle_close(id: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    client
        .signal_workflow_execution(
            id.to_string(),
            String::new(), // empty run_id targets latest run
            "close".to_string(),
            None,
            None,
        )
        .await?;
    println!("Closed work item: {id}");
    Ok(())
}

fn workflow_status_str(status: i32) -> &'static str {
    match status {
        0 => "Unspecified",
        1 => "Running",
        2 => "Completed",
        3 => "Failed",
        4 => "Canceled",
        5 => "Terminated",
        6 => "ContinuedAsNew",
        7 => "TimedOut",
        _ => "Unknown",
    }
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
