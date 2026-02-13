use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum ConvoyCommand {
    /// List active convoys
    List,
    /// Show convoy details
    Show {
        /// Convoy ID
        id: String,
    },
    /// Track convoy status
    Status {
        /// Convoy ID
        id: String,
    },
}

pub fn run(cmd: &ConvoyCommand) -> anyhow::Result<()> {
    match cmd {
        ConvoyCommand::List => println!("convoy list: not yet implemented"),
        ConvoyCommand::Show { id } => println!("convoy show {id}: not yet implemented"),
        ConvoyCommand::Status { id } => println!("convoy status {id}: not yet implemented"),
    }
    Ok(())
}
