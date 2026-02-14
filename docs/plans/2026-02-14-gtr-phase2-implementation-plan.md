# GTR Phase 2 Implementation Plan — Feature Parity with Gas Town

**STATUS: COMPLETE** — All 21 tasks (30-50) implemented and merged to main on 2026-02-14.

**Goal:** Close the gap between our Rust+Temporal rewrite and the original Go Gas Town (`https://github.com/steveyegge/gastown`), replacing tmux/beads/filesystem state with Temporal workflows.

**Architecture:** Temporal workflows replace all durable state (agent lifecycle, work assignment, rig status, mail). CLI commands signal workflows or query Temporal. Filesystem is used only for git worktrees and config files. The Go daemon's heartbeat loop becomes the Boot/Patrol workflows with real monitoring logic.

**Tech Stack:** Rust 2021, Temporal SDK rev `7ecb7c0`, clap 4, tokio, serde, git2, chrono, crossterm

**Phase 1 reference:** `docs/plans/2026-02-12-gtr-implementation-plan.md` (Tasks 1-29, complete)

**Codebase root:** `/Users/drew.payment/dev/gt/gtr/crew/drew`

**Test count:** 30 (25 gtr-core + 5 gtr-temporal)

---

## Priority Order

All tasks complete:

1. **Housekeeping** (Tasks 30-31): Fix worker mismatch, wire core types into workflows
2. **Rig & Crew** (Tasks 32-34): Rig lifecycle workflow, crew workspaces, HQ directory structure
3. **Polecat Lifecycle** (Tasks 35-37): Name pools, polecat workflow, spawn/done/nuke
4. **Hook & Session** (Tasks 38-39): Hook durability, prime/handoff context recovery
5. **Molecules** (Tasks 40-41): Instantiated formula tracking, step-by-step execution
6. **Dogs** (Tasks 42): Reusable cross-rig infrastructure workers
7. **Sling v2** (Tasks 43): Full dispatch — batch, formula-on-bead, auto-convoy, multi-runtime
8. **Gates** (Tasks 44): Async wait primitives — timer, human, mail
9. **Mail v2** (Tasks 45): Inbox read/reply/thread, search, DND
10. **Monitoring v2** (Tasks 46-47): Real witness/patrol logic, deacon heartbeat
11. **Refinery v2** (Tasks 48): Conflict-resolution polecats, real git rebase+test
12. **Diagnostics v2** (Tasks 49-50): Feed TUI, checkpoint, session

---

## Task 30: Fix Worker Registration Mismatch

The `gtr worker run` command calls `gtr_temporal::worker::run_worker()` which only registers `work_item_wf` on task queue `"gtr-task-queue"`. The full registration (9 workflows, 5 activities) lives in `services.rs::run_worker()` on task queue `"work"` but is unreachable from CLI.

**Files:**
- Modify: `crates/gtr-temporal/src/worker.rs`
- Modify: `crates/gtr-cli/src/commands/worker.rs`

**Step 1: Replace worker.rs with a call to the full registration**

Replace `crates/gtr-temporal/src/worker.rs` entirely. Move the full worker setup from `services.rs::run_worker()` into `gtr-temporal/src/worker.rs` so it's the single source of truth. Both `worker.rs` and `services.rs` should call the same function.

```rust
// crates/gtr-temporal/src/worker.rs
use std::sync::Arc;
use anyhow::Result;
use temporalio_common::{
    telemetry::TelemetryOptions,
    worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy},
};
use temporalio_sdk::Worker;
use temporalio_sdk_core::{ClientOptions, CoreRuntime, RuntimeOptions, Url, init_worker};

use crate::workflows;
use crate::activities;

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

    // Activities
    worker.register_activity("spawn_agent", activities::spawn_agent::spawn_agent);
    worker.register_activity("read_agent_output", activities::agent_io::read_agent_output);
    worker.register_activity("run_plugin", activities::run_plugin::run_plugin);
    worker.register_activity("git_operation", activities::git_ops::git_operation);
    worker.register_activity("send_notification", activities::notification::send_notification);

    tracing::info!("gtr worker started on task queue '{DEFAULT_TASK_QUEUE}'");
    worker.run().await?;
    Ok(())
}
```

**Step 2: Update CLI worker command to call the canonical worker**

```rust
// crates/gtr-cli/src/commands/worker.rs
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum WorkerCommand {
    /// Run the Temporal worker
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

**Step 3: Simplify services.rs — remove duplicate worker code**

Remove `run_worker()` from `services.rs`. Keep only the `ServicesCommand` enum.

**Step 4: Verify**

Run: `cargo build`
Expected: Compiles

Run: `cargo test`
Expected: All tests pass

**Step 5: Commit**

```bash
git commit -m "fix: consolidate worker registration — single source of truth in gtr-temporal"
```

---

## Task 31: Wire Core Types into Workflows

Our `gtr-core/src/types.rs` defines strong types (`WorkItemId`, `AgentRole`, `WorkItemStatus`, etc.) but workflows use raw strings everywhere. This task adds a shared state module that workflows use.

**Files:**
- Create: `crates/gtr-core/src/state.rs`
- Modify: `crates/gtr-core/src/lib.rs`

**Step 1: Create state types that bridge core types and workflow serialization**

```rust
// crates/gtr-core/src/state.rs
use serde::{Deserialize, Serialize};

