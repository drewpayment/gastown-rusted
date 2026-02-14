use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaDef {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub vars: Vec<String>,
    pub steps: Vec<FormulaStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaStep {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl FormulaDef {
    pub fn from_toml(content: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(content)?)
    }

    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml(&content)
    }

    /// Returns steps in topological order. Errors if there's a cycle.
    pub fn topo_sort(&self) -> anyhow::Result<Vec<&FormulaStep>> {
        let name_to_idx: HashMap<&str, usize> = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.as_str(), i))
            .collect();

        let n = self.steps.len();
        let mut in_degree = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![vec![]; n];

        for (i, step) in self.steps.iter().enumerate() {
            for dep in &step.depends_on {
                let j = *name_to_idx
                    .get(dep.as_str())
                    .ok_or_else(|| anyhow::anyhow!("unknown dependency: {dep}"))?;
                adj[j].push(i);
                in_degree[i] += 1;
            }
        }

        let mut queue: Vec<usize> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, &d)| d == 0)
            .map(|(i, _)| i)
            .collect();

        let mut order = Vec::with_capacity(n);

        while let Some(idx) = queue.pop() {
            order.push(&self.steps[idx]);
            for &next in &adj[idx] {
                in_degree[next] -= 1;
                if in_degree[next] == 0 {
                    queue.push(next);
                }
            }
        }

        if order.len() != n {
            anyhow::bail!("cycle detected in formula steps");
        }

        Ok(order)
    }
}

/// Interpolate `{{var_name}}` in a string with provided variables.
pub fn interpolate(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{{{key}}}}}"), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_formula_toml() {
        let toml_str = r#"
name = "deploy"
description = "Deploy the app"
vars = ["env", "version"]

[[steps]]
name = "build"
command = "cargo"
args = ["build", "--release"]

[[steps]]
name = "test"
command = "cargo"
args = ["test"]
depends_on = ["build"]

[[steps]]
name = "deploy"
command = "deploy.sh"
args = ["{{env}}", "{{version}}"]
depends_on = ["test"]
"#;
        let def = FormulaDef::from_toml(toml_str).unwrap();
        assert_eq!(def.name, "deploy");
        assert_eq!(def.steps.len(), 3);
        assert_eq!(def.steps[2].depends_on, vec!["test"]);
    }

    #[test]
    fn topo_sort_linear() {
        let def = FormulaDef {
            name: "test".into(),
            description: None,
            vars: vec![],
            steps: vec![
                FormulaStep { name: "a".into(), command: "echo".into(), args: vec![], depends_on: vec![] },
                FormulaStep { name: "b".into(), command: "echo".into(), args: vec![], depends_on: vec!["a".into()] },
                FormulaStep { name: "c".into(), command: "echo".into(), args: vec![], depends_on: vec!["b".into()] },
            ],
        };
        let sorted = def.topo_sort().unwrap();
        assert_eq!(sorted[0].name, "a");
        assert_eq!(sorted[1].name, "b");
        assert_eq!(sorted[2].name, "c");
    }

    #[test]
    fn topo_sort_diamond() {
        let def = FormulaDef {
            name: "test".into(),
            description: None,
            vars: vec![],
            steps: vec![
                FormulaStep { name: "a".into(), command: "echo".into(), args: vec![], depends_on: vec![] },
                FormulaStep { name: "b".into(), command: "echo".into(), args: vec![], depends_on: vec!["a".into()] },
                FormulaStep { name: "c".into(), command: "echo".into(), args: vec![], depends_on: vec!["a".into()] },
                FormulaStep { name: "d".into(), command: "echo".into(), args: vec![], depends_on: vec!["b".into(), "c".into()] },
            ],
        };
        let sorted = def.topo_sort().unwrap();
        assert_eq!(sorted[0].name, "a");
        assert_eq!(sorted[3].name, "d");
    }

    #[test]
    fn topo_sort_cycle() {
        let def = FormulaDef {
            name: "test".into(),
            description: None,
            vars: vec![],
            steps: vec![
                FormulaStep { name: "a".into(), command: "echo".into(), args: vec![], depends_on: vec!["b".into()] },
                FormulaStep { name: "b".into(), command: "echo".into(), args: vec![], depends_on: vec!["a".into()] },
            ],
        };
        assert!(def.topo_sort().is_err());
    }

    #[test]
    fn interpolation() {
        let mut vars = HashMap::new();
        vars.insert("env".into(), "prod".into());
        vars.insert("version".into(), "1.2.3".into());

        assert_eq!(interpolate("deploy {{env}} v{{version}}", &vars), "deploy prod v1.2.3");
        assert_eq!(interpolate("no vars here", &vars), "no vars here");
    }
}
