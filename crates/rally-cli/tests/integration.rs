use std::io::{BufRead, BufReader, Write};

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

    // List workspaces — assert contents, not just response kind
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
    let ws_items = listed["items"].as_array().expect("items should be array");
    assert_eq!(ws_items.len(), 1, "should list exactly 1 workspace");
    assert_eq!(ws_items[0]["name"], "test-ws");
    assert_eq!(ws_items[0]["id"].as_str().unwrap(), ws_id);

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
    let agent_items = agents["items"].as_array().expect("items should be array");
    assert_eq!(agent_items.len(), 1, "should list exactly 1 agent");
    assert_eq!(agent_items[0]["role"], "impl");
    assert_eq!(agent_items[0]["state"], "initializing");
    assert_eq!(agent_items[0]["workspace_id"].as_str().unwrap(), ws_id);
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

#[test]
fn alias_set_resolve_list() {
    let harness = DaemonHarness::start(&DaemonHarness::find_rallyd());

    // Create a workspace to alias
    let create_output = rally_cmd(harness.socket_path())
        .args(["--json", "workspace", "new", "--name", "aliased-ws"])
        .output()
        .expect("failed to create workspace");
    assert!(create_output.status.success());

    let created: serde_json::Value =
        serde_json::from_slice(&create_output.stdout).expect("invalid JSON");
    let ws_id = created["id"].as_str().unwrap();

    // Set alias
    let set_output = rally_cmd(harness.socket_path())
        .args(["--json", "alias", "set", "myalias", ws_id])
        .output()
        .expect("failed to set alias");
    assert!(
        set_output.status.success(),
        "alias set failed: {}",
        String::from_utf8_lossy(&set_output.stderr)
    );

    // Resolve alias
    let get_output = rally_cmd(harness.socket_path())
        .args(["--json", "alias", "get", "myalias"])
        .output()
        .expect("failed to get alias");
    assert!(get_output.status.success());

    let resolved: serde_json::Value =
        serde_json::from_slice(&get_output.stdout).expect("invalid JSON from alias get");
    assert_eq!(resolved["kind"], "alias_resolved");
    assert_eq!(resolved["workspace_id"].as_str().unwrap(), ws_id);

    // Resolve non-existent alias
    let missing = rally_cmd(harness.socket_path())
        .args(["--json", "alias", "get", "no-such-alias"])
        .output()
        .expect("failed to get missing alias");
    assert!(missing.status.success());
    let missing_json: serde_json::Value =
        serde_json::from_slice(&missing.stdout).expect("invalid JSON");
    assert_eq!(missing_json["kind"], "alias_resolved");
    assert!(missing_json["workspace_id"].is_null());

    // List aliases
    let ls_output = rally_cmd(harness.socket_path())
        .args(["--json", "alias", "ls"])
        .output()
        .expect("failed to list aliases");
    assert!(ls_output.status.success());

    let list: serde_json::Value =
        serde_json::from_slice(&ls_output.stdout).expect("invalid JSON from alias ls");
    assert_eq!(list["kind"], "alias_list");
    let items = list["items"].as_array().expect("items should be array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["alias"], "myalias");
    assert_eq!(items[0]["workspace_id"].as_str().unwrap(), ws_id);

    // Upsert: set same alias to different workspace
    let create2 = rally_cmd(harness.socket_path())
        .args(["--json", "workspace", "new", "--name", "other-ws"])
        .output()
        .expect("failed to create workspace");
    let ws2: serde_json::Value = serde_json::from_slice(&create2.stdout).unwrap();
    let ws2_id = ws2["id"].as_str().unwrap();

    rally_cmd(harness.socket_path())
        .args(["--json", "alias", "set", "myalias", ws2_id])
        .output()
        .expect("failed to upsert alias");

    let get2 = rally_cmd(harness.socket_path())
        .args(["--json", "alias", "get", "myalias"])
        .output()
        .expect("failed to get upserted alias");
    let resolved2: serde_json::Value = serde_json::from_slice(&get2.stdout).unwrap();
    assert_eq!(
        resolved2["workspace_id"].as_str().unwrap(),
        ws2_id,
        "upsert should update the alias to point to the new workspace"
    );
}

fn ipc_call(socket_path: &std::path::Path, method_fields_json: &str) -> serde_json::Value {
    let mut stream =
        std::os::unix::net::UnixStream::connect(socket_path).expect("failed to connect to daemon");
    // RequestEnvelope uses #[serde(flatten)] on payload, and Request uses #[serde(tag = "method")]
    // So the wire format is: {"request_id":"...", "method":"bind_pane", "agent_id":"...", ...}
    let envelope = format!(r#"{{"request_id":"test-req",{}}}"#, method_fields_json);
    writeln!(stream, "{}", envelope).expect("failed to write request");
    stream.flush().unwrap();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("failed to read response");
    serde_json::from_str(&line).expect("invalid JSON response")
}

#[test]
fn bind_pane_updates_agent_pane_ref() {
    let harness = DaemonHarness::start(&DaemonHarness::find_rallyd());

    // Create workspace
    let create = rally_cmd(harness.socket_path())
        .args(["--json", "workspace", "new", "--name", "pane-test"])
        .output()
        .unwrap();
    let ws: serde_json::Value = serde_json::from_slice(&create.stdout).unwrap();
    let ws_id = ws["id"].as_str().unwrap();

    // Spawn agent
    let spawn = rally_cmd(harness.socket_path())
        .args([
            "--json", "agent", "spawn", "--workspace", ws_id, "--role", "test-agent", "--runtime",
            "claude-code",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&spawn.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    // Verify agent has no pane_ref initially
    let list1 = rally_cmd(harness.socket_path())
        .args(["--json", "agent", "ls"])
        .output()
        .unwrap();
    let agents1: serde_json::Value = serde_json::from_slice(&list1.stdout).unwrap();
    let items1 = agents1["items"].as_array().unwrap();
    assert!(
        items1[0]["pane_session"].is_null(),
        "agent should have no pane initially"
    );

    // Send BindPane via raw IPC
    let bind_fields = format!(
        r#""method":"bind_pane","agent_id":"{}","session_name":"test-sess","tab_index":0,"pane_id":42"#,
        agent_id
    );
    let bind_resp = ipc_call(harness.socket_path(), &bind_fields);
    assert_eq!(bind_resp["kind"], "ok", "BindPane should succeed");

    // Verify agent now has pane info
    let list2 = rally_cmd(harness.socket_path())
        .args(["--json", "agent", "ls"])
        .output()
        .unwrap();
    let agents2: serde_json::Value = serde_json::from_slice(&list2.stdout).unwrap();
    let items2 = agents2["items"].as_array().unwrap();
    assert_eq!(items2[0]["pane_session"], "test-sess");
    assert_eq!(items2[0]["pane_id"], 42);
}
