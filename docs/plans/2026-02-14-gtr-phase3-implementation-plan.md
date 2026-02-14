# GTR Phase 3 Implementation Plan — Make It Real

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the original Gas Town's tmux/beads runtime with a native Rust daemon using PTY process management. Agents actually spawn, work actually gets done, and `gtr attach` lets you interact with live Claude Code sessions.

**Architecture:** The Temporal worker IS the daemon (`gtr up` starts it). Agent subprocesses are spawned via PTY with Unix domain sockets for detach/reattach. All state lives in Temporal workflows — no SQLite, no JSONL. `gtr attach <agent>` reconnects to a live Claude Code session's PTY.

**Tech Stack:** Rust 2021, Temporal SDK rev `7ecb7c0`, `nix` crate (PTY + Unix sockets + signals), `tokio`, `crossterm` (raw terminal mode)

**Phase 1 reference:** `docs/plans/2026-02-12-gtr-implementation-plan.md` (Tasks 1-29, complete)
**Phase 2 reference:** `docs/plans/2026-02-14-gtr-phase2-implementation-plan.md` (Tasks 30-50, complete)
**Design doc:** `docs/plans/2026-02-14-gtr-phase3-design.md`

**Codebase root:** `/Users/drew.payment/dev/gt/gtr/crew/drew`

---

## Priority Order

1. **PTY Foundation** (Tasks 51-53): PTY spawn, Unix socket server, attach client
2. **Real Agent Spawning** (Tasks 54-56): Replace mock spawn_agent, agent heartbeat, runtime dir
3. **Boot Sequence** (Tasks 57-59): Real `gtr up` flow, mayor spawns agents, rig bootstrap
4. **Sling-to-Done Loop** (Tasks 60-63): Real sling dispatch, polecat lifecycle, `gtr done` wiring, refinery merge
5. **Attach & Chat** (Tasks 64-66): `gtr attach`, `gtr chat`, detach handling
6. **Prime & Handoff** (Tasks 67-68): Real SessionStart hook, context injection
7. **Crash Recovery** (Tasks 69-70): Heartbeat-based detection, auto-respawn
8. **Witness & Patrol** (Tasks 71-72): Real monitoring, real plugin execution
9. **Notifications** (Tasks 73): Real webhook dispatch
10. **Integration Tests** (Tasks 74-76): Mock agent script, end-to-end sling-to-done, multi-rig test
11. **Polish** (Tasks 77-80): `gtr down` graceful shutdown, `gtr status` rich output, install command, config validation

---

## Task 51: Add `nix` Dependency and PTY Module Scaffold

**Files:**
- Modify: `crates/gtr-temporal/Cargo.toml`
- Create: `crates/gtr-temporal/src/pty.rs`
- Modify: `crates/gtr-temporal/src/lib.rs`

**Step 1: Add nix dependency**

Add to `crates/gtr-temporal/Cargo.toml` under `[dependencies]`:

```toml
nix = { version = "0.29", features = ["term", "process", "socket", "signal", "user", "fs"] }
```

**Step 2: Create PTY module scaffold**

```rust
// crates/gtr-temporal/src/pty.rs
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
```

**Step 3: Add module to lib.rs**

Add `pub mod pty;` to `crates/gtr-temporal/src/lib.rs`.

**Step 4: Verify and commit**

Run: `cargo build && cargo test`
Expected: All tests pass, including 2 new PTY tests.

```bash
git commit -m "feat: PTY module scaffold — runtime directory, is_alive, socket paths"
```

---

## Task 52: PTY Spawn — Fork/Exec with Pseudo-Terminal

**Files:**
- Modify: `crates/gtr-temporal/src/pty.rs`

**Step 1: Implement `spawn` function**

Add to `pty.rs`:

```rust
use std::collections::HashMap;
use std::ffi::CString;

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
```

**Step 2: Add test**

```rust
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
```

**Step 3: Verify and commit**

Run: `cargo test`

```bash
git commit -m "feat: PTY spawn — fork/exec subprocess with pseudo-terminal"
```

---

## Task 53: Unix Socket Server for PTY Attach/Detach

**Files:**
- Modify: `crates/gtr-temporal/src/pty.rs`

**Step 1: Implement socket server and client**

Add to `pty.rs`:

