# Gas Town Rusted (gtr) — Rust + Temporal Design

A full rewrite of Gas Town in Rust, replacing Beads (git-backed JSONL/SQLite) with Temporal workflows as the durable execution layer.

## Motivation

Greenfield rewrite to own the codebase in Rust + Temporal. Not a 1:1 port — rethinks the state model while preserving all Gas Town concepts.

## Approach

Pure Rust using the pre-alpha Temporal Rust SDK (`temporal-sdk` from `temporalio/sdk-core` pinned to a git rev). Single language across the entire stack. The SDK is functional (built on the production Core) but API-unstable, which is acceptable for a greenfield project.

---

## 1. Architecture Overview

Every Gas Town concept maps to a Temporal primitive:

| Go Gas Town | Rust Gas Town |
|---|---|
| Beads (JSONL + SQLite ledger) | Temporal Workflows |
| File-based mail | Temporal Signals |
| Git-backed state | Temporal's event-sourced history |
| Process spawning for agents | Temporal Activities |
| Formulas (TOML templates) | Temporal Child Workflows |
| Convoys (bead groups) | Parent Workflows coordinating child workflows |

**Single binary:** `gtr` serves as both the CLI (user commands) and the Temporal worker (`gtr worker`).

**Temporal infrastructure:** Local dev server first (`temporal server start-dev`), designed for easy migration to Temporal Cloud later.

---

## 2. Crate Structure

Cargo workspace with 6 crates:

```
gtr/
├── Cargo.toml                    (workspace root)
├── crates/
│   ├── gtr-cli/                  (binary — clap CLI, all subcommands)
│   │   └── src/
│   │       ├── main.rs
│   │       ├── client.rs         (shared Temporal client setup)
│   │       └── commands/
│   │           ├── mod.rs
│   │           ├── convoy.rs
│   │           ├── sling.rs
│   │           ├── mail.rs
│   │           ├── mayor.rs
│   │           ├── agents.rs
│   │           ├── work.rs
│   │           ├── formula.rs
│   │           ├── services.rs
│   │           ├── workspace.rs
│   │           ├── config.rs
│   │           ├── comms.rs
│   │           └── diagnostics.rs
│   │
│   ├── gtr-worker/               (binary — Temporal worker runtime)
│   │   └── src/
│   │       ├── main.rs
│   │       └── lib.rs
│   │
│   ├── gtr-core/                 (lib — shared types, config, models)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs
│   │       ├── ids.rs
│   │       ├── config.rs
│   │       ├── rig.rs
│   │       ├── crew.rs
│   │       └── errors.rs
│   │
│   ├── gtr-temporal/             (lib — workflow & activity definitions)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── signals.rs
│   │       ├── workflows/
│   │       │   ├── mod.rs
│   │       │   ├── mayor.rs
│   │       │   ├── convoy.rs
│   │       │   ├── work_item.rs
│   │       │   ├── agent.rs
│   │       │   ├── formula.rs
│   │       │   ├── refinery.rs
│   │       │   └── patrol.rs
│   │       └── activities/
│   │           ├── mod.rs
│   │           ├── spawn_agent.rs
│   │           ├── agent_io.rs
│   │           ├── git.rs
│   │           ├── plugin.rs
│   │           ├── notification.rs
│   │           ├── health.rs
│   │           ├── merge.rs
│   │           └── formula_step.rs
│   │
│   ├── gtr-mail/                 (lib — mail via Temporal signals)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── inbox.rs
│   │       ├── send.rs
│   │       └── thread.rs
│   │
│   └── gtr-plugins/              (lib — plugin system)
│       └── src/
│           ├── lib.rs
│           ├── gate.rs
│           ├── registry.rs
│           └── runner.rs
```

---

## 3. Temporal Workflow Model

### Workflow Types (7)

**MayorWorkflow** — Singleton, long-running town coordinator.
- One per town, runs for the lifetime of the workspace.
- Maintains town state: registered rigs, active agents, escalation config.
- Receives signals: new convoys, escalations, agent status updates.
- Queries: town status, active convoys, agent health.
- Spawns child workflows for convoys.

**ConvoyWorkflow** — Medium-lived parent workflow.
- One per batch of work. Created by Mayor or CLI.
- Groups related work items.
- Tracks progress: pending → in_progress → done/failed.
- Spawns child WorkItem workflows.
- Closes when all children complete.
- Signals: add work items, cancel, synthesis steps.

**WorkItemWorkflow** — Replaces "bead" as the fundamental work unit.
- State machine: `pending → assigned → in_progress → review → done | failed`
- Assigned to an agent via signal (replaces `gt sling`).
- Tracks lifecycle, doesn't execute agent code directly.
- Signals: assign, start, heartbeat, complete, fail, escalate.
- Queries: status, assigned agent, history.

