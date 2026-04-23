use crate::agent::Agent;
use crate::event::DomainEvent;
use crate::ids::{AgentId, HookId, InboxItemId, PaneId, Timestamp, WorkspaceId};
use crate::inbox::InboxItem;
use crate::workspace::Workspace;

pub trait WorkspaceRepo {
    type Error: std::error::Error;

    fn get(&self, id: WorkspaceId) -> Result<Option<Workspace>, Self::Error>;
    fn list(&self) -> Result<Vec<Workspace>, Self::Error>;
    fn save(&mut self, workspace: &Workspace) -> Result<(), Self::Error>;
}

pub trait AgentRepo {
    type Error: std::error::Error;

    fn get(&self, id: AgentId) -> Result<Option<Agent>, Self::Error>;
    fn list_by_workspace(&self, workspace_id: WorkspaceId) -> Result<Vec<Agent>, Self::Error>;
    fn save(&mut self, agent: &Agent) -> Result<(), Self::Error>;
}

pub trait InboxRepo {
    type Error: std::error::Error;

    fn get(&self, id: InboxItemId) -> Result<Option<InboxItem>, Self::Error>;
    fn list_unacked(&self) -> Result<Vec<InboxItem>, Self::Error>;
    fn save(&mut self, item: &InboxItem) -> Result<(), Self::Error>;
}

pub trait EventLog {
    type Error: std::error::Error;

    fn append(&mut self, event: &DomainEvent) -> Result<(), Self::Error>;
    fn list_for_workspace(&self, id: WorkspaceId) -> Result<Vec<DomainEvent>, Self::Error>;
}

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

pub trait IdGen: Send + Sync {
    fn next_workspace_id(&self) -> WorkspaceId;
    fn next_agent_id(&self) -> AgentId;
    fn next_pane_id(&self) -> PaneId;
    fn next_inbox_item_id(&self) -> InboxItemId;
    fn next_hook_id(&self) -> HookId;
}
