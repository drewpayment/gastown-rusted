use std::collections::HashMap;
use std::os::unix::io::BorrowedFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Args;
use crossterm::terminal;

#[derive(Debug, Args)]
#[command(about = "Attach to a live agent PTY session (interactive Claude Code)")]
pub struct AttachCommand {
    /// Agent ID to attach to (e.g., "mayor", "gtr-polecat-furiosa")
    pub agent: String,
}

/// Why the I/O loop exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetachReason {
    /// User pressed Ctrl+\ to detach
    UserDetach,
    /// PTY slave closed (child process exited)
    PtyClosed,
    /// Stdin closed
    StdinClosed,
    /// Invalid fd
    InvalidFd,
}

/// Check if a PTY master fd is alive by doing a non-blocking probe read.
/// Returns true if the fd is alive, false if the slave side has closed.
fn pty_is_alive(master_fd: nix::libc::c_int) -> bool {
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    let Ok(flags) = fcntl(master_fd, FcntlArg::F_GETFL) else {
        return false;
    };
    let flags = OFlag::from_bits_truncate(flags);
    if fcntl(master_fd, FcntlArg::F_SETFL(flags | OFlag::O_NONBLOCK)).is_err() {
        return false;
    }
    let mut probe = [0u8; 1];
    let result = nix::unistd::read(master_fd, &mut probe);
    let _ = fcntl(master_fd, FcntlArg::F_SETFL(flags));
    !matches!(result, Err(nix::Error::EIO))
}

/// Respawn an agent's PTY session. Reads env.json from the old runtime dir,
/// cleans up, then spawns a fresh process + socket server.
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

    // Ensure gtr binary is on PATH
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
    }

    let prompt = "You are being reattached after your previous session ended. \
                  Run `gtr prime` to restore context, then `gtr hook` and `gtr mail inbox`.";

    // Clean up stale runtime dir
    gtr_temporal::pty::cleanup(agent_id)?;

    // Spawn new PTY session
    gtr_temporal::pty::spawn_with_server(
        agent_id,
        "claude",
        &["--dangerously-skip-permissions".into(), prompt.into()],
        &PathBuf::from(&work_dir),
        &env,
    )?;

    Ok(())
}

