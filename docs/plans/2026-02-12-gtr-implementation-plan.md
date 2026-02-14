# Gas Town Rusted (gtr) Implementation Plan

**STATUS: COMPLETE** — All 29 tasks implemented and merged to main. See Phase 2: `docs/plans/2026-02-14-gtr-phase2-implementation-plan.md`

**Goal:** Build a Rust + Temporal rewrite of Gas Town that replaces Beads with Temporal workflows for durable agent orchestration.

**Architecture:** Single `gtr` binary (Cargo workspace with 6 crates). Temporal workflows replace git-backed state. Agent interactions are Temporal activities. Mail is Temporal signals. Local TOML files for configuration.

**Tech Stack:** Rust 2024 edition, Temporal Rust SDK (pre-alpha, git-pinned), clap 4, tokio, serde, tracing, git2, chrono

**Design doc:** `docs/plans/2026-02-12-gtr-rust-temporal-design.md`

**Temporal SDK pin:** `temporalio/sdk-core` rev `7ecb7c0` (2026-02-12)

**SDK API patterns (from research):**
- Workflows: `async fn my_wf(ctx: WfContext) -> WorkflowResult<()>`
- Activities: `async fn my_act(ctx: ActContext, input: MyInput) -> Result<MyOutput, anyhow::Error>`
- Worker: `Worker::new_from_core()`, `worker.register_wf("name", my_wf)`, `worker.register_activity("name", my_act)`, `worker.run().await`
- Signals: `ctx.make_signal_channel("name")` to receive, `ctx.signal_workflow(opts)` to send
- Activities from workflow: `ctx.activity(ActivityOptions { activity_type: "name", input: payload, ... })`
- Child workflows: `ctx.child_workflow(ChildWorkflowOptions { ... }).start(&ctx)`
- Heartbeat: `ctx.record_heartbeat(details)`
- Client: `sdk_client_options()` → connect → `client.start_workflow()` / `client.signal_workflow()` / `client.query_workflow()`

---

## Task 1: Scaffold Cargo Workspace

**Files:**
- Create: `gtr/Cargo.toml` (workspace root)
- Create: `gtr/crates/gtr-core/Cargo.toml`
- Create: `gtr/crates/gtr-core/src/lib.rs`
- Create: `gtr/crates/gtr-cli/Cargo.toml`
- Create: `gtr/crates/gtr-cli/src/main.rs`

**Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/gtr-core",
    "crates/gtr-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"

[workspace.dependencies]
# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Time
chrono = { version = "0.4", features = ["serde"] }

# Errors
thiserror = "2"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# CLI
clap = { version = "4", features = ["derive"] }

# Async
tokio = { version = "1", features = ["full"] }

# ID generation
nanoid = "0.4"

# Directory resolution
dirs = "6"
```

**Step 2: Create gtr-core crate**

`gtr/crates/gtr-core/Cargo.toml`:
```toml
[package]
name = "gtr-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
nanoid = { workspace = true }
dirs = { workspace = true }
```

`gtr/crates/gtr-core/src/lib.rs`:
```rust
pub mod types;
pub mod ids;
pub mod config;
pub mod errors;
```

**Step 3: Create gtr-cli crate**

`gtr/crates/gtr-cli/Cargo.toml`:
```toml
[package]
name = "gtr-cli"
version.workspace = true
edition.workspace = true

[[bin]]
name = "gtr"
path = "src/main.rs"

[dependencies]
gtr-core = { path = "../gtr-core" }
clap = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

`gtr/crates/gtr-cli/src/main.rs`:
```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "gtr", about = "Gas Town Rusted — multi-agent workspace manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Print version information
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Version => {
            println!("gtr {}", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}
```

**Step 4: Verify it builds**

Run: `cd gtr && cargo build`
Expected: Compiles successfully

**Step 5: Verify CLI runs**

Run: `cd gtr && cargo run --bin gtr -- version`
Expected: `gtr 0.1.0`

**Step 6: Commit**

```bash
git add gtr/
git commit -m "feat: scaffold gtr cargo workspace with core and cli crates"
```

---

## Task 2: Core Types (gtr-core/src/types.rs)

**Files:**
- Create: `gtr/crates/gtr-core/src/types.rs`

**Step 1: Write the failing test**

Add to bottom of `gtr/crates/gtr-core/src/types.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkItemId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConvoyId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkItemStatus {
    Pending,
    Assigned,
    InProgress,
    Review,
    Done,
    Failed,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: WorkItemId,
    pub title: String,
    pub description: String,
    pub status: WorkItemStatus,
    pub priority: Priority,
    pub assigned_to: Option<AgentId>,
    pub convoy_id: Option<ConvoyId>,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConvoyStatus {
    Open,
    Closed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Convoy {
    pub id: ConvoyId,
    pub title: String,
    pub work_items: Vec<WorkItemId>,
    pub status: ConvoyStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    Mayor,
    Deacon,
    Witness,
    Refinery,
    Polecat,
    Crew,
    Dog,
    Boot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRuntime {
    Claude,
    Codex,
    Cursor,
    Gemini,
    Amp,
    Custom { command: String, args: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Working,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: AgentId,
    pub role: AgentRole,
    pub runtime: AgentRuntime,
    pub rig: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_status_default_is_pending() {
        let status = WorkItemStatus::Pending;
        assert_eq!(status, WorkItemStatus::Pending);
    }

    #[test]
    fn priority_ordering() {
        assert!(Priority::Critical < Priority::High);
        assert!(Priority::High < Priority::Medium);
        assert!(Priority::Medium < Priority::Low);
    }

    #[test]
    fn work_item_serializes_to_json() {
        let item = WorkItem {
            id: WorkItemId("hq-abc12".into()),
            title: "Test item".into(),
            description: "A test".into(),
            status: WorkItemStatus::Pending,
            priority: Priority::Medium,
            assigned_to: None,
            convoy_id: None,
            labels: vec!["test".into()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: WorkItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, item.id);
        assert_eq!(parsed.status, WorkItemStatus::Pending);
    }

    #[test]
    fn agent_runtime_custom_variant() {
        let rt = AgentRuntime::Custom {
            command: "/usr/bin/my-agent".into(),
            args: vec!["--flag".into()],
        };
        let json = serde_json::to_string(&rt).unwrap();
        let parsed: AgentRuntime = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, rt);
    }
}
```

