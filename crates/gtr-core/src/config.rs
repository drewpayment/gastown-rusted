use crate::types::AgentRuntime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TownConfig {
    pub name: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default = "default_temporal_address")]
    pub temporal_address: String,
}

fn default_namespace() -> String {
    "default".into()
}

fn default_temporal_address() -> String {
    "http://localhost:7233".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigEntry {
    pub name: String,
    pub path: PathBuf,
    pub git_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigsConfig {
    pub rigs: Vec<RigEntry>,
}

impl RigsConfig {
    /// Load from a specific path; returns empty config if file doesn't exist.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(RigsConfig { rigs: vec![] });
        }
        let content = std::fs::read_to_string(path)?;
        let config: RigsConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load from the default location (~/.gtr/config/rigs.toml).
    pub fn load() -> anyhow::Result<Self> {
        let path = crate::dirs::config_dir().join("rigs.toml");
        Self::load_from(&path)
    }

    /// Save to a specific path.
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Save to the default location (~/.gtr/config/rigs.toml).
    pub fn save(&self) -> anyhow::Result<()> {
        let path = crate::dirs::config_dir().join("rigs.toml");
        self.save_to(&path)
    }

    /// Add a rig entry. Idempotent — skips if name already exists.
    pub fn add(&mut self, name: &str, git_url: &str) {
        if self.rigs.iter().any(|r| r.name == name) {
            return;
        }
        self.rigs.push(RigEntry {
            name: name.to_string(),
            path: crate::dirs::rig_dir(name),
            git_url: Some(git_url.to_string()),
        });
    }

    /// Remove a rig entry by name.
    pub fn remove(&mut self, name: &str) {
        self.rigs.retain(|r| r.name != name);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigConfig {
    pub name: String,
    #[serde(default)]
    pub default_runtime: Option<AgentRuntime>,
    #[serde(default)]
    pub agents: HashMap<String, AgentRuntimeOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeOverride {
    pub runtime: AgentRuntime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationConfig {
    pub routes: HashMap<String, Vec<String>>,
    pub thresholds: EscalationThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationThresholds {
    pub stale_after: String,
    #[serde(default = "default_max_re_escalations")]
    pub max_re_escalations: u32,
}

fn default_max_re_escalations() -> u32 {
    2
}

/// Resolve the town root directory. Walks up from `start` looking for `.gtr/config.toml`.
pub fn find_town_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".gtr").join("config.toml").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Load and parse a TOML config file.
pub fn load_config<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let content = std::fs::read_to_string(path)?;
    let config: T = toml::from_str(&content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_town_config() {
        let toml_str = r#"
name = "my-town"
namespace = "gastown"
temporal_address = "http://localhost:7233"
"#;
        let config: TownConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.name, "my-town");
        assert_eq!(config.namespace, "gastown");
    }

    #[test]
    fn town_config_defaults() {
        let toml_str = r#"name = "my-town""#;
        let config: TownConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.namespace, "default");
        assert_eq!(config.temporal_address, "http://localhost:7233");
    }

    #[test]
    fn parse_escalation_config() {
        let toml_str = r#"
[routes]
critical = ["signal:mayor", "activity:email", "activity:sms"]
high = ["signal:mayor", "activity:email"]
medium = ["signal:mayor"]
low = []

[thresholds]
stale_after = "4h"
max_re_escalations = 2
"#;
        let config: EscalationConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.routes["critical"].len(), 3);
        assert_eq!(config.thresholds.stale_after, "4h");
    }

    #[test]
    fn find_town_root_walks_up() {
        let dir = tempdir().unwrap();
        let gtr_dir = dir.path().join(".gtr");
        fs::create_dir_all(&gtr_dir).unwrap();
        fs::write(gtr_dir.join("config.toml"), "name = \"test\"").unwrap();

        let nested = dir.path().join("some").join("nested").join("dir");
        fs::create_dir_all(&nested).unwrap();

        let found = find_town_root(&nested).unwrap();
        assert_eq!(found, dir.path());
    }

    #[test]
    fn find_town_root_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        assert!(find_town_root(dir.path()).is_none());
    }

    #[test]
    fn rigs_config_load_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rigs.toml");
        let config = RigsConfig::load_from(&path).unwrap();
        assert!(config.rigs.is_empty());
    }

    #[test]
    fn rigs_config_add_and_save() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rigs.toml");
        let mut config = RigsConfig::load_from(&path).unwrap();
        config.add("myrig", "git@github.com:user/repo.git");
        config.save_to(&path).unwrap();

        let reloaded = RigsConfig::load_from(&path).unwrap();
        assert_eq!(reloaded.rigs.len(), 1);
        assert_eq!(reloaded.rigs[0].name, "myrig");
        assert_eq!(
            reloaded.rigs[0].git_url,
            Some("git@github.com:user/repo.git".into())
        );
    }

    #[test]
    fn rigs_config_add_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rigs.toml");
        let mut config = RigsConfig::load_from(&path).unwrap();
        config.add("myrig", "git@github.com:user/repo.git");
        config.add("myrig", "git@github.com:user/repo.git");
        assert_eq!(config.rigs.len(), 1);
    }

    #[test]
    fn rigs_config_remove() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rigs.toml");
        let mut config = RigsConfig::load_from(&path).unwrap();
        config.add("rig-a", "git@github.com:user/a.git");
        config.add("rig-b", "git@github.com:user/b.git");
        config.remove("rig-a");
        config.save_to(&path).unwrap();

        let reloaded = RigsConfig::load_from(&path).unwrap();
        assert_eq!(reloaded.rigs.len(), 1);
        assert_eq!(reloaded.rigs[0].name, "rig-b");
    }
}
