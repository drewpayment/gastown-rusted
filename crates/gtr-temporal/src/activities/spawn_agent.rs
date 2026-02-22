use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use temporalio_sdk::{ActContext, ActivityError};

use crate::pty;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentInput {
    pub agent_id: String,
    pub runtime: String,    // "claude" or "shell"
    pub work_dir: String,
    pub role: String,
    pub rig: Option<String>,
    pub initial_prompt: Option<String>,
    pub env_extra: Option<HashMap<String, String>>,
    #[serde(default)]
    pub resume_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentOutput {
    pub agent_id: String,
    pub pid: u32,
    pub tmux_session: String,
}

pub async fn spawn_agent(
    _ctx: ActContext,
    input: SpawnAgentInput,
) -> Result<SpawnAgentOutput, ActivityError> {
    // Check if already running
    if pty::is_alive(&input.agent_id) {
        return Err(ActivityError::NonRetryable(anyhow::anyhow!(
            "Agent '{}' is already running",
            input.agent_id
        )));
    }

    // Clean up any stale runtime dir
    pty::cleanup(&input.agent_id).ok();

    // Build environment variables
    let mut env = HashMap::new();
    env.insert("GTR_AGENT".into(), input.agent_id.clone());
    env.insert("GTR_ROLE".into(), input.role.clone());
    if let Some(rig) = &input.rig {
        env.insert("GTR_RIG".into(), rig.clone());
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    env.insert("GTR_ROOT".into(), format!("{home}/.gtr"));

    // Ensure our rgt binary is on PATH for the spawned agent
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let existing_path = std::env::var("PATH").unwrap_or_default();
            env.insert("PATH".into(), format!("{}:{existing_path}", exe_dir.display()));
        }
        // Set RGT_BIN so agents can always find the exact binary path
        env.insert("RGT_BIN".into(), current_exe.to_string_lossy().to_string());
    }

    if let Some(extra) = &input.env_extra {
        env.extend(extra.clone());
    }

    // Determine program and args based on runtime
    let (program, args) = match input.runtime.as_str() {
        "claude" => {
            let mut args = vec![
                "--dangerously-skip-permissions".to_string(),
                "--disable-slash-commands".to_string(),
                "--disallowedTools=Skill,AskUserQuestion,EnterPlanMode".to_string(),
            ];
            if let Some(session_id) = &input.resume_session_id {
                args.push("--resume".to_string());
                args.push(session_id.clone());
            }
            if let Some(prompt) = &input.initial_prompt {
                args.push(prompt.clone());
            }
            ("claude".to_string(), args)
        }
        "shell" => {
            let args = if let Some(prompt) = &input.initial_prompt {
                vec!["-c".to_string(), prompt.clone()]
            } else {
                vec![]
            };
            ("sh".to_string(), args)
        }
        other => {
            return Err(ActivityError::NonRetryable(anyhow::anyhow!(
                "Unknown runtime: '{other}'. Supported: claude, shell"
            )));
        }
    };

    // Ensure work directory exists
    let work_dir = PathBuf::from(&input.work_dir);
    std::fs::create_dir_all(&work_dir).map_err(|e| {
        ActivityError::NonRetryable(anyhow::anyhow!("Failed to create work dir: {e}"))
    })?;

    // Spawn with PTY and socket server
    let pid = pty::spawn_with_server(
        &input.agent_id,
        &program,
        &args,
        &work_dir,
        &env,
    )
    .map_err(|e| {
        ActivityError::NonRetryable(anyhow::anyhow!("Failed to spawn agent: {e}"))
    })?;

    let tmux_session = pty::tmux_session_name(&input.agent_id);

    tracing::info!(
        "Spawned agent '{}' (PID {}, runtime {}, session {})",
        input.agent_id,
        pid,
        input.runtime,
        tmux_session
    );

    Ok(SpawnAgentOutput {
        agent_id: input.agent_id,
        pid: pid.as_raw() as u32,
        tmux_session,
    })
}
