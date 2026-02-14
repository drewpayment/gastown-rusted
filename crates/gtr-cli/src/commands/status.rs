use temporalio_sdk_core::WorkflowClientTrait;

pub async fn run() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    // Mayor status
    let mayor_status = match client
        .describe_workflow_execution("mayor".to_string(), None)
        .await
    {
        Ok(resp) => resp
            .workflow_execution_info
            .map(|info| match info.status {
                1 => "Running".to_string(),
                2 => "Completed".to_string(),
                _ => format!("Status({})", info.status),
            })
            .unwrap_or_else(|| "Not found".to_string()),
        Err(_) => "Not found".to_string(),
    };

    println!("Gas Town Status");
    println!("===============");
    println!("Mayor:    {mayor_status}");

    // Count running workflows by type
    for (label, query) in [
        ("Agents", "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'"),
        ("Convoys", "WorkflowType = 'convoy_wf' AND ExecutionStatus = 'Running'"),
        ("Work Items", "WorkflowType = 'work_item_wf' AND ExecutionStatus = 'Running'"),
    ] {
        let count = match client
            .list_workflow_executions(100, vec![], query.to_string())
            .await
        {
            Ok(resp) => resp.executions.len(),
            Err(_) => 0,
        };
        println!("{label:<12}{count}");
    }

    Ok(())
}
