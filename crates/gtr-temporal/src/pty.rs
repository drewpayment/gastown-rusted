use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};

use nix::pty::{openpty, Winsize};
use nix::unistd::{close, dup2, execvp, fork, setsid, ForkResult, Pid};

/// Runtime directory for a single agent's PTY session.
/// Layout: ~/.gtr/runtime/<agent-id>/
///   - pty.sock    Unix domain socket for attach
///   - pid         Process ID file
///   - env.json    Env vars used at spawn
pub fn runtime_dir(agent_id: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".gtr").join("runtime").join(agent_id)
}

/// Check if an agent's PTY server process is alive.
pub fn is_alive(agent_id: &str) -> bool {
    let pid_path = runtime_dir(agent_id).join("pid");
    if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            // Signal 0 checks if process exists without sending a signal
            return nix::sys::signal::kill(Pid::from_raw(pid), None).is_ok();
        }
    }
    false
}

/// Read the PID of an agent's PTY server process.
pub fn read_pid(agent_id: &str) -> Option<Pid> {
    let pid_path = runtime_dir(agent_id).join("pid");
    let pid_str = std::fs::read_to_string(&pid_path).ok()?;
    let pid: i32 = pid_str.trim().parse().ok()?;
    Some(Pid::from_raw(pid))
}

/// Get the Unix socket path for an agent.
pub fn socket_path(agent_id: &str) -> PathBuf {
    runtime_dir(agent_id).join("pty.sock")
}

/// Clean up runtime directory for an agent.
pub fn cleanup(agent_id: &str) -> std::io::Result<()> {
    let dir = runtime_dir(agent_id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
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
    fn is_alive_returns_false_for_nonexistent() {
        assert!(!is_alive("nonexistent-agent-xyz"));
    }
}