**Step 2: Run tests**

Run: `cd gtr && cargo test -p gtr-core`
Expected: All 4 tests pass

**Step 3: Commit**

```bash
git add gtr/crates/gtr-core/src/types.rs
git commit -m "feat: add core types — WorkItem, Convoy, Agent models"
```

---

## Task 3: ID Generation (gtr-core/src/ids.rs)

**Files:**
- Create: `gtr/crates/gtr-core/src/ids.rs`

**Step 1: Write the module with tests**

```rust
use crate::types::{AgentId, ConvoyId, WorkItemId};
use nanoid::nanoid;

const ID_ALPHABET: [char; 36] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
    'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];

const ID_LEN: usize = 5;

fn gen_suffix() -> String {
    nanoid!(ID_LEN, &ID_ALPHABET)
}

pub fn work_item_id(prefix: &str) -> WorkItemId {
    WorkItemId(format!("{}-{}", prefix, gen_suffix()))
}

pub fn convoy_id(prefix: &str) -> ConvoyId {
    ConvoyId(format!("{}-cv-{}", prefix, gen_suffix()))
}

pub fn agent_id(name: &str) -> AgentId {
    AgentId(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_id_has_correct_format() {
        let id = work_item_id("hq");
        assert!(id.0.starts_with("hq-"));
        assert_eq!(id.0.len(), 3 + ID_LEN); // "hq-" + 5 chars
    }

    #[test]
    fn convoy_id_has_correct_format() {
        let id = convoy_id("hq");
        assert!(id.0.starts_with("hq-cv-"));
        assert_eq!(id.0.len(), 6 + ID_LEN); // "hq-cv-" + 5 chars
    }

    #[test]
    fn work_item_ids_are_unique() {
        let a = work_item_id("hq");
        let b = work_item_id("hq");
        assert_ne!(a, b);
    }

    #[test]
    fn agent_id_preserves_name() {
        let id = agent_id("mayor");
        assert_eq!(id.0, "mayor");
    }
}
```

**Step 2: Run tests**

Run: `cd gtr && cargo test -p gtr-core`
Expected: All tests pass (types + ids)

**Step 3: Commit**

```bash
git add gtr/crates/gtr-core/src/ids.rs
git commit -m "feat: add ID generation — prefix + 5-char alphanumeric"
```

---

## Task 4: Error Types (gtr-core/src/errors.rs)

**Files:**
- Create: `gtr/crates/gtr-core/src/errors.rs`

**Step 1: Write errors**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GtrError {
    #[error("config not found: {path}")]
    ConfigNotFound { path: String },

    #[error("config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("invalid state transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },

    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("work item not found: {0}")]
    WorkItemNotFound(String),

    #[error("convoy not found: {0}")]
    ConvoyNotFound(String),

    #[error("temporal error: {0}")]
    Temporal(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
```

**Step 2: Run tests**

Run: `cd gtr && cargo test -p gtr-core`
Expected: All existing tests still pass, errors module compiles

**Step 3: Commit**

```bash
git add gtr/crates/gtr-core/src/errors.rs
git commit -m "feat: add GtrError types"
```

---

## Task 5: Config Parsing (gtr-core/src/config.rs)

**Files:**
- Create: `gtr/crates/gtr-core/src/config.rs`

**Step 1: Write config types with tests**

```rust
use crate::types::AgentRuntime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TownConfig {
    pub name: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default = "default_temporal_address")]
    pub temporal_address: String,
}

fn default_namespace() -> String {
    "default".into()
}

