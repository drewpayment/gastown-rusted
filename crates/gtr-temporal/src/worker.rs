use std::{str::FromStr, sync::Arc};

use anyhow::Result;
use temporalio_common::{
    telemetry::TelemetryOptions,
    worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy},
};
use temporalio_sdk::{sdk_client_options, Worker};
use temporalio_sdk_core::{init_worker, CoreRuntime, RuntimeOptions, Url};

use crate::workflows::work_item_wf;

const DEFAULT_TASK_QUEUE: &str = "gtr-task-queue";
const DEFAULT_NAMESPACE: &str = "default";
const DEFAULT_TARGET_URL: &str = "http://localhost:7233";

/// Start a Temporal worker that registers all gtr workflows and activities.
pub async fn run_worker() -> Result<()> {
    let server_options =
        sdk_client_options(Url::from_str(DEFAULT_TARGET_URL)?).build();

    let telemetry_options = TelemetryOptions::builder().build();
    let runtime_options = RuntimeOptions::builder()
        .telemetry_options(telemetry_options)
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;
    let runtime = CoreRuntime::new_assume_tokio(runtime_options)?;

    let client = server_options
        .connect(DEFAULT_NAMESPACE, None)
        .await?;

    let worker_config = WorkerConfig::builder()
        .namespace(DEFAULT_NAMESPACE)
        .task_queue(DEFAULT_TASK_QUEUE)
        .task_types(WorkerTaskTypes::all())
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: format!("gtr-{}", env!("CARGO_PKG_VERSION")),
        })
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;

    let core_worker = init_worker(&runtime, worker_config, client)?;

    let mut worker =
        Worker::new_from_core(Arc::new(core_worker), DEFAULT_TASK_QUEUE);

    worker.register_wf("work_item_wf", work_item_wf);

    eprintln!(
        "gtr worker listening on task queue '{}' at {}",
        DEFAULT_TASK_QUEUE, DEFAULT_TARGET_URL
    );

    worker.run().await?;
    Ok(())
}
