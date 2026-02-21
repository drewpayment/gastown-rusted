//! Integration tests for GTR tmux + Temporal workflows.
//!
//! These tests require either:
//! - tmux installed for spawn/kill tests
//! - A running Temporal dev server for workflow tests
//!
//! Run tmux tests: `cargo test --test integration`
//! Run all (with Temporal): `TEMPORAL_TEST=1 cargo test --test integration --ignored`

use std::collections::HashMap;
use std::path::Path;

fn tmux_available() -> bool {
    std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn test_spawn_mock_agent() {
    if !tmux_available() {
        eprintln!("Skipping test -- tmux not installed");
        return;
    }

    let agent_id = "test-integration-mock";

    // Clean up from previous runs
    gtr_temporal::pty::cleanup(agent_id).ok();
    let _ = gtr_temporal::pty::kill_agent(agent_id);

    // The integration test binary lives in target/debug/deps/ inside the workspace.
    // Walk up from CARGO_MANIFEST_DIR (crates/gtr-temporal) to the workspace root.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("Could not find workspace root");
    let script_path = workspace_root.join("tests").join("mock-agent.sh");

    // Skip if mock agent script doesn't exist
    if !script_path.exists() {
        eprintln!(
            "Skipping test -- mock-agent.sh not found at {}",
            script_path.display()
        );
        return;
    }

    let mut env = HashMap::new();
    env.insert("GTR_AGENT".into(), agent_id.into());
    env.insert("GTR_ROLE".into(), "test".into());
    env.insert("GTR_RIG".into(), "test-rig".into());

    let result = gtr_temporal::pty::spawn(
        agent_id,
        script_path.to_str().unwrap(),
        &[],
        Path::new("/tmp"),
        &env,
    );

    assert!(result.is_ok(), "spawn failed: {:?}", result.err());
    let _pid = result.unwrap();

    // Verify alive via tmux
    assert!(gtr_temporal::pty::is_alive(agent_id));

    // Verify PID file
    let pid_path = gtr_temporal::pty::runtime_dir(agent_id).join("pid");
    assert!(pid_path.exists());

    // Verify tmux session name matches expected pattern
    let session = gtr_temporal::pty::tmux_session_name(agent_id);
    assert!(session.contains(agent_id));

    // Wait for script to finish (2s sleep + work)
    std::thread::sleep(std::time::Duration::from_secs(4));

    // Kill and cleanup
    let _ = gtr_temporal::pty::kill_agent(agent_id);
}

#[test]
fn test_spawn_with_server_and_tmux_session() {
    if !tmux_available() {
        eprintln!("Skipping test -- tmux not installed");
        return;
    }

    let agent_id = "test-integration-server";

    // Clean up from previous runs
    gtr_temporal::pty::cleanup(agent_id).ok();
    let _ = gtr_temporal::pty::kill_agent(agent_id);

    let mut env = HashMap::new();
    env.insert("GTR_AGENT".into(), agent_id.into());

    // Spawn with server (now just spawns a tmux session)
    let result = gtr_temporal::pty::spawn_with_server(
        agent_id,
        "/bin/sh",
        &["-c".into(), "sleep 30".into()],
        Path::new("/tmp"),
        &env,
    );

    assert!(
        result.is_ok(),
        "spawn_with_server failed: {:?}",
        result.err()
    );
    let pid = result.unwrap();

    // Verify alive via tmux
    assert!(gtr_temporal::pty::is_alive(agent_id));

    // Verify PID matches
    let read_pid = gtr_temporal::pty::read_pid(agent_id);
    assert!(read_pid.is_some());
    assert_eq!(read_pid.unwrap(), pid);

    // Verify tmux session exists
    let session = gtr_temporal::pty::tmux_session_name(agent_id);
    let has_session = std::process::Command::new("tmux")
        .args(["-L", "gtr", "has-session", "-t", &session])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(has_session, "tmux session should exist: {session}");

    // Kill agent
    let kill_result = gtr_temporal::pty::kill_agent(agent_id);
    assert!(kill_result.is_ok());

    // Verify no longer alive
    assert!(!gtr_temporal::pty::is_alive(agent_id));
}

#[test]
#[ignore] // Requires running Temporal dev server
fn test_spawn_and_heartbeat_workflow() {
    // 1. Start Temporal worker in background
    // 2. Start polecat workflow with mock agent (runtime: "shell")
    // 3. Verify agent spawns
    // 4. Signal polecat done
    // 5. Verify polecat workflow completes
    eprintln!("This test requires a running Temporal dev server (temporal server start-dev)");
    eprintln!("Run with: TEMPORAL_TEST=1 cargo test --test integration -- --ignored");
}

#[test]
#[ignore] // Requires running Temporal dev server
fn test_end_to_end_sling_to_done() {
    eprintln!("E2E test: sling -> polecat spawn -> done -> refinery enqueue");
    eprintln!("Requires Temporal dev server");
}

#[test]
#[ignore] // Requires running Temporal dev server
fn test_multi_rig() {
    eprintln!("Multi-rig test: two rigs with independent agents");
    eprintln!("Requires Temporal dev server");
}
