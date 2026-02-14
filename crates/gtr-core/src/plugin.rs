use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDef {
    pub name: String,
    pub description: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub gate: Gate,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Gate {
    #[default]
    None,
    Cooldown {
        seconds: u64,
    },
    Cron {
        schedule: String,
    },
    Event {
        event: String,
    },
}

pub fn discover_plugins(dir: &Path) -> anyhow::Result<Vec<(PathBuf, PluginDef)>> {
    let mut plugins = Vec::new();

    if !dir.exists() {
        return Ok(plugins);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            let content = std::fs::read_to_string(&path)?;
            match toml::from_str::<PluginDef>(&content) {
                Ok(def) => plugins.push((path, def)),
                Err(e) => {
                    eprintln!("warning: skipping invalid plugin {:?}: {e}", path);
                }
            }
        }
    }

    plugins.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    Ok(plugins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plugin_toml() {
        let toml_str = r#"
name = "hello"
description = "A test plugin"
command = "echo"
args = ["hello", "world"]

[gate]
type = "cooldown"
seconds = 60
"#;
        let def: PluginDef = toml::from_str(toml_str).unwrap();
        assert_eq!(def.name, "hello");
        assert_eq!(def.command, "echo");
        assert_eq!(def.args, vec!["hello", "world"]);
        assert!(matches!(def.gate, Gate::Cooldown { seconds: 60 }));
    }

    #[test]
    fn parse_plugin_no_gate() {
        let toml_str = r#"
name = "simple"
command = "ls"
args = ["-la"]
"#;
        let def: PluginDef = toml::from_str(toml_str).unwrap();
        assert_eq!(def.name, "simple");
        assert!(matches!(def.gate, Gate::None));
    }

    #[test]
    fn parse_cron_gate() {
        let toml_str = r#"
name = "cron-test"
command = "date"
args = []

[gate]
type = "cron"
schedule = "*/5 * * * *"
"#;
        let def: PluginDef = toml::from_str(toml_str).unwrap();
        assert!(matches!(def.gate, Gate::Cron { .. }));
    }

    #[test]
    fn discover_plugins_in_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("hello.toml"),
            r#"name = "hello"
command = "echo"
args = ["hi"]
"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("world.toml"),
            r#"name = "world"
command = "echo"
args = ["world"]
"#,
        )
        .unwrap();
        // Non-toml file should be ignored
        std::fs::write(dir.path().join("readme.txt"), "not a plugin").unwrap();

        let plugins = discover_plugins(dir.path()).unwrap();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].1.name, "hello");
        assert_eq!(plugins[1].1.name, "world");
    }
}