```rust
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};

use nix::sys::socket::{
    recvmsg, sendmsg, ControlMessage, ControlMessageOwned, MsgFlags,
};

/// Start a Unix socket server that passes the PTY master fd to connecting clients.
/// This function blocks — run it in a dedicated thread.
pub fn serve_pty(agent_id: &str, master_fd: RawFd) -> anyhow::Result<()> {
    let sock_path = socket_path(agent_id);
    // Remove stale socket if it exists
    if sock_path.exists() {
        std::fs::remove_file(&sock_path)?;
    }

    let listener = UnixListener::bind(&sock_path)?;
    // Set socket permissions to owner-only
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600))?;
    }

    tracing::info!("PTY server listening on {}", sock_path.display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = send_fd(&stream, master_fd) {
                    tracing::warn!("Failed to send PTY fd to client: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("Accept failed: {e}");
                break;
            }
        }
    }

    Ok(())
}

/// Send a file descriptor over a Unix socket using SCM_RIGHTS.
fn send_fd(stream: &UnixStream, fd: RawFd) -> nix::Result<()> {
    let fds = [fd];
    let cmsg = [ControlMessage::ScmRights(&fds)];
    let iov = [std::io::IoSlice::new(b"P")]; // "P" for PTY

    sendmsg::<()>(stream.as_raw_fd(), &iov, &cmsg, MsgFlags::empty(), None)?;
    Ok(())
}

/// Receive a file descriptor from a Unix socket using SCM_RIGHTS.
pub fn recv_fd(stream: &UnixStream) -> nix::Result<RawFd> {
    let mut buf = [0u8; 1];
    let mut iov = [std::io::IoSliceMut::new(&mut buf)];
    let mut cmsgspace = nix::cmsg_space!([RawFd; 1]);

    let msg = recvmsg::<()>(
        stream.as_raw_fd(),
        &mut iov,
        Some(&mut cmsgspace),
        MsgFlags::empty(),
    )?;

    for cmsg in msg.cmsgs()? {
        if let ControlMessageOwned::ScmRights(fds) = cmsg {
            if let Some(&fd) = fds.first() {
                return Ok(fd);
            }
        }
    }

    Err(nix::Error::EINVAL)
}

/// Connect to an agent's PTY socket and receive the master fd.
pub fn connect_pty(agent_id: &str) -> anyhow::Result<RawFd> {
    let sock_path = socket_path(agent_id);
    if !sock_path.exists() {
        anyhow::bail!("No PTY session for agent '{agent_id}' — is it running?");
    }
    let stream = UnixStream::connect(&sock_path)?;
    let fd = recv_fd(&stream)?;
    Ok(fd)
}

/// Spawn a process with PTY and start the socket server in a background thread.
/// This is the main entry point for launching an agent.
pub fn spawn_with_server(
    agent_id: &str,
    program: &str,
    args: &[String],
    work_dir: &Path,
    env_vars: &HashMap<String, String>,
) -> anyhow::Result<Pid> {
    let (pid, master_fd) = spawn(agent_id, program, args, work_dir, env_vars)?;
    let master_raw = master_fd.as_raw_fd();

    // Leak the OwnedFd so it stays open for the lifetime of the server thread.
    // The fd will be closed when the process exits.
    std::mem::forget(master_fd);

    let agent_id_owned = agent_id.to_string();
    std::thread::spawn(move || {
        if let Err(e) = serve_pty(&agent_id_owned, master_raw) {
            tracing::error!("PTY server for '{}' exited: {e}", agent_id_owned);
        }
    });

    Ok(pid)
}
```

**Step 2: Add serde dependency if not present in gtr-temporal Cargo.toml**

The `pty.rs` module uses `serde_json` for env.json — verify `serde_json` is already in gtr-temporal's Cargo.toml (it should be from Phase 1).

**Step 3: Verify and commit**

Run: `cargo build && cargo test`

```bash
git commit -m "feat: PTY Unix socket server — fd passing for attach/detach"
```

---

## Task 54: Replace Mock `spawn_agent` Activity

**Files:**
- Modify: `crates/gtr-temporal/src/activities/spawn_agent.rs`

**Step 1: Rewrite spawn_agent to use PTY**

Replace the entire file:

```rust
// crates/gtr-temporal/src/activities/spawn_agent.rs
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use temporalio_sdk::ActContext;
use temporalio_sdk::ActivityError;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentOutput {
    pub agent_id: String,
    pub pid: u32,
    pub socket_path: String,
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
    if let Some(extra) = &input.env_extra {
        env.extend(extra.clone());
    }

    // Determine program and args based on runtime
    let (program, args) = match input.runtime.as_str() {
        "claude" => {
            let mut args = vec!["--dangerously-skip-permissions".to_string()];
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

    let socket_path = pty::socket_path(&input.agent_id)
        .to_string_lossy()
        .to_string();

    tracing::info!(
        "Spawned agent '{}' (PID {}, runtime {})",
        input.agent_id,
        pid,
        input.runtime
    );

    Ok(SpawnAgentOutput {
        agent_id: input.agent_id,
        pid: pid.as_raw() as u32,
        socket_path,
    })
}
```

**Step 2: Verify and commit**

Run: `cargo build && cargo test`

```bash
git commit -m "feat: real spawn_agent — PTY subprocess with Unix socket server"
```

---

## Task 55: Agent Heartbeat Activity

**Files:**
- Create: `crates/gtr-temporal/src/activities/heartbeat.rs`
- Modify: `crates/gtr-temporal/src/activities/mod.rs`
- Modify: `crates/gtr-temporal/src/worker.rs`

**Step 1: Create heartbeat activity**

```rust
// crates/gtr-temporal/src/activities/heartbeat.rs
use serde::{Deserialize, Serialize};
use temporalio_sdk::ActContext;
use temporalio_sdk::ActivityError;

use crate::pty;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatInput {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatOutput {
    pub agent_id: String,
    pub alive: bool,
    pub pid: Option<u32>,
}

pub async fn check_agent_alive(
    _ctx: ActContext,
    input: HeartbeatInput,
) -> Result<HeartbeatOutput, ActivityError> {
    let alive = pty::is_alive(&input.agent_id);
    let pid = pty::read_pid(&input.agent_id).map(|p| p.as_raw() as u32);

    Ok(HeartbeatOutput {
        agent_id: input.agent_id,
        alive,
        pid,
    })
}
```

**Step 2: Add to mod.rs**

Add `pub mod heartbeat;` to `crates/gtr-temporal/src/activities/mod.rs`.

**Step 3: Register in worker.rs**

Add after the `send_notification` registration:

```rust
worker.register_activity("check_agent_alive", activities::heartbeat::check_agent_alive);
```

**Step 4: Verify and commit**

Run: `cargo build && cargo test`

```bash
git commit -m "feat: agent heartbeat activity — check if subprocess is alive"
```