fn default_temporal_address() -> String {
    "http://localhost:7233".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigEntry {
    pub name: String,
    pub path: PathBuf,
    pub git_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigsConfig {
    pub rigs: Vec<RigEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigConfig {
    pub name: String,
    #[serde(default)]
    pub default_runtime: Option<AgentRuntime>,
    #[serde(default)]
    pub agents: HashMap<String, AgentRuntimeOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeOverride {
    pub runtime: AgentRuntime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationConfig {
    pub routes: HashMap<String, Vec<String>>,
    pub thresholds: EscalationThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationThresholds {
    pub stale_after: String,
    #[serde(default = "default_max_re_escalations")]
    pub max_re_escalations: u32,
}

fn default_max_re_escalations() -> u32 {
    2
}

/// Resolve the town root directory. Walks up from `start` looking for `.gtr/config.toml`.
pub fn find_town_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".gtr").join("config.toml").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Load and parse a TOML config file.
pub fn load_config<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let content = std::fs::read_to_string(path)?;
    let config: T = toml::from_str(&content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_town_config() {
        let toml_str = r#"
name = "my-town"
namespace = "gastown"
temporal_address = "http://localhost:7233"
"#;
        let config: TownConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.name, "my-town");
        assert_eq!(config.namespace, "gastown");
    }

    #[test]
    fn town_config_defaults() {
        let toml_str = r#"name = "my-town""#;
        let config: TownConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.namespace, "default");
        assert_eq!(config.temporal_address, "http://localhost:7233");
    }

    #[test]
    fn parse_escalation_config() {
        let toml_str = r#"
[routes]
critical = ["signal:mayor", "activity:email", "activity:sms"]
high = ["signal:mayor", "activity:email"]
medium = ["signal:mayor"]
low = []

[thresholds]
stale_after = "4h"
max_re_escalations = 2
"#;
        let config: EscalationConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.routes["critical"].len(), 3);
        assert_eq!(config.thresholds.stale_after, "4h");
    }

    #[test]
    fn find_town_root_walks_up() {
        let dir = tempdir().unwrap();
        let gtr_dir = dir.path().join(".gtr");
        fs::create_dir_all(&gtr_dir).unwrap();
        fs::write(gtr_dir.join("config.toml"), "name = \"test\"").unwrap();

        let nested = dir.path().join("some").join("nested").join("dir");
        fs::create_dir_all(&nested).unwrap();

        let found = find_town_root(&nested).unwrap();
        assert_eq!(found, dir.path());
    }

    #[test]
    fn find_town_root_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        assert!(find_town_root(dir.path()).is_none());
    }
}
```

**Step 2: Add tempfile dev-dependency**

Add to `gtr/crates/gtr-core/Cargo.toml`:
```toml
[dev-dependencies]
tempfile = "3"
```

**Step 3: Run tests**

Run: `cd gtr && cargo test -p gtr-core`
Expected: All tests pass

**Step 4: Commit**

```bash
git add gtr/crates/gtr-core/
git commit -m "feat: add config parsing — TownConfig, RigConfig, EscalationConfig"
```

---

## Task 6: CLI Skeleton with All Subcommands

**Files:**
- Create: `gtr/crates/gtr-cli/src/commands/mod.rs`
- Create: `gtr/crates/gtr-cli/src/commands/convoy.rs`
- Create: `gtr/crates/gtr-cli/src/commands/work.rs`
- Create: `gtr/crates/gtr-cli/src/commands/sling.rs`
- Create: `gtr/crates/gtr-cli/src/commands/mail.rs`
- Create: `gtr/crates/gtr-cli/src/commands/agents.rs`
- Create: `gtr/crates/gtr-cli/src/commands/services.rs`
- Create: `gtr/crates/gtr-cli/src/commands/workspace.rs`
- Create: `gtr/crates/gtr-cli/src/commands/diagnostics.rs`
- Modify: `gtr/crates/gtr-cli/src/main.rs`

**Step 1: Create commands/mod.rs**

```rust
pub mod agents;
pub mod convoy;
pub mod diagnostics;
pub mod mail;
pub mod services;
pub mod sling;
pub mod work;
pub mod workspace;
```

**Step 2: Create each command module as a stub**

Each module follows this pattern — define the clap structs, print "not implemented" for the handler. Example for `convoy.rs`:

```rust
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ConvoyCommands {
    /// Create a new convoy
    Create {
        /// Convoy title
        title: String,
        /// Priority
        #[arg(short, long, default_value = "medium")]
        priority: String,
    },
    /// List active convoys
    List,
    /// Show convoy details
    Show {
        /// Convoy ID
        id: String,
    },
    /// Close a convoy
    Close {
        /// Convoy ID
        id: String,
    },
}

pub async fn handle(cmd: ConvoyCommands) -> anyhow::Result<()> {
    match cmd {
        ConvoyCommands::Create { title, priority } => {
            println!("[not implemented] convoy create: {title} (priority: {priority})");
        }
        ConvoyCommands::List => {
            println!("[not implemented] convoy list");
        }
        ConvoyCommands::Show { id } => {
            println!("[not implemented] convoy show: {id}");
        }
        ConvoyCommands::Close { id } => {
            println!("[not implemented] convoy close: {id}");
        }
    }
    Ok(())
}
```

Create similar stubs for each module:

**work.rs** — `Show { id }`, `Close { id }`, `Done`, `Release { id }`, `Hook`, `Ready`
**sling.rs** — `Sling { work_id, agent }`, `Unsling { agent }`
**mail.rs** — `Send { to, message }`, `Inbox`, `Thread { id }`
**agents.rs** — `List`, `Mayor { subcommand: MayorCommands }`, `Deacon { action: StartStop }`, `Witness { action }`, `Refinery { action }`, `Polecat { subcommand: PolecatCommands }`, `Boot`
**services.rs** — `Worker`, `Up`, `Down`, `Daemon { action: StartStop }`
**workspace.rs** — `Install { path }`, `Init`, `Rig { subcommand: RigCommands }`, `Crew { subcommand: CrewCommands }`, `Config`, `Doctor`, `Prime`, `Hooks`
**diagnostics.rs** — `Status`, `Dashboard`, `Audit { actor }`, `Trail`, `Feed`

**Step 3: Update main.rs to wire all commands**

```rust
mod commands;

use clap::Parser;
use commands::*;

#[derive(Parser)]
#[command(name = "gtr", about = "Gas Town Rusted — multi-agent workspace manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Print version information
    Version,

    // Work Management
    /// Manage convoys (batches of work)
    Convoy {
        #[command(subcommand)]
        command: convoy::ConvoyCommands,
    },
    /// Show work item details
    Show { id: String },
    /// Close a work item
    Close { id: String },
    /// Signal work is done
    Done,
    /// Release a stuck work item
    Release { id: String },
    /// Show hooked work
    Hook,
    /// Show ready work
    Ready,
    /// Assign work to an agent
    Sling {
        /// Work item ID
        work_id: String,
        /// Agent to assign to
        agent: String,
    },
    /// Remove work from an agent
    Unsling {
        /// Agent to unassign from
        agent: String,
    },

    // Communication
    /// Agent messaging system
    Mail {
        #[command(subcommand)]
        command: mail::MailCommands,
    },
    /// Send a nudge to an agent
    Nudge {
        /// Target agent
        agent: String,
        /// Message
        message: String,
    },
    /// Broadcast to all agents
    Broadcast {
        /// Message
        message: String,
    },
    /// Escalate a work item
    Escalate {
        /// Work item ID
        id: String,
    },

    // Agent Management
    /// List active agents
    Agents,
    /// Manage the Mayor
    Mayor {
        #[command(subcommand)]
        command: agents::MayorCommands,
    },
    /// Manage the Deacon
    Deacon {
        /// start or stop
        action: String,
    },
    /// Manage the Witness
    Witness {
        /// start or stop
        action: String,
    },
    /// Manage the Refinery
    Refinery {
        /// start or stop
        action: String,
    },
    /// Manage polecats
    Polecat {
        #[command(subcommand)]
        command: agents::PolecatCommands,
    },
    /// Start the Boot watchdog
    Boot,

    // Services
    /// Start the Temporal worker
    Worker,
    /// Start all services
    Up,
    /// Stop all services
    Down,

    // Workspace
    /// Create a new Gas Town HQ
    Install {
        /// Path for the HQ directory
        path: String,
    },
    /// Initialize current directory as a rig
    Init,
    /// Manage rigs
    Rig {
        #[command(subcommand)]
        command: workspace::RigCommands,
    },
    /// Manage crew workspaces
    Crew {
        #[command(subcommand)]
        command: workspace::CrewCommands,
    },
    /// Run health checks
    Doctor,
    /// Output role context
    Prime,
    /// Show overall status
    Status,

    // Formulas
    /// Manage workflow formulas
    Formula {
        #[command(subcommand)]
        command: work::FormulaCommands,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Version => println!("gtr {}", env!("CARGO_PKG_VERSION")),
        Commands::Convoy { command } => convoy::handle(command).await?,
        Commands::Show { id } => println!("[not implemented] show {id}"),
        Commands::Close { id } => println!("[not implemented] close {id}"),
        Commands::Done => println!("[not implemented] done"),
        Commands::Release { id } => println!("[not implemented] release {id}"),
        Commands::Hook => println!("[not implemented] hook"),
        Commands::Ready => println!("[not implemented] ready"),
        Commands::Sling { work_id, agent } => println!("[not implemented] sling {work_id} -> {agent}"),
        Commands::Unsling { agent } => println!("[not implemented] unsling {agent}"),
        Commands::Mail { command } => mail::handle(command).await?,
        Commands::Nudge { agent, message } => println!("[not implemented] nudge {agent}: {message}"),
        Commands::Broadcast { message } => println!("[not implemented] broadcast: {message}"),
        Commands::Escalate { id } => println!("[not implemented] escalate {id}"),
        Commands::Agents => println!("[not implemented] agents"),
        Commands::Mayor { command } => agents::handle_mayor(command).await?,
        Commands::Deacon { action } => println!("[not implemented] deacon {action}"),
        Commands::Witness { action } => println!("[not implemented] witness {action}"),
        Commands::Refinery { action } => println!("[not implemented] refinery {action}"),
        Commands::Polecat { command } => agents::handle_polecat(command).await?,
        Commands::Boot => println!("[not implemented] boot"),
        Commands::Worker => println!("[not implemented] worker"),
        Commands::Up => println!("[not implemented] up"),
        Commands::Down => println!("[not implemented] down"),
        Commands::Install { path } => workspace::handle_install(&path).await?,
        Commands::Init => println!("[not implemented] init"),
        Commands::Rig { command } => workspace::handle_rig(command).await?,
        Commands::Crew { command } => workspace::handle_crew(command).await?,
        Commands::Doctor => println!("[not implemented] doctor"),
        Commands::Prime => println!("[not implemented] prime"),
        Commands::Status => println!("[not implemented] status"),
        Commands::Formula { command } => work::handle_formula(command).await?,
    }
    Ok(())
}
```

**Step 4: Verify it builds and help works**

Run: `cd gtr && cargo build`
Expected: Compiles

Run: `cd gtr && cargo run --bin gtr -- --help`
Expected: Shows all subcommands

Run: `cd gtr && cargo run --bin gtr -- convoy --help`
Expected: Shows convoy subcommands

**Step 5: Commit**

```bash
git add gtr/crates/gtr-cli/
git commit -m "feat: add CLI skeleton with all subcommands (stubs)"
```

---

## Task 7: Workspace Install Command

**Files:**
- Modify: `gtr/crates/gtr-cli/src/commands/workspace.rs`

**Step 1: Implement `gtr install`**

This command creates the HQ directory structure with default config files:

```rust
pub async fn handle_install(path: &str) -> anyhow::Result<()> {
    let root = PathBuf::from(path);
    if root.join(".gtr").exists() {
        anyhow::bail!("already initialized: {}", root.display());
    }

    let gtr_dir = root.join(".gtr");
    fs::create_dir_all(&gtr_dir)?;

    let config = TownConfig {
        name: root.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "gastown".into()),
        namespace: "default".into(),
        temporal_address: "http://localhost:7233".into(),
    };
    fs::write(gtr_dir.join("config.toml"), toml::to_string_pretty(&config)?)?;

    let rigs = RigsConfig { rigs: vec![] };
    fs::write(gtr_dir.join("rigs.toml"), toml::to_string_pretty(&rigs)?)?;

    fs::create_dir_all(root.join("plugins"))?;

    println!("Initialized Gas Town HQ at {}", root.display());
    Ok(())
}
```

**Step 2: Verify manually**

Run: `cd gtr && cargo run --bin gtr -- install /tmp/test-gtr`
Expected: Creates `/tmp/test-gtr/.gtr/config.toml`, `/tmp/test-gtr/.gtr/rigs.toml`, `/tmp/test-gtr/plugins/`

Run: `cat /tmp/test-gtr/.gtr/config.toml`
Expected: Shows TOML with name, namespace, temporal_address

**Step 3: Clean up and commit**

```bash
rm -rf /tmp/test-gtr
git add gtr/crates/gtr-cli/src/commands/workspace.rs
git commit -m "feat: implement gtr install — creates HQ directory structure"
```

---

## Task 8: Add Temporal SDK Dependencies

**Files:**
- Modify: `gtr/Cargo.toml` (workspace)
- Create: `gtr/crates/gtr-temporal/Cargo.toml`
- Create: `gtr/crates/gtr-temporal/src/lib.rs`

**Step 1: Add temporal crates to workspace**

Add to workspace `Cargo.toml` members:
```toml
members = [
    "crates/gtr-core",
    "crates/gtr-cli",
    "crates/gtr-temporal",
]
```

Add to workspace dependencies:
```toml
# Temporal SDK (pre-alpha, pinned)
temporalio-sdk = { git = "https://github.com/temporalio/sdk-core", rev = "7ecb7c0" }
temporalio-sdk-core = { git = "https://github.com/temporalio/sdk-core", rev = "7ecb7c0" }
temporalio-client = { git = "https://github.com/temporalio/sdk-core", rev = "7ecb7c0" }
```

**Step 2: Create gtr-temporal crate**

`gtr/crates/gtr-temporal/Cargo.toml`:
```toml
[package]
name = "gtr-temporal"
version.workspace = true
edition.workspace = true

