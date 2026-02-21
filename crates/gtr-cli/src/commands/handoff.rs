use std::path::Path;

use clap::Args;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::AgentMailSignal;

#[derive(Debug, Args)]
pub struct HandoffCommand {
    /// Handoff message (summary of current work, decisions, blockers)
    pub message: String,

    /// Agent workflow ID (overrides GTR_AGENT env var)
    #[arg(long)]
    pub agent: Option<String>,
}

pub async fn run(cmd: &HandoffCommand) -> anyhow::Result<()> {
    let agent_id = cmd
        .agent
        .clone()
        .or_else(|| std::env::var("GTR_AGENT").ok())
        .ok_or_else(|| anyhow::anyhow!("No agent specified. Set GTR_AGENT or use --agent"))?;

    let client = crate::client::connect().await?;

    // Step 1: Create checkpoint in current directory
    let checkpoint = gtr_core::checkpoint::Checkpoint {
        molecule_id: None,
        current_step: None,
        step_title: Some("handoff".to_string()),
        modified_files: vec![],
        last_commit: None,
        branch: None,
        hooked_work: std::env::var("GTR_WORK_ITEM").ok(),
        timestamp: chrono::Utc::now(),
        session_id: Some(agent_id.clone()),
        notes: Some(cmd.message.clone()),
    };
    checkpoint.write(Path::new("."))?;
    println!("Checkpoint saved to .gtr-checkpoint.json");

    // Step 2: Send handoff mail to self
    let mail = AgentMailSignal {
        from: format!("{agent_id} (handoff)"),
        message: format!("[HANDOFF] {}", cmd.message),
    };
    let payload = mail.as_json_payload()?;

    client
        .signal_workflow_execution(
            agent_id.clone(),
            String::new(),
            "agent_mail".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;

    println!("Handoff saved for {agent_id}");
    println!("Next session: run `rgt prime` to recover context.");

    Ok(())
}
