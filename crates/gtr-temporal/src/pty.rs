use std::collections::HashMap;
use std::path::{Path, PathBuf};

use nix::unistd::Pid;

/// Runtime directory for a single agent's PTY session.
/// Layout: ~/.gtr/runtime/<agent-id>/
///   - pid         Process ID file
///   - env.json    Env vars used at spawn
pub fn runtime_dir(agent_id: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".gtr").join("runtime").join(agent_id)
}

/// Derive the tmux session name for an agent.
pub fn tmux_session_name(agent_id: &str) -> String {
    format!("gtr-{agent_id}")
}

/// Verify tmux is installed and >= 3.2.
pub fn ensure_tmux() -> anyhow::Result<()> {
    let output = std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .map_err(|_| anyhow::anyhow!("tmux not found — install tmux >= 3.2"))?;

    if !output.status.success() {
        anyhow::bail!("tmux -V failed");
    }

    let version_str = String::from_utf8_lossy(&output.stdout);
    // Parse "tmux 3.6a" -> "3.6"
    let version_part = version_str
        .trim()
        .strip_prefix("tmux ")
        .unwrap_or(version_str.trim());
    // Extract major.minor (strip trailing letters like "a")
    let numeric: String = version_part
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let major_minor: f64 = numeric.parse().unwrap_or(0.0);
    if major_minor < 3.2 {
        anyhow::bail!("tmux >= 3.2 required (found {version_str})");
    }

    Ok(())
}

/// Ensure the GTR tmux config file exists at ~/.gtr/config/tmux.conf.
/// Returns the path to the config file.
pub fn ensure_tmux_config() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let config_dir = PathBuf::from(&home).join(".gtr").join("config");
    std::fs::create_dir_all(&config_dir)?;

    let config_path = config_dir.join("tmux.conf");
    if !config_path.exists() {
        std::fs::write(
            &config_path,
            "set -g status off\n\
             set -g mouse on\n\
             set -g history-limit 50000\n\
             set -g default-terminal \"xterm-256color\"\n\
             set -g prefix None\n\
             unbind-key C-b\n\
             bind-key -n C-\\\\ detach-client\n",
        )?;
    }

    Ok(config_path)
}

