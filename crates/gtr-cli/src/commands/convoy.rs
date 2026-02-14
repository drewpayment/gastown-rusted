use clap::Subcommand;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

#[derive(Debug, Subcommand)]
pub enum ConvoyCommand {
    /// Create a new convoy
    Create {
        /// Convoy title
        title: String,
        /// Priority
        #[arg(short, long, default_value = "medium")]
        priority: String,
    },
    /// List active convoys
    List,
    /// Show convoy details
    Show {
        /// Convoy ID
        id: String,
    },
}

pub async fn run(cmd: &ConvoyCommand) -> anyhow::Result<()> {
    match cmd {
        ConvoyCommand::Create { title, priority } => handle_create(title, priority).await,
        ConvoyCommand::List => handle_list().await,
        ConvoyCommand::Show { id } => handle_show(id).await,
    }
}

async fn handle_create(title: &str, _priority: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let id = gtr_core::ids::convoy_id();

    let input_payload = (id.as_str(), title).as_json_payload()?;
    client
        .start_workflow(
            vec![input_payload],
            "work".to_string(),
            id.clone(),
            "convoy_wf".to_string(),
            None,
            Default::default(),
        )
        .await?;

    println!("Created convoy: {id} — {title}");
    Ok(())
}

async fn handle_list() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let resp = client
        .list_workflow_executions(
            100,
            vec![],
            "WorkflowType = 'convoy_wf'".to_string(),
        )
        .await?;

    if resp.executions.is_empty() {
        println!("No convoys found.");
        return Ok(());
    }

    for exec in &resp.executions {
        let wf_id = exec
            .execution
            .as_ref()
            .map(|e| e.workflow_id.as_str())
            .unwrap_or("?");
        let status = workflow_status_str(exec.status);
        println!("{wf_id}  {status}");
    }
    Ok(())
}

async fn handle_show(id: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let resp = client
        .describe_workflow_execution(id.to_string(), None)
        .await?;

    if let Some(info) = resp.workflow_execution_info {
        let status = workflow_status_str(info.status);
        let wf_id = info
            .execution
            .as_ref()
            .map(|e| e.workflow_id.as_str())
            .unwrap_or(id);

        println!("Convoy:    {wf_id}");
        println!("Status:    {status}");
        println!("History:   {} events", info.history_length);
    } else {
        println!("No execution info for {id}");
    }

    Ok(())
}

pub fn workflow_status_str(status: i32) -> &'static str {
    match status {
        0 => "Unspecified",
        1 => "Running",
        2 => "Completed",
        3 => "Failed",
        4 => "Canceled",
        5 => "Terminated",
        6 => "ContinuedAsNew",
        7 => "TimedOut",
        _ => "Unknown",
    }
}
