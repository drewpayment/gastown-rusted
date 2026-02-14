use std::process::Stdio;

use serde::{Deserialize, Serialize};
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActContext, ActivityError};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAgentInput {
    pub agent_id: String,
    pub runtime: String,
    pub args: Vec<String>,
    pub work_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAgentOutput {
    pub agent_id: String,
    pub exit_code: Option<i32>,
    pub output_lines: Vec<String>,
}

pub async fn read_agent_output(
    ctx: ActContext,
    input: RunAgentInput,
) -> Result<RunAgentOutput, ActivityError> {
    let work_dir = input.work_dir.unwrap_or_else(|| ".".to_string());

    let (cmd_name, cmd_args): (&str, Vec<String>) = match input.runtime.as_str() {
        "claude" => ("claude", input.args.clone()),
        "shell" => ("sh", vec!["-c".to_string(), input.args.join(" ")]),
        other => {
            return Err(ActivityError::NonRetryable(anyhow::anyhow!(
                "unsupported runtime: {other}"
            )));
        }
    };

    let mut child = Command::new(cmd_name)
        .args(&cmd_args)
        .current_dir(&work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ActivityError::Retryable {
            source: anyhow::anyhow!("failed to spawn {cmd_name}: {e}"),
            explicit_delay: None,
        })?;

    let stdout = child.stdout.take().ok_or_else(|| {
        ActivityError::NonRetryable(anyhow::anyhow!("failed to capture stdout"))
    })?;

    let mut reader = BufReader::new(stdout).lines();
    let mut output_lines = Vec::new();
    let mut lines_read: usize = 0;

    loop {
        if ctx.is_cancelled() {
            child.kill().await.ok();
            tracing::info!("Agent {} output reader cancelled", input.agent_id);
            return Err(ActivityError::Cancelled { details: None });
        }

        match reader.next_line().await {
            Ok(Some(line)) => {
                lines_read += 1;
                tracing::debug!("Agent {}: {}", input.agent_id, line);
                output_lines.push(line);

                if lines_read % 10 == 0 {
                    let progress = format!("{{\"lines_read\":{lines_read}}}");
                    if let Ok(payload) = progress.as_json_payload() {
                        ctx.record_heartbeat(vec![payload]);
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                tracing::warn!("Agent {}: read error: {e}", input.agent_id);
                break;
            }
        }
    }

    let status = child.wait().await.map_err(|e| ActivityError::Retryable {
        source: anyhow::anyhow!("failed to wait for process: {e}"),
        explicit_delay: None,
    })?;

    let exit_code = status.code();
    tracing::info!(
        "Agent {} exited with code {:?}, read {} lines",
        input.agent_id, exit_code, lines_read
    );

    Ok(RunAgentOutput {
        agent_id: input.agent_id,
        exit_code,
        output_lines,
    })
}