[dependencies]
gtr-core = { path = "../gtr-core" }
temporalio-sdk = { workspace = true }
temporalio-sdk-core = { workspace = true }
temporalio-client = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
```

`gtr/crates/gtr-temporal/src/lib.rs`:
```rust
pub mod workflows;
pub mod activities;
pub mod signals;
```

Create empty module files:
- `gtr/crates/gtr-temporal/src/workflows/mod.rs` — `pub mod work_item;`
- `gtr/crates/gtr-temporal/src/workflows/work_item.rs` — empty
- `gtr/crates/gtr-temporal/src/activities/mod.rs` — empty
- `gtr/crates/gtr-temporal/src/signals.rs` — empty

**Step 3: Verify it compiles**

Run: `cd gtr && cargo build`
Expected: Compiles (may take a while first time to fetch temporal deps)

Note: If the temporal SDK git rev doesn't resolve or has breaking changes, try the latest master commit. The SDK is pre-alpha so you may need to adjust. Check `cargo build` errors and fix imports as needed.

**Step 4: Commit**

```bash
git add gtr/Cargo.toml gtr/Cargo.lock gtr/crates/gtr-temporal/
git commit -m "feat: add gtr-temporal crate with Temporal SDK dependencies"
```

---

## Task 9: Hello-World Temporal Workflow

**Files:**
- Modify: `gtr/crates/gtr-temporal/src/workflows/work_item.rs`
- Modify: `gtr/crates/gtr-cli/src/commands/services.rs`
- Modify: `gtr/crates/gtr-cli/Cargo.toml` (add gtr-temporal dep)

This task proves the Temporal SDK integration works end-to-end. We write a minimal workflow, register it with a worker, and start it from the CLI.

**Step 1: Write a minimal workflow**

`gtr/crates/gtr-temporal/src/workflows/work_item.rs`:
```rust
use temporalio_sdk::WfContext;
use temporalio_sdk::WfExitValue;
use anyhow::Result;

