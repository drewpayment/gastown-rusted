use std::net::TcpStream;
use std::time::{Duration, Instant};

use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

fn ensure_tmux() -> anyhow::Result<()> {
    let ok = std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        anyhow::bail!("tmux is required but not found. Install with: brew install tmux");
    }
    Ok(())
}

fn tmux_session_exists(session: &str) -> bool {
    std::process::Command::new("tmux")
        .args(["-L", "gtr", "has-session", "-t", session])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn start_tmux_session(session: &str, cmd: &str, args: &[&str]) -> anyhow::Result<()> {
    let mut tmux_args = vec![
        "-L", "gtr",
        "new-session", "-d",
        "-s", session,
        cmd,
    ];
    tmux_args.extend_from_slice(args);

    let status = std::process::Command::new("tmux")
        .args(&tmux_args)
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to start tmux session '{session}'");
    }
    Ok(())
}

/// Load temporal_address from town.toml, falling back to default.
fn resolve_temporal_address() -> String {
    let config_path = gtr_core::dirs::config_dir().join("town.toml");
    if config_path.exists() {
        if let Ok(config) = gtr_core::config::load_config::<gtr_core::config::TownConfig>(&config_path) {
            return config.temporal_address;
        }
    }
    "http://localhost:7233".into()
}

/// Extract host:port from a Temporal address URL for TCP probing.
fn temporal_host_port(address: &str) -> String {
    // Strip scheme (http://, https://)
    let without_scheme = address
        .strip_prefix("http://")
        .or_else(|| address.strip_prefix("https://"))
        .unwrap_or(address);
    // Strip trailing slash
    without_scheme.trim_end_matches('/').to_string()
}

/// Check whether the configured Temporal address points to localhost.
fn is_localhost(host_port: &str) -> bool {
    let host = host_port.split(':').next().unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "0.0.0.0")
}

pub async fn run() -> anyhow::Result<()> {
    ensure_tmux()?;

    let temporal_addr = resolve_temporal_address();
    let host_port = temporal_host_port(&temporal_addr);

    println!("Starting Gas Town...");
    println!();

    // Step 1: Ensure Temporal is reachable
    let already_reachable = TcpStream::connect_timeout(
        &host_port.parse().unwrap_or_else(|_| "127.0.0.1:7233".parse().unwrap()),
        Duration::from_secs(1),
    ).is_ok();

    if already_reachable {
        println!("[ok] Temporal server reachable at {host_port}");
    } else if tmux_session_exists("gtr-temporal-server") {
        println!("[ok] Temporal dev server session exists (gtr-temporal-server)");
    } else if is_localhost(&host_port) {
        // Only auto-start if targeting localhost
        println!("[..] Starting Temporal dev server...");
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let db_path = format!("{home}/.gtr/temporal.db");
        start_tmux_session(
            "gtr-temporal-server",
            "temporal",
            &["server", "start-dev", "--db-filename", &db_path],
        )?;

        // Poll for readiness
        let addr = host_port.parse().unwrap_or_else(|_| "127.0.0.1:7233".parse().unwrap());
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut ready = false;
        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
                ready = true;
                break;
            }
        }

        if ready {
            println!("[ok] Temporal server ready at {host_port}");
        } else {
            anyhow::bail!(
                "Temporal server did not become ready within 15s. \
                 Check: tmux -L gtr attach -t gtr-temporal-server"
            );
        }
    } else {
        anyhow::bail!(
            "Temporal server not reachable at {host_port} (configured in town.toml). \
             Start your Temporal server, then run `rgt start` again."
        );
    }

    // Step 2: Start worker if not running
    if tmux_session_exists("gtr-worker") {
        println!("[ok] Worker already running (gtr-worker)");
    } else {
        println!("[..] Starting worker...");
        let exe = std::env::current_exe()?;
        let exe_str = exe.to_string_lossy().to_string();
        start_tmux_session("gtr-worker", &exe_str, &["worker", "run"])?;
        // Give worker a moment to connect
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("[ok] Worker started (gtr-worker)");
    }

    // Step 3: Start workflows
    println!("[..] Starting workflows...");
    let (mayor_started, boot_started) = crate::commands::up::start_workflows().await?;
    println!("[ok] Mayor workflow: {}", if mayor_started { "started" } else { "already running" });
    println!("[ok] Boot workflow: {}", if boot_started { "started" } else { "already running" });

    // Step 4: Re-register rigs from registry
    let rigs_config = gtr_core::config::RigsConfig::load()?;
    if !rigs_config.rigs.is_empty() {
        println!("[..] Re-registering rigs...");
        let client = crate::client::connect().await?;
        for rig_entry in &rigs_config.rigs {
            let wf_id = format!("rig-{}", rig_entry.name);
            let already_running = crate::commands::up::is_workflow_running_pub(&client, &wf_id).await;
            if already_running {
                println!("[ok] Rig '{}' already running", rig_entry.name);
            } else {
                let git_url = rig_entry.git_url.as_deref().unwrap_or("");
                let input_payload = (rig_entry.name.as_str(), git_url)
                    .as_json_payload()?;
                if let Err(e) = client
                    .start_workflow(
                        vec![input_payload],
                        "work".to_string(),
                        wf_id.clone(),
                        "rig_wf".to_string(),
                        None,
                        Default::default(),
                    )
                    .await
                {
                    println!("[!!] Failed to start rig '{}': {e}", rig_entry.name);
                    continue;
                }
                // Signal rig to boot
                if let Err(e) = client
                    .signal_workflow_execution(
                        wf_id,
                        String::new(),
                        "rig_boot".to_string(),
                        None,
                        None,
                    )
                    .await
                {
                    println!("[!!] Failed to boot rig '{}': {e}", rig_entry.name);
                    continue;
                }
                println!("[ok] Rig '{}' re-registered and booted", rig_entry.name);
            }
        }
    }

    println!();
    println!("Gas Town is up. Run `rgt sessions` to see active sessions.");
    println!("To stop: `rgt stop`");

    Ok(())
}