**AgentWorkflow** — One per active agent, manages agent lifecycle.
- Tracks: spawned → idle → working → stopped.
- Receives work assignments via signal.
- Executes Agent Activities for actual work.
- Heartbeats back to WorkItem workflows.
- Polecats: workflow completes when task is done (ephemeral).
- Persistent agents (mayor, deacon): run continuously, awaiting signals.

**FormulaWorkflow** — Parameterized multi-step template execution.
- Parses TOML formula, interpolates variables.
- Builds DAG from step dependencies.
- Executes steps as child workflows/activities respecting DAG order.

**RefineryWorkflow** — Merge queue processor.
- FIFO with priority override.
- Per-rig sequential processing to avoid merge conflicts.
- Steps: validate (tests/checks) → merge (git) → report.

**PatrolWorkflow** — Deacon patrol cycle.
- Runs on configured interval.
- Iterates registered plugins, checks gates, dispatches eligible ones.

### Activity Types (9)

| Activity | Purpose |
|---|---|
| `SpawnAgent` | Launch a Claude/Codex/Cursor CLI process |
| `SendAgentCommand` | Inject input to a running agent process |
| `ReadAgentOutput` | Stream agent output with Temporal heartbeating |
| `GitOperation` | Clone, checkout, commit, push, create worktree |
| `RunPlugin` | Execute a gated plugin |
| `SendNotification` | Escalation: email, SMS, webhook |
| `CheckHealth` | Agent/daemon health probe |
| `MergeQueueProcess` | Validate + merge |
| `FormulaStep` | Execute a single formula step |

### Signal Flow

```
Human → CLI → Signal(MayorWorkflow, "create_convoy")
  → Mayor spawns ConvoyWorkflow
  → Convoy spawns WorkItemWorkflows
  → Mayor signals AgentWorkflow("assign", work_item_id)
  → Agent executes SpawnAgent activity
  → Agent signals WorkItemWorkflow("heartbeat" / "complete")
  → WorkItem signals ConvoyWorkflow("item_done")
  → Convoy checks if all done → signals MayorWorkflow("convoy_closed")
```

### Temporal Namespace & Task Queues

```
Namespace: "gastown" (configurable)

Task Queues:
  - "mayor"     (mayor workflow + activities)
  - "agents"    (all agent workflows)
  - "work"      (work item + convoy workflows)
  - "plugins"   (plugin execution activities)
```

---

## 4. CLI Command Surface

Binary name: `gtr`. Every command is a thin wrapper that signals a workflow, queries a workflow, or performs a local operation.

### Work Management

| Command | Temporal Operation |
|---|---|
| `gtr convoy create` | Start ConvoyWorkflow |
| `gtr convoy list` | Query MayorWorkflow("list_convoys") |
| `gtr convoy show <id>` | Query ConvoyWorkflow(id) |
| `gtr close <id>` | Signal WorkItemWorkflow(id, "close") |
| `gtr sling <work> <agent>` | Signal AgentWorkflow(agent, "assign", work) |
| `gtr unsling <agent>` | Signal AgentWorkflow(agent, "unassign") |
| `gtr hook` | Query current AgentWorkflow("hooked_work") |
| `gtr done` | Signal WorkItemWorkflow("done") + trigger merge queue |
| `gtr show <id>` | Query WorkItemWorkflow(id) |
| `gtr ready` | Query MayorWorkflow("ready_work") |
| `gtr release <id>` | Signal WorkItemWorkflow(id, "release") |
| `gtr formula cook <name>` | Start FormulaWorkflow(name) |
| `gtr mol <subcommand>` | Query/Signal molecule workflows |

### Agent Management

| Command | Temporal Operation |
|---|---|
| `gtr mayor attach` | Query + follow MayorWorkflow |
| `gtr deacon start/stop` | Start/Cancel DeaconAgentWorkflow |
| `gtr witness start/stop` | Start/Cancel WitnessAgentWorkflow |
| `gtr refinery start/stop` | Start/Cancel RefineryAgentWorkflow |
| `gtr polecat spawn` | Start PolecatAgentWorkflow |
| `gtr agents` | Query MayorWorkflow("list_agents") |
| `gtr boot` | Start BootAgentWorkflow |

### Communication

| Command | Temporal Operation |
|---|---|
| `gtr mail send <to> <msg>` | Signal AgentWorkflow(to, "mail", msg) |
| `gtr mail inbox` | Query AgentWorkflow(self, "inbox") |
| `gtr mail thread <id>` | Query AgentWorkflow(self, "thread", id) |
| `gtr nudge <agent> <msg>` | Signal AgentWorkflow(agent, "nudge", msg) |
| `gtr broadcast <msg>` | Signal all AgentWorkflows("nudge", msg) |
| `gtr escalate <id>` | Signal WorkItemWorkflow(id, "escalate") |

### Services

