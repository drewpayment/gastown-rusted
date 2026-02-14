use temporalio_sdk_core::WorkflowClientTrait;

pub async fn run() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    println!("Gas Town Status");
    println!("===============");

    // Mayor status
    let mayor_running = match client
        .describe_workflow_execution("mayor".to_string(), None)
        .await
    {
        Ok(resp) => {
            let status = resp
                .workflow_execution_info
                .map(|info| info.status == 1)
                .unwrap_or(false);
            status
        }
        Err(_) => false,
    };

    let mayor_pid = if gtr_temporal::pty::is_alive("mayor") {
        gtr_temporal::pty::read_pid("mayor")
            .map(|p| format!(" (PID {})", p.as_raw()))
            .unwrap_or_default()
    } else {
        String::new()
    };

    println!(
        "  Mayor:    {}{}",
        if mayor_running { "running" } else { "not running" },
        mayor_pid
    );

    // Boot status
    let boot_running = match client
        .describe_workflow_execution("boot".to_string(), None)
        .await
    {
        Ok(resp) => resp
            .workflow_execution_info
            .map(|info| info.status == 1)
            .unwrap_or(false),
        Err(_) => false,
    };
    println!(
        "  Boot:     {}",
        if boot_running { "running" } else { "not running" }
    );

    // Rigs
    let rig_query = "WorkflowType = 'rig_wf' AND ExecutionStatus = 'Running'".to_string();
    let rigs = client
        .list_workflow_executions(100, vec![], rig_query)
        .await
        .unwrap_or_default();

    if !rigs.executions.is_empty() {
        println!("  Rigs:");
        for exec in &rigs.executions {
            let wf_id = exec
                .execution
                .as_ref()
                .map(|e| e.workflow_id.as_str())
                .unwrap_or("?");
            let rig_name = wf_id.strip_prefix("rig-").unwrap_or(wf_id);

            println!("    {rig_name}:");

            // Check witness
            let witness_id = format!("{rig_name}-witness");
            let witness_alive = gtr_temporal::pty::is_alive(&witness_id);
            let witness_pid = if witness_alive {
                gtr_temporal::pty::read_pid(&witness_id)
                    .map(|p| format!(" (PID {})", p.as_raw()))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            println!(
                "      Witness:   {}{}",
                if witness_alive { "running" } else { "not running" },
                witness_pid
            );

            // Check refinery
            let refinery_id = format!("{rig_name}-refinery");
            let refinery_alive = gtr_temporal::pty::is_alive(&refinery_id);
            let refinery_pid = if refinery_alive {
                gtr_temporal::pty::read_pid(&refinery_id)
                    .map(|p| format!(" (PID {})", p.as_raw()))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            println!(
                "      Refinery:  {}{}",
                if refinery_alive { "running" } else { "not running" },
                refinery_pid
            );

            // Count polecats
            let polecat_query =
                "WorkflowType = 'polecat_wf' AND ExecutionStatus = 'Running'".to_string();
            let polecats = client
                .list_workflow_executions(100, vec![], polecat_query)
                .await
                .unwrap_or_default();

            let rig_polecats: Vec<_> = polecats
                .executions
                .iter()
                .filter(|e| {
                    e.execution
                        .as_ref()
                        .map(|x| x.workflow_id.starts_with(rig_name))
                        .unwrap_or(false)
                })
                .collect();

            println!("      Polecats:  {} active", rig_polecats.len());
            for pc in &rig_polecats {
                let pc_id = pc
                    .execution
                    .as_ref()
                    .map(|e| e.workflow_id.as_str())
                    .unwrap_or("?");
                let pc_alive = gtr_temporal::pty::is_alive(pc_id);
                let pc_pid = if pc_alive {
                    gtr_temporal::pty::read_pid(pc_id)
                        .map(|p| format!(" (PID {})", p.as_raw()))
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                println!(
                    "        {pc_id}: {}{pc_pid}",
                    if pc_alive { "alive" } else { "dead" }
                );
            }
        }
    } else {
        println!("  Rigs:     none");
    }

    // Convoys
    let convoy_query = "WorkflowType = 'convoy_wf' AND ExecutionStatus = 'Running'".to_string();
    let convoys = client
        .list_workflow_executions(100, vec![], convoy_query)
        .await
        .unwrap_or_default();
    println!("  Convoys:  {} active", convoys.executions.len());

    // Merge queue (refineries)
    let refinery_query =
        "WorkflowType = 'refinery_wf' AND ExecutionStatus = 'Running'".to_string();
    let refineries = client
        .list_workflow_executions(100, vec![], refinery_query)
        .await
        .unwrap_or_default();
    println!("  Refineries: {} active", refineries.executions.len());

    Ok(())
}
