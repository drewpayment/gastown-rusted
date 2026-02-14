use clap::Subcommand;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::RigAgentEntry;

#[derive(Debug, Subcommand)]
pub enum CrewCommand {
    /// Create a persistent crew workspace
    Add {
        /// Crew member name
        name: String,
        /// Rig to create workspace in
        #[arg(long)]
        rig: String,
    },
    /// List crew workspaces
    List,
    /// Start a crew session
    Start {
        /// Crew member name
        name: String,
    },
    /// Stop a crew session
    Stop {
        /// Crew member name
        name: String,
    },
    /// Remove a crew workspace
    Remove {
        /// Crew member name
        name: String,
    },
}

pub async fn run(cmd: &CrewCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    match cmd {
        CrewCommand::Add { name, rig } => {
            let agent_id = format!("{rig}-crew-{name}");
            // Start agent workflow with crew role
            let input_payload = (agent_id.as_str(), "crew").as_json_payload()?;
            client
                .start_workflow(
                    vec![input_payload],
                    "work".to_string(),
                    agent_id.clone(),
                    "agent_wf".to_string(),
                    None,
                    Default::default(),
                )
                .await?;

            // Register with rig workflow
            let reg = RigAgentEntry {
                agent_id: agent_id.clone(),
                role: "crew".to_string(),
            };
            let payload = reg.as_json_payload()?;
            client
                .signal_workflow_execution(
                    format!("rig-{rig}"),
                    String::new(),
                    "rig_register_agent".to_string(),
                    Some(payload.into()),
                    None,
                )
                .await?;
            println!("Created crew workspace: {name} on rig {rig}");
        }
        CrewCommand::List => {
            let query =
                "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'".to_string();
            let resp = client
                .list_workflow_executions(100, vec![], query)
                .await?;
            let mut found = false;
            for exec in &resp.executions {
                if let Some(info) = &exec.execution {
                    if info.workflow_id.contains("-crew-") {
                        let status =
                            crate::commands::convoy::workflow_status_str(exec.status);
                        println!("  {}  {status}", info.workflow_id);
                        found = true;
                    }
                }
            }
            if !found {
                println!("No crew workspaces found.");
            }
        }
        CrewCommand::Start { name } => {
            println!("Starting crew session for {name}");
            println!("  (session management not yet implemented — needs spawn_agent activity)");
        }
        CrewCommand::Stop { name } => {
            let query =
                "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'".to_string();
            let resp = client
                .list_workflow_executions(100, vec![], query)
                .await?;
            for exec in &resp.executions {
                if let Some(info) = &exec.execution {
                    if info.workflow_id.contains(&format!("-crew-{name}")) {
                        client
                            .signal_workflow_execution(
                                info.workflow_id.clone(),
                                String::new(),
                                "agent_stop".to_string(),
                                None,
                                None,
                            )
                            .await?;
                        println!("Stopped crew: {name}");
                        return Ok(());
                    }
                }
            }
            println!("Crew member not found: {name}");
        }
        CrewCommand::Remove { name } => {
            println!("crew remove {name}: not yet fully implemented (need git worktree cleanup)");
        }
    }
    Ok(())
}
