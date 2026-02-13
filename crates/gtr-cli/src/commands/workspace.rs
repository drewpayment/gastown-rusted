use clap::Subcommand;
use gtr_core::config::{RigsConfig, TownConfig};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Create a new Gas Town HQ directory structure
    Install {
        /// Path for the HQ directory
        path: String,
    },
    /// Initialize current directory as a rig
    Init,
    /// Show workspace info
    Info,
}

pub fn run(cmd: &WorkspaceCommand) -> anyhow::Result<()> {
    match cmd {
        WorkspaceCommand::Install { path } => handle_install(path),
        WorkspaceCommand::Init => {
            println!("[not implemented] init");
            Ok(())
        }
        WorkspaceCommand::Info => {
            println!("[not implemented] info");
            Ok(())
        }
    }
}

fn handle_install(path: &str) -> anyhow::Result<()> {
    let root = PathBuf::from(path);
    if root.join(".gtr").exists() {
        anyhow::bail!("already initialized: {}", root.display());
    }

    let gtr_dir = root.join(".gtr");
    fs::create_dir_all(&gtr_dir)?;

    let config = TownConfig {
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "gastown".into()),
        namespace: "default".into(),
        temporal_address: "http://localhost:7233".into(),
    };
    fs::write(
        gtr_dir.join("config.toml"),
        toml::to_string_pretty(&config)?,
    )?;

    let rigs = RigsConfig { rigs: vec![] };
    fs::write(
        gtr_dir.join("rigs.toml"),
        toml::to_string_pretty(&rigs)?,
    )?;

    fs::create_dir_all(root.join("plugins"))?;

    println!("Initialized Gas Town HQ at {}", root.display());
    Ok(())
}
