use std::os::unix::io::BorrowedFd;
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

pub async fn run(cmd: &AttachCommand) -> anyhow::Result<()> {
    let agent_id = &cmd.agent;

    // Check if agent is running
    if !gtr_temporal::pty::is_alive(agent_id) {
        anyhow::bail!("Agent '{agent_id}' is not running. Check `gtr feed` for active agents.");
    }

    // Connect to PTY socket and receive master fd
    let master_fd = gtr_temporal::pty::connect_pty(agent_id)?;

    // Quick sanity check: try a non-blocking read to see if the PTY is still alive.
    // On macOS, a PTY master whose slave has closed will return EIO immediately.
    {
        use nix::fcntl::{fcntl, FcntlArg, OFlag};
        let flags = fcntl(master_fd, FcntlArg::F_GETFL)?;
        let flags = OFlag::from_bits_truncate(flags);
        // Set non-blocking temporarily
        fcntl(master_fd, FcntlArg::F_SETFL(flags | OFlag::O_NONBLOCK))?;
        let mut probe = [0u8; 1];
        let probe_result = nix::unistd::read(master_fd, &mut probe);
        // Restore original flags
        fcntl(master_fd, FcntlArg::F_SETFL(flags))?;

        match probe_result {
            Err(nix::Error::EIO) => {
                // Slave side closed — child process has exited
                anyhow::bail!(
                    "Agent '{agent_id}' process has exited (PTY closed). \
                     The boot workflow should respawn it automatically, or run: gtr agents show {agent_id}"
                );
            }
            // EAGAIN = no data yet but fd is alive (good)
            // Ok(n) = there's pending data (good, we'll re-read it in the loop)
            _ => {}
        }
    }

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
