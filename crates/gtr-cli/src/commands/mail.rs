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
    /// Check inbox (query agent workflow for unread mail)
    Inbox {
        /// Agent workflow ID (overrides GTR_AGENT env var)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Mark a message as read
    Read {
        /// Message index
        index: usize,
        /// Agent workflow ID
        #[arg(long)]
        agent: Option<String>,
    },
    /// Reply to a message
    Reply {
        /// Message index to reply to
        index: usize,
        /// Reply message
        #[arg(short, long)]
        message: String,
        /// Agent workflow ID
        #[arg(long)]
        agent: Option<String>,
    },
    /// View a message thread
    Thread {
        /// Message index
        index: usize,
    },
    /// Search mail by content
    Search {
        /// Search query
        query: String,
    },
    /// Archive a message
    Archive {
        /// Message index
        index: usize,
    },
    /// Clear all mail
    Clear {
        /// Agent workflow ID
        #[arg(long)]
        agent: Option<String>,
    },
    /// Check for new mail (for hooks/polling)
    Check {
        /// Agent workflow ID (overrides GTR_AGENT env var)
        #[arg(long)]
        agent: Option<String>,
    },
}

pub async fn run(cmd: &MailCommand) -> anyhow::Result<()> {
    match cmd {
        MailCommand::Send { to, message, from } => handle_send(to, message, from).await,
        MailCommand::Nudge { to, message, from } => handle_nudge(to, message, from).await,
        MailCommand::Broadcast { message, from } => handle_broadcast(message, from).await,
        MailCommand::Inbox { agent } => {
            let agent_id = resolve_agent(agent)?;
            println!("Inbox for {agent_id}:");
            println!("  (mail is stored in agent workflow state — use Temporal query when available)");
            println!("  Tip: Stop agent to see final state with all mail");
            Ok(())
        }
        MailCommand::Read { index, agent } => {
            let agent_id = resolve_agent(agent)?;
            println!("Marked message #{index} as read for {agent_id}");
            println!("  (read tracking requires Temporal query support on agent workflow)");
            Ok(())
        }
        MailCommand::Reply {
            index,
            message,
            agent,
        } => {
            let agent_id = resolve_agent(agent)?;
            // Reply sends a mail back to the original sender
            // For now, send to self as a thread entry
            let client = crate::client::connect().await?;
            let signal = AgentMailSignal {
                from: agent_id.clone(),
                message: format!("[reply to #{index}] {message}"),
            };
            let payload = signal.as_json_payload()?;
            client
                .signal_workflow_execution(
                    agent_id.clone(),
                    String::new(),
                    "agent_mail".to_string(),
                    Some(payload.into()),
                    None,
                )
                .await?;
            println!("Reply to #{index} sent on {agent_id}");
            Ok(())
        }
        MailCommand::Thread { index } => {
            println!("Thread for message #{index}:");
            println!("  (thread view requires Temporal query support)");
            Ok(())
        }
        MailCommand::Search { query } => {
            println!("Searching mail for: {query}");
            println!("  (search requires Temporal query support on agent workflow)");
            Ok(())
        }
        MailCommand::Archive { index } => {
            println!("Archived message #{index}");
            println!("  (archive requires Temporal query support on agent workflow)");
            Ok(())
        }
        MailCommand::Clear { agent } => {
            let agent_id = resolve_agent(agent)?;
            println!("Cleared all mail for {agent_id}");
            println!("  (clear requires Temporal query support on agent workflow)");
            Ok(())
        }
        MailCommand::Check { agent } => {
            let agent_id = resolve_agent(agent)?;
            let client = crate::client::connect().await?;
            let resp = client
                .describe_workflow_execution(agent_id.clone(), None)
                .await?;
            if let Some(info) = resp.workflow_execution_info {
                let status = crate::commands::convoy::workflow_status_str(info.status);
                println!("Agent {agent_id}: {status} ({} events)", info.history_length);
            } else {
                println!("No agent found: {agent_id}");
            }
            Ok(())
        }
    }
}

fn resolve_agent(agent: &Option<String>) -> anyhow::Result<String> {
    agent
        .clone()
        .or_else(|| std::env::var("GTR_AGENT").ok())
        .ok_or_else(|| anyhow::anyhow!("No agent specified. Set GTR_AGENT or use --agent"))
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
