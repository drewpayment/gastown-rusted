use clap::Subcommand;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Subcommand)]
pub enum MayorCommand {
    /// Show mayor workflow status
    Status,
}

pub async fn run(cmd: &MayorCommand) -> anyhow::Result<()> {
    match cmd {
        MayorCommand::Status => handle_status().await,
    }
}

async fn handle_status() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let resp = client
        .describe_workflow_execution("mayor".to_string(), None)
        .await?;

    if let Some(info) = resp.workflow_execution_info {
        let status = match info.status {
            1 => "Running",
            2 => "Completed",
            3 => "Failed",
            4 => "Canceled",
            5 => "Terminated",
            _ => "Unknown",
        };
        println!("Mayor:   {status}");
        println!("History: {} events", info.history_length);
    } else {
        println!("Mayor workflow not found. Run `rgt up` to start.");
    }

    Ok(())
}
