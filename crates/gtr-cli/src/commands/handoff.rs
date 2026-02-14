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

    // Send a mail to self with handoff content
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
    println!("Next session: run `gtr prime` to recover context.");

    Ok(())
}
