use clap::Args;

#[derive(Debug, Args)]
pub struct SlingCommand {
    /// Bead or formula to sling
    pub target: String,
    /// Agent to assign work to
    pub agent: Option<String>,
    /// Additional context message
    #[arg(short, long)]
    pub message: Option<String>,
}

pub fn run(cmd: &SlingCommand) -> anyhow::Result<()> {
    println!("sling {}: not yet implemented", cmd.target);
    Ok(())
}
