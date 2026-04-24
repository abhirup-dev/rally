use crate::agent::Agent;
use crate::event::DomainEvent;
use crate::ids::{AgentId, HookId, InboxItemId, PaneId, Timestamp, WorkspaceId};
use crate::inbox::InboxItem;
use crate::workspace::Workspace;

/// Persistence for workspaces.
pub trait WorkspaceRepo {
    /// Error type for this repository.
    type Error: std::error::Error;

    /// Look up a workspace by ID.
    fn get(&self, id: WorkspaceId) -> Result<Option<Workspace>, Self::Error>;
    /// List all workspaces.
    fn list(&self) -> Result<Vec<Workspace>, Self::Error>;
    /// Insert or update a workspace.
    fn save(&mut self, workspace: &Workspace) -> Result<(), Self::Error>;
}

/// Persistence for agents.
pub trait AgentRepo {
    /// Error type for this repository.
    type Error: std::error::Error;

    /// Look up an agent by ID.
    fn get(&self, id: AgentId) -> Result<Option<Agent>, Self::Error>;
    /// List agents belonging to a workspace.
    fn list_by_workspace(&self, workspace_id: WorkspaceId) -> Result<Vec<Agent>, Self::Error>;
    /// Insert or update an agent.
    fn save(&mut self, agent: &Agent) -> Result<(), Self::Error>;
}

/// Persistence for inbox items.
pub trait InboxRepo {
    /// Error type for this repository.
    type Error: std::error::Error;

    /// Look up an inbox item by ID.
    fn get(&self, id: InboxItemId) -> Result<Option<InboxItem>, Self::Error>;
    /// List all unacknowledged inbox items.
    fn list_unacked(&self) -> Result<Vec<InboxItem>, Self::Error>;
    /// Insert or update an inbox item.
    fn save(&mut self, item: &InboxItem) -> Result<(), Self::Error>;
}

/// Append-only event log for domain events.
pub trait EventLog {
    /// Error type for this log.
    type Error: std::error::Error;

    /// Append a domain event to the log.
    fn append(&mut self, event: &DomainEvent) -> Result<(), Self::Error>;
    /// List all events for a workspace, ordered by sequence.
    fn list_for_workspace(&self, id: WorkspaceId) -> Result<Vec<DomainEvent>, Self::Error>;
}

/// Workspace alias resolution.
pub trait AliasRepo {
    /// Error type for this repository.
    type Error: std::error::Error;

    /// Resolve an alias to a workspace ID.
    fn resolve(&self, alias: &str) -> Result<Option<WorkspaceId>, Self::Error>;
    /// Create or update an alias pointing to a workspace.
    fn set_alias(&mut self, alias: &str, workspace_id: WorkspaceId) -> Result<(), Self::Error>;
    /// Remove an alias.
    fn remove_alias(&mut self, alias: &str) -> Result<(), Self::Error>;
    /// List all alias→workspace mappings.
    fn list_aliases(&self) -> Result<Vec<(String, WorkspaceId)>, Self::Error>;
}

/// Clock abstraction for deterministic testing.
pub trait Clock: Send + Sync {
    /// Current time as a `Timestamp`.
    fn now(&self) -> Timestamp;
}

/// ID generation abstraction for deterministic testing.
pub trait IdGen: Send + Sync {
    /// Generate a new workspace ID.
    fn next_workspace_id(&self) -> WorkspaceId;
    /// Generate a new agent ID.
    fn next_agent_id(&self) -> AgentId;
    /// Generate a new pane ID.
    fn next_pane_id(&self) -> PaneId;
    /// Generate a new inbox item ID.
    fn next_inbox_item_id(&self) -> InboxItemId;
    /// Generate a new hook registration ID.
    fn next_hook_id(&self) -> HookId;
}
