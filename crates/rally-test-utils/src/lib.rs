use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rally_core::agent::Agent;
use rally_core::event::DomainEvent;
use rally_core::ids::{AgentId, HookId, InboxItemId, PaneId, Timestamp, WorkspaceId};
use rally_core::inbox::InboxItem;
use rally_core::ports::{AgentRepo, Clock, EventLog, IdGen, InboxRepo, WorkspaceRepo};
use rally_core::workspace::Workspace;
use ulid::Ulid;

// ---------------------------------------------------------------------------
// FakeClock
// ---------------------------------------------------------------------------

/// A clock whose current time is set manually — perfect for deterministic tests.
#[derive(Debug, Clone)]
pub struct FakeClock {
    now: Arc<AtomicU64>,
}

impl FakeClock {
    pub fn new(start_ms: u64) -> Self {
        Self {
            now: Arc::new(AtomicU64::new(start_ms)),
        }
    }

    pub fn advance_ms(&self, ms: u64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }

    pub fn set_ms(&self, ms: u64) {
        self.now.store(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(self.now.load(Ordering::SeqCst))
    }
}

// ---------------------------------------------------------------------------
// FakeIdGen
// ---------------------------------------------------------------------------

/// An id generator backed by a monotonic counter — stable across test runs.
#[derive(Debug, Default, Clone)]
pub struct FakeIdGen {
    counter: Arc<AtomicU64>,
}

impl FakeIdGen {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_ulid(&self) -> Ulid {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        // Encode the counter into the random bits so each id is unique.
        Ulid::from_parts(0, n as u128)
    }
}

impl IdGen for FakeIdGen {
    fn next_workspace_id(&self) -> WorkspaceId {
        WorkspaceId::new(self.next_ulid())
    }
    fn next_agent_id(&self) -> AgentId {
        AgentId::new(self.next_ulid())
    }
    fn next_pane_id(&self) -> PaneId {
        PaneId::new(self.next_ulid())
    }
    fn next_inbox_item_id(&self) -> InboxItemId {
        InboxItemId::new(self.next_ulid())
    }
    fn next_hook_id(&self) -> HookId {
        HookId::new(self.next_ulid())
    }
}

// ---------------------------------------------------------------------------
// InMemoryRepo
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct InMemoryRepo {
    workspaces: HashMap<WorkspaceId, Workspace>,
    agents: HashMap<AgentId, Agent>,
    inbox: HashMap<InboxItemId, InboxItem>,
    events: Vec<DomainEvent>,
}

impl InMemoryRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("in-memory repo error (infallible)")]
pub enum InMemoryError {}

impl WorkspaceRepo for InMemoryRepo {
    type Error = InMemoryError;

    fn get(&self, id: WorkspaceId) -> Result<Option<Workspace>, Self::Error> {
        Ok(self.workspaces.get(&id).cloned())
    }

    fn list(&self) -> Result<Vec<Workspace>, Self::Error> {
        Ok(self.workspaces.values().cloned().collect())
    }

    fn save(&mut self, workspace: &Workspace) -> Result<(), Self::Error> {
        self.workspaces.insert(workspace.id, workspace.clone());
        Ok(())
    }
}

impl AgentRepo for InMemoryRepo {
    type Error = InMemoryError;

    fn get(&self, id: AgentId) -> Result<Option<Agent>, Self::Error> {
        Ok(self.agents.get(&id).cloned())
    }

    fn list_by_workspace(&self, workspace_id: WorkspaceId) -> Result<Vec<Agent>, Self::Error> {
        Ok(self
            .agents
            .values()
            .filter(|a| a.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    fn save(&mut self, agent: &Agent) -> Result<(), Self::Error> {
        self.agents.insert(agent.id, agent.clone());
        Ok(())
    }
}

impl InboxRepo for InMemoryRepo {
    type Error = InMemoryError;

    fn get(&self, id: InboxItemId) -> Result<Option<InboxItem>, Self::Error> {
        Ok(self.inbox.get(&id).cloned())
    }

    fn list_unacked(&self) -> Result<Vec<InboxItem>, Self::Error> {
        Ok(self.inbox.values().filter(|i| !i.acked).cloned().collect())
    }

    fn save(&mut self, item: &InboxItem) -> Result<(), Self::Error> {
        self.inbox.insert(item.id, item.clone());
        Ok(())
    }
}

impl EventLog for InMemoryRepo {
    type Error = InMemoryError;

    fn append(&mut self, event: &DomainEvent) -> Result<(), Self::Error> {
        self.events.push(event.clone());
        Ok(())
    }

    fn list_for_workspace(&self, _id: WorkspaceId) -> Result<Vec<DomainEvent>, Self::Error> {
        // In-memory: return all events (no workspace filter needed in tests).
        Ok(self.events.clone())
    }
}