---

## Task 56: Kill Agent Helper

**Files:**
- Modify: `crates/gtr-temporal/src/pty.rs`

**Step 1: Add kill function**

Add to `pty.rs`:

```rust
/// Kill an agent's subprocess and clean up its runtime directory.
pub fn kill_agent(agent_id: &str) -> anyhow::Result<bool> {
    if let Some(pid) = read_pid(agent_id) {
        // Send SIGTERM first
        nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM).ok();

        // Wait briefly for graceful shutdown
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Force kill if still alive
        if nix::sys::signal::kill(pid, None).is_ok() {
            nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL).ok();
        }

        cleanup(agent_id)?;
        Ok(true)
    } else {
        cleanup(agent_id)?;
        Ok(false)
    }
}
```

**Step 2: Verify and commit**

Run: `cargo build && cargo test`

```bash
git commit -m "feat: kill_agent — SIGTERM then SIGKILL with cleanup"
```

---

## Task 57: Real `gtr up` — Start Worker + Boot Sequence

**Files:**
- Modify: `crates/gtr-cli/src/commands/up.rs`
- Modify: `crates/gtr-temporal/src/workflows/boot.rs`

**Step 1: Rewrite `gtr up` to start worker in foreground**

The current `gtr up` just starts the mayor workflow. The new version starts the Temporal worker (which IS the daemon), then the boot workflow handles agent spawning.

```rust
// crates/gtr-cli/src/commands/up.rs
use temporalio_sdk_core::WorkflowClientTrait;

pub async fn run() -> anyhow::Result<()> {
    println!("Starting Gas Town...");

    // Ensure runtime directory exists
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let runtime_dir = format!("{home}/.gtr/runtime");
    std::fs::create_dir_all(&runtime_dir)?;

    // Check if mayor is already running
    let client = crate::client::connect().await?;
    let resp = client
        .describe_workflow_execution("mayor".to_string(), None)
        .await;
    if let Ok(r) = &resp {
        if let Some(info) = &r.workflow_execution_info {
            if info.status == 1 {
                println!("Gas Town is already running (mayor workflow active).");
                println!("Run `gtr worker run` to start the worker if needed.");
                return Ok(());
            }
        }
    }

    // Start mayor workflow
    let payload = temporalio_common::protos::coresdk::AsJsonPayloadExt::as_json_payload(&"default")?;
    client
        .start_workflow(
            vec![payload],
            "work".to_string(),
            "mayor".to_string(),
            "mayor_wf".to_string(),
            None,
            Default::default(),
        )
        .await?;

    // Start boot workflow
    let boot_payload = temporalio_common::protos::coresdk::AsJsonPayloadExt::as_json_payload(&120u64)?;
    let _ = client
        .start_workflow(
            vec![boot_payload],
            "work".to_string(),
            "boot".to_string(),
            "boot_wf".to_string(),
            None,
            Default::default(),
        )
        .await;

    println!("Gas Town is up.");
    println!("  Mayor workflow: running");
    println!("  Boot workflow: running");
    println!();
    println!("Now start the worker: gtr worker run");

    Ok(())
}
```

**Step 2: Rewrite boot workflow to spawn agents**

```rust
// crates/gtr-temporal/src/workflows/boot.rs
use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::spawn_agent::SpawnAgentInput;
use crate::signals::SIGNAL_AGENT_STOP;

pub async fn boot_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let interval_secs = if let Some(payload) = args.first() {
        serde_json::from_slice::<u64>(&payload.data).unwrap_or(120)
    } else {
        120
    };

    let mut stop_ch = ctx.make_signal_channel(SIGNAL_AGENT_STOP);
    let mut checks: u64 = 0;
    let mut spawned: Vec<String> = vec![];

    tracing::info!("Boot started — health check interval {interval_secs}s");

    // Initial spawn: mayor agent
    let mayor_input = SpawnAgentInput {
        agent_id: "mayor".to_string(),
        runtime: "claude".to_string(),
        work_dir: std::env::var("HOME").unwrap_or("/tmp".into()) + "/.gtr",
        role: "mayor".to_string(),
        rig: None,
        initial_prompt: Some(
            "You are the Mayor of Gas Town. Check your hook and mail, then act accordingly:\n\
             1. `gtr hook` - shows hooked work (if any)\n\
             2. `gtr mail inbox` - check for messages\n\
             3. If work is hooked -> execute it immediately\n\
             4. If nothing hooked -> wait for instructions".to_string()
        ),
        env_extra: None,
    };

    let result = ctx
        .activity(ActivityOptions {
            activity_type: "spawn_agent".to_string(),
            input: mayor_input.as_json_payload()?,
            start_to_close_timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        })
        .await;

    if result.completed_ok() {
        spawned.push("mayor".to_string());
        tracing::info!("Boot: spawned mayor agent");
    } else {
        tracing::warn!("Boot: failed to spawn mayor agent");
    }

    // Health check loop
    loop {
        tokio::select! {
            biased;
            Some(_) = stop_ch.next() => {
                tracing::info!("Boot stopped after {checks} checks");
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&serde_json::json!({
                        "checks": checks,
                        "spawned": spawned,
                    }))?
                ));
            }
            _ = ctx.timer(Duration::from_secs(interval_secs)) => {
                checks += 1;
                tracing::info!("Boot health check #{checks}");

                // Check all spawned agents are alive
                for agent_id in &spawned {
                    let input = crate::activities::heartbeat::HeartbeatInput {
                        agent_id: agent_id.clone(),
                    };

                    let result = ctx
                        .activity(ActivityOptions {
                            activity_type: "check_agent_alive".to_string(),
                            input: input.as_json_payload()?,
                            start_to_close_timeout: Some(Duration::from_secs(10)),
                            ..Default::default()
                        })
                        .await;

                    if result.completed_ok() {
                        // Parse result to check alive status
                        // If not alive, could respawn here in a full implementation
                        tracing::debug!("Boot: {agent_id} health check passed");
                    } else {
                        tracing::warn!("Boot: {agent_id} health check failed");
                    }
                }
            }
        }
    }
}
```

