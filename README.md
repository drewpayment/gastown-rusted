# Gas Town Rusted (gtr)

A Rust + Temporal orchestration system that manages fleets of AI coding agents working across git repositories. GTR spawns, monitors, and coordinates Claude Code sessions through native PTY management — no tmux, no screen, just direct process control.

## How It Works

GTR models a **town** of AI agents organized around git repositories (**rigs**):

- **Mayor** — singleton agent that oversees the entire town, dispatches work, and handles escalations
- **Polecats** — ephemeral workers spawned per task on a rig (one polecat = one work item)
- **Witness** — per-rig monitor that watches polecat health and reports issues
- **Refinery** — per-rig merge queue processor that rebases, tests, and merges completed work
- **Dogs** — reusable cross-rig workers for infrastructure tasks
- **Boot** — background health checker that respawns crashed agents

Work flows through the system as **work items** grouped into **convoys** (batches). You **sling** work to a rig and GTR spawns a polecat to handle it. When the polecat finishes, it signals **done** and the refinery picks up the branch for merge.

All state lives in Temporal workflows — no database, no local state files. The Temporal worker _is_ the daemon.

## Prerequisites

| Dependency | Purpose | Install |
|---|---|---|
| **Rust** (stable, edition 2021) | Build gtr | [rustup.rs](https://rustup.rs) |
| **Temporal CLI** | Local dev server | [temporal.io/download](https://temporal.io/download) |
| **Claude Code CLI** | AI agent runtime | [claude.ai/download](https://claude.ai/download) |

## Installation

### Build from source

```sh
git clone git@github.com:drewpayment/gastown-rusted.git
cd gastown-rusted
cargo build --release
```

The binary is at `target/release/gtr`. Add it to your PATH or symlink it:

```sh
ln -s "$(pwd)/target/release/gtr" ~/.local/bin/gtr
```

### First-time setup

```sh
gtr install
```

This creates the `~/.gtr` directory structure and validates dependencies:

```
~/.gtr/
  runtime/    — live process state (PIDs, sockets)
  rigs/       — git repository workspaces
  config/     — town.toml, plugins, escalation rules
```

## Quick Start

### 1. Start Temporal

```sh
temporal server start-dev
```

Leave this running in a separate terminal.

### 2. Start Gas Town

In one terminal, start the Temporal worker:

```sh
gtr worker run
```

In another terminal, bring up the town:

```sh
gtr up
```

This launches the **mayor** and **boot** workflows. The mayor agent spawns as a Claude Code session.

### 3. Register a rig

A rig is a git repository that polecats will work in:

```sh
gtr rig add my-project --path /path/to/repo
```

This creates the rig's directory structure, starts the rig workflow, and boots its witness and refinery agents.

### 4. Sling work

Create a work item and assign it to a rig:

```sh
gtr work create "Fix the login bug" --priority p1
gtr sling <work-id> --target my-project
```

GTR auto-spawns a polecat on that rig to handle the work. The polecat gets its own git worktree, a Claude Code session, and instructions via `gtr prime`.

### 5. Monitor

```sh
gtr status          # system overview — agents, rigs, polecats
gtr feed            # real-time activity dashboard (refreshes every 5s)
```

### 6. Interact with agents

```sh
gtr attach <agent>  # interactive PTY session (Ctrl+\ to detach)
gtr chat <agent> "Check on the test failures"  # async message
gtr mail send mayor "Deploy when ready"         # send mail to any agent
```

### 7. Complete work

From inside an agent session (or manually):

```sh
gtr done <work-id> --branch feature/fix-login
```

This signals the polecat, enqueues the branch to the rig's refinery for merge, and the polecat shuts down.

### 8. Shut down

```sh
gtr down
```

Sends stop signals to all running workflows, kills PTY processes, and cleans up runtime state.

## Command Reference

### System

| Command | Description |
|---|---|
| `gtr install` | First-time setup — create dirs, default config, validate deps |
| `gtr up` | Start Gas Town (mayor + boot workflows) |
| `gtr down` | Stop Gas Town (graceful shutdown of all agents) |
| `gtr status` | Hierarchical system overview with PIDs |
| `gtr doctor` | Check system health |
| `gtr feed` | Real-time activity dashboard |
| `gtr version` | Show version and build info |

### Work Management

| Command | Description |
|---|---|
| `gtr work create <title>` | Create a work item |
| `gtr work list` | List work items |
| `gtr work show <id>` | Show work item details |
| `gtr sling <ids...> --target <target>` | Assign work (target: rig name, agent ID, `mayor`, or `dogs`) |
| `gtr unsling <id>` | Unassign work from an agent |
| `gtr hook` | Query current agent's assigned work |
| `gtr done <id> --branch <branch>` | Mark work done and enqueue for merge |
| `gtr escalate <id>` | Escalate a work item immediately |

### Batch Operations

| Command | Description |
|---|---|
| `gtr convoy create <ids...>` | Create a convoy (batch of work) |
| `gtr convoy list` | List convoys |
| `gtr convoy show <id>` | Show convoy details |
| `gtr mol status <id>` | Check molecule (running formula) status |
| `gtr mol cancel <id>` | Cancel a molecule |
| `gtr mq list` | List merge queue entries |

### Agent Interaction

| Command | Description |
|---|---|
| `gtr attach <agent>` | Interactive PTY session with a live agent (Ctrl+\\ to detach) |
| `gtr chat <agent> <message>` | Send async message to an agent |
| `gtr mail send <agent> <message>` | Send mail to an agent |
| `gtr mail inbox` | Check your inbox |
| `gtr mail broadcast <message>` | Message all running agents |
| `gtr agents list` | List all agents |

### Infrastructure

| Command | Description |
|---|---|
| `gtr rig add <name> --path <path>` | Register a git repository |
| `gtr rig list` | List rigs |
| `gtr rig status <name>` | Show rig status |
| `gtr rig park <name>` | Temporarily pause a rig |
| `gtr rig unpark <name>` | Resume a paused rig |
| `gtr polecat list` | List polecats |
| `gtr polecat status <id>` | Show polecat status |
| `gtr crew create <name> --rig <rig>` | Create a persistent workspace |
| `gtr dog create <name>` | Create a reusable cross-rig worker |
| `gtr gate create <name> --type <timer\|human>` | Create an async wait gate |
| `gtr gate approve <name>` | Approve a human gate |

### Session & Context

| Command | Description |
|---|---|
| `gtr prime` | Inject role-specific context for current agent |
| `gtr prime --hook` | Output context for Claude Code SessionStart hook |
| `gtr handoff <message>` | Save context + checkpoint before ending a session |
| `gtr checkpoint write` | Save session state snapshot |
| `gtr checkpoint read` | Read last checkpoint |
| `gtr session list` | List running agent sessions |
| `gtr session show <id>` | Show session details |

### Formulas

| Command | Description |
|---|---|
| `gtr formula run <path>` | Execute a multi-step formula |
| `gtr formula list` | List available formulas |

## Configuration

The main config file is `~/.gtr/config/town.toml`:

```toml
[town]
name = "gas-town"

[escalation]
timeout_minutes = 30
max_retries = 3
```

### Temporal connection

By default, GTR connects to `http://localhost:7233` with the `default` namespace. Override in your town config:

```toml
name = "my-town"
namespace = "gastown"
temporal_address = "http://localhost:7233"
```

### Plugins

Drop `.toml` plugin definitions into `~/.gtr/config/plugins/`. The patrol workflow discovers and runs them on a schedule.

### Environment Variables

Agents receive these environment variables automatically:

| Variable | Description |
|---|---|
| `GTR_AGENT` | Agent workflow ID |
| `GTR_ROLE` | Agent role (mayor, witness, refinery, polecat) |
| `GTR_RIG` | Assigned rig name |
| `GTR_ROOT` | GTR home directory (~/.gtr) |
| `GTR_WORK_ITEM` | Current work item ID (polecats only) |

## Architecture

```
┌──────────────────────────────────────────────┐
│  gtr CLI                                     │
│  (commands → Temporal signals/start_workflow) │
└──────────┬───────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────────────┐
│  Temporal Server (state, scheduling, replay) │
└──────────┬───────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────────────┐
│  gtr worker (Rust Temporal worker = daemon)  │
│  ┌──────────┐ ┌───────────┐ ┌─────────────┐ │
│  │ Workflows│ │ Activities│ │ PTY Manager │ │
│  │ 14 types │ │ 5 types   │ │ fork/exec   │ │
│  └──────────┘ └───────────┘ │ Unix socket │ │
│                             │ SCM_RIGHTS  │ │
│                             └─────────────┘ │
└──────────────────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────────────┐
│  Claude Code sessions (PTY subprocesses)     │
│  Each agent = one PTY with fd-passing        │
│  for detach/reattach via gtr attach          │
└──────────────────────────────────────────────┘
```

### Crates

| Crate | Purpose |
|---|---|
| `gtr-core` | Domain types, config, IDs, formulas, checkpoint, directory layout |
| `gtr-temporal` | Workflows, activities, signals, PTY management, Temporal worker |
| `gtr-cli` | CLI commands, Temporal client connection |

### Workflows

| Workflow | Description |
|---|---|
| `boot_wf` | Health checker — spawns mayor, respawns crashed agents |
| `mayor_wf` | Singleton dispatcher — routes work, handles escalations |
| `rig_wf` | Per-rig lifecycle — boots witness/refinery, manages park/dock |
| `polecat_wf` | Ephemeral worker — worktree + Claude Code + heartbeat |
| `witness_wf` | Per-rig health monitor — watches polecats, reports to mayor |
| `refinery_wf` | Per-rig merge queue — rebase, test, merge |
| `agent_wf` | Generic agent lifecycle with mail and assignments |
| `work_item_wf` | Work item state machine |
| `convoy_wf` | Batch work tracking |
| `dog_wf` | Cross-rig reusable worker |
| `gate_wf` | Async wait primitive (timer or human approval) |
| `molecule_wf` | Running formula instance |
| `formula_wf` | Multi-step recipe executor |
| `patrol_wf` | Plugin discovery and scheduled execution |

## Development

```sh
# Run all tests
cargo test

# Run only integration tests
cargo test --test integration

# Run with Temporal server (includes ignored tests)
TEMPORAL_TEST=1 cargo test --test integration -- --ignored

# Build
cargo build

# Generate shell completions
gtr completions bash > ~/.bash_completion.d/gtr
gtr completions zsh > ~/.zfunc/_gtr
gtr completions fish > ~/.config/fish/completions/gtr.fish
```

## License

Private — all rights reserved.
