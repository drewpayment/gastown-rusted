use std::collections::HashMap;
use std::ffi::CString;
use std::path::PathBuf;

use clap::Args;

#[derive(Debug, Args)]
#[command(about = "Attach to a live agent tmux session (interactive Claude Code)")]
pub struct AttachCommand {
    /// Agent ID to attach to (e.g., "mayor", "gtr-polecat-furiosa")
    pub agent: String,
}

/// Respawn an agent's tmux session. Reads env.json from the old runtime dir,
/// cleans up, then spawns a fresh process.
fn respawn_agent(agent_id: &str) -> anyhow::Result<()> {
    let runtime_dir = gtr_temporal::pty::runtime_dir(agent_id);

    // Read env vars from previous spawn (if available).
    let env_path = runtime_dir.join("env.json");
    let mut env: HashMap<String, String> = if env_path.exists() {
        let data = std::fs::read_to_string(&env_path)?;
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Determine work_dir from saved env or fall back to ~/.gtr
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let work_dir = env
        .remove("__GTR_WORK_DIR")
        .unwrap_or_else(|| format!("{home}/.gtr"));

    // Ensure GTR_AGENT is set
    env.entry("GTR_AGENT".into())
        .or_insert_with(|| agent_id.to_string());

    // Ensure rgt binary is on PATH and RGT_BIN is set
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let existing_path = env
                .get("PATH")
                .cloned()
                .or_else(|| std::env::var("PATH").ok())
                .unwrap_or_default();
            env.insert(
                "PATH".into(),
                format!("{}:{existing_path}", exe_dir.display()),
            );
        }
        env.insert("RGT_BIN".into(), current_exe.to_string_lossy().to_string());
    }

    let prompt = "You are being reattached after your previous session ended. \
                  Use $RGT_BIN instead of rgt (env var has the full path). \
                  Run `$RGT_BIN prime` to restore context, then `$RGT_BIN hook` and `$RGT_BIN mail inbox`.";

    // Clean up stale runtime dir
    gtr_temporal::pty::cleanup(agent_id)?;

    // Spawn new tmux session
    gtr_temporal::pty::spawn_with_server(
        agent_id,
        "claude",
        &["--dangerously-skip-permissions".into(), "--disable-slash-commands".into(), "--disallowedTools=Skill,AskUserQuestion,EnterPlanMode".into(), prompt.into()],
        &PathBuf::from(&work_dir),
        &env,
    )?;

    Ok(())
}

pub async fn run(cmd: &AttachCommand) -> anyhow::Result<()> {
    let agent_id = &cmd.agent;

    // Check if agent session is running
    if !gtr_temporal::pty::is_alive(agent_id) {
        // Check if there's a stale runtime dir with env.json we can respawn from
        let env_path = gtr_temporal::pty::runtime_dir(agent_id).join("env.json");
        if env_path.exists() {
            println!("Agent '{agent_id}' session ended. Respawning...");
            respawn_agent(agent_id)?;
            // Give the new process a moment to initialize
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        } else {
            anyhow::bail!(
                "Agent '{agent_id}' has no session. Start it with `rgt up` or the boot workflow."
            );
        }
    }

    let session = gtr_temporal::pty::tmux_session_name(agent_id);
    println!("Attaching to '{agent_id}' (tmux session '{session}'). Ctrl+\\ to detach.\n");

    // exec into tmux attach-session — this replaces the current process
    let tmux = CString::new("tmux")?;
    let args = [
        CString::new("tmux")?,
        CString::new("-L")?,
        CString::new("gtr")?,
        CString::new("attach-session")?,
        CString::new("-t")?,
        CString::new(session.as_str())?,
    ];
    nix::unistd::execvp(&tmux, &args)?;

    unreachable!("execvp replaces the process")
}
