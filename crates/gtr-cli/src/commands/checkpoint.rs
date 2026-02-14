use std::path::PathBuf;

use clap::Subcommand;
use gtr_core::checkpoint::Checkpoint;

#[derive(Debug, Subcommand)]
pub enum CheckpointCommand {
    /// Write a checkpoint capturing current state
    Write {
        /// Directory to write checkpoint in (default: current dir)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Molecule ID
        #[arg(long)]
        molecule: Option<String>,
        /// Current step
        #[arg(long)]
        step: Option<String>,
        /// Step title
        #[arg(long)]
        title: Option<String>,
        /// Branch name
        #[arg(long)]
        branch: Option<String>,
        /// Last commit hash
        #[arg(long)]
        commit: Option<String>,
        /// Hooked work item ID
        #[arg(long)]
        hooked: Option<String>,
        /// Session ID
        #[arg(long)]
        session: Option<String>,
        /// Notes
        #[arg(long)]
        notes: Option<String>,
    },
    /// Read the current checkpoint
    Read {
        /// Directory to read checkpoint from (default: current dir)
        #[arg(short, long)]
        dir: Option<PathBuf>,
    },
    /// Clear the checkpoint file
    Clear {
        /// Directory to clear checkpoint from (default: current dir)
        #[arg(short, long)]
        dir: Option<PathBuf>,
    },
}

pub async fn run(cmd: &CheckpointCommand) -> anyhow::Result<()> {
    match cmd {
        CheckpointCommand::Write {
            dir,
            molecule,
            step,
            title,
            branch,
            commit,
            hooked,
            session,
            notes,
        } => {
            let dir = dir.clone().unwrap_or_else(|| PathBuf::from("."));
            let cp = Checkpoint {
                molecule_id: molecule.clone(),
                current_step: step.clone(),
                step_title: title.clone(),
                modified_files: vec![],
                last_commit: commit.clone(),
                branch: branch.clone(),
                hooked_work: hooked.clone(),
                timestamp: chrono::Utc::now(),
                session_id: session.clone(),
                notes: notes.clone(),
            };
            cp.write(&dir)?;
            println!("Checkpoint written to {}", dir.join(".gtr-checkpoint.json").display());
            Ok(())
        }
        CheckpointCommand::Read { dir } => {
            let dir = dir.clone().unwrap_or_else(|| PathBuf::from("."));
            match Checkpoint::read(&dir)? {
                Some(cp) => {
                    println!("Checkpoint ({})", cp.timestamp.format("%Y-%m-%d %H:%M:%S"));
                    if let Some(mol) = &cp.molecule_id {
                        println!("  Molecule: {mol}");
                    }
                    if let Some(step) = &cp.current_step {
                        print!("  Step: {step}");
                        if let Some(title) = &cp.step_title {
                            print!(" — {title}");
                        }
                        println!();
                    }
                    if let Some(branch) = &cp.branch {
                        println!("  Branch: {branch}");
                    }
                    if let Some(commit) = &cp.last_commit {
                        println!("  Commit: {commit}");
                    }
                    if let Some(hooked) = &cp.hooked_work {
                        println!("  Hooked: {hooked}");
                    }
                    if let Some(session) = &cp.session_id {
                        println!("  Session: {session}");
                    }
                    if !cp.modified_files.is_empty() {
                        println!("  Modified: {}", cp.modified_files.join(", "));
                    }
                    if let Some(notes) = &cp.notes {
                        println!("  Notes: {notes}");
                    }
                }
                None => {
                    println!("No checkpoint found in {}", dir.display());
                }
            }
            Ok(())
        }
        CheckpointCommand::Clear { dir } => {
            let dir = dir.clone().unwrap_or_else(|| PathBuf::from("."));
            if Checkpoint::clear(&dir)? {
                println!("Checkpoint cleared");
            } else {
                println!("No checkpoint to clear");
            }
            Ok(())
        }
    }
}
