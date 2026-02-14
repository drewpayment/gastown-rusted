use temporalio_sdk_core::WorkflowClientTrait;

pub async fn run() -> anyhow::Result<()> {
    println!("Gas Town Doctor — System Health Check");
    println!("======================================");

    // Check Temporal connection
    print!("Temporal connection... ");
    let client = match crate::client::connect().await {
        Ok(c) => {
            println!("OK");
            c
        }
        Err(e) => {
            println!("FAILED: {e}");
            return Ok(());
        }
    };

    // Check Mayor
    print!("Mayor workflow...      ");
    check_workflow(&client, "mayor").await;

    // Check Witness
    print!("Witness workflow...    ");
    check_workflow(&client, "witness").await;

    // Check Boot monitor...
    print!("Boot monitor...        ");
    check_workflow(&client, "boot").await;

    // Check Refinery
    print!("Refinery...            ");
    check_workflow(&client, "refinery").await;

    // Count running agents
    print!("Agent workflows...     ");
    let query = "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'";
    match client
        .list_workflow_executions(100, vec![], query.to_string())
        .await
    {
        Ok(resp) => {
            let count = resp.executions.len();
            println!("{count} running");
        }
        Err(e) => println!("FAILED: {e}"),
    }

    // Count running work items
    print!("Work item workflows... ");
    let query = "WorkflowType = 'work_item_wf' AND ExecutionStatus = 'Running'";
    match client
        .list_workflow_executions(100, vec![], query.to_string())
        .await
    {
        Ok(resp) => {
            let count = resp.executions.len();
            println!("{count} running");
        }
        Err(e) => println!("FAILED: {e}"),
    }

    println!("======================================");
    Ok(())
}

async fn check_workflow(
    client: &temporalio_sdk_core::RetryClient<temporalio_sdk_core::Client>,
    workflow_id: &str,
) {
    match client
        .describe_workflow_execution(workflow_id.to_string(), None)
        .await
    {
        Ok(resp) => {
            if let Some(info) = resp.workflow_execution_info {
                match info.status {
                    1 => println!("Running ({} events)", info.history_length),
                    2 => println!("Completed"),
                    _ => println!("Status {}", info.status),
                }
            } else {
                println!("No info");
            }
        }
        Err(_) => println!("Not found"),
    }
}
