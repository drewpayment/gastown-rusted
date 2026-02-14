use clap::Subcommand;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::MolStepDoneSignal;

#[derive(Debug, Subcommand)]
pub enum MolCommand {
    /// Show molecule status and step progress
    Status {
        /// Molecule workflow ID
        id: String,
    },
    /// Show current molecule from agent's hook
    Current,
    /// Signal that the current step is done
    StepDone {
        /// Molecule workflow ID
        id: String,
        /// Step ref to mark as done
        step: String,
        /// Optional output message
        #[arg(long)]
        output: Option<String>,
    },
    /// Cancel a molecule
    Cancel {
        /// Molecule workflow ID
        id: String,
    },
    /// Pause a molecule
    Pause {
        /// Molecule workflow ID
        id: String,
    },
    /// Resume a paused molecule
    Resume {
        /// Molecule workflow ID
        id: String,
    },
}

pub async fn run(cmd: &MolCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    match cmd {
        MolCommand::Status { id } => {
            let resp = client
                .describe_workflow_execution(id.clone(), None)
                .await?;
            if let Some(info) = resp.workflow_execution_info {
                let status =
                    crate::commands::convoy::workflow_status_str(info.status);
                println!("Molecule: {id}");
                println!("Status:   {status}");
                println!("History:  {} events", info.history_length);
            } else {
                println!("No molecule found: {id}");
            }
        }
        MolCommand::Current => {
            let agent_id = std::env::var("GTR_AGENT")
                .unwrap_or_else(|_| "unknown".into());
            println!("Current molecule for agent {agent_id}:");
            println!("  (query agent hook for molecule_id — use `gtr hook {agent_id}`)");
        }
        MolCommand::StepDone { id, step, output } => {
            let signal = MolStepDoneSignal {
                step_ref: step.clone(),
                output: output.clone(),
            };
            let payload = signal.as_json_payload()?;
            client
                .signal_workflow_execution(
                    id.clone(),
                    String::new(),
                    "mol_step_done".to_string(),
                    Some(payload.into()),
                    None,
                )
                .await?;
            println!("Marked step {step} as done on molecule {id}");
        }
        MolCommand::Cancel { id } => {
            client
                .signal_workflow_execution(
                    id.clone(),
                    String::new(),
                    "mol_cancel".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Cancelled molecule: {id}");
        }
        MolCommand::Pause { id } => {
            client
                .signal_workflow_execution(
                    id.clone(),
                    String::new(),
                    "mol_pause".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Paused molecule: {id}");
        }
        MolCommand::Resume { id } => {
            client
                .signal_workflow_execution(
                    id.clone(),
                    String::new(),
                    "mol_resume".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Resumed molecule: {id}");
        }
    }
    Ok(())
}
