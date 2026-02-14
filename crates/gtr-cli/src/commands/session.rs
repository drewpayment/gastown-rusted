use clap::Subcommand;
use temporalio_sdk_core::WorkflowClientTrait;

use crate::commands::convoy::workflow_status_str;

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    /// List running agent sessions
    List,
    /// Show status for a specific session
    Status {
        /// Agent workflow ID
        id: String,
    },
}

pub async fn run(cmd: &SessionCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    match cmd {
        SessionCommand::List => {
            let query = "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'".to_string();
            let resp = client
                .list_workflow_executions(100, vec![], query)
                .await?;

            if resp.executions.is_empty() {
                println!("No active sessions.");
                return Ok(());
            }

            println!("Active sessions:");
            for exec in &resp.executions {
                let wf_id = exec
                    .execution
                    .as_ref()
                    .map(|e| e.workflow_id.as_str())
                    .unwrap_or("?");
                let status = workflow_status_str(exec.status);
                let start = exec
                    .start_time
                    .as_ref()
                    .map(|t| {
                        chrono::DateTime::from_timestamp(t.seconds, t.nanos as u32)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "?".into())
                    })
                    .unwrap_or_else(|| "?".into());
                println!("  {wf_id}  {status}  started {start}  ({} events)", exec.history_length);
            }

            Ok(())
        }
        SessionCommand::Status { id } => {
            let resp = client
                .describe_workflow_execution(id.clone(), None)
                .await?;

            if let Some(info) = resp.workflow_execution_info {
                let status = workflow_status_str(info.status);
                let start = info
                    .start_time
                    .as_ref()
                    .map(|t| {
                        chrono::DateTime::from_timestamp(t.seconds, t.nanos as u32)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "?".into())
                    })
                    .unwrap_or_else(|| "?".into());
                let wf_type = info
                    .r#type
                    .as_ref()
                    .map(|t| t.name.as_str())
                    .unwrap_or("?");

                println!("Session: {id}");
                println!("  Type: {wf_type}");
                println!("  Status: {status}");
                println!("  Started: {start}");
                println!("  Events: {}", info.history_length);

                if let Some(close) = &info.close_time {
                    let close_str = chrono::DateTime::from_timestamp(close.seconds, close.nanos as u32)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "?".into());
                    println!("  Closed: {close_str}");
                }
            } else {
                println!("Session not found: {id}");
            }

            Ok(())
        }
    }
}
