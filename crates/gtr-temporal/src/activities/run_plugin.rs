use std::process::Stdio;

use serde::{Deserialize, Serialize};
use temporalio_sdk::{ActContext, ActivityError};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPluginInput {
    pub plugin_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub work_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPluginOutput {
    pub plugin_name: String,
    pub exit_code: Option<i32>,
    pub stdout: Vec<String>,
}

pub async fn run_plugin(
    _ctx: ActContext,
    input: RunPluginInput,
) -> Result<RunPluginOutput, ActivityError> {
    let work_dir = input.work_dir.unwrap_or_else(|| ".".to_string());

    let mut child = Command::new(&input.command)
        .args(&input.args)
        .current_dir(&work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ActivityError::Retryable {
            source: anyhow::anyhow!("failed to spawn plugin {}: {e}", input.plugin_name),
            explicit_delay: None,
        })?;

    let stdout = child.stdout.take().ok_or_else(|| {
        ActivityError::NonRetryable(anyhow::anyhow!("failed to capture stdout"))
    })?;

    let mut reader = BufReader::new(stdout).lines();
    let mut output_lines = Vec::new();

    while let Ok(Some(line)) = reader.next_line().await {
        output_lines.push(line);
    }

    let status = child.wait().await.map_err(|e| ActivityError::Retryable {
        source: anyhow::anyhow!("failed to wait for plugin: {e}"),
        explicit_delay: None,
    })?;

    let exit_code = status.code();
    tracing::info!(
        "Plugin {} exited with code {:?}, {} lines output",
        input.plugin_name, exit_code, output_lines.len()
    );

    Ok(RunPluginOutput {
        plugin_name: input.plugin_name,
        exit_code,
        stdout: output_lines,
    })
}