pub async fn work_item_wf(ctx: WfContext) -> Result<WfExitValue<String>> {
    let args = ctx.get_args();
    let title = if let Some(payload) = args.first() {
        serde_json::from_slice::<String>(&payload.data)
            .unwrap_or_else(|_| "untitled".into())
    } else {
        "untitled".into()
    };
    tracing::info!("WorkItem workflow started: {}", title);
    Ok(WfExitValue::Normal(format!("WorkItem created: {}", title)))
}
```

**Step 2: Implement `gtr worker` command**

This starts a Temporal worker that registers the workflow:

```rust
// In services.rs or a new worker module
use temporalio_sdk::Worker;
use temporalio_sdk_core::{
    init_worker, CoreRuntime,
    telemetry::TelemetryOptionsBuilder,
    WorkerConfigBuilder,
};
use temporalio_client::ClientOptionsBuilder;

pub async fn run_worker() -> anyhow::Result<()> {
    let telemetry = TelemetryOptionsBuilder::default().build()?;
    let runtime = CoreRuntime::new_assume_tokio(telemetry)?;

    let client_opts = ClientOptionsBuilder::default()
        .target_url("http://localhost:7233".parse()?)
        .client_name("gtr-worker")
        .client_version(env!("CARGO_PKG_VERSION"))
        .build()?;

    let client = client_opts.connect("default", None).await?;

    let worker_config = WorkerConfigBuilder::default()
        .task_queue("work")
        .worker_build_id(env!("CARGO_PKG_VERSION"))
        .build()?;

    let core_worker = init_worker(&runtime, worker_config, client)?;
    let mut worker = Worker::new_from_core(core_worker, 200);

    worker.register_wf(
        "work_item_wf",
        gtr_temporal::workflows::work_item::work_item_wf,
    );

    tracing::info!("gtr worker started on task queue 'work'");
    worker.run().await?;
    Ok(())
}
```

**Step 3: Test manually**

Prerequisites: `temporal server start-dev` running in another terminal.

Run: `cd gtr && cargo run --bin gtr -- worker`
Expected: "gtr worker started on task queue 'work'" — worker is polling

In another terminal:
Run: `temporal workflow start --task-queue work --type work_item_wf --input '"Test Item"' --workflow-id test-1`
Expected: Workflow starts and completes

Run: `temporal workflow show --workflow-id test-1`
Expected: Shows completed workflow with result "WorkItem created: Test Item"

**Step 4: Commit**

```bash
git add gtr/
git commit -m "feat: hello-world Temporal workflow — proves SDK integration"
```

---

## Task 10: WorkItem Workflow with State Machine

**Files:**
- Modify: `gtr/crates/gtr-temporal/src/workflows/work_item.rs`
- Modify: `gtr/crates/gtr-temporal/src/signals.rs`

**Step 1: Define signal types**

`gtr/crates/gtr-temporal/src/signals.rs`:
```rust
use serde::{Deserialize, Serialize};

pub const SIGNAL_ASSIGN: &str = "assign";
pub const SIGNAL_START: &str = "start";
pub const SIGNAL_COMPLETE: &str = "complete";
pub const SIGNAL_FAIL: &str = "fail";
pub const SIGNAL_CLOSE: &str = "close";
pub const SIGNAL_RELEASE: &str = "release";
pub const SIGNAL_HEARTBEAT: &str = "heartbeat";
pub const SIGNAL_ESCALATE: &str = "escalate";

