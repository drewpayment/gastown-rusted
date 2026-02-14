use clap::Subcommand;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Subcommand)]
pub enum PolecatCommand {
    /// List active polecats
    List,
    /// Show polecat status
    Status {
        /// Polecat workflow ID
        name: String,
    },
    /// Kill a polecat
    Kill {
        /// Polecat workflow ID
        name: String,
    },
    /// List stuck polecats
    Stuck,
}

pub async fn run(cmd: &PolecatCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    match cmd {
        PolecatCommand::List => {
            let query =
                "WorkflowType = 'polecat_wf' AND ExecutionStatus = 'Running'".to_string();
            let resp = client
                .list_workflow_executions(100, vec![], query)
                .await?;
            if resp.executions.is_empty() {
                println!("No active polecats.");
            } else {
                for exec in &resp.executions {
                    let wf_id = exec
                        .execution
                        .as_ref()
                        .map(|e| e.workflow_id.as_str())
                        .unwrap_or("?");
                    let status =
                        crate::commands::convoy::workflow_status_str(exec.status);
                    println!("  {wf_id}  {status}");
                }
            }
        }
        PolecatCommand::Status { name } => {
            let resp = client
                .describe_workflow_execution(name.clone(), None)
                .await?;
            if let Some(info) = resp.workflow_execution_info {
                let status =
                    crate::commands::convoy::workflow_status_str(info.status);
                println!("Polecat: {name}");
                println!("Status:  {status}");
                println!("History: {} events", info.history_length);
            } else {
                println!("No polecat found: {name}");
            }
        }
        PolecatCommand::Kill { name } => {
            client
                .signal_workflow_execution(
                    name.clone(),
                    String::new(),
                    "polecat_kill".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Killed polecat: {name}");
        }
        PolecatCommand::Stuck => {
            // List all running polecats — the "stuck" ones are identified
            // by the witness workflow, but we can list all and let the user inspect
            let query =
                "WorkflowType = 'polecat_wf' AND ExecutionStatus = 'Running'".to_string();
            let resp = client
                .list_workflow_executions(100, vec![], query)
                .await?;
            if resp.executions.is_empty() {
                println!("No stuck polecats.");
            } else {
                println!("Running polecats (inspect for stuck status):");
                for exec in &resp.executions {
                    let wf_id = exec
                        .execution
                        .as_ref()
                        .map(|e| e.workflow_id.as_str())
                        .unwrap_or("?");
                    println!("  {wf_id}");
                }
            }
        }
    }
    Ok(())
}