/// Check if an agent's tmux session is alive.
pub fn is_alive(agent_id: &str) -> bool {
    let session = tmux_session_name(agent_id);
    std::process::Command::new("tmux")
        .args(["-L", "gtr", "has-session", "-t", &session])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Read the PID of an agent's process (the pane command) from tmux.
pub fn read_pid(agent_id: &str) -> Option<Pid> {
    let session = tmux_session_name(agent_id);
    let output = std::process::Command::new("tmux")
        .args([
            "-L",
            "gtr",
            "list-panes",
            "-t",
            &session,
            "-F",
            "#{pane_pid}",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        // Fall back to PID file
        let pid_path = runtime_dir(agent_id).join("pid");
        let pid_str = std::fs::read_to_string(&pid_path).ok()?;
        let pid: i32 = pid_str.trim().parse().ok()?;
        return Some(Pid::from_raw(pid));
    }

    let pid_str = String::from_utf8_lossy(&output.stdout);
    let pid: i32 = pid_str.trim().parse().ok()?;
    Some(Pid::from_raw(pid))
}

/// Capture the last N lines of an agent's tmux pane output.
pub fn capture_pane(agent_id: &str, lines: u32) -> Option<String> {
    let session = tmux_session_name(agent_id);
    let output = std::process::Command::new("tmux")
        .args([
            "-L",
            "gtr",
            "capture-pane",
            "-t",
            &session,
            "-p",
            "-S",
            &format!("-{lines}"),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Clean up runtime directory for an agent.
pub fn cleanup(agent_id: &str) -> std::io::Result<()> {
    let dir = runtime_dir(agent_id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// Spawn a subprocess inside a detached tmux session. Returns the child PID.
pub fn spawn(
    agent_id: &str,
    program: &str,
    args: &[String],
    work_dir: &Path,
    env_vars: &HashMap<String, String>,
) -> anyhow::Result<Pid> {
    ensure_tmux()?;
    let config_path = ensure_tmux_config()?;
    let session = tmux_session_name(agent_id);

    // Create runtime directory
    let dir = runtime_dir(agent_id);
    std::fs::create_dir_all(&dir)?;

    // Write env.json for debugging and respawn recovery.
    let mut env_save = env_vars.clone();
    env_save.insert(
        "__GTR_WORK_DIR".into(),
        work_dir.to_string_lossy().to_string(),
    );
    let env_json = serde_json::to_string_pretty(&env_save)?;
    std::fs::write(dir.join("env.json"), env_json)?;

    // Build the shell command string.
    // Unset CLAUDECODE to prevent Claude Code from refusing to start
    // (it detects nested sessions via this env var).
    let escaped_program = shell_escape::escape(program.into());
    let escaped_args: Vec<String> = args
        .iter()
        .map(|a| shell_escape::escape(a.into()).to_string())
        .collect();
    let inner_cmd = if escaped_args.is_empty() {
        escaped_program.to_string()
    } else {
        format!("{escaped_program} {}", escaped_args.join(" "))
    };
    let shell_cmd = format!("unset CLAUDECODE; {inner_cmd}");

    // Build tmux new-session command
    let mut cmd = std::process::Command::new("tmux");
    cmd.args([
        "-L",
        "gtr",
        "-f",
        config_path.to_str().unwrap_or(""),
        "new-session",
        "-d",
        "-s",
        &session,
        "-c",
        work_dir.to_str().unwrap_or("."),
        "-x",
        "200",
        "-y",
        "50",
    ]);

    // Add environment variables via -e flags (tmux >= 3.2)
    for (k, v) in env_vars {
        cmd.arg("-e");
        cmd.arg(format!("{k}={v}"));
    }

    // The shell command to run
    cmd.arg(shell_cmd);

    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux new-session failed: {stderr}");
    }

    // Get PID via tmux list-panes
    let pid_output = std::process::Command::new("tmux")
        .args([
            "-L",
            "gtr",
            "list-panes",
            "-t",
            &session,
            "-F",
            "#{pane_pid}",
        ])
        .output()?;

    let pid_str = String::from_utf8_lossy(&pid_output.stdout);
    let pid: i32 = pid_str
        .trim()
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse pane PID '{pid_str}': {e}"))?;

    // Write PID file for backward compat
    std::fs::write(dir.join("pid"), pid.to_string())?;

    tracing::info!(
        "Spawned agent '{agent_id}' in tmux session '{session}' (PID {pid})"
    );

    Ok(Pid::from_raw(pid))
}

/// Spawn a process in a tmux session.
/// This is the main entry point for launching an agent.
/// (Thin wrapper around spawn — no more server thread or reaper thread needed.)
pub fn spawn_with_server(
    agent_id: &str,
    program: &str,
    args: &[String],
    work_dir: &Path,
    env_vars: &HashMap<String, String>,
) -> anyhow::Result<Pid> {
    spawn(agent_id, program, args, work_dir, env_vars)
}

/// Kill an agent's tmux session and all processes in its process group.
pub fn kill_agent(agent_id: &str) -> anyhow::Result<bool> {
    let session = tmux_session_name(agent_id);

    // Get the pane PID before killing the session
    let pane_pid = read_pid(agent_id);

    // Kill the tmux session (sends SIGHUP to foreground process)
    let kill_output = std::process::Command::new("tmux")
        .args(["-L", "gtr", "kill-session", "-t", &session])
        .output();

    let session_existed = kill_output
        .as_ref()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Kill the process group to catch subagents spawned by Claude Code
    if let Some(pid) = pane_pid {
        // Try to kill the entire process group
        let pgid = nix::unistd::getpgid(Some(pid)).ok();
        if let Some(pgid) = pgid {
            // Kill process group with SIGTERM
            nix::sys::signal::killpg(pgid, nix::sys::signal::Signal::SIGTERM).ok();
            std::thread::sleep(std::time::Duration::from_millis(500));
            // Force kill if still around
            nix::sys::signal::killpg(pgid, nix::sys::signal::Signal::SIGKILL).ok();
        } else {
            // Fall back to killing the individual PID
            nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM).ok();
            std::thread::sleep(std::time::Duration::from_millis(500));
            nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL).ok();
        }
    }

    cleanup(agent_id)?;
    Ok(session_existed || pane_pid.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_dir_structure() {
        let dir = runtime_dir("mayor");
        assert!(dir.ends_with(".gtr/runtime/mayor"));
    }

    #[test]
    fn tmux_session_name_format() {
        assert_eq!(tmux_session_name("mayor"), "gtr-mayor");
        assert_eq!(
            tmux_session_name("cfb-stats-polecat-nux"),
            "gtr-cfb-stats-polecat-nux"
        );
    }

    #[test]
    fn is_alive_returns_false_for_nonexistent() {
        assert!(!is_alive("nonexistent-agent-xyz"));
    }

    #[test]
    fn capture_pane_returns_none_for_nonexistent() {
        assert!(capture_pane("nonexistent-agent-xyz", 100).is_none());
    }

    #[test]
    fn spawn_and_kill_echo() {
        // Skip if tmux not installed
        if std::process::Command::new("tmux")
            .arg("-V")
            .output()
            .is_err()
        {
            eprintln!("Skipping test -- tmux not installed");
            return;
        }

        let agent_id = "test-spawn-echo";
        cleanup(agent_id).ok();
        // Kill any leftover session
        let session = tmux_session_name(agent_id);
        let _ = std::process::Command::new("tmux")
            .args(["-L", "gtr", "kill-session", "-t", &session])
            .output();

        let mut env = HashMap::new();
        env.insert("TEST_VAR".into(), "hello".into());

        let result = spawn(
            agent_id,
            "/bin/sh",
            &["-c".into(), "sleep 30".into()],
            Path::new("/tmp"),
            &env,
        );
        assert!(result.is_ok(), "spawn failed: {:?}", result.err());
        let pid = result.unwrap();

        // Verify PID file written
        assert!(runtime_dir(agent_id).join("pid").exists());

        // Verify tmux session exists
        assert!(is_alive(agent_id));

        // Verify read_pid works
        let read = read_pid(agent_id);
        assert!(read.is_some());
        assert_eq!(read.unwrap(), pid);

        // Kill it
        let killed = kill_agent(agent_id);
        assert!(killed.is_ok());
        assert!(killed.unwrap());

        // Verify session is gone
        assert!(!is_alive(agent_id));
    }
}