pub const QUERY_STATUS: &str = "status";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignSignal {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatSignal {
    pub progress: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailSignal {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemState {
    pub id: String,
    pub title: String,
    pub status: String,
    pub assigned_to: Option<String>,
}
```

**Step 2: Implement full WorkItem workflow with signal handling**

`gtr/crates/gtr-temporal/src/workflows/work_item.rs`:
```rust
use temporalio_sdk::{WfContext, WfExitValue};
use crate::signals::*;
use anyhow::Result;

pub async fn work_item_wf(ctx: WfContext) -> Result<WfExitValue<String>> {
    // Parse input
    let args = ctx.get_args();
    let (id, title) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "untitled".into()))
    } else {
        ("unknown".into(), "untitled".into())
    };

    let mut status = "pending".to_string();
    let mut assigned_to: Option<String> = None;

    // Set up signal channels
    let assign_signals = ctx.make_signal_channel(SIGNAL_ASSIGN);
    let start_signals = ctx.make_signal_channel(SIGNAL_START);
    let complete_signals = ctx.make_signal_channel(SIGNAL_COMPLETE);
    let fail_signals = ctx.make_signal_channel(SIGNAL_FAIL);
    let close_signals = ctx.make_signal_channel(SIGNAL_CLOSE);
    let release_signals = ctx.make_signal_channel(SIGNAL_RELEASE);

    // Register query handler
    // Note: The Rust SDK query API may differ — adapt as needed
    // ctx.update_handler(...) or similar for queries

    tracing::info!("WorkItem {id} started: {title}");

    // Main signal loop — wait for signals and transition state
    loop {
        tokio::select! {
            signal = assign_signals.recv() => {
                if let Some(signal) = signal {
                    if status == "pending" {
                        let data: AssignSignal = serde_json::from_slice(&signal.data)?;
                        assigned_to = Some(data.agent_id.clone());
                        status = "assigned".to_string();
                        tracing::info!("WorkItem {id} assigned to {}", data.agent_id);
                    }
                }
            }
            _ = start_signals.recv() => {
                if status == "assigned" {
                    status = "in_progress".to_string();
                    tracing::info!("WorkItem {id} in progress");
                }
            }
            _ = complete_signals.recv() => {
                if status == "in_progress" || status == "assigned" {
                    status = "done".to_string();
                    tracing::info!("WorkItem {id} completed");
                    return Ok(WfExitValue::Normal(
                        serde_json::to_string(&WorkItemState {
                            id: id.clone(),
                            title: title.clone(),
                            status: status.clone(),
                            assigned_to: assigned_to.clone(),
                        })?
                    ));
                }
            }
            signal = fail_signals.recv() => {
                if let Some(signal) = signal {
                    let data: FailSignal = serde_json::from_slice(&signal.data)?;
                    status = "failed".to_string();
                    tracing::warn!("WorkItem {id} failed: {}", data.reason);
                    return Ok(WfExitValue::Normal(
                        serde_json::to_string(&WorkItemState {
                            id, title, status, assigned_to,
                        })?
                    ));
                }
            }
            _ = close_signals.recv() => {
                status = "closed".to_string();
                tracing::info!("WorkItem {id} closed");
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&WorkItemState {
                        id, title, status, assigned_to,
                    })?
                ));
            }
            _ = release_signals.recv() => {
                if status == "assigned" || status == "in_progress" {
                    assigned_to = None;
                    status = "pending".to_string();
                    tracing::info!("WorkItem {id} released back to pending");
                }
            }
        }
    }
}
```

**Important note:** The exact signal channel API (`make_signal_channel`, `recv()`) is based on the SDK research. The pre-alpha API may use `DrainableSignalStream` or a different receive pattern. Adapt the code to match what compiles. The key pattern is: create named signal channels, loop waiting for signals, transition state.

**Step 3: Verify it compiles**

Run: `cd gtr && cargo build`
Expected: Compiles (may need API adjustments)

**Step 4: Manual test with Temporal CLI**

With worker running:
```bash
temporal workflow start --task-queue work --type work_item_wf \
  --input '["hq-test1", "My first work item"]' --workflow-id hq-test1

temporal workflow signal --workflow-id hq-test1 --name assign \
  --input '{"agent_id": "polecat-Toast"}'

temporal workflow signal --workflow-id hq-test1 --name complete
```

Check: `temporal workflow show --workflow-id hq-test1`
Expected: Completed with state showing status "done", assigned_to "polecat-Toast"

**Step 5: Commit**

```bash
git add gtr/crates/gtr-temporal/
git commit -m "feat: WorkItem workflow with full state machine and signal handling"
```

---

## Task 11: CLI Commands for WorkItem (show, close)

**Files:**
- Create: `gtr/crates/gtr-cli/src/client.rs`
- Modify: `gtr/crates/gtr-cli/src/commands/work.rs`
- Modify: `gtr/crates/gtr-cli/src/main.rs`

**Step 1: Create shared Temporal client helper**

`gtr/crates/gtr-cli/src/client.rs`:
```rust
use temporalio_client::{ClientOptionsBuilder, RetryClient, Client};

pub async fn connect() -> anyhow::Result<RetryClient<Client>> {
    // TODO: read address from .gtr/config.toml
    let opts = ClientOptionsBuilder::default()
        .target_url("http://localhost:7233".parse()?)
        .client_name("gtr-cli")
        .client_version(env!("CARGO_PKG_VERSION"))
        .build()?;
    let client = opts.connect("default", None).await?;
    Ok(client)
}
```

**Step 2: Implement `gtr show <id>` and `gtr close <id>`**

```rust
// work.rs
pub async fn handle_show(id: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    // Query the workflow for its state
    let result = client.describe_workflow_execution(id, None).await?;
    // Print workflow status from Temporal
    println!("{:#?}", result);
    Ok(())
}

