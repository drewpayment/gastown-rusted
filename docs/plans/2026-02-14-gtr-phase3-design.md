# GTR Phase 3 Design: Make It Real

**Goal:** Replace the original Gas Town's tmux/beads runtime with a native Rust daemon using Temporal as both state store and process orchestrator. Add `gtr attach` for interactive Claude Code sessions and full end-to-end agent lifecycle management.

**Phase 1:** Scaffolding + Temporal workflows (Tasks 1-29, complete)
**Phase 2:** Feature parity workflows + CLI (Tasks 30-50, complete)
**Phase 3:** Runtime execution layer — agents actually run, work actually gets done

---

## Architecture

### Daemon Model

The Temporal worker IS the daemon. `gtr up` starts a foreground Temporal worker process that registers all workflows and activities. Agent subprocesses are spawned by activities and managed via PTY file descriptors stored in a runtime directory. No separate daemon PID, no tmux dependency.

### Agent Process Model

Each agent (mayor, deacon, witness, refinery, polecat, dog) runs as a Claude Code CLI subprocess with a PTY. The `spawn_agent` activity creates the process, writes the PTY master FD path to `~/.gtr/runtime/<agent-id>/`, and the agent workflow monitors it via heartbeat activities. `gtr attach <agent>` reconnects to the PTY.

### State Model

Temporal only. No SQLite, no JSONL. All state queries go through Temporal's `list_workflow_executions` and `describe_workflow_execution`. The Temporal UI provides the observability dashboard.

### Interaction Model

- `gtr attach <agent>` — PTY reconnect to live Claude Code session (interactive)
- `gtr chat <agent>` — Send messages via Temporal signals (async)
- `gtr feed` — Queries Temporal for real-time dashboard (already built in Phase 2)

### Crash Recovery

Temporal retries failed activities. If an agent subprocess dies, the heartbeat activity detects it and the workflow can respawn. No separate daemon heartbeat loop needed — Temporal IS the heartbeat.

---

## Components

### 1. PTY Process Manager

A new module in `gtr-temporal` (or standalone `gtr-runtime` if large enough) that handles spawning Claude Code as a subprocess with a PTY, storing the PTY socket path in a runtime directory (`~/.gtr/runtime/<agent-id>/pty.sock`), and reconnecting. Replaces tmux entirely.

Key operations:
- `spawn(agent_id, claude_args, work_dir, env_vars) -> PtyHandle`
- `attach(agent_id) -> interactive terminal session`
- `is_alive(agent_id) -> bool`
- `kill(agent_id)`

### 2. Real `spawn_agent` Activity

Replace the mock with: create work directory, set env vars (`GTR_AGENT`, `GTR_ROLE`, `GTR_RIG`, `GTR_ROOT`), spawn Claude Code via PTY manager, inject initial prompt (role context, hooked work, mail). Return agent PID and PTY socket path.

### 3. Agent Heartbeat Activity

New activity that checks if an agent subprocess is still alive (via PID/PTY), reads recent output lines, and reports back. Called periodically by agent workflows to detect crashes.

### 4. Real `gtr prime` / SessionStart Hook

When Claude Code starts, the SessionStart hook runs `gtr prime --hook` which queries Temporal for the agent's state (hooked work, mail, role) and injects context. Bridge between Temporal workflow world and Claude Code session.

### 5. Real `gtr up` Flow

`gtr up` starts the Temporal worker in foreground mode, then the boot workflow spawns the mayor agent, which spawns deacon, which spawns witnesses/refineries per rig. The full agent tree comes alive through Temporal workflow orchestration.

### 6. `gtr attach` / `gtr chat` Commands

- `attach`: Connects terminal to the PTY socket for the named agent
- `chat`: Sends a Temporal signal (mail) and waits for response

### 7. Integration Tests

End-to-end tests using a mock agent script (not Claude Code) that responds to `gtr hook`, `gtr done` commands. Validates the full sling-to-done-to-merge loop.

### 8. Real Notification Activity

Replace the log-only stub with actual webhook dispatch for escalations.

---

## Data Flow: Sling to Done

