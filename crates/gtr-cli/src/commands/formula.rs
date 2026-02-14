use std::collections::HashMap;

use clap::Subcommand;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk_core::WorkflowClientTrait;

use gtr_temporal::workflows::formula::FormulaInput;

#[derive(Debug, Subcommand)]
pub enum FormulaCommand {
    /// Execute a formula
    Cook {
        /// Path to formula TOML file
        path: String,
        /// Variables in key=value format
        #[arg(short, long, value_parser = parse_var)]
        var: Vec<(String, String)>,
    },
}

fn parse_var(s: &str) -> Result<(String, String), String> {
    let (key, value) = s
        .split_once('=')
        .ok_or_else(|| format!("expected key=value, got: {s}"))?;
    Ok((key.to_string(), value.to_string()))
}

pub async fn run(cmd: &FormulaCommand) -> anyhow::Result<()> {
    match cmd {
        FormulaCommand::Cook { path, var } => handle_cook(path, var).await,
    }
}

async fn handle_cook(path: &str, vars: &[(String, String)]) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)?;

    // Validate it parses before sending to workflow
    let def = gtr_core::formula::FormulaDef::from_toml(&content)?;
    def.topo_sort()?;

    let vars_map: HashMap<String, String> = vars.iter().cloned().collect();

    let input = FormulaInput {
        formula_toml: content,
        vars: vars_map,
    };

    let client = crate::client::connect().await?;
    let wf_id = format!("formula-{}-{}", def.name, gtr_core::ids::work_item_id());

    let payload = input.as_json_payload()?;
    client
        .start_workflow(
            vec![payload],
            "work".to_string(),
            wf_id.clone(),
            "formula_wf".to_string(),
            None,
            Default::default(),
        )
        .await?;

    println!("Cooking formula '{}' — {} steps", def.name, def.steps.len());
    println!("Workflow: {wf_id}");
    Ok(())
}