pub async fn handle_close(id: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    client.signal_workflow_execution(
        id.to_string(),
        None, // run_id
        "close".to_string(),
        None, // input
        None, // header
    ).await?;
    println!("Closed work item: {id}");
    Ok(())
}
```

**Step 3: Wire into main.rs, build, test**

Run: `cd gtr && cargo build`

With worker + temporal running:
```bash
cargo run --bin gtr -- show hq-test1
cargo run --bin gtr -- close hq-test1
```

**Step 4: Commit**

```bash
git add gtr/crates/gtr-cli/
git commit -m "feat: implement gtr show and gtr close commands"
```

---

## Task 12: Convoy Workflow

**Files:**
- Create: `gtr/crates/gtr-temporal/src/workflows/convoy.rs`
- Modify: `gtr/crates/gtr-temporal/src/workflows/mod.rs`
- Modify: `gtr/crates/gtr-temporal/src/signals.rs`

**Step 1: Add convoy signals**

Add to `signals.rs`:
```rust
pub const SIGNAL_ADD_WORK_ITEM: &str = "add_work_item";
pub const SIGNAL_ITEM_DONE: &str = "item_done";
pub const SIGNAL_CANCEL_CONVOY: &str = "cancel_convoy";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddWorkItemSignal {
    pub work_item_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDoneSignal {
    pub work_item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvoyState {
    pub id: String,
    pub title: String,
    pub status: String,
    pub work_items: Vec<String>,
    pub completed_items: Vec<String>,
}
```

**Step 2: Implement convoy workflow**

The convoy starts child WorkItem workflows and tracks their completion. When all items are done, the convoy closes.

```rust
// convoy.rs
use temporalio_sdk::{WfContext, WfExitValue, ChildWorkflowOptions};
use crate::signals::*;
use anyhow::Result;
use std::collections::HashSet;

pub async fn convoy_wf(ctx: WfContext) -> Result<WfExitValue<String>> {
    let args = ctx.get_args();
    let (id, title) = if let Some(payload) = args.first() {
        serde_json::from_slice::<(String, String)>(&payload.data)
            .unwrap_or(("unknown".into(), "untitled".into()))
    } else {
        ("unknown".into(), "untitled".into())
    };

    let mut work_items: Vec<String> = vec![];
    let mut completed: HashSet<String> = HashSet::new();
    let mut status = "open".to_string();

    let add_item_signals = ctx.make_signal_channel(SIGNAL_ADD_WORK_ITEM);
    let item_done_signals = ctx.make_signal_channel(SIGNAL_ITEM_DONE);
    let cancel_signals = ctx.make_signal_channel(SIGNAL_CANCEL_CONVOY);
    let close_signals = ctx.make_signal_channel(SIGNAL_CLOSE);

    tracing::info!("Convoy {id} started: {title}");

    loop {
        tokio::select! {
            signal = add_item_signals.recv() => {
                if let Some(signal) = signal {
                    let data: AddWorkItemSignal = serde_json::from_slice(&signal.data)?;
                    work_items.push(data.work_item_id.clone());

                    // Start child WorkItem workflow
                    let child_opts = ChildWorkflowOptions {
                        workflow_id: data.work_item_id.clone(),
                        workflow_type: "work_item_wf".into(),
                        task_queue: "work".into(),
                        input: vec![serde_json::to_vec(&(
                            &data.work_item_id,
                            &data.title,
                        ))?],
                        ..Default::default()
                    };
                    let _child = ctx.child_workflow(child_opts).start(&ctx).await?;
                    tracing::info!("Convoy {id}: added work item {}", data.work_item_id);
                }
            }
            signal = item_done_signals.recv() => {
                if let Some(signal) = signal {
                    let data: ItemDoneSignal = serde_json::from_slice(&signal.data)?;
                    completed.insert(data.work_item_id.clone());
                    tracing::info!("Convoy {id}: item {} done ({}/{})",
                        data.work_item_id, completed.len(), work_items.len());

                    if !work_items.is_empty() && completed.len() == work_items.len() {
                        status = "closed".to_string();
                        tracing::info!("Convoy {id} complete — all items done");
                        return Ok(WfExitValue::Normal(
                            serde_json::to_string(&ConvoyState {
                                id, title, status,
                                work_items,
                                completed_items: completed.into_iter().collect(),
                            })?
                        ));
                    }
                }
            }
            _ = close_signals.recv() => {
                status = "closed".to_string();
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&ConvoyState {
                        id, title, status,
                        work_items,
                        completed_items: completed.into_iter().collect(),
                    })?
                ));
            }
            _ = cancel_signals.recv() => {
                status = "failed".to_string();
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&ConvoyState {
                        id, title, status,
                        work_items,
                        completed_items: completed.into_iter().collect(),
                    })?
                ));
            }
        }
    }
}
```

**Step 3: Register in worker, build, test**

Add to worker: `worker.register_wf("convoy_wf", convoy_wf);`

Test with Temporal CLI:
```bash
temporal workflow start --task-queue work --type convoy_wf \
  --input '["hq-cv-test1", "Feature X"]' --workflow-id hq-cv-test1

temporal workflow signal --workflow-id hq-cv-test1 --name add_work_item \
  --input '{"work_item_id": "hq-a1b2c", "title": "Build login page"}'