| Command | Temporal Operation |
|---|---|
| `gtr worker` | Start Temporal worker (long-running) |
| `gtr up` | Start worker + all persistent agent workflows |
| `gtr down` | Cancel all agent workflows + stop worker |
| `gtr daemon start/stop` | Local daemon for health monitoring |

### Workspace (local, no Temporal)

| Command | Operation |
|---|---|
| `gtr install` | Create HQ directory structure |
| `gtr init` | Initialize a rig |
| `gtr rig add/list` | Manage rig config |
| `gtr crew add/list` | Manage crew config |
| `gtr config` | Read/write config files |
| `gtr doctor` | Health checks |
| `gtr prime` | Output role context |
| `gtr hooks` | List Claude Code hooks |

### Diagnostics

| Command | Temporal Operation |
|---|---|
| `gtr status` | Query MayorWorkflow + local state |
| `gtr dashboard` | Web UI (queries Temporal) |
| `gtr audit <actor>` | Query workflow histories by agent |
| `gtr trail` | Query recent activity |
| `gtr feed` | Stream workflow events (visibility API) |

---

## 5. Data Model

### Core Types

```rust
// Identifiers — prefix + 5-char alphanumeric
struct WorkItemId(String);    // e.g. "hq-abc12"
struct ConvoyId(String);      // e.g. "hq-cv-xyz45"
struct AgentId(String);       // e.g. "mayor", "deacon", "polecat-Toast"

enum WorkItemStatus {
    Pending, Assigned, InProgress, Review, Done, Failed, Closed,
}

enum Priority { Critical, High, Medium, Low }

struct WorkItem {
    id: WorkItemId,
    title: String,
    description: String,
    status: WorkItemStatus,
    priority: Priority,
    assigned_to: Option<AgentId>,
    convoy_id: Option<ConvoyId>,
    labels: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

enum AgentRole { Mayor, Deacon, Witness, Refinery, Polecat, Crew, Dog, Boot }

enum AgentRuntime {
    Claude, Codex, Cursor, Gemini, Amp,
    Custom { command: String, args: Vec<String> },
}

struct AgentConfig {
    id: AgentId,
    role: AgentRole,
    runtime: AgentRuntime,
    rig: String,
}

struct Convoy {
    id: ConvoyId,
    title: String,
    work_items: Vec<WorkItemId>,
    status: ConvoyStatus,
    created_at: DateTime<Utc>,
}
```

### What Lives Where

| Data | Where | Why |
|---|---|---|
| Work items, convoys | Temporal workflows | Durable, queryable, event-sourced |
| Agent state & mail | Temporal workflows | Signals, queries, lifecycle tracking |
| Formulas (templates) | Local TOML files | Static definitions, version-controlled |
| Rig/crew config | Local TOML files | Workspace structure, git-tracked |
| Escalation config | Local TOML files | Policy, git-tracked |
| Plugin definitions | Local files | Code, git-tracked |
| Agent runtime config | Local TOML files | Which CLI to spawn, args |
| Git worktrees | Local filesystem | Managed by git |

### Directory Layout

```
~/gtr/                          (town root / HQ)
├── .gtr/
│   ├── config.toml             (town-wide config)
│   ├── rigs.toml               (registered rigs)
│   └── escalation.toml         (escalation routes)
├── <rig-name>/
│   ├── .gtr/
│   │   ├── rig.toml            (rig config: runtime, agents)
│   │   └── plugins/            (rig-level plugins)
│   ├── crew/<name>/            (crew workspaces)
│   ├── mayor/rig/              (mayor worktree)
│   ├── refinery/rig/           (refinery worktree)
│   ├── witness/                (witness config)
│   └── polecats/               (transient polecat dirs)
└── plugins/                    (town-level plugins)
```

---

## 6. Agent Lifecycle & Process Management

### Agent Workflow State Machine

```
Start → Idle (awaiting signal)
          │
          ├── Signal("assign", work_item_id)
          │     → AssignWork activity
          │     → SpawnAgent activity (launches CLI process)
          │     → Loop: ReadAgentOutput activity (heartbeating)
          │     → Agent completes → Signal WorkItemWorkflow("done")
          │     → Back to Idle
          │
          ├── Signal("mail", message)
          │     → Append to inbox (workflow state)
          │     → If agent running, inject via SendAgentCommand
          │
          ├── Signal("nudge", message)
          │     → Inject into running agent, return response
          │
          ├── Signal("stop")
          │     → Graceful shutdown → Workflow completes
          │
          └── Query("status") / Query("inbox") / Query("hooked_work")
                → Return current workflow state
```

### Crash Recovery

**Agent process dies:** `ReadAgentOutput` activity fails → Temporal retries → `SpawnAgent` re-launches → agent re-primed via `gtr prime` → work continues.

