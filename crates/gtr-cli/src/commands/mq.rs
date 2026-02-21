use clap::Subcommand;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::RefineryDequeueSignal;

#[derive(Debug, Subcommand)]
pub enum MqCommand {
    /// Show merge queue status
    Status,
    /// List items in the merge queue
    List,
    /// Remove an item from the merge queue
    Remove {
        /// Work item ID to remove
        work_item_id: String,
    },
}

pub async fn run(cmd: &MqCommand) -> anyhow::Result<()> {
    match cmd {
        MqCommand::Status => handle_status().await,
        MqCommand::List => handle_list().await,
        MqCommand::Remove { work_item_id } => handle_remove(work_item_id).await,
    }
}

async fn handle_status() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let resp = client
        .describe_workflow_execution("refinery".to_string(), None)
        .await?;

    if let Some(info) = resp.workflow_execution_info {
        let status = match info.status {
            1 => "Running",
            2 => "Completed",
            _ => "Unknown",
        };
        println!("Refinery:  {status}");
        println!("History:   {} events", info.history_length);
    } else {
        println!("Refinery not running. Start it with: rgt up");
    }

    Ok(())
}

async fn handle_list() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    // Describe to confirm refinery exists
    let resp = client
        .describe_workflow_execution("refinery".to_string(), None)
        .await;

    match resp {
        Ok(desc) => {
            if let Some(info) = desc.workflow_execution_info {
                if info.status == 1 {
                    println!("Refinery is running ({} events)", info.history_length);
                    println!("(Queue contents visible on workflow completion or via Temporal UI)");
                } else {
                    println!("Refinery is not running (status: {})", info.status);
                }
            }
        }
        Err(_) => {
            println!("Refinery not found. Start it with: rgt up");
        }
    }

    Ok(())
}

async fn handle_remove(work_item_id: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    let signal = RefineryDequeueSignal {
        work_item_id: work_item_id.to_string(),
    };

    let payload = signal.as_json_payload()?;
    client
        .signal_workflow_execution(
            "refinery".to_string(),
            String::new(),
            "refinery_dequeue".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;

    println!("Removed '{work_item_id}' from merge queue");
    Ok(())
}
