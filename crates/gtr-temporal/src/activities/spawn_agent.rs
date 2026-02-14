use serde::{Deserialize, Serialize};
use temporalio_sdk::{ActContext, ActivityError};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentInput {
    pub agent_id: String,
    pub runtime: String,
    pub work_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentOutput {
    pub agent_id: String,
    pub pid: u32,
}

pub async fn spawn_agent(
    _ctx: ActContext,
    input: SpawnAgentInput,
) -> Result<SpawnAgentOutput, ActivityError> {
    let (cmd, args) = match input.runtime.as_str() {
        "claude" => ("claude", vec!["--print".to_string(), "echo hello from mock agent".to_string()]),
        "human" => {
            return Err(ActivityError::NonRetryable(anyhow::anyhow!(
                "human runtime cannot be spawned"
            )));
        }
        other => {
            return Err(ActivityError::NonRetryable(anyhow::anyhow!(
                "unknown runtime: {other}"
            )));
        }
    };

    let work_dir = input.work_dir.unwrap_or_else(|| ".".to_string());

    let child = Command::new(cmd)
        .args(&args)
        .current_dir(&work_dir)
        .spawn()
        .map_err(|e| ActivityError::Retryable {
            source: anyhow::anyhow!("failed to spawn {cmd}: {e}"),
            explicit_delay: None,
        })?;

    let pid = child.id().unwrap_or(0);
    tracing::info!("Spawned agent {} (pid={}) runtime={}", input.agent_id, pid, input.runtime);

    Ok(SpawnAgentOutput {
        agent_id: input.agent_id,
        pid,
    })
}
