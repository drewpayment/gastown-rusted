use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum WorkCommand {
    /// Show a work item
    Show {
        /// Work item ID
        id: String,
    },
    /// List work items
    List,
    /// Close a work item
    Close {
        /// Work item ID
        id: String,
    },
}

pub fn run(cmd: &WorkCommand) -> anyhow::Result<()> {
    match cmd {
        WorkCommand::Show { id } => println!("work show {id}: not yet implemented"),
        WorkCommand::List => println!("work list: not yet implemented"),
        WorkCommand::Close { id } => println!("work close {id}: not yet implemented"),
    }
    Ok(())
}
