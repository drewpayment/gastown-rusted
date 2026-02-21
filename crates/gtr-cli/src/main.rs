mod client;
mod commands;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

/// rgt — Rusted Gas Town CLI
#[derive(Debug, Parser)]
#[command(name = "rgt", version, about, long_about = "\
rgt — Rusted Gas Town CLI

Quick reference:
  rgt up                      Start Gas Town (launch mayor)
  rgt down                    Stop Gas Town gracefully
  rgt status                  Show system overview
  rgt rig list                List rigs (repos)
  rgt agents list             List running agents
  rgt work list               List active work items
  rgt hook [AGENT]            Query agent's current work (env: GTR_AGENT)
  rgt mail inbox [AGENT]      Check agent inbox (env: GTR_AGENT)
  rgt mail send <TO> <MSG>    Send message to agent
  rgt done <WORK_ID> -b <BR>  Mark work done, enqueue merge (env: GTR_WORK_ITEM)
  rgt feed                    Real-time activity dashboard

Environment variables:
  GTR_AGENT       Default agent ID for hook, mail, checkpoint
  GTR_WORK_ITEM   Default work item ID for done, checkpoint
  GTR_RIG         Default rig for refinery routing
  TEMPORAL_ADDRESS Temporal server (default: localhost:7233)
")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Attach to a live agent PTY session (interactive Claude Code)
    Attach(commands::attach::AttachCommand),

    /// Send a message to an agent (async via Temporal signal)
    Chat(commands::chat::ChatCommand),

    /// Manage convoys — list, show, create batches of work items
    #[command(subcommand)]
    Convoy(commands::convoy::ConvoyCommand),

    /// Manage work items — show, list, close (Temporal work_item_wf)
    #[command(subcommand)]
    Work(commands::work::WorkCommand),

    /// Assign work to a rig (auto-spawns polecat), agent, mayor, or dogs
    Sling(commands::sling::SlingCommand),

    /// Unassign work from an agent
    Unsling(commands::unsling::UnslingCommand),

    /// Query agent's current work assignment (defaults to GTR_AGENT env var)
    Hook(commands::hook::HookCommand),

    /// Agent messaging — send, inbox/list, check, nudge, broadcast
    #[command(subcommand)]
    Mail(commands::mail::MailCommand),

    /// Execute formulas (multi-step recipes)
    #[command(subcommand)]
    Formula(commands::formula::FormulaCommand),

    /// List and inspect running agents (Temporal agent_wf)
    #[command(subcommand)]
    Agents(commands::agents::AgentsCommand),

    /// Mayor workflow management — the singleton orchestrator
    #[command(subcommand)]
    Mayor(commands::mayor::MayorCommand),

    /// Mark work done and enqueue branch for merge (defaults to GTR_WORK_ITEM env var)
    Done(commands::done::DoneCommand),

    /// Escalate a work item — signal immediate priority boost
    Escalate(commands::escalate::EscalateCommand),

    /// Molecule management — running formula instances
    #[command(subcommand)]
    Mol(commands::mol::MolCommand),

    /// Merge queue management — list and inspect refinery queue
    #[command(subcommand)]
    Mq(commands::mq::MqCommand),

    /// Manage polecats — ephemeral workers that run agents on rigs
    #[command(subcommand)]
    Polecat(commands::polecat::PolecatCommand),

    /// Manage rigs — git repositories registered with Gas Town
    #[command(subcommand)]
    Rig(commands::rig::RigCommand),

    /// Manage crew workspaces — persistent developer workspaces
    #[command(subcommand)]
    Crew(commands::crew::CrewCommand),

    /// Manage dogs — reusable cross-rig infrastructure workers
    #[command(subcommand)]
    Dog(commands::dog::DogCommand),

    /// Manage gates — async wait primitives for workflow coordination
    #[command(subcommand)]
    Gate(commands::gate::GateCommand),

    /// Inject context for a new agent session (prime file)
    Prime(commands::prime::PrimeCommand),

    /// Save context for the next session (handoff file)
    Handoff(commands::handoff::HandoffCommand),

    /// Checkpoint management — save/restore agent session state
    #[command(subcommand)]
    Checkpoint(commands::checkpoint::CheckpointCommand),

    /// Real-time activity dashboard — stream workflow events
    Feed(commands::feed::FeedCommand),

    /// Check system health — verify Temporal, mayor, and dependencies
    Doctor,

    /// Start Gas Town (launch mayor workflow)
    Up,

    /// Stop Gas Town gracefully (stop mayor and all agents)
    Down,

    /// Start everything — Temporal server, worker, and workflows (via tmux)
    Start,

    /// Stop everything — workflows, worker, and Temporal server
    Stop,

    /// List active tmux sessions
    Sessions,

    /// First-time setup — create directories, default config, validate dependencies
    Install(commands::install::InstallCommand),

    /// Show Gas Town status — agents, rigs, PIDs overview
    Status,

    /// Session management — list/status of agent sessions
    #[command(subcommand)]
    Session(commands::session::SessionCommand),

    /// Manage services
    #[command(subcommand)]
    Services(commands::services::ServicesCommand),

    /// Manage workspaces
    #[command(subcommand)]
    Workspace(commands::workspace::WorkspaceCommand),

    /// Diagnostics and system health — detailed inspection
    #[command(subcommand)]
    Diagnostics(commands::diagnostics::DiagnosticsCommand),

    /// Run a Temporal worker (start workflow/activity processing)
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
        Command::Attach(cmd) => commands::attach::run(&cmd).await,
        Command::Chat(cmd) => commands::chat::run(&cmd).await,
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
        Command::Agents(cmd) => commands::agents::run(cmd).await,
        Command::Mayor(cmd) => commands::mayor::run(cmd).await,
        Command::Checkpoint(cmd) => commands::checkpoint::run(cmd).await,
        Command::Feed(cmd) => commands::feed::run(cmd).await,
        Command::Doctor => commands::doctor::run().await,
        Command::Up => commands::up::run().await,
        Command::Down => commands::down::run().await,
        Command::Start => commands::start::run().await,
        Command::Stop => commands::stop::run().await,
        Command::Sessions => commands::sessions::run(),
        Command::Install(cmd) => commands::install::run(&cmd).await,
        Command::Status => commands::status::run().await,
        Command::Session(cmd) => commands::session::run(cmd).await,
        Command::Services(cmd) => commands::services::run(cmd),
        Command::Workspace(cmd) => commands::workspace::run(cmd),
        Command::Diagnostics(cmd) => commands::diagnostics::run(cmd).await,
        Command::Worker(cmd) => commands::worker::run(cmd).await,
        Command::Version => {
            println!(
                "rgt {} ({})",
                env!("CARGO_PKG_VERSION"),
                env!("GIT_VERSION")
            );
            println!("Built: {}", env!("BUILD_DATE"));
            Ok(())
        }
        Command::Completions { shell } => {
            clap_complete::generate(
                *shell,
                &mut Cli::command(),
                "rgt",
                &mut std::io::stdout(),
            );
            Ok(())
        }
    }
}