/// Canonical status strings used across all workflows.
/// These are the wire-format values that Temporal signals carry.
pub mod status {
    pub const PENDING: &str = "pending";
    pub const ASSIGNED: &str = "assigned";
    pub const IN_PROGRESS: &str = "in_progress";
    pub const DONE: &str = "done";
    pub const FAILED: &str = "failed";
    pub const CLOSED: &str = "closed";
    pub const IDLE: &str = "idle";
    pub const WORKING: &str = "working";
    pub const STOPPED: &str = "stopped";
    pub const OPEN: &str = "open";
    pub const QUEUED: &str = "queued";
    pub const VALIDATING: &str = "validating";
    pub const MERGING: &str = "merging";
    pub const MERGED: &str = "merged";
}

/// Canonical agent roles.
pub mod roles {
    pub const MAYOR: &str = "mayor";
    pub const DEACON: &str = "deacon";
    pub const WITNESS: &str = "witness";
    pub const REFINERY: &str = "refinery";
    pub const POLECAT: &str = "polecat";
    pub const CREW: &str = "crew";
    pub const DOG: &str = "dog";
    pub const BOOT: &str = "boot";
}

/// Workflow ID conventions — used to ensure singleton workflows
/// have deterministic IDs.
pub fn mayor_workflow_id() -> String {
    "mayor".to_string()
}

pub fn witness_workflow_id(rig: &str) -> String {
    format!("{rig}-witness")
}

pub fn refinery_workflow_id(rig: &str) -> String {
    format!("{rig}-refinery")
}

pub fn patrol_workflow_id() -> String {
    "patrol".to_string()
}

pub fn boot_workflow_id() -> String {
    "boot".to_string()
}

pub fn polecat_workflow_id(rig: &str, name: &str) -> String {
    format!("{rig}-polecat-{name}")
}

