use std::os::unix::io::BorrowedFd;

use clap::Args;
use crossterm::terminal;

#[derive(Debug, Args)]
pub struct AttachCommand {
    /// Agent ID to attach to (e.g., "mayor", "gtr-polecat-furiosa")
    pub agent: String,
}

pub async fn run(cmd: &AttachCommand) -> anyhow::Result<()> {
    let agent_id = &cmd.agent;

    // Check if agent is running
    if !gtr_temporal::pty::is_alive(agent_id) {
        anyhow::bail!("Agent '{agent_id}' is not running. Check `gtr feed` for active agents.");
    }

    // Connect to PTY socket and receive master fd
    let master_fd = gtr_temporal::pty::connect_pty(agent_id)?;

    println!("Attached to '{agent_id}'. Ctrl+\\ to detach.\n");

    // Set up a panic hook to restore terminal on panic
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        orig_hook(info);
    }));

    // Put terminal in raw mode
    terminal::enable_raw_mode()?;

    // I/O forwarding loop
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

        let mut buf = [0u8; 4096];

        // SAFETY: master_fd is a valid open file descriptor received from connect_pty.
        // We keep this BorrowedFd alive for the entire loop scope.
        let pty_borrowed = unsafe { BorrowedFd::borrow_raw(master_fd) };
        // SAFETY: stdin (fd 0) is always a valid file descriptor.
        let stdin_borrowed = unsafe { BorrowedFd::borrow_raw(0) };

        loop {
            let pty_poll = PollFd::new(pty_borrowed, PollFlags::POLLIN);
            let stdin_poll = PollFd::new(stdin_borrowed, PollFlags::POLLIN);

            let mut fds = [pty_poll, stdin_poll];
            let _ready = poll(&mut fds, PollTimeout::from(100u16))?; // 100ms timeout

            // Read from PTY -> stdout
            if let Some(revents) = fds[0].revents() {
                if revents.contains(PollFlags::POLLIN) {
                    match nix::unistd::read(master_fd, &mut buf) {
                        Ok(0) => break, // PTY closed
                        Ok(n) => {
                            use std::io::Write;
                            let mut stdout = std::io::stdout();
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
                            nix::unistd::write(pty_borrowed, &buf[..n])?;
                        }
                        Err(nix::Error::EAGAIN) => {}
                        Err(_) => break,
                    }
                }
            }

            // Check for terminal resize (poll for SIGWINCH every cycle)
            // Full SIGWINCH forwarding can be added later — for now this is
            // a placeholder that reads the current terminal size.
            if let Ok((_cols, _rows)) = crossterm::terminal::size() {
                // TODO: forward winsize to PTY via nix::pty::Winsize + ioctl
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