**Step 3: Verify and commit**

Run: `cargo build && cargo test`

```bash
git commit -m "feat: real gtr up — starts mayor+boot workflows, boot spawns mayor agent"
```

---

## Task 58: Rig Bootstrap — Spawn Witness and Refinery per Rig

**Files:**
- Modify: `crates/gtr-temporal/src/workflows/rig.rs`

**Step 1: Add agent spawning to rig workflow**

When a rig workflow starts (or receives a "boot" signal), it should spawn witness and refinery agents for that rig via `spawn_agent` activities.

Read the current `rig.rs` and add spawn logic to the boot signal handler. The witness gets work_dir `~/.gtr/rigs/<rig>/witness/` and the refinery gets `~/.gtr/rigs/<rig>/refinery/`. Both use runtime "claude" with role-specific prompts.

**Step 2: Verify and commit**

```bash
git commit -m "feat: rig bootstrap — spawn witness + refinery agents per rig"
```

---

## Task 59: Rig Directory Setup

**Files:**
- Modify: `crates/gtr-core/src/state.rs`
- Create: `crates/gtr-core/src/dirs.rs`

**Step 1: Create directory layout module**

```rust
// crates/gtr-core/src/dirs.rs
use std::path::PathBuf;

/// Root GTR directory (~/.gtr)
pub fn gtr_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".gtr")
}

/// Runtime directory for live process state
pub fn runtime_dir() -> PathBuf {
    gtr_root().join("runtime")
}

/// Rigs directory
pub fn rigs_dir() -> PathBuf {
    gtr_root().join("rigs")
}

/// A specific rig's directory
pub fn rig_dir(rig: &str) -> PathBuf {
    rigs_dir().join(rig)
}

/// Polecat work directory within a rig
pub fn polecat_dir(rig: &str, name: &str) -> PathBuf {
    rig_dir(rig).join("polecats").join(name)
}

/// Crew workspace directory within a rig
pub fn crew_dir(rig: &str, name: &str) -> PathBuf {
    rig_dir(rig).join("crew").join(name)
}

/// Witness work directory within a rig
pub fn witness_dir(rig: &str) -> PathBuf {
    rig_dir(rig).join("witness")
}

/// Refinery work directory within a rig
pub fn refinery_dir(rig: &str) -> PathBuf {
    rig_dir(rig).join("refinery")
}

/// Config directory
pub fn config_dir() -> PathBuf {
    gtr_root().join("config")
}

/// Ensure all directories for a rig exist
pub fn ensure_rig_dirs(rig: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(rig_dir(rig).join("polecats"))?;
    std::fs::create_dir_all(rig_dir(rig).join("crew"))?;
    std::fs::create_dir_all(witness_dir(rig))?;
    std::fs::create_dir_all(refinery_dir(rig))?;
    Ok(())
}

/// Ensure the base GTR directory structure exists
pub fn ensure_base_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(runtime_dir())?;
    std::fs::create_dir_all(rigs_dir())?;
    std::fs::create_dir_all(config_dir())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polecat_dir_structure() {
        let dir = polecat_dir("gtr", "furiosa");
        assert!(dir.to_string_lossy().contains(".gtr/rigs/gtr/polecats/furiosa"));
    }

    #[test]
    fn crew_dir_structure() {
        let dir = crew_dir("gtr", "drew");
        assert!(dir.to_string_lossy().contains(".gtr/rigs/gtr/crew/drew"));
    }
}
```

**Step 2: Add to lib.rs, verify and commit**

Add `pub mod dirs;` to `crates/gtr-core/src/lib.rs`.

Run: `cargo build && cargo test`

```bash
git commit -m "feat: directory layout module — ~/.gtr/rigs, polecats, crew, witness, refinery"
```

---

## Task 60: Real Sling Dispatch — Polecat Spawning

**Files:**
- Modify: `crates/gtr-cli/src/commands/sling.rs`

**Step 1: Wire sling to start polecat workflow that spawns a real agent**

Update the `--target <rig>` path in sling to:
1. Generate polecat name via `gtr_core::namepool::next_name()`
2. Start `polecat_wf` workflow with the rig, work item, and polecat name
3. The polecat workflow (Task 61) handles the actual spawning

The current sling already does most of this — the key change is that `polecat_wf` will now call `spawn_agent` with `runtime: "claude"` instead of just tracking state.

**Step 2: Verify and commit**

```bash
git commit -m "feat: sling dispatch wires polecat workflow for real agent spawning"
```

---

## Task 61: Polecat Workflow — Real Lifecycle with Spawn + Heartbeat

**Files:**
- Modify: `crates/gtr-temporal/src/workflows/polecat.rs`

**Step 1: Rewrite polecat workflow with real agent lifecycle**

The current polecat workflow creates a git worktree and waits for signals. The new version:
1. Creates git worktree (already does this)
2. Spawns Claude Code agent via `spawn_agent` activity
3. Enters heartbeat loop (every 60s, calls `check_agent_alive`)
4. On `done` signal: kills agent, removes worktree, exits
5. On `stuck` signal: marks stuck
6. On `kill` signal: force kills agent, cleans up
7. On heartbeat failure (agent died): marks stuck, notifies witness

