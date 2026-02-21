use clap::Subcommand;

/// Run a Temporal worker
#[derive(Debug, Subcommand)]
pub enum WorkerCommand {
    /// Start the worker and begin polling for tasks
    Run,
}

pub async fn run(cmd: &WorkerCommand) -> anyhow::Result<()> {
    match cmd {
        WorkerCommand::Run => gtr_temporal::worker::run_worker().await,
    }
}
