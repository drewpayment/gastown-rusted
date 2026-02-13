use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Initialize a new workspace
    Init {
        /// Workspace path
        path: Option<String>,
    },
    /// Install workspace dependencies
    Install,
    /// Show workspace info
    Info,
}

pub fn run(cmd: &WorkspaceCommand) -> anyhow::Result<()> {
    match cmd {
        WorkspaceCommand::Init { path } => {
            let p = path.as_deref().unwrap_or(".");
            println!("workspace init {p}: not yet implemented");
        }
        WorkspaceCommand::Install => println!("workspace install: not yet implemented"),
        WorkspaceCommand::Info => println!("workspace info: not yet implemented"),
    }
    Ok(())
}