pub fn dog_workflow_id(name: &str) -> String {
    format!("dog-{name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_id_conventions() {
        assert_eq!(mayor_workflow_id(), "mayor");
        assert_eq!(witness_workflow_id("gt"), "gt-witness");
        assert_eq!(refinery_workflow_id("gt"), "gt-refinery");
        assert_eq!(polecat_workflow_id("gt", "nux"), "gt-polecat-nux");
        assert_eq!(dog_workflow_id("alpha"), "dog-alpha");
    }
}
```

**Step 2: Add to lib.rs**

Add `pub mod state;` to `crates/gtr-core/src/lib.rs`.

**Step 3: Verify**

Run: `cargo test -p gtr-core`
Expected: New test passes along with existing 22

**Step 4: Commit**

```bash
git commit -m "feat: add state module — canonical status strings, workflow ID conventions"
```

---

## Task 32: Rig Workflow

A rig is a registered git repository that agents work on. In the original, rigs have a lifecycle (operational/parked/docked) and track their agents. We model this as a long-running Temporal workflow.

**Files:**
- Create: `crates/gtr-temporal/src/workflows/rig.rs`
- Modify: `crates/gtr-temporal/src/workflows/mod.rs`
- Modify: `crates/gtr-temporal/src/signals.rs`
- Modify: `crates/gtr-temporal/src/worker.rs` (register new workflow)

**Step 1: Add rig signals to signals.rs**

```rust
// Rig signal names
pub const SIGNAL_RIG_PARK: &str = "rig_park";
pub const SIGNAL_RIG_UNPARK: &str = "rig_unpark";
pub const SIGNAL_RIG_DOCK: &str = "rig_dock";
pub const SIGNAL_RIG_UNDOCK: &str = "rig_undock";
pub const SIGNAL_RIG_REGISTER_AGENT: &str = "rig_register_agent";
pub const SIGNAL_RIG_UNREGISTER_AGENT: &str = "rig_unregister_agent";
pub const SIGNAL_RIG_STOP: &str = "rig_stop";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigAgentEntry {
    pub agent_id: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigState {
    pub name: String,
    pub git_url: String,
    pub status: String, // "operational", "parked", "docked"
    pub agents: Vec<RigAgentEntry>,
    pub polecats: Vec<String>,
    pub crew: Vec<String>,
    pub has_witness: bool,
    pub has_refinery: bool,
}
```

**Step 2: Create rig workflow**

```rust
// crates/gtr-temporal/src/workflows/rig.rs
use futures_util::StreamExt;
use temporalio_sdk::{WfContext, WfExitValue};
use crate::signals::*;

pub async fn rig_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (name, git_url) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "".into()))
    } else {
        ("unknown".into(), "".into())
    };

    let mut status = "operational".to_string();
    let mut agents: Vec<RigAgentEntry> = vec![];
    let mut polecats: Vec<String> = vec![];
    let mut crew: Vec<String> = vec![];
    let mut has_witness = false;
    let mut has_refinery = false;

    let mut park_ch = ctx.make_signal_channel(SIGNAL_RIG_PARK);
    let mut unpark_ch = ctx.make_signal_channel(SIGNAL_RIG_UNPARK);
    let mut dock_ch = ctx.make_signal_channel(SIGNAL_RIG_DOCK);
    let mut undock_ch = ctx.make_signal_channel(SIGNAL_RIG_UNDOCK);
    let mut reg_ch = ctx.make_signal_channel(SIGNAL_RIG_REGISTER_AGENT);
    let mut unreg_ch = ctx.make_signal_channel(SIGNAL_RIG_UNREGISTER_AGENT);
    let mut stop_ch = ctx.make_signal_channel(SIGNAL_RIG_STOP);

    tracing::info!("Rig {name} started — operational");

    loop {
        tokio::select! {
            biased;
            Some(_) = stop_ch.next() => {
                tracing::info!("Rig {name} stopped");
                return Ok(WfExitValue::Normal(serde_json::to_string(&RigState {
                    name, git_url, status, agents, polecats, crew, has_witness, has_refinery,
                })?));
            }
            Some(_) = park_ch.next() => {
                if status == "operational" {
                    status = "parked".to_string();
                    tracing::info!("Rig {name} parked");
                }
            }
            Some(_) = unpark_ch.next() => {
                if status == "parked" {
                    status = "operational".to_string();
                    tracing::info!("Rig {name} unparked");
                }
            }
            Some(_) = dock_ch.next() => {
                status = "docked".to_string();
                tracing::info!("Rig {name} docked");
            }
            Some(_) = undock_ch.next() => {
                if status == "docked" {
                    status = "operational".to_string();
                    tracing::info!("Rig {name} undocked");
                }
            }
            Some(signal) = reg_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<RigAgentEntry>(&payload.data) {
                        match data.role.as_str() {
                            "witness" => has_witness = true,
                            "refinery" => has_refinery = true,
                            "polecat" => polecats.push(data.agent_id.clone()),
                            "crew" => crew.push(data.agent_id.clone()),
                            _ => {}
                        }
                        agents.push(data);
                    }
                }
            }
            Some(signal) = unreg_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(id) = serde_json::from_slice::<String>(&payload.data) {
                        if let Some(pos) = agents.iter().position(|a| a.agent_id == id) {
                            let removed = agents.remove(pos);
                            match removed.role.as_str() {
                                "witness" => has_witness = false,
                                "refinery" => has_refinery = false,
                                "polecat" => polecats.retain(|p| p != &id),
                                "crew" => crew.retain(|c| c != &id),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
}
```

**Step 3: Register in mod.rs and worker.rs**

Add `pub mod rig;` to `crates/gtr-temporal/src/workflows/mod.rs`.
Add `worker.register_wf("rig_wf", workflows::rig::rig_wf);` to `worker.rs`.

**Step 4: Verify**

Run: `cargo build`
Expected: Compiles

**Step 5: Commit**

```bash
git commit -m "feat: Rig workflow — lifecycle states (operational/parked/docked), agent tracking"
```

---

## Task 33: Rig CLI Commands

**Files:**
- Create: `crates/gtr-cli/src/commands/rig.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Create rig CLI module**

```rust
// crates/gtr-cli/src/commands/rig.rs
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum RigCommand {
    /// Register a new rig (git repository)
    Add {
        /// Rig name
        name: String,
        /// Git URL to clone
        #[arg(long)]
        git_url: String,
    },
    /// List registered rigs
    List,
    /// Show rig status
    Status {
        /// Rig name
        name: String,
    },
    /// Temporarily pause a rig (no agent auto-starts)
    Park {
        /// Rig name
        name: String,
    },
    /// Resume a parked rig
    Unpark {
        /// Rig name
        name: String,
    },
    /// Long-term shutdown of a rig
    Dock {
        /// Rig name
        name: String,
    },
    /// Resume a docked rig
    Undock {
        /// Rig name
        name: String,
    },
    /// Boot a rig (start witness + refinery)
    Boot {
        /// Rig name
        name: String,
    },
    /// Stop all agents on a rig
    Stop {
        /// Rig name
        name: String,
    },
}

pub async fn run(cmd: &RigCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    match cmd {
        RigCommand::Add { name, git_url } => {
            let input = serde_json::to_vec(&(name, git_url))?;
            client.start_workflow(
                vec![input.into()],
                "work".to_string(),
                format!("rig-{name}"),
                "rig_wf".to_string(),
                None,
            ).await?;
            println!("Registered rig: {name} ({git_url})");
        }
        RigCommand::List => {
            let query = "WorkflowType = 'rig_wf' AND ExecutionStatus = 'Running'".to_string();
            let results = client.list_workflow_executions(100, vec![], query).await?;
            if results.executions.is_empty() {
                println!("No rigs registered");
            } else {
                for exec in &results.executions {
                    if let Some(info) = &exec.execution {
                        println!("  {}", info.workflow_id);
                    }
                }
            }
        }
        RigCommand::Status { name } => {
            let result = client.describe_workflow_execution(
                &format!("rig-{name}"), None,
            ).await?;
            println!("Rig: {name}");
            println!("Status: {:?}", result.workflow_execution_info
                .map(|i| format!("{:?}", i.status)));
        }
        RigCommand::Park { name } => {
            client.signal_workflow_execution(
                format!("rig-{name}"), None,
                "rig_park".to_string(), None, None,
            ).await?;
            println!("Parked rig: {name}");
        }
        RigCommand::Unpark { name } => {
            client.signal_workflow_execution(
                format!("rig-{name}"), None,
                "rig_unpark".to_string(), None, None,
            ).await?;
            println!("Unparked rig: {name}");
        }
        RigCommand::Dock { name } => {
            client.signal_workflow_execution(
                format!("rig-{name}"), None,
                "rig_dock".to_string(), None, None,
            ).await?;
            println!("Docked rig: {name}");
        }
        RigCommand::Undock { name } => {
            client.signal_workflow_execution(
                format!("rig-{name}"), None,
                "rig_undock".to_string(), None, None,
            ).await?;
            println!("Undocked rig: {name}");
        }
        RigCommand::Boot { name } => {
            // Start witness and refinery for this rig
            let witness_input = serde_json::to_vec(&(format!("{name}-witness"), "witness"))?;
            client.start_workflow(
                vec![witness_input.into()],
                "work".to_string(),
                format!("{name}-witness"),
                "witness_wf".to_string(),
                None,
            ).await?;
            let refinery_id = format!("{name}-refinery");
            client.start_workflow(
                vec![],
                "work".to_string(),
                refinery_id,
                "refinery_wf".to_string(),
                None,
            ).await?;
            println!("Booted rig {name}: witness + refinery started");
        }
        RigCommand::Stop { name } => {
            client.signal_workflow_execution(
                format!("rig-{name}"), None,
                "rig_stop".to_string(), None, None,
            ).await?;
            println!("Stopped rig: {name}");
        }
    }
    Ok(())
}
```

**Step 2: Wire into mod.rs and main.rs**

Add `pub mod rig;` to `commands/mod.rs`.
Add `Rig` variant to `Command` enum and match arm in `main.rs`.

**Step 3: Verify**

Run: `cargo build`
Expected: Compiles

**Step 4: E2E test**

```bash
gtr rig add myrig --git-url https://github.com/example/repo
gtr rig list
gtr rig status myrig
gtr rig park myrig
gtr rig unpark myrig
gtr rig stop myrig
```

**Step 5: Commit**

```bash
git commit -m "feat: gtr rig — add/list/status/park/dock/boot/stop commands"
```

---

## Task 34: Crew CLI Commands

Crew workspaces are persistent developer workspaces (git worktrees). Unlike polecats, they are never auto-cleaned.

**Files:**
- Create: `crates/gtr-cli/src/commands/crew.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Create crew CLI**

Implement `add`, `list`, `start`, `stop`, `remove` subcommands. Crew workers are agent workflows with role `"crew"`. `add` creates a git worktree via the `git_operation` activity, then starts an agent workflow.

```rust
// crates/gtr-cli/src/commands/crew.rs
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum CrewCommand {
    /// Create a persistent crew workspace
    Add {
        /// Crew member name
        name: String,
        /// Rig to create workspace in
        #[arg(long)]
        rig: String,
    },
    /// List crew workspaces
    List,
    /// Start a crew session
    Start {
        /// Crew member name
        name: String,
    },
    /// Stop a crew session
    Stop {
        /// Crew member name
        name: String,
    },
    /// Remove a crew workspace
    Remove {
        /// Crew member name
        name: String,
    },
}

pub async fn run(cmd: &CrewCommand) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    match cmd {
        CrewCommand::Add { name, rig } => {
            // Start agent workflow with crew role
            let input = serde_json::to_vec(&(format!("crew-{name}"), "crew"))?;
            client.start_workflow(
                vec![input.into()],
                "work".to_string(),
                format!("{rig}-crew-{name}"),
                "agent_wf".to_string(),
                None,
            ).await?;
            // Register with rig workflow
            let reg = serde_json::to_vec(&serde_json::json!({
                "agent_id": format!("crew-{name}"),
                "role": "crew"
            }))?;
            client.signal_workflow_execution(
                format!("rig-{rig}"), None,
                "rig_register_agent".to_string(),
                Some(vec![reg.into()]), None,
            ).await?;
            println!("Created crew workspace: {name} on rig {rig}");
        }
        CrewCommand::List => {
            let query = "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'".to_string();
            let results = client.list_workflow_executions(100, vec![], query).await?;
            for exec in &results.executions {
                if let Some(info) = &exec.execution {
                    if info.workflow_id.contains("-crew-") {
                        println!("  {}", info.workflow_id);
                    }
                }
            }
        }
        CrewCommand::Start { name } => {
            println!("Starting crew session for {name}");
            // In a full implementation, this would spawn a Claude Code session
            // via the spawn_agent activity
            println!("crew start: session management not yet implemented");
        }
        CrewCommand::Stop { name } => {
            // Find the crew workflow and send stop signal
            let query = format!(
                "WorkflowType = 'agent_wf' AND ExecutionStatus = 'Running'"
            );
            let results = client.list_workflow_executions(100, vec![], query).await?;
            for exec in &results.executions {
                if let Some(info) = &exec.execution {
                    if info.workflow_id.contains(&format!("-crew-{name}")) {
                        client.signal_workflow_execution(
                            info.workflow_id.clone(), None,
                            "agent_stop".to_string(), None, None,
                        ).await?;
                        println!("Stopped crew: {name}");
                        return Ok(());
                    }
                }
            }
            println!("Crew member not found: {name}");
        }
        CrewCommand::Remove { name } => {
            // Stop workflow then clean up
            println!("crew remove: not yet fully implemented (need git worktree cleanup)");
        }
    }
    Ok(())
}
```

**Step 2: Wire into mod.rs and main.rs**

**Step 3: Verify and commit**

```bash
git commit -m "feat: gtr crew — add/list/start/stop/remove commands"
```

---

## Task 35: Name Pools

Polecat names come from themed pools (Mad Max names by default). This is a simple utility module.

**Files:**
- Create: `crates/gtr-core/src/namepool.rs`
- Modify: `crates/gtr-core/src/lib.rs`

**Step 1: Create namepool with Mad Max theme**

```rust
// crates/gtr-core/src/namepool.rs
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const MAD_MAX_NAMES: &[&str] = &[
    "nux", "slit", "rictus", "furiosa", "capable", "toast",
    "cheedo", "dag", "angharad", "dementus", "scrotus",
    "morsov", "ace", "valkyrie", "keeper", "glory",
    "corpus", "praetorian", "buzzard", "rock-rider",
];

/// Pick the next available name from the pool.
/// Cycles through names; appends a suffix if pool is exhausted.
pub fn next_name() -> String {
    let idx = COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = MAD_MAX_NAMES[idx % MAD_MAX_NAMES.len()];
    if idx < MAD_MAX_NAMES.len() {
        base.to_string()
    } else {
        format!("{base}-{}", idx / MAD_MAX_NAMES.len())
    }
}

/// Reset the counter (useful for tests).
pub fn reset() {
    COUNTER.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cycle_with_suffix() {
        reset();
        let first = next_name();
        assert_eq!(first, "nux");
        // Exhaust pool
        for _ in 1..MAD_MAX_NAMES.len() {
            next_name();
        }
        let overflow = next_name();
        assert_eq!(overflow, "nux-1");
    }
}
```

**Step 2: Add to lib.rs, verify, commit**

```bash
git commit -m "feat: name pools — Mad Max themed polecat names"
```

---

## Task 36: Polecat Workflow

Polecats are ephemeral workers. They have 3 operating states: Working, Stuck, Zombie (no idle state — polecats that aren't working are dead). The workflow manages the full lifecycle: spawn worktree → assign work → monitor → done → nuke.

**Files:**
- Create: `crates/gtr-temporal/src/workflows/polecat.rs`
- Modify: `crates/gtr-temporal/src/workflows/mod.rs`
- Modify: `crates/gtr-temporal/src/signals.rs`
- Modify: `crates/gtr-temporal/src/worker.rs`

**Step 1: Add polecat signals**

```rust
// Add to signals.rs
pub const SIGNAL_POLECAT_HEARTBEAT: &str = "polecat_heartbeat";
pub const SIGNAL_POLECAT_DONE: &str = "polecat_done";
pub const SIGNAL_POLECAT_STUCK: &str = "polecat_stuck";
pub const SIGNAL_POLECAT_KILL: &str = "polecat_kill";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolecatDoneSignal {
    pub branch: String,
    pub status: String, // "completed", "escalated", "deferred"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolecatState {
    pub name: String,
    pub rig: String,
    pub work_item_id: String,
    pub status: String, // "working", "done", "stuck", "zombie"
    pub branch: String,
    pub worktree_path: String,
}
```

**Step 2: Create polecat workflow**

The polecat workflow:
1. Creates a git worktree (via `git_operation` activity)
2. Spawns an agent process (via `spawn_agent` activity)
3. Monitors for done/stuck/kill signals
4. On done: submits to refinery, cleans up worktree
5. On kill: cleans up immediately

```rust
// crates/gtr-temporal/src/workflows/polecat.rs
use std::time::Duration;
use futures_util::StreamExt;
use temporalio_common::protos::coresdk::AsJsonPayloadExt;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};
use crate::activities::git_ops::{GitOperation, GitResult};
use crate::signals::*;

