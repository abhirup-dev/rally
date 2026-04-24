use assert_cmd::Command;
use rally_test_utils::harness::DaemonHarness;

fn rally_cmd(socket_path: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("rally").expect("rally binary not found");
    cmd.env("RALLY_DAEMON_SOCKET_PATH", socket_path);
    cmd
}

#[test]
fn workspace_lifecycle_json() {
    let harness = DaemonHarness::start(&DaemonHarness::find_rallyd());

    // Create a workspace
    let create_output = rally_cmd(harness.socket_path())
        .args(["--json", "workspace", "new", "--name", "test-ws"])
        .output()
        .expect("failed to run rally");

    assert!(
        create_output.status.success(),
        "create failed: {}",
        String::from_utf8_lossy(&create_output.stderr)
    );

    let created: serde_json::Value =
        serde_json::from_slice(&create_output.stdout).expect("invalid JSON from create");
    assert_eq!(created["kind"], "workspace");
    assert_eq!(created["name"], "test-ws");
    assert!(created["id"].is_string());
    assert!(created["canonical_key"].is_string());

    let ws_id = created["id"].as_str().unwrap();

    // List workspaces
    let list_output = rally_cmd(harness.socket_path())
        .args(["--json", "workspace", "ls"])
        .output()
        .expect("failed to run rally");

    assert!(
        list_output.status.success(),
        "list failed: stdout={} stderr={}",
        String::from_utf8_lossy(&list_output.stdout),
        String::from_utf8_lossy(&list_output.stderr)
    );
    let listed: serde_json::Value =
        serde_json::from_slice(&list_output.stdout).expect("invalid JSON from list");
    assert_eq!(listed["kind"], "workspace_list");

    // Register an agent
    let spawn_output = rally_cmd(harness.socket_path())
        .args([
            "--json",
            "agent",
            "spawn",
            "--workspace",
            ws_id,
            "--role",
            "impl",
            "--runtime",
            "claude-code",
        ])
        .output()
        .expect("failed to run rally");

    assert!(
        spawn_output.status.success(),
        "spawn failed: {}",
        String::from_utf8_lossy(&spawn_output.stderr)
    );

    let spawned: serde_json::Value =
        serde_json::from_slice(&spawn_output.stdout).expect("invalid JSON from spawn");
    assert_eq!(spawned["kind"], "agent");
    assert_eq!(spawned["role"], "impl");
    assert_eq!(spawned["state"], "initializing");

    // List agents (the gate command: `rally agent ls --json | jq`)
    let agent_list = rally_cmd(harness.socket_path())
        .args(["--json", "agent", "ls"])
        .output()
        .expect("failed to run rally");

    assert!(agent_list.status.success());
    let agents: serde_json::Value =
        serde_json::from_slice(&agent_list.stdout).expect("invalid JSON from agent ls");
    assert_eq!(agents["kind"], "agent_list");

    // DaemonHarness::drop kills the daemon
}

#[test]
fn session_status_when_running() {
    let harness = DaemonHarness::start(&DaemonHarness::find_rallyd());

    let output = rally_cmd(harness.socket_path())
        .args(["--json", "session", "status"])
        .output()
        .expect("failed to run rally");

    assert!(output.status.success());
    // Status checks socket existence, not daemon liveness via connect
}