```

**Step 4: Commit**

```bash
git add gtr/crates/gtr-temporal/
git commit -m "feat: Convoy workflow — tracks work items, auto-closes on completion"
```

---

## Task 13: Convoy CLI Commands

**Files:**
- Modify: `gtr/crates/gtr-cli/src/commands/convoy.rs`

**Step 1: Implement convoy create**

```rust
pub async fn handle_create(title: &str, priority: &str) -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    let id = gtr_core::ids::convoy_id("hq");

    client.start_workflow(
        vec![serde_json::to_vec(&(&id.0, title))?],
        "work".to_string(),          // task queue
        id.0.clone(),                 // workflow id
        "convoy_wf".to_string(),      // workflow type
        None,                         // options
    ).await?;

    println!("Created convoy: {} — {}", id.0, title);
    Ok(())
}
```

**Step 2: Implement convoy list (via Temporal visibility API)**

```rust
pub async fn handle_list() -> anyhow::Result<()> {
    let client = crate::client::connect().await?;
    // Use Temporal visibility API to list open workflows of type convoy_wf
    let query = "WorkflowType = 'convoy_wf' AND ExecutionStatus = 'Running'";
    let results = client.list_workflow_executions(query.into()).await?;
    for execution in results {
        println!("{} — {}", execution.workflow_id, execution.status);
    }
    Ok(())
}
```

**Step 3: Build and test, commit**

```bash
git add gtr/crates/gtr-cli/
git commit -m "feat: implement gtr convoy create/list/show commands"
```

---

## Tasks 14-23: Remaining Build Phases

The remaining tasks follow the same TDD pattern. Summarized here for brevity — each is a self-contained task with the same step structure (write code, build, test manually with Temporal CLI, commit).

### Task 14: Agent Workflow + Signal Channels
- `gtr/crates/gtr-temporal/src/workflows/agent.rs`
- Long-running workflow with Idle/Working states
- Signal channels: assign, mail, nudge, stop
- Query handlers: status, inbox, hooked_work
- Register in worker on "agents" task queue

### Task 15: SpawnAgent Activity
- `gtr/crates/gtr-temporal/src/activities/spawn_agent.rs`
- Takes `AgentRuntime` config, spawns child process via `tokio::process::Command`
- Returns process ID / handle info
- Start with a mock agent script (`echo "hello from mock agent"`)

### Task 16: ReadAgentOutput Activity
- `gtr/crates/gtr-temporal/src/activities/agent_io.rs`
- Reads from agent stdout in a loop
- Calls `ctx.record_heartbeat()` periodically
- Returns when process exits

### Task 17: Sling CLI Command
- `gtr/crates/gtr-cli/src/commands/sling.rs`
- `gtr sling <work-id> <agent>` → signal AgentWorkflow with assign
- `gtr unsling <agent>` → signal AgentWorkflow with unassign
- `gtr hook` → query AgentWorkflow for hooked work

### Task 18: Mayor Workflow
- `gtr/crates/gtr-temporal/src/workflows/mayor.rs`
- Singleton, long-running
- Tracks: active convoys, registered agents, rig list
- Signal channels: create_convoy, agent_status, convoy_closed
- Query handlers: status, list_convoys, list_agents, ready_work
- Register on "mayor" task queue

### Task 19: Mayor CLI + Status + Up/Down
- `gtr mayor attach` — follow mayor workflow events
- `gtr status` — query mayor for town state
- `gtr up` — start worker + mayor + deacon workflows
- `gtr down` — cancel all workflows, stop worker

### Task 20: Mail System (gtr-mail crate)
- Create: `gtr/crates/gtr-mail/`
- Add to workspace members
- Mail is signals on Agent workflows
- `gtr mail send <to> <msg>` → signal AgentWorkflow(to, "mail", msg)
- `gtr mail inbox` → query AgentWorkflow(self, "inbox")
- `gtr mail thread <id>` → query AgentWorkflow(self, "thread", id)
- Nudge and broadcast are thin wrappers

### Task 21: Plugin System (gtr-plugins crate)
- Create: `gtr/crates/gtr-plugins/`
- TOML plugin definition parsing
- Gate types: cooldown, cron, condition, event
- `RunPlugin` activity: spawns command, captures output
- Plugin registry: discovers `.toml` files in `plugins/` dirs

### Task 22: Patrol Workflow + Deacon
- `gtr/crates/gtr-temporal/src/workflows/patrol.rs`
- Timer-based loop: sleep(interval) → check gates → run eligible plugins
- Deacon is an AgentWorkflow with patrol as its work loop
- Witness: queries polecat workflows for staleness

### Task 23: Formula System
- `gtr/crates/gtr-temporal/src/workflows/formula.rs`
- Parse TOML formula definitions
- Build DAG from `depends_on`
- Execute steps as child workflows / activities
- Variable interpolation: `{{var_name}}`
- `gtr formula cook <name> --var key=value`

### Task 24: Refinery (Merge Queue)
- `gtr/crates/gtr-temporal/src/workflows/refinery.rs`
- FIFO queue with priority override
- Activities: validate (run tests), merge (git operations)
- `gtr done` → signal refinery to enqueue
- `gtr mq list/status` — query refinery workflow

### Task 25: GitOperation Activity
- `gtr/crates/gtr-temporal/src/activities/git.rs`
- Uses `git2` crate for: clone, checkout, commit, push, create worktree
- Used by refinery (merge), formula steps (branch creation), sling (worktree setup)

### Task 26: Escalation
- Modify WorkItem workflow: add staleness timer
- If no heartbeat within `stale_after`, walk escalation chain
- `SendNotification` activity: email/SMS/webhook stubs
- `gtr escalate <id>` → signal WorkItem to escalate immediately

### Task 27: Monitoring Agents (Witness, Boot)
- Witness: periodic query of polecat workflows, detect stuck ones
- Boot: periodic health check of Deacon, restart if missing
- `gtr doctor` — check Temporal connection, worker status, agent health

### Task 28: Diagnostics
- `gtr dashboard` — web server (axum) serving Temporal query results as HTML
- `gtr feed` — stream Temporal workflow events to terminal
- `gtr audit <actor>` — query workflow histories filtered by agent
- `gtr trail` — recent activity across all workflows

### Task 29: Polish
- Shell completion generation (`clap_complete`)
- Colored output (`colored` crate)
- Better error messages with context
- `gtr version` with build info
- `gtr help` improvements

---

## Prerequisites

Before starting:
1. Install Rust toolchain: `rustup default stable`
2. Install Temporal CLI: `brew install temporal` (or from GitHub releases)
3. Start local dev server: `temporal server start-dev` (keep running in a terminal)
4. Verify: `temporal server --version` and `temporal workflow list` (should return empty)

## Notes for the implementer

- **The Temporal Rust SDK is pre-alpha.** The API in this plan is based on research as of Feb 2026. If types/methods don't match, check the SDK source at `https://github.com/temporalio/sdk-core/tree/master/crates/sdk/src`. Adapt the code to match what actually compiles.
- **Signal channel API:** The plan uses `ctx.make_signal_channel("name")` and `.recv()`. The actual SDK may use `DrainableSignalStream` with different methods. Check `workflow_context.rs` in the SDK.
- **Client API:** The plan uses methods like `client.start_workflow()`, `client.signal_workflow_execution()`, `client.list_workflow_executions()`. Check `temporalio-client` crate for actual method names.
- **When something doesn't compile:** Don't fight the SDK. Check the source, adapt the pattern. The concepts are right even if the exact API surface shifts.
- **Test with Temporal CLI:** `temporal workflow start`, `temporal workflow signal`, `temporal workflow show` are your best friends for testing workflows manually.
