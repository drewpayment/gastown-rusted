use std::collections::HashMap;
use std::time::Duration;

use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_common::protos::coresdk::activity_result::activity_resolution::Status;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::run_plugin::RunPluginInput;
use gtr_core::formula::{FormulaDef, interpolate};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaInput {
    pub formula_toml: String,
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaResult {
    pub name: String,
    pub steps_completed: usize,
    pub steps_total: usize,
    pub failed_step: Option<String>,
}

pub async fn formula_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let input: FormulaInput = if let Some(payload) = args.first() {
        serde_json::from_slice(&payload.data)?
    } else {
        return Err(anyhow::anyhow!("formula_wf requires FormulaInput"));
    };

    let def = FormulaDef::from_toml(&input.formula_toml)?;
    let formula_name = def.name.clone();
    let steps_total = def.steps.len();
    let sorted_steps = def.topo_sort()?;

    tracing::info!("Formula '{}' — {} steps", formula_name, sorted_steps.len());

    let mut completed = 0;

    for step in &sorted_steps {
        let command = interpolate(&step.command, &input.vars);
        let args: Vec<String> = step
            .args
            .iter()
            .map(|a| interpolate(a, &input.vars))
            .collect();

        tracing::info!("Formula '{}' step '{}': {} {:?}", formula_name, step.name, command, args);

        let plugin_input = RunPluginInput {
            plugin_name: format!("{}:{}", formula_name, step.name),
            command,
            args,
            work_dir: None,
        };

        let result = ctx
            .activity(ActivityOptions {
                activity_type: "run_plugin".to_string(),
                input: plugin_input.as_json_payload()?,
                start_to_close_timeout: Some(Duration::from_secs(300)),
                ..Default::default()
            })
            .await;

        match result.status {
            Some(Status::Completed(_)) => {
                completed += 1;
                tracing::info!("Formula '{}' step '{}' completed", formula_name, step.name);
            }
            _ => {
                tracing::warn!("Formula '{}' step '{}' failed", formula_name, step.name);
                return Ok(WfExitValue::Normal(serde_json::to_string(
                    &FormulaResult {
                        name: formula_name,
                        steps_completed: completed,
                        steps_total,
                        failed_step: Some(step.name.clone()),
                    },
                )?));
            }
        }
    }

    Ok(WfExitValue::Normal(serde_json::to_string(
        &FormulaResult {
            name: formula_name,
            steps_completed: completed,
            steps_total,
            failed_step: None,
        },
    )?))
}
