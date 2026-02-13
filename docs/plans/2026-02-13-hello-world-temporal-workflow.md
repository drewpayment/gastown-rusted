# Hello-World Temporal Workflow (hq-nn6.19) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prove Temporal SDK integration with a minimal `work_item_wf` workflow and `gtr worker run` command.

**Architecture:** Add a workflow function in `gtr-temporal` that returns a string. Wire it to a new `worker` CLI subcommand in `gtr-cli` that connects to Temporal, registers the workflow, and runs the worker. The worker listens on `gtr-task-queue`.

**Tech Stack:** Rust, temporalio-sdk (pinned rev 7ecb7c0), clap, tokio, anyhow

---

### Task 1: Add `temporalio-common` workspace dependency

The SDK example imports `temporalio_common::{worker::*, telemetry::*}` which requires adding the crate.

**Files:**
- Modify: `Cargo.toml` (workspace root, line 14-17)

**Step 1: Add temporalio-common to workspace dependencies**

In root `Cargo.toml`, add after the existing temporal deps:

```toml
temporalio-common = { git = "https://github.com/temporalio/sdk-core", rev = "7ecb7c0" }
```

**Step 2: Add temporalio-common + anyhow to gtr-temporal deps**

In `crates/gtr-temporal/Cargo.toml`, add:

```toml
temporalio-common = { workspace = true }
anyhow = "1"
```

**Step 3: Verify build**

Run: `cargo check -p gtr-temporal`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add Cargo.toml crates/gtr-temporal/Cargo.toml Cargo.lock
git commit -m "Add temporalio-common workspace dependency"
```

---

### Task 2: Implement `work_item_wf` workflow

**Files:**
- Modify: `crates/gtr-temporal/src/workflows.rs`

**Step 1: Write the workflow function**

Replace `crates/gtr-temporal/src/workflows.rs` with:

```rust
use temporalio_sdk::{WfContext, WfExitValue, WorkflowResult};

/// Hello-world workflow proving Temporal SDK integration.
/// Returns a greeting string.
pub async fn work_item_wf(_ctx: WfContext) -> WorkflowResult<String> {
    Ok(WfExitValue::Normal(
        "Hello from gtr work_item_wf".to_string(),
    ))
}
```

**Step 2: Verify build**

Run: `cargo check -p gtr-temporal`
Expected: SUCCESS

**Step 3: Commit**

```bash
git add crates/gtr-temporal/src/workflows.rs
git commit -m "Add work_item_wf hello-world workflow"
```

---

### Task 3: Export worker setup from gtr-temporal

Create a public function that builds and returns a configured Temporal worker. This keeps all Temporal wiring in gtr-temporal and gives CLI a clean API to call.

**Files:**
- Modify: `crates/gtr-temporal/src/lib.rs`

**Step 1: Add worker module with start function**

Create `crates/gtr-temporal/src/worker.rs`:

```rust
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
        .build()?;
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
        .build()?;

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
```

**Step 2: Export the worker module**

Add to `crates/gtr-temporal/src/lib.rs`:

```rust
pub mod activities;
pub mod signals;
pub mod worker;
pub mod workflows;
```

**Step 3: Verify build**

Run: `cargo check -p gtr-temporal`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add crates/gtr-temporal/src/worker.rs crates/gtr-temporal/src/lib.rs
git commit -m "Add Temporal worker setup with work_item_wf registration"
```

---

### Task 4: Add `worker` CLI subcommand

**Files:**
- Create: `crates/gtr-cli/src/commands/worker.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs` (line 8)
- Modify: `crates/gtr-cli/src/main.rs` (lines 14-44, 47-59)
- Modify: `crates/gtr-cli/Cargo.toml` (line 7)

**Step 1: Add gtr-temporal and tokio deps to gtr-cli**

In `crates/gtr-cli/Cargo.toml`, add:

```toml
gtr-temporal = { path = "../gtr-temporal" }
tokio = { workspace = true }
```

**Step 2: Create worker command module**

Create `crates/gtr-cli/src/commands/worker.rs`:

```rust
use clap::Subcommand;

/// Run a Temporal worker
#[derive(Debug, Subcommand)]
pub enum WorkerCommand {
    /// Start the worker and begin polling for tasks
    Run,
}

pub fn run(cmd: &WorkerCommand) -> anyhow::Result<()> {
    match cmd {
        WorkerCommand::Run => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(gtr_temporal::worker::run_worker())
        }
    }
}
```

**Step 3: Register in commands/mod.rs**

Add `pub mod worker;` to `crates/gtr-cli/src/commands/mod.rs`.

**Step 4: Wire into main.rs**

Add `Worker` variant to the `Command` enum:

```rust
    /// Run a Temporal worker
    #[command(subcommand)]
    Worker(commands::worker::WorkerCommand),
```

Add match arm in `main()`:

```rust
        Command::Worker(cmd) => commands::worker::run(cmd),
```

**Step 5: Verify build**

Run: `cargo check --bin gtr`
Expected: SUCCESS

**Step 6: Verify CLI help**

Run: `cargo run --bin gtr -- worker --help`
Expected: Shows `Run` subcommand

**Step 7: Commit**

```bash
git add crates/gtr-cli/Cargo.toml crates/gtr-cli/src/commands/worker.rs crates/gtr-cli/src/commands/mod.rs crates/gtr-cli/src/main.rs Cargo.lock
git commit -m "Add gtr worker run command for Temporal worker"
```

---

### Task 5: Full build verification

**Step 1: Run full workspace build**

Run: `cargo build`
Expected: All 3 crates compile successfully

**Step 2: Run all tests**

Run: `cargo test`
Expected: All existing tests pass (gtr-core tests)

**Step 3: Verify CLI works end to end**

Run: `cargo run --bin gtr -- worker run`
Expected: Prints "gtr worker listening on task queue 'gtr-task-queue' at http://localhost:7233" then either connects or errors with connection refused (no Temporal server running is fine — proves the code path executes)
