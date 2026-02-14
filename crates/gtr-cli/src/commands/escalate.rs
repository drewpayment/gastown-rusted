use clap::Parser;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Parser)]
pub struct EscalateCommand {
    /// Work item ID to escalate
    pub id: String,
}

pub async fn run(cmd: &EscalateCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    client
        .signal_workflow_execution(
            cmd.id.clone(),
            String::new(),
            "escalate".to_string(),
            None,
            None,
        )
        .await?;

    println!("Escalated work item: {}", cmd.id);
    Ok(())
}
