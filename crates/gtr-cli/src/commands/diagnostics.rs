use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum DiagnosticsCommand {
    /// Show system health
    Health,
    /// Show version info
    Version,
    /// Run diagnostic checks
    Check,
}

pub fn run(cmd: &DiagnosticsCommand) -> anyhow::Result<()> {
    match cmd {
        DiagnosticsCommand::Health => println!("diagnostics health: not yet implemented"),
        DiagnosticsCommand::Version => {
            println!("gtr v{}", gtr_core::version());
        }
        DiagnosticsCommand::Check => println!("diagnostics check: not yet implemented"),
    }
    Ok(())
}
