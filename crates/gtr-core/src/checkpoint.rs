use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const CHECKPOINT_FILE: &str = ".gtr-checkpoint.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub molecule_id: Option<String>,
    pub current_step: Option<String>,
    pub step_title: Option<String>,
    pub modified_files: Vec<String>,
    pub last_commit: Option<String>,
    pub branch: Option<String>,
    pub hooked_work: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<String>,
    pub notes: Option<String>,
}

impl Checkpoint {
    /// Write checkpoint to `.gtr-checkpoint.json` in the given directory.
    pub fn write(&self, dir: &Path) -> anyhow::Result<()> {
        let path = dir.join(CHECKPOINT_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Read checkpoint from `.gtr-checkpoint.json` in the given directory.
    pub fn read(dir: &Path) -> anyhow::Result<Option<Self>> {
        let path = dir.join(CHECKPOINT_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read_to_string(&path)?;
        let cp: Checkpoint = serde_json::from_str(&data)?;
        Ok(Some(cp))
    }

    /// Clear checkpoint file from the given directory.
    pub fn clear(dir: &Path) -> anyhow::Result<bool> {
        let path = dir.join(CHECKPOINT_FILE);
        if path.exists() {
            std::fs::remove_file(&path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cp = Checkpoint {
            molecule_id: Some("mol-123".into()),
            current_step: Some("step-2".into()),
            step_title: Some("Run tests".into()),
            modified_files: vec!["src/main.rs".into()],
            last_commit: Some("abc1234".into()),
            branch: Some("feature/x".into()),
            hooked_work: None,
            timestamp: Utc::now(),
            session_id: Some("sess-456".into()),
            notes: Some("WIP".into()),
        };

        cp.write(dir.path()).unwrap();
        let loaded = Checkpoint::read(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.molecule_id, Some("mol-123".into()));
        assert_eq!(loaded.current_step, Some("step-2".into()));
        assert_eq!(loaded.modified_files.len(), 1);

        assert!(Checkpoint::clear(dir.path()).unwrap());
        assert!(Checkpoint::read(dir.path()).unwrap().is_none());
    }
}
