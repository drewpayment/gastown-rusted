mod client;
mod commands;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

/// gtr — Gas Town Rusted CLI
#[derive(Debug, Parser)]
#[command(name = "gtr", version, about, long_about = None)]
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

    /// Unassign work from an agent
    Unsling(commands::unsling::UnslingCommand),

    /// Query agent's current work
    Hook(commands::hook::HookCommand),

    /// Agent messaging system
    #[command(subcommand)]
    Mail(commands::mail::MailCommand),

    /// Execute formulas (multi-step recipes)
    #[command(subcommand)]
    Formula(commands::formula::FormulaCommand),

    /// Manage agents
    #[command(subcommand)]
    Agents(commands::agents::AgentsCommand),

    /// Mayor workflow management
    #[command(subcommand)]
    Mayor(commands::mayor::MayorCommand),

    /// Mark work as done and enqueue for merge
    Done(commands::done::DoneCommand),

    /// Escalate a work item immediately
    Escalate(commands::escalate::EscalateCommand),

    /// Molecule (running formula instance) management
    #[command(subcommand)]
    Mol(commands::mol::MolCommand),

    /// Merge queue management
    #[command(subcommand)]
    Mq(commands::mq::MqCommand),

    /// Manage polecats (ephemeral workers)
    #[command(subcommand)]
    Polecat(commands::polecat::PolecatCommand),

    /// Manage rigs (git repositories)
    #[command(subcommand)]
    Rig(commands::rig::RigCommand),

    /// Manage crew workspaces (persistent developer workspaces)
    #[command(subcommand)]
    Crew(commands::crew::CrewCommand),

    /// Manage dogs (reusable cross-rig infrastructure workers)
    #[command(subcommand)]
    Dog(commands::dog::DogCommand),

    /// Manage gates (async wait primitives)
    #[command(subcommand)]
    Gate(commands::gate::GateCommand),

    /// Inject context for a new agent session
    Prime(commands::prime::PrimeCommand),

    /// Save context for the next session
    Handoff(commands::handoff::HandoffCommand),

    /// Check system health
    Doctor,

    /// Start Gas Town (launch mayor workflow)
    Up,

    /// Stop Gas Town (stop mayor workflow)
    Down,

    /// Show Gas Town status
    Status,

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

    /// Show version and build info
    Version,

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Command::Convoy(cmd) => commands::convoy::run(cmd).await,
        Command::Work(cmd) => commands::work::run(cmd).await,
        Command::Sling(cmd) => commands::sling::run(cmd).await,
        Command::Unsling(cmd) => commands::unsling::run(cmd).await,
        Command::Hook(cmd) => commands::hook::run(cmd).await,
        Command::Mail(cmd) => commands::mail::run(cmd).await,
        Command::Formula(cmd) => commands::formula::run(cmd).await,
        Command::Done(cmd) => commands::done::run(cmd).await,
        Command::Escalate(cmd) => commands::escalate::run(cmd).await,
        Command::Mol(cmd) => commands::mol::run(cmd).await,
        Command::Mq(cmd) => commands::mq::run(cmd).await,
        Command::Polecat(cmd) => commands::polecat::run(cmd).await,
        Command::Rig(cmd) => commands::rig::run(cmd).await,
        Command::Crew(cmd) => commands::crew::run(cmd).await,
        Command::Dog(cmd) => commands::dog::run(cmd).await,
        Command::Gate(cmd) => commands::gate::run(cmd).await,
        Command::Prime(cmd) => commands::prime::run(cmd).await,
        Command::Handoff(cmd) => commands::handoff::run(cmd).await,
        Command::Agents(cmd) => commands::agents::run(cmd),
        Command::Mayor(cmd) => commands::mayor::run(cmd).await,
        Command::Doctor => commands::doctor::run().await,
        Command::Up => commands::up::run().await,
        Command::Down => commands::down::run().await,
        Command::Status => commands::status::run().await,
        Command::Services(cmd) => commands::services::run(cmd),
        Command::Workspace(cmd) => commands::workspace::run(cmd),
        Command::Diagnostics(cmd) => commands::diagnostics::run(cmd).await,
        Command::Worker(cmd) => commands::worker::run(cmd),
        Command::Version => {
            println!("gtr {} ({})", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_NAME"));
            println!("Rust edition: 2021");
            println!("Temporal SDK: rev 7ecb7c0");
            Ok(())
        }
        Command::Completions { shell } => {
            clap_complete::generate(
                *shell,
                &mut Cli::command(),
                "gtr",
                &mut std::io::stdout(),
            );
            Ok(())
        }
    }
}
