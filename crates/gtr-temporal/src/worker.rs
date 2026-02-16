use std::sync::Arc;

use anyhow::Result;
use temporalio_common::{
    telemetry::TelemetryOptions,
    worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy},
};
use temporalio_sdk::Worker;
use temporalio_sdk_core::{init_worker, ClientOptions, CoreRuntime, RuntimeOptions, Url};

use crate::activities;
use crate::workflows;

pub const DEFAULT_TASK_QUEUE: &str = "work";
pub const DEFAULT_NAMESPACE: &str = "default";
pub const DEFAULT_TARGET_URL: &str = "http://localhost:7233";

/// Start a Temporal worker that registers all gtr workflows and activities.
pub async fn run_worker() -> Result<()> {
    let telemetry_options = TelemetryOptions::builder().build();
    let runtime_options = RuntimeOptions::builder()
        .telemetry_options(telemetry_options)
        .build()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let runtime = CoreRuntime::new_assume_tokio(runtime_options)?;

    let client_opts = ClientOptions::builder()
        .target_url(Url::parse(DEFAULT_TARGET_URL)?)
        .client_name("gtr-worker".to_string())
        .client_version(env!("CARGO_PKG_VERSION").to_string())
        .identity("gtr-worker".to_string())
        .build();

    let client = client_opts.connect(DEFAULT_NAMESPACE, None).await?;

    let worker_config = WorkerConfig::builder()
        .namespace(DEFAULT_NAMESPACE)
        .task_queue(DEFAULT_TASK_QUEUE)
        .task_types(WorkerTaskTypes {
            enable_workflows: true,
            enable_remote_activities: true,
            enable_local_activities: false,
            enable_nexus: false,
        })
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: format!("gtr-{}", env!("CARGO_PKG_VERSION")),
        })
        .build()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let core_worker = init_worker(&runtime, worker_config, client)?;
    let mut worker = Worker::new_from_core(Arc::new(core_worker), DEFAULT_TASK_QUEUE);

    // Workflows
    worker.register_wf("work_item_wf", workflows::work_item::work_item_wf);
    worker.register_wf("convoy_wf", workflows::convoy::convoy_wf);
    worker.register_wf("agent_wf", workflows::agent::agent_wf);
    worker.register_wf("mayor_wf", workflows::mayor::mayor_wf);
    worker.register_wf("patrol_wf", workflows::patrol::patrol_wf);
    worker.register_wf("formula_wf", workflows::formula::formula_wf);
    worker.register_wf("refinery_wf", workflows::refinery::refinery_wf);
    worker.register_wf("witness_wf", workflows::witness::witness_wf);
    worker.register_wf("boot_wf", workflows::boot::boot_wf);
    worker.register_wf("rig_wf", workflows::rig::rig_wf);
    worker.register_wf("polecat_wf", workflows::polecat::polecat_wf);
    worker.register_wf("molecule_wf", workflows::molecule::molecule_wf);
    worker.register_wf("dog_wf", workflows::dog::dog_wf);
    worker.register_wf("gate_wf", workflows::gate::gate_wf);

    // Activities
    worker.register_activity("spawn_agent", activities::spawn_agent::spawn_agent);
    worker.register_activity("read_agent_output", activities::agent_io::read_agent_output);
    worker.register_activity("run_plugin", activities::run_plugin::run_plugin);
    worker.register_activity("git_operation", activities::git_ops::git_operation);
    worker.register_activity(
        "send_notification",
        activities::notification::send_notification,
    );
    worker.register_activity("check_agent_alive", activities::heartbeat::check_agent_alive);
    worker.register_activity("kill_agent", activities::heartbeat::kill_agent_activity);
    worker.register_activity("capture_pane", activities::heartbeat::capture_pane_activity);

    tracing::info!("gtr worker started on task queue '{DEFAULT_TASK_QUEUE}'");
    worker.run().await?;
    Ok(())
}