pub async fn polecat_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let (name, rig, work_item_id, title) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String, String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "unknown".into(), "unknown".into(), "untitled".into()))
    } else {
        ("unknown".into(), "unknown".into(), "unknown".into(), "untitled".into())
    };

    let branch = format!("polecat/{name}/{work_item_id}");
    let worktree_path = format!("{rig}/polecats/{name}/{rig}");
    let mut status = "working".to_string();

    tracing::info!("Polecat {name} started on rig {rig}: {title}");

    // Step 1: Create git worktree
    let worktree_op = GitOperation::WorktreeAdd {
        repo_path: format!("{rig}/.repo.git"),
        worktree_path: worktree_path.clone(),
        branch: branch.clone(),
    };
    let worktree_result = ctx.activity(ActivityOptions {
        activity_type: "git_operation".to_string(),
        input: worktree_op.as_json_payload()?,
        start_to_close_timeout: Some(Duration::from_secs(120)),
        ..Default::default()
    }).await;

    if worktree_result.completed_ok().is_none() {
        tracing::error!("Polecat {name}: failed to create worktree");
        return Ok(WfExitValue::Normal(serde_json::to_string(&PolecatState {
            name, rig, work_item_id, status: "failed".into(), branch, worktree_path,
        })?));
    }

    // Step 2: Listen for lifecycle signals
    let mut heartbeat_ch = ctx.make_signal_channel(SIGNAL_POLECAT_HEARTBEAT);
    let mut done_ch = ctx.make_signal_channel(SIGNAL_POLECAT_DONE);
    let mut stuck_ch = ctx.make_signal_channel(SIGNAL_POLECAT_STUCK);
    let mut kill_ch = ctx.make_signal_channel(SIGNAL_POLECAT_KILL);

    // 30-minute staleness timer for polecats
    loop {
        tokio::select! {
            biased;
            Some(_) = kill_ch.next() => {
                tracing::info!("Polecat {name} killed");
                status = "zombie".to_string();
                break;
            }
            Some(signal) = done_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<PolecatDoneSignal>(&payload.data) {
                        tracing::info!("Polecat {name} done: {}", data.status);
                        status = "done".to_string();

                        // Submit to refinery
                        // The CLI `gtr done` handles this — the polecat just records completion
                    }
                }
                break;
            }
            Some(_) = stuck_ch.next() => {
                status = "stuck".to_string();
                tracing::warn!("Polecat {name} reports stuck");
                // Continue running — witness will handle escalation
            }
            Some(_) = heartbeat_ch.next() => {
                tracing::debug!("Polecat {name} heartbeat");
                // Reset any staleness tracking
            }
            _ = ctx.timer(Duration::from_secs(1800)) => {
                if status == "working" {
                    status = "stuck".to_string();
                    tracing::warn!("Polecat {name} stale — no heartbeat in 30m");
                }
            }
        }
    }

    Ok(WfExitValue::Normal(serde_json::to_string(&PolecatState {
        name, rig, work_item_id, status, branch, worktree_path,
    })?))
}
```

**Step 3: Register in mod.rs and worker.rs**

**Step 4: Verify and commit**

```bash
git commit -m "feat: Polecat workflow — ephemeral worker lifecycle (spawn/work/done/kill)"
```

---

## Task 37: Polecat CLI Commands

**Files:**
- Create: `crates/gtr-cli/src/commands/polecat.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Create polecat CLI**