```rust
// Key change in polecat_wf: after worktree creation, spawn the agent
let spawn_input = SpawnAgentInput {
    agent_id: polecat_id.clone(),
    runtime: "claude".to_string(),
    work_dir: worktree_path.clone(),
    role: format!("{rig}/polecats/{name}"),
    rig: Some(rig.clone()),
    initial_prompt: Some(format!(
        "You are polecat '{name}' on rig '{rig}'. Your work item: {work_item_id}.\n\
         Work in this directory. When done, run: gtr done {work_item_id} --branch {branch}"
    )),
    env_extra: None,
};

let spawn_result = ctx.activity(ActivityOptions {
    activity_type: "spawn_agent".to_string(),
    input: spawn_input.as_json_payload()?,
    start_to_close_timeout: Some(Duration::from_secs(30)),
    ..Default::default()
}).await;
```

Then the heartbeat loop replaces the simple timer:

```rust
// Heartbeat loop — check every 60s if agent is alive
loop {
    tokio::select! {
        biased;
        Some(_) = done_ch.next() => { /* kill agent, cleanup worktree, exit */ }
        Some(_) = kill_ch.next() => { /* force kill, cleanup, exit */ }
        _ = ctx.timer(Duration::from_secs(60)) => {
            let hb_input = HeartbeatInput { agent_id: polecat_id.clone() };
            let hb = ctx.activity(ActivityOptions {
                activity_type: "check_agent_alive".to_string(),
                input: hb_input.as_json_payload()?,
                start_to_close_timeout: Some(Duration::from_secs(10)),
                ..Default::default()
            }).await;

            if !hb.completed_ok() {
                status = "stuck".to_string();
                tracing::warn!("Polecat {name}: agent process died — stuck");
                // Continue loop to allow kill/done signals to clean up
            }
        }
    }
}
```

**Step 2: Verify and commit**

Run: `cargo build && cargo test`

```bash
git commit -m "feat: polecat lifecycle — real agent spawn, heartbeat monitoring, cleanup"
```

---

## Task 62: Wire `gtr done` — Commit, Push, Signal, Enqueue

**Files:**
- Modify: `crates/gtr-cli/src/commands/done.rs`

**Step 1: Enhance `gtr done` to commit + push + signal polecat + enqueue refinery**

The current `gtr done` only enqueues to refinery. The new version:
1. Reads `GTR_AGENT` to identify the calling polecat
2. Signals `polecat_done` to the polecat workflow
3. Enqueues to refinery (already does this)

```rust
// In done command handler, add:
// 1. Signal polecat that work is done
if let Ok(agent_id) = std::env::var("GTR_AGENT") {
    let done_signal = PolecatDoneSignal {
        branch: branch.clone(),
        status: "completed".to_string(),
    };
    let payload = done_signal.as_json_payload()?;
    client
        .signal_workflow_execution(
            agent_id,
            String::new(),
            "polecat_done".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;
}

// 2. Enqueue to refinery (existing code)
```

**Step 2: Verify and commit**

```bash
git commit -m "feat: gtr done — signals polecat, enqueues refinery"
```

---

## Task 63: Refinery Merge — Real Git Operations (already wired in Phase 2)

The refinery workflow was already upgraded in Phase 2 (Task 48) to use real `git_operation` activities (checkout, rebase, test, merge). This task is just verification that the end-to-end flow works.

**Step 1: Verify refinery workflow uses git_operation activities**

Read `crates/gtr-temporal/src/workflows/refinery.rs` and confirm it calls:
- `git_operation` with `GitOperation::Checkout`
- `git_operation` with `GitOperation::Rebase`
- `run_plugin` with `cargo test`
- `git_operation` with `GitOperation::Merge`

**Step 2: No code changes needed — this is a verification task**

```bash
# No commit needed — already done in Phase 2
```

---

## Task 64: `gtr attach` — Connect to Live Agent Session

**Files:**
- Create: `crates/gtr-cli/src/commands/attach.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Create attach command**

```rust
// crates/gtr-cli/src/commands/attach.rs
use std::io::{Read, Write};
use std::os::unix::io::FromRawFd;

use clap::Args;
use crossterm::terminal;

#[derive(Debug, Args)]
pub struct AttachCommand {
    /// Agent ID to attach to (e.g., "mayor", "gtr-polecat-furiosa")
    agent: String,
}