```
Human runs: gtr sling WI-abc123 --target gtr

1. CLI sends signal to mayor_wf -> "assign WI-abc123 to rig gtr"
2. Mayor workflow creates polecat name (e.g., "furiosa")
3. Mayor starts polecat_wf("furiosa", rig="gtr", work_item="WI-abc123")
4. Polecat workflow runs spawn_agent activity:
   a. Creates work dir: ~/.gtr/rigs/gtr/polecats/furiosa/
   b. Creates git worktree on feature branch
   c. Spawns Claude Code via PTY manager with env vars + initial prompt
   d. Returns PID + PTY socket path
5. Polecat workflow enters heartbeat loop:
   a. Every 60s: check_agent_alive activity -> is PID running?
   b. If dead -> mark stuck, notify witness
   c. If alive -> continue
6. Agent (Claude Code) works on the task, eventually runs: gtr done
7. gtr done CLI:
   a. Commits + pushes branch
   b. Signals polecat_wf -> "done"
   c. Signals refinery_wf -> "enqueue WI-abc123 branch furiosa/WI-abc123"
8. Polecat workflow receives done signal:
   a. Kills agent subprocess
   b. Removes git worktree
   c. Cleans up work dir
   d. Workflow completes
9. Refinery workflow processes merge queue:
   a. Checkout branch -> Rebase onto main -> Run tests -> Merge
   b. If conflict -> spawn new polecat for conflict resolution
10. Work item workflow receives "merged" signal -> status = closed
```

## Data Flow: Crash Recovery

```
1. Agent subprocess crashes (Claude Code exits unexpectedly)
2. Polecat heartbeat activity detects PID is gone
3. Polecat workflow marks status = "stuck"
4. Witness workflow detects stuck polecat (periodic check)
5. Witness escalates to mayor via signal
6. Mayor decides: respawn polecat (same branch, gtr prime restores context)
   OR: escalate to human
```

## Data Flow: gtr attach

```
Human runs: gtr attach mayor

1. CLI reads ~/.gtr/runtime/mayor/pty.sock
2. Connects terminal stdin/stdout to PTY socket
3. Human sees live Claude Code session, can interact
4. Ctrl+D or disconnect: detaches (agent keeps running)
```

---

## Directory Structure

```
~/.gtr/
+-- runtime/                    # Live process state (ephemeral)
|   +-- mayor/
|   |   +-- pty.sock           # PTY socket for attach
|   |   +-- pid                # Process ID
|   |   +-- env.json           # Env vars used at spawn
|   +-- deacon/
|   +-- gtr-witness/
|   +-- gtr-furiosa/           # Polecat
+-- rigs/                       # Rig working directories
|   +-- gtr/
|   |   +-- polecats/          # Polecat work dirs (ephemeral)
|   |   |   +-- furiosa/       # Git worktree
|   |   +-- crew/
|   |   |   +-- drew/          # Persistent crew workspace
|   |   +-- witness/           # Witness work dir
|   |   +-- refinery/          # Refinery work dir
|   +-- cmp/
+-- config/                     # Configuration
    +-- town.toml
    +-- rigs/
        +-- gtr.toml
        +-- cmp.toml
```

---

## Key Differences from Original Gas Town

| Aspect | Gas Town (Go) | GTR (Rust) Phase 3 |
|--------|---------------|---------------------|
| Process manager | tmux | Native PTY (no tmux) |
| State storage | SQLite (beads) | Temporal workflows |
| Daemon | Go binary + PID file | Temporal worker IS the daemon |
| Crash recovery | Daemon heartbeat loop | Temporal activity retry + heartbeat |
| Agent interaction | `tmux send-keys` | `gtr attach` (PTY) / `gtr chat` (signals) |
| Observability | JSONL files + gt feed | Temporal UI + gtr feed |
| Config format | JSON | TOML |
| Agent spawning | `tmux new-session` | PTY subprocess via activity |
| Session recovery | tmux detach/reattach | PTY socket reconnect |

---

## Tech Stack Additions

- `portable-pty` or raw `openpty` — PTY management for agent subprocesses
- `nix` crate — Unix signal handling, process management
- `tokio::process` — Async subprocess spawning
- Integration test framework using mock agent scripts
