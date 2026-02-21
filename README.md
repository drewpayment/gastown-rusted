# Rusted Gas Town (rgt)

A Rust + Temporal orchestration system that manages fleets of AI coding agents working across git repositories. RGT spawns, monitors, and coordinates Claude Code sessions inside persistent tmux sessions — agents survive detach/reattach without exiting.

## How It Works

RGT models a **town** of AI agents organized around git repositories (**rigs**):

- **Mayor** — singleton agent that oversees the entire town, dispatches work, and handles escalations
- **Polecats** — ephemeral workers spawned per task on a rig (one polecat = one work item)
- **Witness** — per-rig monitor that watches polecat health and reports issues
- **Refinery** — per-rig merge queue processor that rebases, tests, and merges completed work
- **Dogs** — reusable cross-rig workers for infrastructure tasks
- **Boot** — background health checker that respawns crashed agents

Work flows through the system as **work items** grouped into **convoys** (batches). You **sling** work to a rig and RGT spawns a polecat to handle it. When the polecat finishes, it signals **done** and the refinery picks up the branch for merge.

All state lives in Temporal workflows — no database, no local state files. The Temporal worker _is_ the daemon.

## Prerequisites

| Dependency | Purpose | Install |
|---|---|---|
| **Rust** (stable, edition 2021) | Build rgt | [rustup.rs](https://rustup.rs) |
| **tmux** (>= 3.2) | Persistent agent terminals | `brew install tmux` or [github.com/tmux/tmux](https://github.com/tmux/tmux) |
| **Temporal CLI** | Local dev server | [temporal.io/download](https://temporal.io/download) |
| **Claude Code CLI** | AI agent runtime | [claude.ai/download](https://claude.ai/download) |

## Installation

### Build from source

```sh
git clone git@github.com:drewpayment/gastown-rusted.git
cd gastown-rusted
cargo build --release
```

The binary is at `target/release/rgt`. Add it to your PATH or symlink it:

```sh
ln -s "$(pwd)/target/release/rgt" ~/.local/bin/rgt
```

### First-time setup

```sh
rgt install
```

This creates the `~/.gtr` directory structure and validates dependencies:

```
~/.gtr/
  runtime/    — live process state (PIDs, env.json)
  rigs/       — git repository workspaces
  config/     — town.toml, tmux.conf, plugins, escalation rules
```

## Quick Start

### Simple (recommended)

One command starts everything — Temporal server, worker, and workflows:

```sh
rgt start
```

Check what's running:

```sh
rgt sessions
```

When done:

```sh
rgt stop
```

### Manual (advanced)

If you prefer to manage each component separately:

#### 1. Start Temporal

```sh
temporal server start-dev
```

Leave this running in a separate terminal.

#### 2. Start the worker and bring up the town

```sh
rgt worker run   # in one terminal
rgt up           # in another terminal
```

This launches the **mayor** and **boot** workflows. The mayor agent spawns as a Claude Code session.

#### 3. Shut down

```sh
rgt down
```

### 3. Register a rig

A rig is a git repository that polecats will work in:

```sh
rgt rig add my-project --path /path/to/repo
```

This creates the rig's directory structure, starts the rig workflow, and boots its witness and refinery agents.

### 4. Sling work

Create a work item and assign it to a rig:

```sh
rgt work create "Fix the login bug" --priority p1
rgt sling <work-id> --target my-project
```

RGT auto-spawns a polecat on that rig to handle the work. The polecat gets its own git worktree, a Claude Code session, and instructions via `rgt prime`.

### 5. Monitor

```sh
rgt status          # system overview — agents, rigs, polecats
rgt feed            # real-time activity dashboard (refreshes every 5s)
rgt sessions        # list active tmux sessions
```

### 6. Interact with agents

```sh
rgt attach <agent>  # interactive PTY session (Ctrl+\ to detach)
rgt chat <agent> "Check on the test failures"  # async message
rgt mail send mayor "Deploy when ready"         # send mail to any agent
```

### 7. Complete work

From inside an agent session (or manually):

```sh
rgt done <work-id> --branch feature/fix-login
```

This signals the polecat, enqueues the branch to the rig's refinery for merge, and the polecat shuts down.

### 8. Shut down

```sh
rgt stop            # stops everything (workflows + worker + Temporal)
rgt down            # or just stop workflows/agents (manual mode)
```

## Command Reference

### System

| Command | Description |
|---|---|
| `rgt install` | First-time setup — create dirs, default config, validate deps |
| `rgt start` | Start everything — Temporal server, worker, and workflows (via tmux) |
| `rgt stop` | Stop everything — workflows, worker, and Temporal server |
| `rgt up` | Start workflows only (mayor + boot) |
| `rgt down` | Stop workflows only (graceful shutdown of all agents) |
| `rgt status` | Hierarchical system overview with PIDs |
| `rgt sessions` | List active tmux sessions |
| `rgt doctor` | Check system health |
| `rgt feed` | Real-time activity dashboard |
| `rgt version` | Show version and build info |

### Work Management

| Command | Description |
|---|---|
| `rgt work create <title>` | Create a work item |
| `rgt work list` | List work items |
| `rgt work show <id>` | Show work item details |
| `rgt sling <ids...> --target <target>` | Assign work (target: rig name, agent ID, `mayor`, or `dogs`) |
| `rgt unsling <id>` | Unassign work from an agent |
| `rgt hook` | Query current agent's assigned work |
| `rgt done <id> --branch <branch>` | Mark work done and enqueue for merge |
| `rgt escalate <id>` | Escalate a work item immediately |

### Batch Operations

| Command | Description |
|---|---|
| `rgt convoy create <ids...>` | Create a convoy (batch of work) |
| `rgt convoy list` | List convoys |
| `rgt convoy show <id>` | Show convoy details |
| `rgt mol status <id>` | Check molecule (running formula) status |
| `rgt mol cancel <id>` | Cancel a molecule |
| `rgt mq list` | List merge queue entries |

### Agent Interaction

| Command | Description |
|---|---|
| `rgt attach <agent>` | Interactive PTY session with a live agent (Ctrl+\\ to detach) |
| `rgt chat <agent> <message>` | Send async message to an agent |
| `rgt mail send <agent> <message>` | Send mail to an agent |
| `rgt mail inbox` | Check your inbox |
| `rgt mail broadcast <message>` | Message all running agents |
| `rgt agents list` | List all agents |

### Infrastructure

| Command | Description |
|---|---|
| `rgt rig add <name> --path <path>` | Register a git repository |
| `rgt rig list` | List rigs |
| `rgt rig status <name>` | Show rig status |
| `rgt rig park <name>` | Temporarily pause a rig |
| `rgt rig unpark <name>` | Resume a paused rig |
| `rgt polecat list` | List polecats |
| `rgt polecat status <id>` | Show polecat status |
| `rgt crew create <name> --rig <rig>` | Create a persistent workspace |
| `rgt dog create <name>` | Create a reusable cross-rig worker |
| `rgt gate create <name> --type <timer\|human>` | Create an async wait gate |
| `rgt gate approve <name>` | Approve a human gate |

### Session & Context

| Command | Description |
|---|---|
| `rgt prime` | Inject role-specific context for current agent |
| `rgt prime --hook` | Output context for Claude Code SessionStart hook |
| `rgt handoff <message>` | Save context + checkpoint before ending a session |
| `rgt checkpoint write` | Save session state snapshot |
| `rgt checkpoint read` | Read last checkpoint |
| `rgt session list` | List running agent sessions |
| `rgt session show <id>` | Show session details |

### Formulas

| Command | Description |
|---|---|
| `rgt formula run <path>` | Execute a multi-step formula |
| `rgt formula list` | List available formulas |

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

By default, RGT connects to `http://localhost:7233` with the `default` namespace. Override in your town config:

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
│  rgt CLI                                     │
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
│  rgt worker (Rust Temporal worker = daemon)  │
│  ┌──────────┐ ┌───────────┐ ┌─────────────┐ │
│  │ Workflows│ │ Activities│ │ tmux Manager│ │
│  │ 14 types │ │ 5 types   │ │ -L gtr      │ │
│  └──────────┘ └───────────┘ │ sessions    │ │
│                             └─────────────┘ │
└──────────────────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────────────┐
│  Claude Code sessions (tmux sessions)        │
│  Each agent = one tmux session (gtr-<id>)    │
│  rgt attach execs into tmux attach-session   │
└──────────────────────────────────────────────┘
```

### Crates

| Crate | Purpose |
|---|---|
| `gtr-core` | Domain types, config, IDs, formulas, checkpoint, directory layout |
| `gtr-temporal` | Workflows, activities, signals, tmux session management, Temporal worker |
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
rgt completions bash > ~/.bash_completion.d/rgt
rgt completions zsh > ~/.zfunc/_rgt
rgt completions fish > ~/.config/fish/completions/rgt.fish
```

## Acknowledgments

RGT is a from-scratch Rust + Temporal rewrite inspired by [Gas Town](https://github.com/steveyegge/gastown) by [Steve Yegge](https://github.com/steveyegge). The original Gas Town (Go) pioneered the multi-agent workspace manager concept — mayor, polecats, rigs, refinery, and the town metaphor all originate from Steve's design. RGT rebuilds those ideas on Temporal workflows for durability and replay safety.

## License

Private — all rights reserved.
