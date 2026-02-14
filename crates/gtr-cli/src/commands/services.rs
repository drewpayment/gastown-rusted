use std::sync::Arc;

use clap::Subcommand;
use temporalio_common::{
    telemetry::TelemetryOptions,
    worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy},
};
use temporalio_sdk::Worker;
use temporalio_sdk_core::{ClientOptions, CoreRuntime, RuntimeOptions, Url, init_worker};

#[derive(Debug, Subcommand)]
pub enum ServicesCommand {
    /// Start services
    Up,
    /// Stop services
    Down,
    /// Show service status
    Status,
}

pub fn run(cmd: &ServicesCommand) -> anyhow::Result<()> {
    match cmd {
        ServicesCommand::Up => println!("services up: not yet implemented"),
        ServicesCommand::Down => println!("services down: not yet implemented"),
        ServicesCommand::Status => println!("services status: not yet implemented"),
    }
    Ok(())
}

pub async fn run_worker() -> anyhow::Result<()> {
    let telemetry_options = TelemetryOptions::builder().build();
    let runtime_options = RuntimeOptions::builder()
        .telemetry_options(telemetry_options)
        .build()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let runtime = CoreRuntime::new_assume_tokio(runtime_options)?;

    let client_opts = ClientOptions::builder()
        .target_url(Url::parse("http://localhost:7233")?)
        .client_name("gtr-worker".to_string())
        .client_version(env!("CARGO_PKG_VERSION").to_string())
        .identity("gtr-worker".to_string())
        .build();

    let client = client_opts.connect("default", None).await?;

    let worker_config = WorkerConfig::builder()
        .namespace("default")
        .task_queue("work")
        .task_types(WorkerTaskTypes::workflow_only())
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: format!("gtr-{}", env!("CARGO_PKG_VERSION")),
        })
        .build()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let core_worker = init_worker(&runtime, worker_config, client)?;
    let mut worker = Worker::new_from_core(Arc::new(core_worker), "work");

    worker.register_wf(
        "work_item_wf",
        gtr_temporal::workflows::work_item::work_item_wf,
    );
    worker.register_wf(
        "convoy_wf",
        gtr_temporal::workflows::convoy::convoy_wf,
    );

    tracing::info!("gtr worker started on task queue 'work'");
    worker.run().await?;
    Ok(())
}