pub async fn run(cmd: &AttachCommand) -> anyhow::Result<()> {
    let agent_id = &cmd.agent;

    // Check if agent is running
    if !gtr_temporal::pty::is_alive(agent_id) {
        anyhow::bail!("Agent '{agent_id}' is not running. Check `gtr feed` for active agents.");
    }

    // Connect to PTY socket and receive master fd
    let master_fd = gtr_temporal::pty::connect_pty(agent_id)?;
    let mut pty_file = unsafe { std::fs::File::from_raw_fd(master_fd) };

    println!("Attached to '{agent_id}'. Ctrl+\\ to detach.\n");

    // Put terminal in raw mode
    terminal::enable_raw_mode()?;

    // I/O forwarding loop
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let mut stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let mut buf = [0u8; 4096];

        loop {
            // Use poll/select to multiplex stdin and PTY reads
            // Simple approach: non-blocking reads with small timeout
            use nix::poll::{poll, PollFd, PollFlags};

            let stdin_fd = nix::unistd::Pid::from_raw(0); // stdin fd = 0
            let pty_poll = PollFd::new(master_fd, PollFlags::POLLIN);
            let stdin_poll = PollFd::new(0, PollFlags::POLLIN);

            let mut fds = [pty_poll, stdin_poll];
            let _ready = poll(&mut fds, 100)?; // 100ms timeout

            // Read from PTY -> stdout
            if let Some(revents) = fds[0].revents() {
                if revents.contains(PollFlags::POLLIN) {
                    match nix::unistd::read(master_fd, &mut buf) {
                        Ok(0) => break, // PTY closed
                        Ok(n) => {
                            stdout.write_all(&buf[..n])?;
                            stdout.flush()?;
                        }
                        Err(nix::Error::EAGAIN) => {}
                        Err(_) => break,
                    }
                }
                if revents.contains(PollFlags::POLLHUP) {
                    break;
                }
            }

            // Read from stdin -> PTY
            if let Some(revents) = fds[1].revents() {
                if revents.contains(PollFlags::POLLIN) {
                    match nix::unistd::read(0, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            // Check for Ctrl+\ (0x1c) to detach
                            if buf[..n].contains(&0x1c) {
                                break;
                            }
                            nix::unistd::write(master_fd, &buf[..n])?;
                        }
                        Err(nix::Error::EAGAIN) => {}
                        Err(_) => break,
                    }
                }
            }
        }

        Ok(())
    })
    .await?;

    // Restore terminal
    terminal::disable_raw_mode()?;
    println!("\nDetached from '{agent_id}'.");

    result
}
```

**Step 2: Wire into mod.rs and main.rs**

Add `pub mod attach;` to mod.rs.

Add to main.rs Command enum:
```rust
/// Attach to a live agent session (interactive Claude Code)
Attach(commands::attach::AttachCommand),
```

Add match arm:
```rust
Command::Attach(cmd) => commands::attach::run(cmd).await,
```

**Step 3: Verify and commit**

Run: `cargo build && cargo test`

```bash
git commit -m "feat: gtr attach — PTY reconnect to live Claude Code sessions"
```

---

## Task 65: `gtr chat` — Signal-Based Async Messaging

**Files:**
- Create: `crates/gtr-cli/src/commands/chat.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Create chat command**

```rust
// crates/gtr-cli/src/commands/chat.rs
use clap::Args;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::signals::AgentMailSignal;

#[derive(Debug, Args)]
pub struct ChatCommand {
    /// Agent to send message to
    agent: String,
    /// Message to send
    message: String,
    /// Sender identity
    #[arg(short, long, default_value = "human")]
    from: String,
}

pub async fn run(cmd: &ChatCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;

    let signal = AgentMailSignal {
        from: cmd.from.clone(),
        message: cmd.message.clone(),
    };
    let payload = signal.as_json_payload()?;

    client
        .signal_workflow_execution(
            cmd.agent.clone(),
            String::new(),
            "agent_mail".to_string(),
            Some(payload.into()),
            None,
        )
        .await?;

    println!("Sent to {}: {}", cmd.agent, cmd.message);
    println!("(Agent will see this in their mail inbox)");

    Ok(())
}
```

**Step 2: Wire into mod.rs and main.rs**

**Step 3: Verify and commit**

```bash
git commit -m "feat: gtr chat — send async messages to agents via Temporal signals"
```

---

## Task 66: Detach Handling — SIGWINCH and Graceful Cleanup

**Files:**
- Modify: `crates/gtr-cli/src/commands/attach.rs`

**Step 1: Add terminal resize forwarding and signal handling**

Add SIGWINCH handler that updates the PTY window size when the terminal resizes. Add proper cleanup on SIGINT/SIGTERM that restores terminal state.

**Step 2: Verify and commit**

```bash
git commit -m "feat: attach — terminal resize forwarding and graceful detach"
```

---

## Task 67: Real `gtr prime` — SessionStart Context Injection

**Files:**
- Modify: `crates/gtr-cli/src/commands/prime.rs`

**Step 1: Rewrite prime to query Temporal for full agent context**

```rust
// crates/gtr-cli/src/commands/prime.rs
// The new prime command:
// 1. Reads GTR_AGENT, GTR_ROLE, GTR_RIG from env
// 2. Queries Temporal for the agent's workflow state (describe)
// 3. Queries for hooked work items
// 4. Queries for unread mail
// 5. Outputs a markdown context block to stdout
// 6. In --hook mode, also reads session JSON from stdin
```

The key improvement is that prime now queries multiple Temporal workflows to build context:
- Describe agent workflow (status, events)
- List work items assigned to this agent
- Check mail by describing agent workflow
- Output a formatted prompt for Claude Code

**Step 2: Verify and commit**

```bash
git commit -m "feat: real gtr prime — Temporal-backed context injection"
```

---

## Task 68: Real `gtr handoff` — Context Preservation

**Files:**
- Modify: `crates/gtr-cli/src/commands/handoff.rs`

**Step 1: Enhance handoff to capture and send full context**

Current handoff sends a mail. New version:
1. Runs `gtr hook` to capture current work state
2. Creates checkpoint (Task 50 checkpoint module)
3. Sends handoff mail to self
4. Signals the agent workflow to prepare for respawn

**Step 2: Verify and commit**

```bash
git commit -m "feat: real gtr handoff — checkpoint + mail + respawn preparation"
```

---

## Task 69: Crash Recovery — Heartbeat-Based Detection

**Files:**
- Modify: `crates/gtr-temporal/src/workflows/boot.rs`

**Step 1: Add respawn logic to boot workflow**

