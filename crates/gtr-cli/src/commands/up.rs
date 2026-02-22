use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

/// Start mayor + boot workflows. Returns (mayor_started, boot_started).
pub async fn start_workflows() -> anyhow::Result<(bool, bool)> {
    // Ensure runtime directory exists
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let runtime_dir = format!("{home}/.gtr/runtime");
    std::fs::create_dir_all(&runtime_dir)?;

    let client = crate::client::connect().await?;

    // Check and start mayor
    let mut mayor_started = false;
    let mayor_running = is_workflow_running_pub(&client, "mayor").await;
    if !mayor_running {
        let payload = "default".as_json_payload()?;
        client
            .start_workflow(
                vec![payload],
                "work".to_string(),
                "mayor".to_string(),
                "mayor_wf".to_string(),
                None,
                Default::default(),
            )
            .await?;
        mayor_started = true;
    }

    // Check and start boot
    let mut boot_started = false;
    let boot_running = is_workflow_running_pub(&client, "boot").await;
    if !boot_running {
        let boot_payload = 120u64.as_json_payload()?;
        let _ = client
            .start_workflow(
                vec![boot_payload],
                "work".to_string(),
                "boot".to_string(),
                "boot_wf".to_string(),
                None,
                Default::default(),
            )
            .await;
        boot_started = true;
    }

    Ok((mayor_started, boot_started))
}

pub async fn is_workflow_running_pub(
    client: &dyn WorkflowClientTrait,
    workflow_id: &str,
) -> bool {
    if let Ok(r) = client
        .describe_workflow_execution(workflow_id.to_string(), None)
        .await
    {
        if let Some(info) = &r.workflow_execution_info {
            return info.status == 1; // RUNNING
        }
    }
    false
}

pub async fn run() -> anyhow::Result<()> {
    println!("Starting Gas Town...");

    let (mayor_started, boot_started) = start_workflows().await?;

    if !mayor_started && !boot_started {
        println!("Gas Town is already running (mayor + boot workflows active).");
        println!("Run `rgt worker run` to start the worker if needed.");
        return Ok(());
    }

    println!("Gas Town is up.");
    println!("  Mayor workflow: {}", if mayor_started { "started" } else { "already running" });
    println!("  Boot workflow: {}", if boot_started { "started" } else { "already running" });
    println!();
    println!("Now start the worker: rgt worker run");

    Ok(())
}
