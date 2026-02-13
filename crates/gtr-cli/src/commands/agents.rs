use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum AgentsCommand {
    /// List all agents
    List,
    /// Show agent details
    Show {
        /// Agent name
        name: String,
    },
    /// Show agent status
    Status,
}

pub fn run(cmd: &AgentsCommand) -> anyhow::Result<()> {
    match cmd {
        AgentsCommand::List => println!("agents list: not yet implemented"),
        AgentsCommand::Show { name } => println!("agents show {name}: not yet implemented"),
        AgentsCommand::Status => println!("agents status: not yet implemented"),
    }
    Ok(())
}