When the boot workflow's health check detects a dead agent, it should:
1. Clean up the dead agent's runtime directory
2. Respawn the agent via `spawn_agent` activity
3. The new agent's SessionStart hook runs `gtr prime` to restore context

This is the Temporal-native replacement for the Go daemon's heartbeat loop.

**Step 2: Verify and commit**

```bash
git commit -m "feat: boot crash recovery — auto-respawn dead agents"
```

---

## Task 70: Witness Real Monitoring

**Files:**
- Modify: `crates/gtr-temporal/src/workflows/witness.rs`

**Step 1: Wire witness to use heartbeat activity**

Replace the placeholder staleness tracking with real `check_agent_alive` calls for each polecat on the rig. The witness should:
1. List running polecat workflows via Temporal query (activity)
2. Check each polecat's subprocess via heartbeat activity
3. If stuck, signal the polecat workflow
4. If stuck for too long, escalate to mayor

**Step 2: Verify and commit**

```bash
git commit -m "feat: witness real monitoring — heartbeat checks on polecats"
```

---

## Task 71: Patrol Real Plugin Execution

**Files:**
- Modify: `crates/gtr-temporal/src/workflows/patrol.rs`

**Step 1: Wire patrol to discover and run real plugins**

The patrol workflow currently runs hardcoded "health-check" and "git-status" commands. Update it to:
1. Scan `~/.gtr/config/plugins/` directory for plugin TOML files
2. Use `gtr_core::plugin` module to parse and evaluate gates
3. Run eligible plugins via `run_plugin` activity
4. Track results for digest

**Step 2: Verify and commit**

```bash
git commit -m "feat: patrol real plugin execution — TOML discovery, gate evaluation"
```

---

## Task 72: Real Notification Activity

**Files:**
- Modify: `crates/gtr-temporal/src/activities/notification.rs`

**Step 1: Implement webhook dispatch**

Replace the log-only stub with real HTTP webhook dispatch using `reqwest` (already a transitive dependency via Temporal SDK):

```rust
"webhook" => {
    let url = &input.target;
    let body = serde_json::json!({
        "subject": input.subject,
        "message": input.message,
        "channel": input.channel,
    });
    let client = reqwest::Client::new();
    let resp = client.post(url).json(&body).send().await
        .map_err(|e| ActivityError::Retryable {
            source: anyhow::anyhow!("webhook failed: {e}"),
            explicit_delay: None,
        })?;
    tracing::info!("Notification webhook to {url}: {}", resp.status());
}
```

Keep "email", "sms", "signal" as log-only stubs with a tracing::warn that they're not yet implemented.

**Step 2: Verify and commit**

```bash
git commit -m "feat: real webhook notifications — HTTP POST dispatch"
```

---

## Task 73: Mock Agent Script for Integration Tests

**Files:**
- Create: `tests/mock-agent.sh`
- Create: `crates/gtr-temporal/tests/integration.rs`

**Step 1: Create mock agent shell script**

```bash
#!/bin/bash
# tests/mock-agent.sh
# Mock agent that simulates a Claude Code session.
# Reads GTR_AGENT and GTR_ROLE from environment.
# Responds to commands via stdin/stdout.

echo "Mock agent started: $GTR_AGENT ($GTR_ROLE)"
echo "Working directory: $(pwd)"

# Simulate work
sleep 2

# Run gtr done if we have a work item
if [ -n "$GTR_WORK_ITEM" ]; then
    echo "Completing work item: $GTR_WORK_ITEM"
    # In a real test, would run: gtr done $GTR_WORK_ITEM
fi

echo "Mock agent exiting"
```

**Step 2: Create integration test scaffold**

```rust
// crates/gtr-temporal/tests/integration.rs
// Integration tests require a running Temporal dev server.
// Run with: TEMPORAL_TEST=1 cargo test --test integration

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // Run with --ignored flag when Temporal is available
    fn test_spawn_and_heartbeat() {
        // 1. Spawn mock agent via PTY
        // 2. Verify is_alive returns true
        // 3. Verify socket_path exists
        // 4. Kill agent
        // 5. Verify is_alive returns false
    }

    #[test]
    #[ignore]
    fn test_attach_detach() {
        // 1. Spawn mock agent
        // 2. Connect to PTY socket
        // 3. Read output
        // 4. Disconnect
        // 5. Agent still running
    }
}
```

**Step 3: Verify and commit**

```bash
git commit -m "feat: mock agent script and integration test scaffold"
```

---

## Task 74: Integration Test — Spawn and Heartbeat

**Files:**
- Modify: `crates/gtr-temporal/tests/integration.rs`

**Step 1: Implement spawn + heartbeat integration test**

Test that:
1. `pty::spawn_with_server("test-agent", "sh", ["-c", "sleep 30"], "/tmp", {})` works
2. `pty::is_alive("test-agent")` returns true
3. `pty::socket_path("test-agent")` exists
4. `pty::connect_pty("test-agent")` returns a valid fd
5. `pty::kill_agent("test-agent")` succeeds
6. `pty::is_alive("test-agent")` returns false

**Step 2: Verify and commit**

```bash
git commit -m "test: integration test — PTY spawn, heartbeat, kill lifecycle"
```

---

## Task 75: Integration Test — End-to-End Sling to Done

**Files:**
- Modify: `crates/gtr-temporal/tests/integration.rs`

**Step 1: Implement e2e test (requires running Temporal)**

This test requires `temporal server start-dev` running. It tests:
1. Start worker (in background thread)
2. Start mayor workflow
3. Start polecat workflow with mock agent (runtime: "shell")
4. Verify agent spawns
5. Signal polecat done
6. Verify polecat workflow completes
7. Verify refinery receives enqueue signal

