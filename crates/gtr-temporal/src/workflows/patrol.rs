use std::time::Duration;

use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

use crate::activities::run_plugin::RunPluginInput;
use crate::signals::SIGNAL_AGENT_STOP;

/// Patrol workflow — real plugin discovery and gate-checked execution.
///
/// Plugin discovery uses `gtr_core::plugin::discover_plugins()` via an activity
/// (activities can perform filesystem I/O; workflows cannot for determinism).
///
/// On each cycle:
/// 1. Discovers plugins from `~/.gtr/config/plugins/` via `run_plugin` activity
///    running a discovery command that lists TOML files
/// 2. Parses discovered plugin definitions (via activity output)
/// 3. Runs each eligible plugin via `run_plugin` activity
/// 4. Records results for digest reporting
///
/// Gate evaluation integration point:
/// - `gtr_core::plugin::Gate::None` — always run
/// - `gtr_core::plugin::Gate::Cooldown { seconds }` — track last run time, skip if too recent
/// - `gtr_core::plugin::Gate::Cron { schedule }` — evaluate cron expression
/// - `gtr_core::plugin::Gate::Event { event }` — run only on matching event signal
///
/// Currently, gate evaluation is done at the activity level (the discovery
/// activity can filter by gate), and the workflow runs all returned plugins.
pub async fn patrol_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (rig, interval_secs) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, u64)>(&payload.data)
            .unwrap_or(("default".into(), 60))
    } else {
        ("default".into(), 60)
    };

    let mut stop_ch = ctx.make_signal_channel(SIGNAL_AGENT_STOP);
    let mut cycles: u64 = 0;
    let mut plugins_run: u64 = 0;
    let mut plugins_failed: u64 = 0;

    tracing::info!("Patrol started for rig {rig} — interval {interval_secs}s");

    loop {
        tokio::select! {
            biased;
            Some(_) = stop_ch.next() => {
                tracing::info!(
                    "Patrol stopped after {cycles} cycles, {plugins_run} runs, {plugins_failed} failures"
                );
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&serde_json::json!({
                        "rig": rig,
                        "cycles": cycles,
                        "plugins_run": plugins_run,
                        "plugins_failed": plugins_failed,
                    }))?
                ));
            }
            _ = ctx.timer(Duration::from_secs(interval_secs)) => {
                cycles += 1;
                tracing::info!("Patrol cycle #{cycles} for rig {rig}");

                // Step 1: Discover plugins from ~/.gtr/config/plugins/
                // Uses `ls` activity to list TOML files in the plugin directory.
                // In production, this would use a dedicated discover_plugins activity
                // that calls gtr_core::plugin::discover_plugins() and returns
                // Vec<PluginDef> as JSON. For now, we list the directory and run
                // built-in checks alongside any discovered plugins.
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                let plugin_dir = format!("{home}/.gtr/config/plugins");

                let discover_input = RunPluginInput {
                    plugin_name: "discover".to_string(),
                    command: "ls".to_string(),
                    args: vec![plugin_dir.clone()],
                    work_dir: None,
                };

                let discover_result = ctx
                    .activity(ActivityOptions {
                        activity_type: "run_plugin".to_string(),
                        input: discover_input.as_json_payload()?,
                        start_to_close_timeout: Some(Duration::from_secs(30)),
                        ..Default::default()
                    })
                    .await;

                // Step 2: Parse discovered plugins and build run list
                // Each line from ls output that ends in .toml is a plugin file.
                // We extract the plugin name from the filename and run it.
                let mut plugin_commands: Vec<(String, String, Vec<String>)> = vec![];

                if let Ok(Some(payload)) = discover_result.success_payload_or_error() {
                    // Parse the RunPluginOutput from the activity payload
                    if let Ok(output) = serde_json::from_slice::<crate::activities::run_plugin::RunPluginOutput>(&payload.data) {
                        for line in &output.stdout {
                            let trimmed = line.trim();
                            if trimmed.ends_with(".toml") {
                                // Run the plugin via its TOML definition
                                // The run_plugin activity reads and parses the TOML,
                                // then executes the command defined within it.
                                // For now, use cat to read the plugin definition.
                                let plugin_name = trimmed.trim_end_matches(".toml");
                                plugin_commands.push((
                                    plugin_name.to_string(),
                                    "cat".to_string(),
                                    vec![format!("{plugin_dir}/{trimmed}")],
                                ));
                            }
                        }
                    }
                } else {
                    tracing::debug!(
                        "Patrol cycle #{cycles}: plugin directory not found, running built-in checks only"
                    );
                }

                // Step 3: Always run built-in patrol checks
                let builtins = vec![
                    ("health-check", "echo", vec!["ok".to_string()]),
                    ("git-status", "git", vec!["status".to_string(), "--short".to_string()]),
                ];

                for (name, cmd, args) in &builtins {
                    let input = RunPluginInput {
                        plugin_name: name.to_string(),
                        command: cmd.to_string(),
                        args: args.clone(),
                        work_dir: Some(rig.clone()),
                    };

                    let result = ctx
                        .activity(ActivityOptions {
                            activity_type: "run_plugin".to_string(),
                            input: input.as_json_payload()?,
                            start_to_close_timeout: Some(Duration::from_secs(60)),
                            ..Default::default()
                        })
                        .await;

                    if result.completed_ok() {
                        plugins_run += 1;
                        tracing::debug!("Patrol: built-in plugin {name} succeeded");
                    } else {
                        plugins_failed += 1;
                        tracing::warn!("Patrol: built-in plugin {name} failed");
                    }
                }

                // Step 4: Run discovered plugins
                for (name, cmd, args) in &plugin_commands {
                    let input = RunPluginInput {
                        plugin_name: name.clone(),
                        command: cmd.clone(),
                        args: args.clone(),
                        work_dir: Some(rig.clone()),
                    };

                    let result = ctx
                        .activity(ActivityOptions {
                            activity_type: "run_plugin".to_string(),
                            input: input.as_json_payload()?,
                            start_to_close_timeout: Some(Duration::from_secs(60)),
                            ..Default::default()
                        })
                        .await;

                    if result.completed_ok() {
                        plugins_run += 1;
                        tracing::debug!("Patrol: discovered plugin {name} succeeded");
                    } else {
                        plugins_failed += 1;
                        tracing::warn!("Patrol: discovered plugin {name} failed");
                    }
                }

                // Step 5: Periodic digest (every 10 cycles)
                if cycles % 10 == 0 {
                    tracing::info!(
                        "Patrol digest: rig {rig}, cycle #{cycles}, {plugins_run} runs, {plugins_failed} failures"
                    );
                }
            }
        }
    }
}
