use clap::Subcommand;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::GateType;

#[derive(Debug, Subcommand)]
pub enum GateCommand {
    /// Create a timer gate (auto-closes after duration)
    Timer {
        /// Duration in seconds
        #[arg(long)]
        secs: u64,
        /// Work item to park on this gate
        #[arg(long)]
        work: Option<String>,
    },
    /// Create a human approval gate
    Human {
        /// Description of what needs approval
        description: String,
        /// Work item to park on this gate
        #[arg(long)]
        work: Option<String>,
    },
    /// Approve a gate
    Approve {
        /// Gate workflow ID
        id: String,
    },
    /// Close/deny a gate
    Close {
        /// Gate workflow ID
        id: String,
    },
    /// Show gate status
    Status {
        /// Gate workflow ID
        id: String,
    },
    /// List active gates
    List,
}

pub async fn run(cmd: &GateCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    match cmd {
        GateCommand::Timer { secs, work } => {
            let gate_id = format!("gate-timer-{}", gtr_core::ids::work_item_id().replace("wi-", ""));
            let gate_type = GateType::Timer {
                duration_secs: *secs,
            };
            let input_payload =
                (gate_id.as_str(), &gate_type, work).as_json_payload()?;
            client
                .start_workflow(
                    vec![input_payload],
                    "work".to_string(),
                    gate_id.clone(),
                    "gate_wf".to_string(),
                    None,
                    Default::default(),
                )
                .await?;
            println!("Created timer gate: {gate_id} ({secs}s)");
        }
        GateCommand::Human { description, work } => {
            let gate_id = format!("gate-human-{}", gtr_core::ids::work_item_id().replace("wi-", ""));
            let gate_type = GateType::Human {
                description: description.clone(),
            };
            let input_payload =
                (gate_id.as_str(), &gate_type, work).as_json_payload()?;
            client
                .start_workflow(
                    vec![input_payload],
                    "work".to_string(),
                    gate_id.clone(),
                    "gate_wf".to_string(),
                    None,
                    Default::default(),
                )
                .await?;
            println!("Created human gate: {gate_id} — {description}");
        }
        GateCommand::Approve { id } => {
            client
                .signal_workflow_execution(
                    id.clone(),
                    String::new(),
                    "gate_approve".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Approved gate: {id}");
        }
        GateCommand::Close { id } => {
            client
                .signal_workflow_execution(
                    id.clone(),
                    String::new(),
                    "gate_close".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Closed gate: {id}");
        }
        GateCommand::Status { id } => {
            let resp = client
                .describe_workflow_execution(id.clone(), None)
                .await?;
            if let Some(info) = resp.workflow_execution_info {
                let status =
                    crate::commands::convoy::workflow_status_str(info.status);
                println!("Gate:    {id}");
                println!("Status:  {status}");
                println!("History: {} events", info.history_length);
            } else {
                println!("No gate found: {id}");
            }
        }
        GateCommand::List => {
            let query =
                "WorkflowType = 'gate_wf' AND ExecutionStatus = 'Running'".to_string();
            let resp = client
                .list_workflow_executions(100, vec![], query)
                .await?;
            if resp.executions.is_empty() {
                println!("No active gates.");
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
    }
    Ok(())
}
