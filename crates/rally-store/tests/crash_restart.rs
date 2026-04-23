/// Gate: crash-restart test — write data, drop connection (simulates crash),
/// reopen, verify all committed data is intact and invariants hold.
use compact_str::CompactString;
use rally_core::agent::AgentState;
use rally_core::event::DomainEvent;
use rally_core::ids::Timestamp;
use rally_core::ports::{AgentRepo, EventLog, IdGen, WorkspaceRepo};
use rally_core::workspace::Workspace;
use rally_store::Store;
use rally_test_utils::FakeIdGen;
use tempfile::NamedTempFile;

fn make_workspace(gen: &FakeIdGen, name: &str) -> Workspace {
    Workspace::new(
        gen.next_workspace_id(),
        CompactString::from(name),
        None,
        Timestamp::from_millis(1_000_000),
    )
}

#[test]
fn workspace_survives_reopen() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();
    let gen = FakeIdGen::new();

    // --- first run ---
    let ws_id = {
        let mut store = Store::open(&path).unwrap();
        let ws = make_workspace(&gen, "my-workspace");
        let id = ws.id;
        WorkspaceRepo::save(&mut store, &ws).unwrap();
        let loaded = WorkspaceRepo::get(&store, id).unwrap().expect("should exist");
        assert_eq!(loaded.name.as_str(), "my-workspace");
        id
    };

    // --- reopen ---
    {
        let store = Store::open(&path).unwrap();
        let workspaces = WorkspaceRepo::list(&store).unwrap();
        assert_eq!(workspaces.len(), 1, "workspace must persist across reopen");
        assert_eq!(workspaces[0].id, ws_id);
        assert_eq!(workspaces[0].name.as_str(), "my-workspace");
        assert!(!workspaces[0].archived);
    }
}

#[test]
fn agent_survives_reopen() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();
    let gen = FakeIdGen::new();

    {
        let mut store = Store::open(&path).unwrap();
        let ws = make_workspace(&gen, "ws");
        WorkspaceRepo::save(&mut store, &ws).unwrap();

        let mut agent = rally_core::agent::Agent::new(
            gen.next_agent_id(),
            ws.id,
            CompactString::from("impl"),
            CompactString::from("claude-code"),
            Timestamp::from_millis(1_000_000),
        );
        AgentRepo::save(&mut store, &agent).unwrap();

        agent.apply(&rally_core::agent::AgentTrigger::Started).unwrap();
        AgentRepo::save(&mut store, &agent).unwrap();
    }

    {
        let store = Store::open(&path).unwrap();
        let workspaces = WorkspaceRepo::list(&store).unwrap();
        let ws_id = workspaces[0].id;
        let agents = AgentRepo::list_by_workspace(&store, ws_id).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].state, AgentState::Running);
        assert_eq!(agents[0].role.as_str(), "impl");
    }
}

#[test]
fn events_append_only_across_reopen() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();
    let gen = FakeIdGen::new();

    let ws_id = gen.next_workspace_id();
    let at = Timestamp::from_millis(1_000_000);

    {
        let mut store = Store::open(&path).unwrap();
        let ws = Workspace::new(ws_id, CompactString::from("ev-ws"), None, at);
        WorkspaceRepo::save(&mut store, &ws).unwrap();

        for i in 0u32..3 {
            let event = DomainEvent::WorkspaceCreated {
                id: ws_id,
                name: CompactString::from(format!("event-{i}")),
                repo: None,
                at,
            };
            EventLog::append(&mut store, &event).unwrap();
        }
    }

    {
        let store = Store::open(&path).unwrap();
        let count = store.raw_count("SELECT COUNT(*) FROM events");
        assert_eq!(count, 3, "all 3 events must persist after reopen");

        let seqs = store.raw_i64_vec("SELECT seq FROM events ORDER BY seq");
        assert_eq!(seqs, vec![1, 2, 3], "seq must be strictly increasing");
    }
}

#[test]
fn uncommitted_write_is_rolled_back() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();
    let gen = FakeIdGen::new();

    {
        let mut store = Store::open(&path).unwrap();
        let ws = make_workspace(&gen, "committed");
        WorkspaceRepo::save(&mut store, &ws).unwrap();

        // Simulate a mid-write crash: begin, insert, then rollback without commit.
        store.raw_exec("BEGIN");
        store.raw_exec(
            "INSERT INTO workspaces (id, name, created_at, archived) \
             VALUES ('deadbeef00000000000000000', 'not-committed', 0, 0)",
        );
        store.raw_exec("ROLLBACK");
    }

    {
        let store = Store::open(&path).unwrap();
        let workspaces = WorkspaceRepo::list(&store).unwrap();
        assert_eq!(workspaces.len(), 1, "rolled-back row must not appear");
        assert_eq!(workspaces[0].name.as_str(), "committed");
    }
}
