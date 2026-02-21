use clap::Subcommand;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Subcommand)]
pub enum RigCommand {
    /// Register a new rig (git repository)
    Add {
        /// Rig name
        name: String,
        /// Git URL to clone
        #[arg(long)]
        git_url: String,
    },
    /// List registered rigs
    List,
    /// Show rig status
    Status {
        /// Rig name
        name: String,
    },
    /// Temporarily pause a rig (no agent auto-starts)
    Park {
        /// Rig name
        name: String,
    },
    /// Resume a parked rig
    Unpark {
        /// Rig name
        name: String,
    },
    /// Long-term shutdown of a rig
    Dock {
        /// Rig name
        name: String,
    },
    /// Resume a docked rig
    Undock {
        /// Rig name
        name: String,
    },
    /// Boot a rig (start witness + refinery)
    Boot {
        /// Rig name
        name: String,
    },
    /// Stop all agents on a rig
    Stop {
        /// Rig name
        name: String,
    },
}

pub async fn run(cmd: &RigCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    match cmd {
        RigCommand::Add { name, git_url } => {
            // Create rig directory structure
            gtr_core::dirs::ensure_rig_dirs(name)?;
            println!("Created directory structure for rig '{name}'");

            // Bare-clone the repo so worktrees can be created later
            let repo_git = gtr_core::dirs::rig_dir(name).join(".repo.git");
            if repo_git.exists() {
                println!("Repo already cloned at {}", repo_git.display());
            } else {
                println!("Cloning {git_url} (bare) ...");
                let status = std::process::Command::new("git")
                    .args(["clone", "--bare", git_url.as_str()])
                    .arg(&repo_git)
                    .status()?;
                if !status.success() {
                    anyhow::bail!("git clone --bare failed (exit {})", status);
                }
                println!("Cloned bare repo to {}", repo_git.display());
            }

            // Start rig workflow
            let input_payload = (name.as_str(), git_url.as_str()).as_json_payload()?;
            client
                .start_workflow(
                    vec![input_payload],
                    "work".to_string(),
                    format!("rig-{name}"),
                    "rig_wf".to_string(),
                    None,
                    Default::default(),
                )
                .await?;
            println!("Started rig workflow: rig-{name}");

            // Signal rig to boot (spawn witness + refinery)
            client
                .signal_workflow_execution(
                    format!("rig-{name}"),
                    String::new(),
                    "rig_boot".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Booted rig '{name}' — witness + refinery will spawn");

            println!("Registered rig: {name} ({git_url})");
        }
        RigCommand::List => {
            let query =
                "WorkflowType = 'rig_wf' AND ExecutionStatus = 'Running'".to_string();
            let resp = client
                .list_workflow_executions(100, vec![], query)
                .await?;
            if resp.executions.is_empty() {
                println!("No rigs registered.");
            } else {
                for exec in &resp.executions {
                    let wf_id = exec
                        .execution
                        .as_ref()
                        .map(|e| e.workflow_id.as_str())
                        .unwrap_or("?");
                    let status = crate::commands::convoy::workflow_status_str(exec.status);
                    println!("  {wf_id}  {status}");
                }
            }
        }
        RigCommand::Status { name } => {
            let resp = client
                .describe_workflow_execution(format!("rig-{name}"), None)
                .await?;
            if let Some(info) = resp.workflow_execution_info {
                let status = crate::commands::convoy::workflow_status_str(info.status);
                println!("Rig:     {name}");
                println!("Status:  {status}");
                println!("History: {} events", info.history_length);
            } else {
                println!("No execution info for rig {name}");
            }
        }
        RigCommand::Park { name } => {
            client
                .signal_workflow_execution(
                    format!("rig-{name}"),
                    String::new(),
                    "rig_park".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Parked rig: {name}");
        }
        RigCommand::Unpark { name } => {
            client
                .signal_workflow_execution(
                    format!("rig-{name}"),
                    String::new(),
                    "rig_unpark".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Unparked rig: {name}");
        }
        RigCommand::Dock { name } => {
            client
                .signal_workflow_execution(
                    format!("rig-{name}"),
                    String::new(),
                    "rig_dock".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Docked rig: {name}");
        }
        RigCommand::Undock { name } => {
            client
                .signal_workflow_execution(
                    format!("rig-{name}"),
                    String::new(),
                    "rig_undock".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Undocked rig: {name}");
        }
        RigCommand::Boot { name } => {
            client
                .signal_workflow_execution(
                    format!("rig-{name}"),
                    String::new(),
                    "rig_boot".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Booted rig {name}: signaled to spawn witness + refinery");
        }
        RigCommand::Stop { name } => {
            client
                .signal_workflow_execution(
                    format!("rig-{name}"),
                    String::new(),
                    "rig_stop".to_string(),
                    None,
                    None,
                )
                .await?;
            println!("Stopped rig: {name}");
        }
    }
    Ok(())
}
