//! Integration tests for GTR PTY + Temporal workflows.
//!
//! These tests require either:
//! - A running Temporal dev server for workflow tests
//! - Just the PTY module for spawn/heartbeat tests
//!
//! Run PTY tests: `cargo test --test integration`
//! Run all (with Temporal): `TEMPORAL_TEST=1 cargo test --test integration --ignored`

use std::collections::HashMap;
use std::path::Path;

#[test]
fn test_spawn_mock_agent() {
    let agent_id = "test-integration-mock";

    // Clean up from previous runs
    gtr_temporal::pty::cleanup(agent_id).ok();

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
    let (_pid, _master_fd) = result.unwrap();

    // Verify alive
    assert!(gtr_temporal::pty::is_alive(agent_id));

    // Verify PID file
    let pid_path = gtr_temporal::pty::runtime_dir(agent_id).join("pid");
    assert!(pid_path.exists());

    // Verify socket path helper returns a path containing the agent id
    let sock = gtr_temporal::pty::socket_path(agent_id);
    assert!(sock.to_string_lossy().contains(agent_id));

    // Wait for script to finish (2s sleep + work)
    std::thread::sleep(std::time::Duration::from_secs(4));

    // Kill and cleanup
    let _ = gtr_temporal::pty::kill_agent(agent_id);
}

#[test]
fn test_spawn_with_server_and_connect() {
    let agent_id = "test-integration-server";

    // Clean up from previous runs
    gtr_temporal::pty::cleanup(agent_id).ok();

    let mut env = HashMap::new();
    env.insert("GTR_AGENT".into(), agent_id.into());

    // Spawn with server (background thread serves PTY fd)
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

    // Verify alive
    assert!(gtr_temporal::pty::is_alive(agent_id));

    // Verify PID matches
    let read_pid = gtr_temporal::pty::read_pid(agent_id);
    assert!(read_pid.is_some());
    assert_eq!(read_pid.unwrap(), pid);

    // Verify socket path exists (give server thread time to bind)
    std::thread::sleep(std::time::Duration::from_millis(500));
    let sock_path = gtr_temporal::pty::socket_path(agent_id);
    assert!(
        sock_path.exists(),
        "Socket path should exist: {}",
        sock_path.display()
    );

    // Connect and receive fd
    let fd_result = gtr_temporal::pty::connect_pty(agent_id);
    assert!(
        fd_result.is_ok(),
        "connect_pty failed: {:?}",
        fd_result.err()
    );
    let fd = fd_result.unwrap();
    assert!(fd >= 0, "Received fd should be non-negative");

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
