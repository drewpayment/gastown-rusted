use std::io::{self, Write};
use std::time::Duration;

use clap::Args;
use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};
use temporalio_sdk_core::WorkflowClientTrait;

use crate::commands::convoy::workflow_status_str;

#[derive(Debug, Args)]
pub struct FeedCommand {
    /// Refresh interval in seconds
    #[arg(short, long, default_value = "5")]
    interval: u64,
    /// Run once and exit (no auto-refresh)
    #[arg(long)]
    once: bool,
}

pub async fn run(cmd: &FeedCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    loop {
        let mut out = io::stdout();

        if !cmd.once {
            execute!(out, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        }

        let now = chrono::Local::now().format("%H:%M:%S");
        writeln!(out, "=== GTR Feed [{now}] ===\n")?;

        // Section 1: Agent tree
        writeln!(out, "--- Agents ---")?;
        let agent_query = "ExecutionStatus = 'Running'".to_string();
        let agents = client
            .list_workflow_executions(100, vec![], agent_query)
            .await?;

        let mut mayors = vec![];
        let mut deacons = vec![];
        let mut witnesses = vec![];
        let mut refineries = vec![];
        let mut polecats = vec![];
        let mut dogs = vec![];
        let mut gates = vec![];
        let mut convoys = vec![];
        let mut other = vec![];

        for exec in &agents.executions {
            let wf_type = exec
                .r#type
                .as_ref()
                .map(|t| t.name.as_str())
                .unwrap_or("?");
            let wf_id = exec
                .execution
                .as_ref()
                .map(|e| e.workflow_id.as_str())
                .unwrap_or("?");
            let status = workflow_status_str(exec.status);

            let entry = format!("  {wf_id} ({status})");

            match wf_type {
                "mayor_wf" => mayors.push(entry),
                "agent_wf" if wf_id.contains("deacon") => deacons.push(entry),
                "agent_wf" if wf_id.contains("witness") => witnesses.push(entry),
                "refinery_wf" => refineries.push(entry),
                "polecat_wf" => polecats.push(entry),
                "dog_wf" => dogs.push(entry),
                "gate_wf" => gates.push(entry),
                "convoy_wf" => convoys.push(entry),
                "agent_wf" => other.push(entry),
                _ => other.push(format!("  {wf_id} [{wf_type}] ({status})")),
            }
        }

        if !mayors.is_empty() {
            writeln!(out, "Mayor:")?;
            for m in &mayors {
                writeln!(out, "{m}")?;
            }
        }
        if !deacons.is_empty() {
            writeln!(out, "Deacons:")?;
            for d in &deacons {
                writeln!(out, "{d}")?;
            }
        }
        if !witnesses.is_empty() {
            writeln!(out, "Witnesses:")?;
            for w in &witnesses {
                writeln!(out, "{w}")?;
            }
        }
        if !refineries.is_empty() {
            writeln!(out, "Refineries:")?;
            for r in &refineries {
                writeln!(out, "{r}")?;
            }
        }
        if !polecats.is_empty() {
            writeln!(out, "Polecats ({}):", polecats.len())?;
            for p in &polecats {
                writeln!(out, "{p}")?;
            }
        }
        if !dogs.is_empty() {
            writeln!(out, "Dogs ({}):", dogs.len())?;
            for d in &dogs {
                writeln!(out, "{d}")?;
            }
        }
        if !other.is_empty() {
            writeln!(out, "Other agents ({}):", other.len())?;
            for o in &other {
                writeln!(out, "{o}")?;
            }
        }

        if agents.executions.is_empty() {
            writeln!(out, "  (no running workflows)")?;
        }

        // Section 2: Convoys
        if !convoys.is_empty() {
            writeln!(out, "\n--- Convoys ---")?;
            for c in &convoys {
                writeln!(out, "{c}")?;
            }
        }

        // Section 3: Gates
        if !gates.is_empty() {
            writeln!(out, "\n--- Gates ---")?;
            for g in &gates {
                writeln!(out, "{g}")?;
            }
        }

        // Section 4: Recent completions
        writeln!(out, "\n--- Recent Completions ---")?;
        let completed_query =
            "ExecutionStatus != 'Running' ORDER BY CloseTime DESC".to_string();
        let completed = client
            .list_workflow_executions(10, vec![], completed_query)
            .await?;

        if completed.executions.is_empty() {
            writeln!(out, "  (none)")?;
        } else {
            for exec in &completed.executions {
                let wf_type = exec
                    .r#type
                    .as_ref()
                    .map(|t| t.name.as_str())
                    .unwrap_or("?");
                let wf_id = exec
                    .execution
                    .as_ref()
                    .map(|e| e.workflow_id.as_str())
                    .unwrap_or("?");
                let status = workflow_status_str(exec.status);
                let close_time = exec
                    .close_time
                    .as_ref()
                    .map(|t| {
                        chrono::DateTime::from_timestamp(t.seconds, t.nanos as u32)
                            .map(|dt| dt.format("%H:%M:%S").to_string())
                            .unwrap_or_else(|| "?".into())
                    })
                    .unwrap_or_else(|| "?".into());
                writeln!(out, "  {wf_id} [{wf_type}] {status} at {close_time}")?;
            }
        }

        writeln!(out)?;
        out.flush()?;

        if cmd.once {
            break;
        }

        writeln!(out, "(Refreshing every {}s — Ctrl+C to exit)", cmd.interval)?;
        out.flush()?;
        tokio::time::sleep(Duration::from_secs(cmd.interval)).await;
    }

    Ok(())
}
