use clap::Subcommand;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Subcommand)]
pub enum DiagnosticsCommand {
    /// Show system health
    Health,
    /// Show version info
    Version,
    /// Run diagnostic checks
    Check,
    /// Stream recent workflow events
    Feed {
        /// Number of recent workflows to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Query workflow history for an agent
    Audit {
        /// Agent ID to audit
        actor: String,
    },
    /// Show recent activity across all workflows
    Trail {
        /// Number of recent workflows to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
}

pub async fn run(cmd: &DiagnosticsCommand) -> anyhow::Result<()> {
    match cmd {
        DiagnosticsCommand::Health => {
            println!("diagnostics health: use 'rgt doctor' for health checks");
            Ok(())
        }
        DiagnosticsCommand::Version => {
            println!("rgt v{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        DiagnosticsCommand::Check => {
            println!("diagnostics check: use 'rgt doctor' for system checks");
            Ok(())
        }
        DiagnosticsCommand::Feed { limit } => handle_feed(*limit).await,
        DiagnosticsCommand::Audit { actor } => handle_audit(actor).await,
        DiagnosticsCommand::Trail { limit } => handle_trail(*limit).await,
    }
}

async fn handle_feed(limit: usize) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    let query = "ExecutionStatus = 'Running' OR ExecutionStatus = 'Completed'";
    let resp = client
        .list_workflow_executions(limit as i32, vec![], query.to_string())
        .await?;

    println!("Recent Workflow Activity (last {limit})");
    println!("{:-<70}", "");

    for exec in &resp.executions {
        let wf_id = exec
            .execution
            .as_ref()
            .map(|e| e.workflow_id.as_str())
            .unwrap_or("?");
        let wf_type = exec
            .r#type
            .as_ref()
            .map(|t| t.name.as_str())
            .unwrap_or("?");
        let status = workflow_status_str(exec.status);
        let start = exec
            .start_time
            .as_ref()
            .map(|t| format_timestamp(t))
            .unwrap_or_else(|| "?".into());

        println!("{wf_id:<40} {wf_type:<20} {status:<12} {start}");
    }

    if resp.executions.is_empty() {
        println!("  No workflow executions found.");
    }

    Ok(())
}

async fn handle_audit(actor: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    // Search for agent workflows matching this actor
    let query = format!("WorkflowId = '{actor}'");
    let resp = client
        .list_workflow_executions(50, vec![], query)
        .await?;

    println!("Audit: {actor}");
    println!("{:-<70}", "");

    if resp.executions.is_empty() {
        // Try prefix match
        let query = format!("WorkflowId STARTS_WITH '{actor}'");
        let resp2 = client
            .list_workflow_executions(50, vec![], query)
            .await?;

        for exec in &resp2.executions {
            print_execution(exec);
        }

        if resp2.executions.is_empty() {
            println!("  No workflows found for actor '{actor}'");
        }
    } else {
        for exec in &resp.executions {
            print_execution(exec);
        }
    }

    Ok(())
}

async fn handle_trail(limit: usize) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    // Get all workflows (Temporal returns most recent first by default)
    let query = "";
    let resp = client
        .list_workflow_executions(limit as i32, vec![], query.to_string())
        .await?;

    println!("Activity Trail (most recent {limit})");
    println!("{:-<80}", "");
    println!(
        "{:<35} {:<18} {:<12} {:<15}",
        "Workflow ID", "Type", "Status", "Started"
    );
    println!("{:-<80}", "");

    for exec in &resp.executions {
        let wf_id = exec
            .execution
            .as_ref()
            .map(|e| e.workflow_id.as_str())
            .unwrap_or("?");
        let wf_type = exec
            .r#type
            .as_ref()
            .map(|t| t.name.as_str())
            .unwrap_or("?");
        let status = workflow_status_str(exec.status);
        let start = exec
            .start_time
            .as_ref()
            .map(|t| format_timestamp(t))
            .unwrap_or_else(|| "?".into());

        // Truncate long IDs
        let wf_id_display = if wf_id.len() > 33 {
            format!("{}…", &wf_id[..32])
        } else {
            wf_id.to_string()
        };

        println!("{wf_id_display:<35} {wf_type:<18} {status:<12} {start}");
    }

    if resp.executions.is_empty() {
        println!("  No recent activity found.");
    }

    Ok(())
}

fn print_execution(exec: &temporalio_common::protos::temporal::api::workflow::v1::WorkflowExecutionInfo) {
    let wf_id = exec
        .execution
        .as_ref()
        .map(|e| e.workflow_id.as_str())
        .unwrap_or("?");
    let wf_type = exec
        .r#type
        .as_ref()
        .map(|t| t.name.as_str())
        .unwrap_or("?");
    let status = workflow_status_str(exec.status);
    let events = exec.history_length;
    let start = exec
        .start_time
        .as_ref()
        .map(|t| format_timestamp(t))
        .unwrap_or_else(|| "?".into());

    println!("  {wf_id} ({wf_type}) — {status}, {events} events, started {start}");
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
        Some(d) => d.format("%Y-%m-%d %H:%M").to_string(),
        None => format!("{secs}s"),
    }
}
