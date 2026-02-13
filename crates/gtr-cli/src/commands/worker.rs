use clap::Subcommand;

/// Run a Temporal worker
#[derive(Debug, Subcommand)]
pub enum WorkerCommand {
    /// Start the worker and begin polling for tasks
    Run,
}

pub fn run(cmd: &WorkerCommand) -> anyhow::Result<()> {
    match cmd {
        WorkerCommand::Run => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(gtr_temporal::worker::run_worker())
        }
    }
}
