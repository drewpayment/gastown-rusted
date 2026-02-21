pub fn run() -> anyhow::Result<()> {
    let output = std::process::Command::new("tmux")
        .args(["-L", "gtr", "ls"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            println!("Active tmux sessions:");
            println!();
            for line in stdout.lines() {
                // Lines look like: "gtr-mayor: 1 windows ..."
                // Strip "gtr-" prefix for friendly display
                let friendly = if let Some(rest) = line.strip_prefix("gtr-") {
                    rest.to_string()
                } else {
                    line.to_string()
                };
                println!("  {friendly}");
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            if stderr.contains("no server running")
                || stderr.contains("error connecting")
                || stderr.contains("no sessions")
            {
                println!("No active tmux sessions.");
            } else {
                println!("No active tmux sessions.");
            }
        }
        Err(e) => {
            anyhow::bail!("Failed to run tmux: {e}. Is tmux installed?");
        }
    }

    Ok(())
}
