use temporalio_sdk_core::WorkflowClientTrait;

pub async fn run() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    // Check if mayor is already running
    let resp = client
        .describe_workflow_execution("mayor".to_string(), None)
        .await;

    if let Ok(resp) = resp {
        if let Some(info) = resp.workflow_execution_info {
            if info.status == 1 {
                println!("Mayor already running.");
                return Ok(());
            }
        }
    }

    // Start the mayor workflow
    client
        .start_workflow(
            vec![],
            "work".to_string(),
            "mayor".to_string(),
            "mayor_wf".to_string(),
            None,
            Default::default(),
        )
        .await?;

    println!("Gas Town is up. Mayor workflow started.");
    println!("Run `gtr worker` in another terminal to start processing.");
    Ok(())
}