pub async fn run(cmd: &AttachCommand) -> anyhow::Result<()> {
    let agent_id = &cmd.agent;

    // Check if agent process is running, or if PTY needs respawn
    let needs_respawn = if !gtr_temporal::pty::is_alive(agent_id) {
        // No process at all — check if there's at least a socket (stale runtime dir)
        let sock = gtr_temporal::pty::socket_path(agent_id);
        if sock.exists() {
            true // stale session, respawn
        } else {
            anyhow::bail!(
                "Agent '{agent_id}' has no PTY session. Start it with `gtr up` or the boot workflow."
            );
        }
    } else {
        // Process exists — probe the PTY fd to see if it's alive
        match gtr_temporal::pty::connect_pty(agent_id) {
            Ok(fd) => !pty_is_alive(fd),
            Err(_) => true, // can't connect to socket, respawn
        }
    };

    if needs_respawn {
        println!("Agent '{agent_id}' session ended. Respawning...");
        respawn_agent(agent_id)?;
        // Give the new process a moment to initialize
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Connect to PTY socket and receive master fd
    let master_fd = gtr_temporal::pty::connect_pty(agent_id)?;

    // Set PTY window size to match our terminal BEFORE entering raw mode
    if let Ok((cols, rows)) = terminal::size() {
        let _ = gtr_temporal::pty::set_winsize(master_fd, rows, cols);
    }

    println!("Attached to '{agent_id}'. Ctrl+\\ to detach.\n");

    // Set up a panic hook to restore terminal on panic
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        orig_hook(info);
    }));

    // Track SIGWINCH via an atomic flag
    let winch_flag = Arc::new(AtomicBool::new(false));
    let winch_flag_handler = winch_flag.clone();
    // SAFETY: setting an AtomicBool is signal-safe
    unsafe {
        signal_hook::low_level::register(signal_hook::consts::SIGWINCH, move || {
            winch_flag_handler.store(true, Ordering::Relaxed);
        })?;
    }

    // Put terminal in raw mode
    terminal::enable_raw_mode()?;

    // I/O forwarding loop
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<DetachReason> {
        use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

        let mut buf = [0u8; 4096];

        // SAFETY: master_fd is a valid open file descriptor received from connect_pty.
        let pty_borrowed = unsafe { BorrowedFd::borrow_raw(master_fd) };
        // SAFETY: stdin (fd 0) is always a valid file descriptor.
        let stdin_borrowed = unsafe { BorrowedFd::borrow_raw(0) };

        loop {
            // Check for terminal resize
            if winch_flag.swap(false, Ordering::Relaxed) {
                if let Ok((cols, rows)) = terminal::size() {
                    let _ = gtr_temporal::pty::set_winsize(master_fd, rows, cols);
                }
            }

            let pty_poll = PollFd::new(pty_borrowed, PollFlags::POLLIN);
            let stdin_poll = PollFd::new(stdin_borrowed, PollFlags::POLLIN);

            let mut fds = [pty_poll, stdin_poll];
            let ready = poll(&mut fds, PollTimeout::from(100u16)); // 100ms timeout

            // poll can return EINTR when SIGWINCH fires — just retry
            match ready {
                Err(nix::Error::EINTR) => continue,
                Err(e) => return Err(e.into()),
                Ok(_) => {}
            }

            // Read from PTY -> stdout
            if let Some(revents) = fds[0].revents() {
                // POLLNVAL means the fd is not valid
                if revents.contains(PollFlags::POLLNVAL) {
                    return Ok(DetachReason::InvalidFd);
                }

                if revents.contains(PollFlags::POLLIN) {
                    match nix::unistd::read(master_fd, &mut buf) {
                        Ok(0) => return Ok(DetachReason::PtyClosed),
                        Ok(n) => {
                            use std::io::Write;
                            let mut stdout = std::io::stdout();
                            stdout.write_all(&buf[..n])?;
                            stdout.flush()?;
                        }
                        Err(nix::Error::EAGAIN) => {}
                        // macOS returns EIO on PTY master when slave closes
                        Err(nix::Error::EIO) => return Ok(DetachReason::PtyClosed),
                        Err(_) => return Ok(DetachReason::PtyClosed),
                    }
                } else if revents.contains(PollFlags::POLLHUP) {
                    // Only break on POLLHUP when POLLIN is not also set.
                    // macOS can report POLLHUP alongside POLLIN on PTY fds;
                    // we must drain all data before treating HUP as disconnect.
                    return Ok(DetachReason::PtyClosed);
                }
            }

            // Read from stdin -> PTY
            if let Some(revents) = fds[1].revents() {
                if revents.contains(PollFlags::POLLIN) {
                    match nix::unistd::read(0, &mut buf) {
                        Ok(0) => return Ok(DetachReason::StdinClosed),
                        Ok(n) => {
                            // Check for Ctrl+\ (0x1c) to detach
                            if buf[..n].contains(&0x1c) {
                                return Ok(DetachReason::UserDetach);
                            }
                            nix::unistd::write(pty_borrowed, &buf[..n])?;
                        }
                        Err(nix::Error::EAGAIN) => {}
                        Err(_) => return Ok(DetachReason::StdinClosed),
                    }
                }
            }
        }
    })
    .await?;

    // Restore terminal
    terminal::disable_raw_mode()?;

    match result? {
        DetachReason::UserDetach => {
            println!("\nDetached from '{agent_id}'.");
        }
        DetachReason::PtyClosed => {
            println!("\nAgent '{agent_id}' process exited. Session ended.");
            println!("  The boot workflow will respawn it, or check: gtr agents show {agent_id}");
        }
        DetachReason::StdinClosed => {
            println!("\nStdin closed. Detached from '{agent_id}'.");
        }
        DetachReason::InvalidFd => {
            println!("\nPTY connection lost (invalid fd). Detached from '{agent_id}'.");
        }
    }

    Ok(())
}
