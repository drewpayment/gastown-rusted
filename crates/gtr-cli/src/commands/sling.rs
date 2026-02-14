use clap::Args;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::{AgentAssignSignal, DogDispatchSignal};

#[derive(Debug, Args)]
pub struct SlingCommand {
    /// Work item IDs to assign (one or more)
    pub work_ids: Vec<String>,

    /// Target: agent ID, rig name (auto-spawns polecat), "mayor", or "dogs"
    #[arg(short, long)]
    pub target: String,

    /// Agent runtime to use (claude, codex, gemini)
    #[arg(long, default_value = "claude")]
    pub agent: String,

    /// Title for the work item(s)
    #[arg(long)]
    pub title: Option<String>,
}

pub async fn run(cmd: &SlingCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    if cmd.work_ids.is_empty() {
        anyhow::bail!("No work item IDs provided");
    }

    match cmd.target.as_str() {
        "mayor" => {
            // Send all work items to the Mayor for dispatch
            for work_id in &cmd.work_ids {
                let signal = AgentAssignSignal {
                    work_item_id: work_id.clone(),
                    title: cmd.title.clone().unwrap_or_else(|| work_id.clone()),
                };
                let payload = signal.as_json_payload()?;
                client
                    .signal_workflow_execution(
                        "mayor".to_string(),
                        String::new(),
                        "agent_assign".to_string(),
                        Some(payload.into()),
                        None,
                    )
                    .await?;
                println!("Slung {work_id} → mayor");
            }
        }
        "dogs" => {
            // Dispatch to idle dogs
            for work_id in &cmd.work_ids {
                // Find an idle dog — for now, dispatch to first running dog_wf
                let query =
                    "WorkflowType = 'dog_wf' AND ExecutionStatus = 'Running'".to_string();
                let resp = client
                    .list_workflow_executions(100, vec![], query)
                    .await?;
                if let Some(exec) = resp.executions.first() {
                    let dog_id = exec
                        .execution
                        .as_ref()
                        .map(|e| e.workflow_id.clone())
                        .unwrap_or_default();
                    let signal = DogDispatchSignal {
                        rig: "default".to_string(),
                        work_item_id: work_id.clone(),
                        plugin: None,
                    };
                    let payload = signal.as_json_payload()?;
                    client
                        .signal_workflow_execution(
                            dog_id.clone(),
                            String::new(),
                            "dog_dispatch".to_string(),
                            Some(payload.into()),
                            None,
                        )
                        .await?;
                    println!("Slung {work_id} → dog {dog_id}");
                } else {
                    println!("No idle dogs available for {work_id}");
                }
            }
        }
        target => {
            // Target is either an agent ID or a rig name
            // If target contains "rig-" or matches a known rig, auto-spawn polecat
            if target.starts_with("rig-") || !target.contains('-') {
                // Auto-spawn polecat per work item
                let rig = target.strip_prefix("rig-").unwrap_or(target);
                for work_id in &cmd.work_ids {
                    let polecat_name = gtr_core::namepool::next_name();
                    let polecat_id =
                        gtr_core::state::polecat_workflow_id(rig, &polecat_name);
                    let title =
                        cmd.title.clone().unwrap_or_else(|| work_id.clone());
                    let input_payload = (
                        polecat_name.as_str(),
                        rig,
                        work_id.as_str(),
                        title.as_str(),
                    )
                        .as_json_payload()?;
                    client
                        .start_workflow(
                            vec![input_payload],
                            "work".to_string(),
                            polecat_id.clone(),
                            "polecat_wf".to_string(),
                            None,
                            Default::default(),
                        )
                        .await?;
                    println!(
                        "Slung {work_id} → polecat {polecat_name} on rig {rig} ({polecat_id})"
                    );
                }
            } else {
                // Direct agent assignment
                for work_id in &cmd.work_ids {
                    let signal = AgentAssignSignal {
                        work_item_id: work_id.clone(),
                        title: cmd
                            .title
                            .clone()
                            .unwrap_or_else(|| work_id.clone()),
                    };
                    let payload = signal.as_json_payload()?;
                    client
                        .signal_workflow_execution(
                            target.to_string(),
                            String::new(),
                            "agent_assign".to_string(),
                            Some(payload.into()),
                            None,
                        )
                        .await?;
                    println!("Slung {work_id} → {target}");
                }
            }
        }
    }

    if cmd.work_ids.len() > 1 {
        println!(
            "Batch dispatched {} work items to {}",
            cmd.work_ids.len(),
            cmd.target
        );
    }

    Ok(())
}