Subcommands: `list`, `status <name>`, `kill <name>`, `stuck`. These query/signal polecat workflows.

**Step 2: Wire into main.rs**

**Step 3: Verify and commit**

```bash
git commit -m "feat: gtr polecat — list/status/kill commands"
```

---

## Task 38: Hook System — Durable Work Assignment

The hook is how work survives session restarts. In Temporal, the hook is part of the agent workflow state.

**Files:**
- Modify: `crates/gtr-temporal/src/signals.rs` (add hook signals)
- Modify: `crates/gtr-temporal/src/workflows/agent.rs` (add hook state)
- Modify: `crates/gtr-cli/src/commands/hook.rs` (query hook from agent workflow)

**Step 1: Add hook signals**

```rust
// Add to signals.rs
pub const SIGNAL_HOOK: &str = "hook";
pub const SIGNAL_HOOK_CLEAR: &str = "hook_clear";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSignal {
    pub work_item_id: String,
    pub title: String,
    pub molecule_id: Option<String>,
    pub current_step: Option<String>,
}
```

**Step 2: Update agent workflow to track hook state**

Add `hook: Option<HookSignal>` to the agent workflow's mutable state. On `SIGNAL_HOOK`, set the hook. On `SIGNAL_HOOK_CLEAR`, clear it. Include hook in the serialized `AgentState`.

