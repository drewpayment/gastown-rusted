use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

pub async fn run() -> anyhow::Result<()> {
    println!("Starting Gas Town...");

    // Ensure runtime directory exists
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let runtime_dir = format!("{home}/.gtr/runtime");
    std::fs::create_dir_all(&runtime_dir)?;

    let client = crate::client::connect().await?;

    // Check if mayor is already running
    let resp = client
        .describe_workflow_execution("mayor".to_string(), None)
        .await;
    if let Ok(r) = &resp {
        if let Some(info) = &r.workflow_execution_info {
            if info.status == 1 {
                println!("Gas Town is already running (mayor workflow active).");
                println!("Run `gtr worker run` to start the worker if needed.");
                return Ok(());
            }
        }
    }

    // Start mayor workflow
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

    // Start boot workflow
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

    println!("Gas Town is up.");
    println!("  Mayor workflow: running");
    println!("  Boot workflow: running");
    println!();
    println!("Now start the worker: gtr worker run");

    Ok(())
}
