use std::path::PathBuf;

/// Root GTR directory (~/.gtr)
pub fn gtr_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".gtr")
}

/// Runtime directory for live process state
pub fn runtime_dir() -> PathBuf {
    gtr_root().join("runtime")
}

/// Rigs directory
pub fn rigs_dir() -> PathBuf {
    gtr_root().join("rigs")
}

/// A specific rig's directory
pub fn rig_dir(rig: &str) -> PathBuf {
    rigs_dir().join(rig)
}

/// Polecat work directory within a rig
pub fn polecat_dir(rig: &str, name: &str) -> PathBuf {
    rig_dir(rig).join("polecats").join(name)
}

/// Crew workspace directory within a rig
pub fn crew_dir(rig: &str, name: &str) -> PathBuf {
    rig_dir(rig).join("crew").join(name)
}

/// Witness work directory within a rig
pub fn witness_dir(rig: &str) -> PathBuf {
    rig_dir(rig).join("witness")
}

/// Refinery work directory within a rig
pub fn refinery_dir(rig: &str) -> PathBuf {
    rig_dir(rig).join("refinery")
}

/// Config directory
pub fn config_dir() -> PathBuf {
    gtr_root().join("config")
}

/// Ensure all directories for a rig exist
pub fn ensure_rig_dirs(rig: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(rig_dir(rig).join("polecats"))?;
    std::fs::create_dir_all(rig_dir(rig).join("crew"))?;
    std::fs::create_dir_all(witness_dir(rig))?;
    std::fs::create_dir_all(refinery_dir(rig))?;
    Ok(())
}

/// Ensure the base GTR directory structure exists
pub fn ensure_base_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(runtime_dir())?;
    std::fs::create_dir_all(rigs_dir())?;
    std::fs::create_dir_all(config_dir())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polecat_dir_structure() {
        let dir = polecat_dir("gtr", "furiosa");
        assert!(dir.to_string_lossy().contains(".gtr/rigs/gtr/polecats/furiosa"));
    }

    #[test]
    fn crew_dir_structure() {
        let dir = crew_dir("gtr", "drew");
        assert!(dir.to_string_lossy().contains(".gtr/rigs/gtr/crew/drew"));
    }
}