**Step 2: Verify and commit**

```bash
git commit -m "test: end-to-end sling-to-done integration test"
```

---

## Task 76: Integration Test — Multi-Rig

**Files:**
- Modify: `crates/gtr-temporal/tests/integration.rs`

**Step 1: Test with two rigs**

Verify that the system can manage agents across two rigs simultaneously without interference.

**Step 2: Verify and commit**

```bash
git commit -m "test: multi-rig integration test"
```

---

## Task 77: `gtr down` — Graceful Shutdown

**Files:**
- Modify: `crates/gtr-cli/src/commands/down.rs`

**Step 1: Rewrite `gtr down` to stop all agents gracefully**

1. Signal all agent workflows to stop (agent_stop)
2. Wait for agents to exit gracefully (2s timeout)
3. Kill any remaining PTY processes
4. Signal boot workflow to stop
5. Signal mayor workflow to stop
6. Clean up runtime directories

```rust
// Stop all running agents
let query = "ExecutionStatus = 'Running'".to_string();
let resp = client.list_workflow_executions(100, vec![], query).await?;

for exec in &resp.executions {
    let wf_id = exec.execution.as_ref().map(|e| e.workflow_id.clone()).unwrap_or_default();

    // Signal stop
    client.signal_workflow_execution(
        wf_id.clone(), String::new(), "agent_stop".to_string(), None, None,
    ).await.ok();

    // Kill PTY process if it exists
    gtr_temporal::pty::kill_agent(&wf_id).ok();
}
```

**Step 2: Verify and commit**

```bash
git commit -m "feat: gtr down — graceful shutdown of all agents and workflows"
```

---

## Task 78: `gtr status` — Rich System Overview

**Files:**
- Modify: `crates/gtr-cli/src/commands/status.rs`

**Step 1: Enhance status to show running agents and PTY state**

```
Gas Town Status
  Mayor:    running (PID 12345)
  Deacon:   running (PID 12346)
  Rigs:
    gtr:
      Witness:   running (PID 12347)
      Refinery:  running (PID 12348)
      Polecats:  2 active
        furiosa: working on WI-abc123 (PID 12349)
        nux:     working on WI-def456 (PID 12350)
    cmp:
      Witness:   running (PID 12351)
      Refinery:  not running
      Polecats:  0 active
  Convoys:  1 active
  Merge queue: 3 items
```

**Step 2: Verify and commit**

```bash
git commit -m "feat: gtr status — rich system overview with agent tree and PIDs"
```

---

## Task 79: `gtr install` — First-Time Setup

**Files:**
- Modify: `crates/gtr-cli/src/commands/workspace.rs` (or create `install.rs`)

**Step 1: Create/update install command**

`gtr install` should:
1. Create `~/.gtr/` directory structure (runtime, rigs, config)
2. Create default `town.toml` config if it doesn't exist
3. Verify `claude` CLI is available in PATH
4. Verify `temporal` server is reachable
5. Print setup summary

**Step 2: Verify and commit**

```bash
git commit -m "feat: gtr install — first-time setup with validation"
```

---

## Task 80: Config Validation and Rig Registration

**Files:**
- Modify: `crates/gtr-cli/src/commands/rig.rs`

**Step 1: Update `gtr rig add` to clone repo and create directory structure**

When adding a rig, the command should:
1. Clone the git repo to `~/.gtr/rigs/<name>/repo/`
2. Create subdirectories (polecats, crew, witness, refinery)
3. Start the rig workflow
4. Register with mayor via signal

The current `gtr rig add` only starts a workflow and signals — it needs to also set up the filesystem.

**Step 2: Verify and commit**

```bash
git commit -m "feat: gtr rig add — clone repo, create dirs, register with mayor"
```

---

## Testing Strategy

For each task:
1. `cargo build` must compile
2. `cargo test` must pass all existing + new unit tests
3. Integration tests (Tasks 73-76) run with `TEMPORAL_TEST=1 cargo test --test integration --ignored`
4. Manual E2E test: `gtr install && gtr up && gtr worker run` in separate terminal

## Notes for the Implementer

- **`nix` crate:** Version 0.29. Use features: `term`, `process`, `socket`, `signal`, `user`, `fs`. All work on macOS aarch64.
- **PTY on macOS:** Uses `/dev/ttys###` devices. `openpty` works natively.
- **SCM_RIGHTS:** File descriptor passing over Unix domain sockets. The `nix` crate's `sendmsg`/`recvmsg` handle this.
- **`OwnedFd` vs `RawFd`:** Prefer `OwnedFd` for ownership safety. Use `std::mem::forget` to prevent auto-close when handing off to the socket server thread.
- **Signal handling:** `SIGWINCH` for terminal resize, `SIGCHLD` for child process exit. Both handled via `nix::sys::signal`.
- **Claude Code CLI:** The `claude` binary must be in PATH. Use `--dangerously-skip-permissions` for non-interactive agents. The initial prompt is passed as the first positional argument.
- **Temporal SDK patterns** (from Phase 1/2):
  - `start_workflow` takes 6 args: `(vec![payload], task_queue, workflow_id, workflow_type, None, Default::default())`
  - `signal_workflow_execution` takes 5 args: `(workflow_id, String::new(), signal_name, Some(payload.into()), None)`
  - `completed_ok()` returns `bool` (not `Option`)
  - Signal channels: `ctx.make_signal_channel(NAME)`, use `StreamExt::next()` in `tokio::select!` with `biased;`
