use clap::Args;

#[derive(Debug, Args)]
pub struct InstallCommand {
    /// Skip validation checks
    #[arg(long)]
    pub skip_checks: bool,
}

pub async fn run(cmd: &InstallCommand) -> anyhow::Result<()> {
    println!("Setting up Gas Town...");
    println!();

    // Step 1: Create directory structure
    gtr_core::dirs::ensure_base_dirs()?;
    println!("[ok] Created ~/.gtr directory structure");
    println!("     ~/.gtr/runtime/  — live process state");
    println!("     ~/.gtr/rigs/     — git repository workspaces");
    println!("     ~/.gtr/config/   — configuration files");

    // Step 2: Create default config if not present
    let config_path = gtr_core::dirs::config_dir().join("town.toml");
    if !config_path.exists() {
        let default_config = r#"# Gas Town Configuration
# See: rgt help

[town]
name = "gas-town"

[escalation]
timeout_minutes = 30
max_retries = 3
"#;
        std::fs::write(&config_path, default_config)?;
        println!("[ok] Created default config at {}", config_path.display());
    } else {
        println!("[ok] Config already exists at {}", config_path.display());
    }

    // Step 3: Create plugins directory
    let plugins_dir = gtr_core::dirs::config_dir().join("plugins");
    std::fs::create_dir_all(&plugins_dir)?;
    println!("[ok] Created plugins directory at {}", plugins_dir.display());

    if !cmd.skip_checks {
        println!();
        println!("Checking dependencies...");

        // Check Claude CLI
        let claude_ok = std::process::Command::new("which")
            .arg("claude")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if claude_ok {
            println!("[ok] Claude CLI found in PATH");
        } else {
            println!("[!!] Claude CLI not found — install from https://claude.ai/claude-code");
        }

        // Check tmux
        let tmux_ok = std::process::Command::new("tmux")
            .arg("-V")
            .output()
            .map(|o| {
                if !o.status.success() {
                    return false;
                }
                let ver = String::from_utf8_lossy(&o.stdout);
                let part = ver.trim().strip_prefix("tmux ").unwrap_or(ver.trim());
                let numeric: String = part
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                numeric.parse::<f64>().unwrap_or(0.0) >= 3.2
            })
            .unwrap_or(false);
        if tmux_ok {
            println!("[ok] tmux >= 3.2 found");
        } else {
            println!("[!!] tmux >= 3.2 not found — install from https://github.com/tmux/tmux");
        }

        // Check Temporal CLI
        let temporal_ok = std::process::Command::new("which")
            .arg("temporal")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if temporal_ok {
            println!("[ok] Temporal CLI found in PATH");
        } else {
            println!("[!!] Temporal CLI not found — install from https://temporal.io/download");
        }

        // Check if Temporal server is reachable
        let server_ok = std::process::Command::new("temporal")
            .args(["server", "check", "--address", "localhost:7233"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if server_ok {
            println!("[ok] Temporal server reachable at localhost:7233");
        } else {
            println!("[--] Temporal server not reachable (start with: rgt start)");
        }
    }

    println!();
    println!("Gas Town installed. Next steps:");
    println!("  1. Start everything:  rgt start");
    println!("  2. Check sessions:    rgt sessions");
    println!("  3. Or manually:       rgt up && rgt worker run");

    Ok(())
}
