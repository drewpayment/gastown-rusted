use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum MailCommand {
    /// Show inbox
    Inbox,
    /// Send a message
    Send {
        /// Recipient agent
        to: String,
        /// Message body
        #[arg(short, long)]
        message: String,
    },
    /// Read a specific message
    Read {
        /// Message ID
        id: String,
    },
}

pub fn run(cmd: &MailCommand) -> anyhow::Result<()> {
    match cmd {
        MailCommand::Inbox => println!("mail inbox: not yet implemented"),
        MailCommand::Send { to, message } => {
            println!("mail send to {to}: {message}: not yet implemented")
        }
        MailCommand::Read { id } => println!("mail read {id}: not yet implemented"),
    }
    Ok(())
}
