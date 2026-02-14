use clap::Subcommand;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::{AgentMailSignal, AgentNudgeSignal};

#[derive(Debug, Subcommand)]
pub enum MailCommand {
    /// Send a message to an agent
    Send {
        /// Recipient agent workflow ID
        to: String,
        /// Message body
        message: String,
        /// Sender identity
        #[arg(short, long, default_value = "cli")]
        from: String,
    },
    /// Send a nudge to an agent
    Nudge {
        /// Recipient agent workflow ID
        to: String,
        /// Nudge message
        message: String,
        /// Sender identity
        #[arg(short, long, default_value = "cli")]
        from: String,
    },
    /// Broadcast a message to all running agents
    Broadcast {
        /// Message body
        message: String,
        /// Sender identity
        #[arg(short, long, default_value = "cli")]
        from: String,
    },
}

pub async fn run(cmd: &MailCommand) -> anyhow::Result<()> {
    match cmd {
        MailCommand::Send { to, message, from } => handle_send(to, message, from).await,
        MailCommand::Nudge { to, message, from } => handle_nudge(to, message, from).await,
        MailCommand::Broadcast { message, from } => handle_broadcast(message, from).await,
    }
}

async fn handle_send(to: &str, message: &str, from: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    let signal_data = AgentMailSignal {
        from: from.to_string(),
        message: message.to_string(),
    };
    let payload = signal_data.as_json_payload()?;

    client
        .signal_workflow_execution(
            to.to_string(),
            String::new(),
            "agent_mail".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;

    println!("Mail sent to {to}: {message}");
    Ok(())
}

async fn handle_nudge(to: &str, message: &str, from: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    let signal_data = AgentNudgeSignal {
        from: from.to_string(),
        message: message.to_string(),
    };
    let payload = signal_data.as_json_payload()?;

    client
        .signal_workflow_execution(
            to.to_string(),
            String::new(),
            "agent_nudge".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;

    println!("Nudge sent to {to}: {message}");
    Ok(())
}

async fn handle_broadcast(message: &str, from: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    let resp = client
        .list_workflow_executions(
            100,
            vec![],
            "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'".to_string(),
        )
        .await?;

    if resp.executions.is_empty() {
        println!("No running agents found.");
        return Ok(());
    }

    let signal_data = AgentMailSignal {
        from: from.to_string(),
        message: message.to_string(),
    };
    let payload = signal_data.as_json_payload()?;

    let mut sent = 0;
    for exec in &resp.executions {
        let wf_id = exec
            .execution
            .as_ref()
            .map(|e| e.workflow_id.clone())
            .unwrap_or_default();

        if let Err(e) = client
            .signal_workflow_execution(
                wf_id.clone(),
                String::new(),
                "agent_mail".to_string(),
                Some(payload.clone().into()),
                None,
            )
            .await
        {
            tracing::warn!("Failed to send to {wf_id}: {e}");
        } else {
            sent += 1;
        }
    }

    println!("Broadcast sent to {sent} agents: {message}");
    Ok(())
}
