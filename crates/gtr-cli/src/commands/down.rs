use temporalio_sdk_core::WorkflowClientTrait;

pub async fn run() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    // Signal mayor to stop gracefully
    let resp = client
        .describe_workflow_execution("mayor".to_string(), None)
        .await;

    if let Ok(resp) = resp {
        if let Some(info) = resp.workflow_execution_info {
            if info.status == 1 {
                client
                    .signal_workflow_execution(
                        "mayor".to_string(),
                        String::new(),
                        "mayor_stop".to_string(),
                        None,
                        None,
                    )
                    .await?;
                println!("Mayor stopped.");
            } else {
                println!("Mayor is not running (status={}).", info.status);
            }
        }
    } else {
        println!("No mayor workflow found.");
    }

    println!("Gas Town is down.");
    Ok(())
}
