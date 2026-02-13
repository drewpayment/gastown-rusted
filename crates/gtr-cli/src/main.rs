mod commands;

use clap::{Parser, Subcommand};

/// gtr — Gas Town Rusted CLI
#[derive(Debug, Parser)]
#[command(name = "gtr", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage convoys (batches of work)
    #[command(subcommand)]
    Convoy(commands::convoy::ConvoyCommand),

    /// Manage work items
    #[command(subcommand)]
    Work(commands::work::WorkCommand),

    /// Assign work to an agent
    Sling(commands::sling::SlingCommand),

    /// Agent messaging system
    #[command(subcommand)]
    Mail(commands::mail::MailCommand),

    /// Manage agents
    #[command(subcommand)]
    Agents(commands::agents::AgentsCommand),

    /// Manage services
    #[command(subcommand)]
    Services(commands::services::ServicesCommand),

    /// Manage workspaces
    #[command(subcommand)]
    Workspace(commands::workspace::WorkspaceCommand),

    /// Diagnostics and system health
    #[command(subcommand)]
    Diagnostics(commands::diagnostics::DiagnosticsCommand),

    /// Run a Temporal worker
    #[command(subcommand)]
    Worker(commands::worker::WorkerCommand),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Convoy(cmd) => commands::convoy::run(cmd),
        Command::Work(cmd) => commands::work::run(cmd),
        Command::Sling(cmd) => commands::sling::run(cmd),
        Command::Mail(cmd) => commands::mail::run(cmd),
        Command::Agents(cmd) => commands::agents::run(cmd),
        Command::Services(cmd) => commands::services::run(cmd),
        Command::Workspace(cmd) => commands::workspace::run(cmd),
        Command::Diagnostics(cmd) => commands::diagnostics::run(cmd),
        Command::Worker(cmd) => commands::worker::run(cmd),
    }
}
