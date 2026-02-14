use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum ServicesCommand {
    /// Start services
    Up,
    /// Stop services
    Down,
    /// Show service status
    Status,
}

pub fn run(cmd: &ServicesCommand) -> anyhow::Result<()> {
    match cmd {
        ServicesCommand::Up => println!("services up: not yet implemented"),
        ServicesCommand::Down => println!("services down: not yet implemented"),
        ServicesCommand::Status => println!("services status: not yet implemented"),
    }
    Ok(())
}
