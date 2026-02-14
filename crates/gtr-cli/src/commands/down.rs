use temporalio_sdk_core::WorkflowClientTrait;

pub async fn run() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    println!("Shutting down Gas Town...");

    // Step 1: List all running workflows
    let query = "ExecutionStatus = 'Running'".to_string();
    let resp = client.list_workflow_executions(500, vec![], query).await?;

    let total = resp.executions.len();
    if total == 0 {
        println!("No running workflows found.");
        println!("Gas Town is down.");
        return Ok(());
    }

    // Step 2: Signal stop to all workflows and kill PTY processes
    let mut stopped = 0;
    let mut killed = 0;

    for exec in &resp.executions {
        let wf_id = exec
            .execution
            .as_ref()
            .map(|e| e.workflow_id.clone())
            .unwrap_or_default();

        let wf_type = exec.r#type.as_ref().map(|t| t.name.as_str()).unwrap_or("");

        // Determine the right stop signal based on workflow type
        let signal = match wf_type {
            "mayor_wf" => "mayor_stop",
            "boot_wf" => "agent_stop",
            "rig_wf" => "rig_stop",
            "refinery_wf" => "refinery_stop",
            "polecat_wf" => "polecat_kill",
            "dog_wf" => "dog_stop",
            "gate_wf" => "gate_close",
            _ => "agent_stop",
        };

        // Send stop signal
        let signal_result = client
            .signal_workflow_execution(
                wf_id.clone(),
                String::new(),
                signal.to_string(),
                None,
                None,
            )
            .await;

        if signal_result.is_ok() {
            stopped += 1;
        }

        // Kill PTY process if it exists
        if gtr_temporal::pty::kill_agent(&wf_id).unwrap_or(false) {
            killed += 1;
        }
    }

    println!("  Signaled {stopped}/{total} workflows to stop");
    if killed > 0 {
        println!("  Killed {killed} agent processes");
    }

    // Step 3: Clean up runtime directory
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let runtime_dir = format!("{home}/.gtr/runtime");
    if let Ok(entries) = std::fs::read_dir(&runtime_dir) {
        let mut cleaned = 0;
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                std::fs::remove_dir_all(entry.path()).ok();
                cleaned += 1;
            }
        }
        if cleaned > 0 {
            println!("  Cleaned {cleaned} runtime directories");
        }
    }

    println!("Gas Town is down.");
    Ok(())
}
