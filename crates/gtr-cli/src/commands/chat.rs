use clap::Args;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::AgentMailSignal;

#[derive(Debug, Args)]
pub struct ChatCommand {
    /// Agent to send message to
    pub agent: String,
    /// Message to send
    pub message: String,
    /// Sender identity
    #[arg(short, long, default_value = "human")]
    pub from: String,
}

pub async fn run(cmd: &ChatCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    let signal = AgentMailSignal {
        from: cmd.from.clone(),
        message: cmd.message.clone(),
    };
    let payload = signal.as_json_payload()?;

    client
        .signal_workflow_execution(
            cmd.agent.clone(),
            String::new(),
            "agent_mail".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;

    println!("Sent to {}: {}", cmd.agent, cmd.message);
    println!("(Agent will see this in their mail inbox)");

    Ok(())
}