**Worker process dies:** Temporal server holds all workflow state → on restart, workflows resume from last checkpoint → in-flight activities retried automatically.

**Temporal server restarts:** Event-sourced history replays all workflows to current state. No data loss.

### Polecat vs. Persistent Agents

| | Persistent (Mayor, Deacon, etc.) | Polecat |
|---|---|---|
| Workflow lifetime | Runs until explicitly stopped | Completes after single task |
| On work complete | Returns to Idle | Workflow ends, directory cleaned |
| Identity | Fixed (`mayor`, `deacon`) | Named from pool (`Toast`, `Gremlin`) |
| Concurrency | One per role per rig | Many concurrent per rig |

---

## 7. Plugins, Formulas, Escalation & Merge Queue

### Plugins

Run during Deacon patrol cycles as Temporal activities with gating logic.

```toml
# plugins/stale-check.toml
[plugin]
name = "stale-check"
description = "Flag work items idle for too long"

[gate]
type = "cooldown"
interval = "30m"

[run]
command = "gtr stale --auto-escalate"
```

Gate types: `cooldown` (time-based), `cron` (scheduled), `condition` (threshold), `event` (trigger-based).

### Formulas

Predefined multi-step workflows defined in TOML, executed as parameterized Temporal child workflows.

```toml
# .gtr/formulas/feature-branch.toml
[formula]
name = "feature-branch"

[[steps]]
name = "create-branch"
action = "git"
args = ["checkout", "-b", "{{branch_name}}"]

[[steps]]
name = "implement"
action = "sling"
agent = "polecat"
depends_on = ["create-branch"]

[[steps]]
name = "review"
action = "sling"
agent = "crew"
depends_on = ["implement"]

[[steps]]
name = "merge"
action = "mq-submit"
depends_on = ["review"]
```

### Escalation

```toml
# .gtr/escalation.toml
[routes]
critical = ["signal:mayor", "activity:email", "activity:sms"]
high     = ["signal:mayor", "activity:email"]
medium   = ["signal:mayor"]
low      = []

[thresholds]
stale_after = "4h"
max_re_escalations = 2
```

WorkItem workflows detect staleness (no heartbeat within threshold) and walk the escalation chain with timeouts between steps.

### Merge Queue (Refinery)

```
gtr done → Signal RefineryWorkflow("enqueue", work_item_id)
  → Validate activity (run tests/checks)
  → Merge activity (git merge + push)
  → Signal WorkItemWorkflow("merged") or ("merge_failed")
  → Signal ConvoyWorkflow("item_merged")
```

FIFO with priority override. One merge at a time per rig.

---

## 8. Dependencies

```toml
[workspace.dependencies]
# Temporal (pinned to git rev)
temporal-sdk-core = { git = "https://github.com/temporalio/sdk-core", rev = "TBD" }
temporal-sdk = { git = "https://github.com/temporalio/sdk-core", rev = "TBD" }
temporal-client = { git = "https://github.com/temporalio/sdk-core", rev = "TBD" }

# CLI
clap = { version = "4", features = ["derive"] }

# Async
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Time
chrono = { version = "0.4", features = ["serde"] }

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Errors
thiserror = "2"
anyhow = "1"

# Process management (Unix signals)
nix = "0.29"

# Git
git2 = "0.19"

# ID generation
nanoid = "0.4"

# Directory resolution
dirs = "6"
```

---

## 9. Testing Strategy

**Unit tests** (per crate, no Temporal): type construction, ID generation, config parsing, state transitions, gate evaluation, command argument parsing.

**Integration tests** (Temporal test server, in-process): workflow state machines, signal/query flows, activity mocking, escalation timers with time-skipping.

**End-to-end tests** (local Temporal dev server + real CLI): full command flows with a mock agent script instead of real Claude.

---

## 10. Build Phases

1. **Foundation:** `gtr-core` types + `gtr-cli` skeleton with clap (commands print "not implemented")
2. **Temporal connection:** `gtr-temporal` crate, connect to local dev server, hello-world workflow
3. **WorkItem workflow:** Full state machine + `gtr show`/`gtr close`
4. **Convoy workflow:** Parent workflow + `gtr convoy create/list/show`
5. **Agent workflow:** Lifecycle + `gtr sling`/`gtr hook` + `SpawnAgent` with mock agent
6. **Mayor workflow:** Singleton coordinator + `gtr status`/`gtr up`/`gtr down`
7. **Mail:** Signal-based messaging + `gtr mail` commands
8. **Plugins & patrol:** `gtr-plugins` crate + Deacon patrol workflow
9. **Formulas:** TOML parsing + FormulaWorkflow + `gtr formula cook`
10. **Refinery:** Merge queue workflow + `gtr done`/`gtr mq`
11. **Monitoring:** Witness, Boot, escalation, `gtr doctor`
12. **Polish:** Dashboard, feed, audit, shell completion, error messages
