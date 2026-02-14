use std::collections::HashMap;
use std::ffi::CString;
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

/// Spawn a subprocess with a PTY. Returns the child PID.
/// The PTY master fd is kept open in the current process.
/// A Unix socket server is NOT started here — that's Task 53.
pub fn spawn(
    agent_id: &str,
    program: &str,
    args: &[String],
    work_dir: &Path,
    env_vars: &HashMap<String, String>,
) -> anyhow::Result<(Pid, OwnedFd)> {
    // Create runtime directory
    let dir = runtime_dir(agent_id);
    std::fs::create_dir_all(&dir)?;

    // Create PTY
    let winsize = Winsize {
        ws_row: 50,
        ws_col: 200,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pty = openpty(Some(&winsize), None)?;

    // Fork
    match unsafe { fork() }? {
        ForkResult::Parent { child } => {
            // Close slave in parent
            drop(pty.slave);

            // Write PID file
            std::fs::write(dir.join("pid"), child.to_string())?;

            // Write env.json for debugging
            let env_json = serde_json::to_string_pretty(&env_vars)?;
            std::fs::write(dir.join("env.json"), env_json)?;

            Ok((child, pty.master))
        }
        ForkResult::Child => {
            // Close master in child
            drop(pty.master);

            // Create new session (detach from parent terminal)
            setsid()?;

            // Set slave as controlling terminal
            let slave_fd = pty.slave.as_raw_fd();
            unsafe {
                nix::libc::ioctl(slave_fd, nix::libc::TIOCSCTTY as _, 0);
            }

            // Redirect stdio to PTY slave
            dup2(slave_fd, 0)?;
            dup2(slave_fd, 1)?;
            dup2(slave_fd, 2)?;
            if slave_fd > 2 {
                drop(pty.slave);
            }

            // Set working directory
            std::env::set_current_dir(work_dir)?;

            // Set environment variables
            for (k, v) in env_vars {
                std::env::set_var(k, v);
            }

            // Exec
            let c_program = CString::new(program)?;
            let c_args: Vec<CString> = std::iter::once(CString::new(program)?)
                .chain(args.iter().map(|a| CString::new(a.as_str()).unwrap()))
                .collect();
            nix::unistd::execvp(&c_program, &c_args)?;

            unreachable!()
        }
    }
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

    #[test]
    fn spawn_and_kill_echo() {
        let agent_id = "test-spawn-echo";
        cleanup(agent_id).ok();

        let mut env = HashMap::new();
        env.insert("TEST_VAR".into(), "hello".into());

        let result = spawn(
            agent_id,
            "/bin/sh",
            &["-c".into(), "sleep 30".into()],
            Path::new("/tmp"),
            &env,
        );
        assert!(result.is_ok());
        let (pid, _master_fd) = result.unwrap();

        // Verify PID file written
        assert!(runtime_dir(agent_id).join("pid").exists());
        assert!(is_alive(agent_id));

        // Kill it
        nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM).ok();
        std::thread::sleep(std::time::Duration::from_millis(100));

        cleanup(agent_id).ok();
    }
}
