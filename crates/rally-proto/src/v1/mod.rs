use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Re-export id types with serde support for the wire layer.
pub use rally_core::ids::{AgentId, HookId, InboxItemId, PaneId, Timestamp, WorkspaceId};

// ---------------------------------------------------------------------------
// Shared value types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Initializing,
    Running,
    Idle,
    WaitingForInput,
    AttentionRequired,
    Completed,
    Failed,
    Stopped,
}

impl From<rally_core::agent::AgentState> for AgentState {
    fn from(s: rally_core::agent::AgentState) -> Self {
        use rally_core::agent::AgentState as C;
        match s {
            C::Initializing => Self::Initializing,
            C::Running => Self::Running,
            C::Idle => Self::Idle,
            C::WaitingForInput => Self::WaitingForInput,
            C::AttentionRequired => Self::AttentionRequired,
            C::Completed => Self::Completed,
            C::Failed => Self::Failed,
            C::Stopped => Self::Stopped,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Low,
    Medium,
    High,
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Request {
    CreateWorkspace {
        name: CompactString,
        repo: Option<PathBuf>,
    },
    ArchiveWorkspace {
        id: WorkspaceId,
    },
    ListWorkspaces,
    GetWorkspace {
        id: WorkspaceId,
    },
    RegisterAgent {
        workspace_id: WorkspaceId,
        role: CompactString,
        runtime: CompactString,
    },
    GetAgent {
        id: AgentId,
    },
    ListAgents {
        workspace_id: Option<WorkspaceId>,
    },
    EmitAgentEvent {
        agent_id: AgentId,
        trigger: AgentTriggerWire,
    },
    ListInbox {
        unacked_only: bool,
    },
    AckInboxItem {
        id: InboxItemId,
    },
}

/// Serializable trigger enum for the wire layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTriggerWire {
    Started,
    IdleTimeout,
    InputReceived,
    HookWaitingForInput,
    CaptureRuleAttention,
    InputResolved,
    Acknowledged,
    HookCompleted,
    HookFailed,
    StopRequested,
    Restarted,
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Error { message: String },
    Workspace(WorkspaceView),
    WorkspaceList(Vec<WorkspaceView>),
    Agent(AgentView),
    AgentList(Vec<AgentView>),
    InboxList(Vec<InboxItemView>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceView {
    pub id: WorkspaceId,
    pub name: CompactString,
    pub repo: Option<PathBuf>,
    pub archived: bool,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentView {
    pub id: AgentId,
    pub workspace_id: WorkspaceId,
    pub role: CompactString,
    pub runtime: CompactString,
    pub state: AgentState,
    pub restart_count: u32,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxItemView {
    pub id: InboxItemId,
    pub agent_id: Option<AgentId>,
    pub urgency: Urgency,
    pub acked: bool,
    pub raised_at: u64,
}

// ---------------------------------------------------------------------------
// Event envelope (for streaming / subscription)
// ---------------------------------------------------------------------------

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub version: u64,
    pub at: u64,
    pub payload: EventPayload,
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventPayload {
    WorkspaceCreated { id: WorkspaceId, name: CompactString },
    WorkspaceArchived { id: WorkspaceId },
    AgentRegistered { id: AgentId, workspace: WorkspaceId, role: CompactString },
    AgentStateChanged { id: AgentId, from: AgentState, to: AgentState },
    InboxItemRaised { id: InboxItemId, urgency: Urgency },
    InboxItemAcked { id: InboxItemId },
}
