use clap::Subcommand;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::DogDispatchSignal;

#[derive(Debug, Subcommand)]
pub enum DogCommand {
    /// Create a new dog worker
    Create {
        /// Dog name
        name: String,
    },
    /// List active dogs
    List,
    /// Show dog status
    Status {
        /// Dog name
        name: String,
    },
    /// Dispatch a dog to a rig with work
    Dispatch {
        /// Dog name
        name: String,
        /// Rig to dispatch to
        #[arg(long)]
        rig: String,
        /// Work item ID
        #[arg(long)]
        work: String,
        /// Optional plugin to run
        #[arg(long)]
        plugin: Option<String>,
    },
    /// Release a dog back to idle
    Release {
        /// Dog name
        name: String,
    },
    /// Stop a dog
    Stop {
        /// Dog name
        name: String,
    },
}

pub async fn run(cmd: &DogCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    match cmd {
        DogCommand::Create { name } => {
            let input_payload = name.as_json_payload()?;
            client
                .start_workflow(
                    vec![input_payload],
                    "work".to_string(),
                    format!("dog-{name}"),
                    "dog_wf".to_string(),
                    None,
                    Default::default(),
                )
                .await?;
            println!("Created dog: {name}");
        }
        DogCommand::List => {
            let query =
                "WorkflowType = 'dog_wf' AND ExecutionStatus = 'Running'".to_string();
            let resp = client
                .list_workflow_executions(100, vec![], query)
                .await?;
            if resp.executions.is_empty() {
                println!("No active dogs.");
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
        DogCommand::Status { name } => {
            let resp = client
                .describe_workflow_execution(format!("dog-{name}"), None)
                .await?;
            if let Some(info) = resp.workflow_execution_info {
                let status =
                    crate::commands::convoy::workflow_status_str(info.status);
                println!("Dog:     {name}");
                println!("Status:  {status}");
                println!("History: {} events", info.history_length);
            } else {
                println!("No dog found: {name}");
            }
        }
        DogCommand::Dispatch {
            name,
            rig,
            work,
            plugin,
        } => {
            let signal = DogDispatchSignal {
                rig: rig.clone(),
                work_item_id: work.clone(),
                plugin: plugin.clone(),
            };
            let payload = signal.as_json_payload()?;
            client
                .signal_workflow_execution(
                    format!("dog-{name}"),
                    String::new(),
                    "dog_dispatch".to_string(),
                    Some(payload.into()),
                    None,
                )
                .await?;
            println!("Dispatched dog {name} → rig {rig}, work {work}");
        }
        DogCommand::Release { name } => {
            client
                .signal_workflow_execution(
                    format!("dog-{name}"),
                    String::new(),
                    "dog_release".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Released dog: {name}");
        }
        DogCommand::Stop { name } => {
            client
                .signal_workflow_execution(
                    format!("dog-{name}"),
                    String::new(),
                    "dog_stop".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Stopped dog: {name}");
        }
    }
    Ok(())
}
