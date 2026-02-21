fn kill_tmux_session(session: &str) -> bool {
    std::process::Command::new("tmux")
        .args(["-L", "gtr", "kill-session", "-t", session])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub async fn run() -> anyhow::Result<()> {
    println!("Stopping Gas Town...");
    println!();

    // Step 1: Stop workflows, kill agents, clean runtime
    crate::commands::down::run().await?;

    // Step 2: Kill worker tmux session
    if kill_tmux_session("gtr-worker") {
        println!("  Killed worker session (gtr-worker)");
    }

    // Step 3: Kill Temporal server tmux session
    if kill_tmux_session("gtr-temporal-server") {
        println!("  Killed Temporal server session (gtr-temporal-server)");
    }

    println!();
    println!("Gas Town is fully stopped.");

    Ok(())
}