**Step 3: Update hook CLI to query agent workflow state**

`gtr hook` should describe the agent workflow and display the hooked work item, molecule, and current step.

**Step 4: Verify and commit**

```bash
git commit -m "feat: Hook system — durable work assignment on agent workflows"
```

---

## Task 39: Prime & Handoff — Context Recovery

`gtr prime` detects the current agent's role from environment and injects context (hooked work, handoff notes, unread mail). `gtr handoff` sends context to the next session.

**Files:**
- Create: `crates/gtr-cli/src/commands/prime.rs`
- Create: `crates/gtr-cli/src/commands/handoff.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Create prime command**

`gtr prime` reads `GTR_ROLE`, `GTR_RIG`, `GTR_AGENT` env vars. Queries the agent workflow for hook, inbox, and handoff content. Outputs context suitable for Claude Code's SessionStart hook.

**Step 2: Create handoff command**

`gtr handoff` sends a mail to self with handoff content (summary of current work, decisions made, blockers). Then signals the agent workflow to restart.

**Step 3: Wire into main.rs**

**Step 4: Verify and commit**

```bash
git commit -m "feat: gtr prime/handoff — context recovery for session continuity"
```

---

## Task 40: Molecule Workflow — Instantiated Formula Tracking

Molecules are running instances of formulas. Each step becomes a child activity/workflow. The molecule tracks which steps are complete, which are in-progress, and what's next.

**Files:**
- Create: `crates/gtr-temporal/src/workflows/molecule.rs`
- Modify: `crates/gtr-temporal/src/workflows/mod.rs`
- Modify: `crates/gtr-temporal/src/signals.rs`
- Modify: `crates/gtr-temporal/src/worker.rs`

**Step 1: Add molecule signals**

```rust
// Add to signals.rs
pub const SIGNAL_MOL_STEP_DONE: &str = "mol_step_done";
pub const SIGNAL_MOL_STEP_FAIL: &str = "mol_step_fail";
pub const SIGNAL_MOL_PAUSE: &str = "mol_pause";
pub const SIGNAL_MOL_RESUME: &str = "mol_resume";
pub const SIGNAL_MOL_CANCEL: &str = "mol_cancel";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MolStepDoneSignal {
    pub step_ref: String,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MolStepFailSignal {
    pub step_ref: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoleculeState {
    pub id: String,
    pub formula_name: String,
    pub status: String, // "running", "paused", "completed", "failed"
    pub steps: Vec<MolStepState>,
    pub current_step: Option<String>,
    pub variables: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MolStepState {
    pub ref_id: String,
    pub title: String,
    pub status: String, // "pending", "in_progress", "done", "failed", "blocked"
    pub output: Option<String>,
}
```

**Step 2: Create molecule workflow**

The molecule workflow:
1. Parses the formula TOML (passed as input)
2. Topo-sorts steps
3. Tracks completion state for each step
4. Advances to next eligible step when `mol_step_done` signal received
5. Returns when all steps complete or on cancel

**Step 3: Register and verify**

**Step 4: Commit**

```bash
git commit -m "feat: Molecule workflow — instantiated formula with step-by-step tracking"
```

---

## Task 41: Molecule CLI Commands

**Files:**
- Create: `crates/gtr-cli/src/commands/mol.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Create mol CLI**

Subcommands:
- `status <id>` — describe molecule workflow, show step progress
- `current` — query current agent's hook for molecule, show current step
- `step done` — signal `mol_step_done` for current step
- `cancel <id>` — signal `mol_cancel`
- `pause <id>` / `resume <id>` — signal pause/resume

**Step 2: Wire into main.rs**

**Step 3: Verify and commit**

```bash
git commit -m "feat: gtr mol — status/current/step-done/cancel/pause/resume commands"
```

---

## Task 42: Dog Workflow — Reusable Infrastructure Workers

Dogs are like polecats but persistent and cross-rig. They have an idle pool managed by the Deacon.

**Files:**
- Create: `crates/gtr-temporal/src/workflows/dog.rs`
- Create: `crates/gtr-cli/src/commands/dog.rs`
- Modify: `crates/gtr-temporal/src/workflows/mod.rs`
- Modify: `crates/gtr-temporal/src/signals.rs`
- Modify: `crates/gtr-temporal/src/worker.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Add dog signals**

```rust
pub const SIGNAL_DOG_DISPATCH: &str = "dog_dispatch";
pub const SIGNAL_DOG_RELEASE: &str = "dog_release";
pub const SIGNAL_DOG_STOP: &str = "dog_stop";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DogDispatchSignal {
    pub rig: String,
    pub work_item_id: String,
    pub plugin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DogState {
    pub name: String,
    pub status: String, // "idle", "working"
    pub current_work: Option<String>,
    pub current_rig: Option<String>,
}
```

**Step 2: Create dog workflow**

Long-running workflow that alternates between idle (waiting for dispatch) and working (processing a task). Unlike polecats, dogs return to idle after completing work.

**Step 3: Create dog CLI — `gtr dog list/dispatch/status`**

**Step 4: Verify and commit**

```bash
git commit -m "feat: Dog workflow and CLI — reusable cross-rig infrastructure workers"
```

---

## Task 43: Sling v2 — Full Work Dispatch

Upgrade sling from basic signal-send to the full dispatch system.

**Files:**
- Modify: `crates/gtr-cli/src/commands/sling.rs`

**Step 1: Implement full sling logic**

- Target resolution: `gtr sling <work-id> <rig>` auto-spawns polecat
- `gtr sling <work-id> mayor` sends to Mayor workflow
- `gtr sling <work-id> dogs` dispatches to idle Dog
- Batch: `gtr sling <id1> <id2> <id3> <rig>` creates one polecat per work item
- `--agent <runtime>` flag for multi-runtime support (claude, codex, gemini)
- Auto-convoy creation when slinging multiple items

**Step 2: Verify and commit**

```bash
git commit -m "feat: Sling v2 — batch dispatch, auto-polecat, dog dispatch, multi-runtime"
```

---

## Task 44: Gates — Async Wait Primitives

Gates allow work to be parked while waiting for external events.

**Files:**
- Create: `crates/gtr-temporal/src/workflows/gate.rs`
- Create: `crates/gtr-cli/src/commands/gate.rs`
- Modify: `crates/gtr-temporal/src/workflows/mod.rs`
- Modify: `crates/gtr-temporal/src/signals.rs`
- Modify: `crates/gtr-temporal/src/worker.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Add gate signals and types**

```rust
pub const SIGNAL_GATE_CLOSE: &str = "gate_close";
pub const SIGNAL_GATE_APPROVE: &str = "gate_approve";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateType {
    Timer { duration_secs: u64 },
    Human { description: String },
    Mail { from: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateState {
    pub id: String,
    pub gate_type: GateType,
    pub status: String, // "waiting", "closed", "approved"
    pub parked_work: Option<String>,
}
```

**Step 2: Create gate workflow**

- `Timer` gates auto-close after duration (using `ctx.timer()`)
- `Human` gates wait for `gate_approve` signal
- `Mail` gates wait for `gate_close` signal

**Step 3: Create gate CLI and park/resume commands**

- `gtr gate create timer:30m` — create timer gate
- `gtr gate create human:"deploy approval"` — create human gate
- `gtr park <gate-id>` — park current work on gate
- `gtr resume` — check if gate cleared, resume work

**Step 4: Verify and commit**

```bash
git commit -m "feat: Gates — async wait primitives (timer, human, mail) with park/resume"
```

---

## Task 45: Mail v2 — Full Messaging System

Upgrade mail from basic send/nudge/broadcast to the full messaging system.

**Files:**
- Modify: `crates/gtr-cli/src/commands/mail.rs`
- Modify: `crates/gtr-temporal/src/signals.rs`

**Step 1: Add mail subcommands**

- `inbox` — list unread mail (query agent workflow inbox)
- `read <id>` — mark message as read
- `reply <id> -m "message"` — reply to message
- `thread <id>` — view message thread
- `search <query>` — search mail by content
- `archive <id>` — archive message
- `clear` — clear all mail
- `check` — check for new mail (for hooks/polling)

**Step 2: Add mail state tracking to agent workflow**

Add `read: bool` and `archived: bool` fields to `MailEntry`. Add thread tracking (reply_to field).

**Step 3: Verify and commit**

```bash
git commit -m "feat: Mail v2 — inbox/read/reply/thread/search/archive"
```

---

## Task 46: Witness v2 — Real Agent Monitoring

Replace the placeholder witness with real staleness detection.

**Files:**
- Modify: `crates/gtr-temporal/src/workflows/witness.rs`

**Step 1: Implement real monitoring**

On each cycle:
1. Query Temporal for running `polecat_wf` workflows on this rig
2. For each polecat, check workflow status and last heartbeat time
3. If stale (no heartbeat > 30m), send escalation to Mayor
4. If zombie (workflow running but tmux session dead), signal kill
5. Track alert count per polecat to avoid spam

**Step 2: Verify and commit**

```bash
git commit -m "feat: Witness v2 — real polecat staleness detection and escalation"
```

---

## Task 47: Patrol v2 — Real Plugin Execution

Replace the placeholder patrol with actual plugin discovery and gate checking.

**Files:**
- Modify: `crates/gtr-temporal/src/workflows/patrol.rs`

**Step 1: Implement real patrol cycle**

On each cycle:
1. Discover plugins (call `run_plugin` activity with a discovery command)
2. For each plugin, check its gate (cooldown, cron)
3. Run eligible plugins via `run_plugin` activity
4. Record results for digest

**Step 2: Verify and commit**

```bash
git commit -m "feat: Patrol v2 — real plugin discovery and gate-checked execution"
```

---

## Task 48: Refinery v2 — Real Git Rebase and Test Execution

Replace placeholder refinery merge logic with real git operations.

**Files:**
- Modify: `crates/gtr-temporal/src/workflows/refinery.rs`

**Step 1: Implement real merge flow**

For each queued item:
1. Fetch branch (via `git_operation` activity — `GitOperation::Checkout`)
2. Rebase onto main (new `GitOperation::Rebase` variant needed)
3. Run tests (via `run_plugin` activity)
4. If tests pass: merge to main (via `git_operation`)
5. If rebase fails: create conflict-resolution work item, spawn polecat

**Step 2: Add `GitOperation::Rebase` and `GitOperation::Merge` variants**

```rust
// Add to git_ops.rs
GitOperation::Rebase { repo_path, branch, onto } => { ... }
GitOperation::Merge { repo_path, branch } => { ... }
```

**Step 3: Verify and commit**

```bash
git commit -m "feat: Refinery v2 — real git rebase, test execution, conflict detection"
```

---

## Task 49: Feed TUI — Real-Time Activity Dashboard

**Files:**
- Create: `crates/gtr-cli/src/commands/feed.rs`
- Modify: `crates/gtr-cli/Cargo.toml` (add `ratatui` or `crossterm`)
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Create a terminal dashboard**

Show:
- Agent tree (Mayor → Deacon → Witnesses/Refineries → Polecats)
- Active convoys with progress
- Recent event stream (workflow starts, completions, signals)
- Auto-refresh every 5 seconds

Use `crossterm` for raw terminal control and a simple layout (no heavy TUI framework needed for v1).

**Step 2: Verify and commit**

```bash
git commit -m "feat: gtr feed — real-time terminal activity dashboard"
```

---

## Task 50: Checkpoint & Session Management

**Files:**
- Create: `crates/gtr-core/src/checkpoint.rs`
- Create: `crates/gtr-cli/src/commands/checkpoint.rs`
- Create: `crates/gtr-cli/src/commands/session.rs`
- Modify: `crates/gtr-core/src/lib.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs`
- Modify: `crates/gtr-cli/src/main.rs`

**Step 1: Create checkpoint types**

```rust
// crates/gtr-core/src/checkpoint.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub molecule_id: Option<String>,
    pub current_step: Option<String>,
    pub step_title: Option<String>,
    pub modified_files: Vec<String>,
    pub last_commit: Option<String>,
    pub branch: Option<String>,
    pub hooked_work: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<String>,
    pub notes: Option<String>,
}
```

**Step 2: Create checkpoint CLI — write/read/clear**

Checkpoints are local JSON files (`.gtr-checkpoint.json`). `gtr checkpoint write` captures current state. `gtr checkpoint read` displays it. `gtr prime` reads checkpoints for crash recovery.

**Step 3: Create session CLI — list/status**

`gtr session list` queries running agent workflows. `gtr session status <id>` describes a specific session.

**Step 4: Verify and commit**

```bash
git commit -m "feat: Checkpoint and session management — crash recovery and session listing"
```

---

## Testing Strategy

For each task:
1. `cargo build` must compile
2. `cargo test` must pass all existing + new unit tests
3. E2E test against local Temporal (`temporal server start-dev`) for workflow tasks
4. E2E test CLI commands for CLI tasks

## Notes for the Implementer

- **Temporal SDK is pre-alpha.** Adapt API calls to what compiles. The patterns are right even if method signatures shift.
- **Signal-per-iteration pattern.** Don't drain multiple signals in one `select!` iteration — causes replay non-determinism. Process one signal, then loop.
- **`ActivityError` variants:** `ActivityError::Retryable { source, explicit_delay }` and `ActivityError::NonRetryable(anyhow::Error)`. Not constructor functions.
- **`list_workflow_executions` takes 3 args:** `(page_size: i32, next_page_token: Vec<u8>, query: String)`.
- **No ORDER BY in Temporal queries.** Temporal returns results in reverse chronological order by default.
- **Clone before borrow.** If you need `def.name` and also call `def.topo_sort()`, clone `def.name` first.
- **Read the Phase 1 plan** (`docs/plans/2026-02-12-gtr-implementation-plan.md`) and the memory file at `~/.claude/projects/-Users-drew-payment-dev-gt-gtr-crew-drew/memory/MEMORY.md` for SDK patterns.
