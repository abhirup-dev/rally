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
// Request envelope (wraps every IPC call)
// ---------------------------------------------------------------------------

/// Wire-level request wrapper. The server generates `request_id` if the
/// client omits it; `client_pid` is self-reported for logging (not security).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    #[serde(default = "default_request_id")]
    pub request_id: CompactString,
    #[serde(default)]
    pub client_pid: Option<u32>,
    #[serde(flatten)]
    pub payload: Request,
}

fn default_request_id() -> CompactString {
    CompactString::from(ulid::Ulid::new().to_string())
}

/// Wire-level response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub request_id: CompactString,
    #[serde(flatten)]
    pub payload: Response,
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
        #[serde(default)]
        cwd: Option<PathBuf>,
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
    GetStateSnapshot,
    BindPane {
        agent_id: AgentId,
        session_name: CompactString,
        tab_index: u32,
        pane_id: u32,
    },
    SetAlias {
        alias: CompactString,
        workspace_id: WorkspaceId,
    },
    ResolveAlias {
        alias: CompactString,
    },
    ListAliases,
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
    WorkspaceList { items: Vec<WorkspaceView> },
    Agent(AgentView),
    AgentList { items: Vec<AgentView> },
    InboxList { items: Vec<InboxItemView> },
    StateSnapshot(StateSnapshotView),
    AliasResolved { workspace_id: Option<WorkspaceId> },
    AliasList { items: Vec<AliasView> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceView {
    pub id: WorkspaceId,
    pub name: CompactString,
    pub canonical_key: CompactString,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pane_session: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<CompactString>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshotView {
    pub version: u64,
    pub workspaces: Vec<WorkspaceView>,
    pub agents: Vec<AgentView>,
    pub inbox_items: Vec<InboxItemView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasView {
    pub alias: CompactString,
    pub workspace_id: WorkspaceId,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_snapshot_response_serializes_as_plugin_payload() {
        let response = Response::StateSnapshot(StateSnapshotView {
            version: 7,
            workspaces: Vec::new(),
            agents: Vec::new(),
            inbox_items: Vec::new(),
        });

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["kind"], "state_snapshot");
        assert_eq!(value["version"], 7);
        assert!(value["workspaces"].is_array());
        assert!(value["agents"].is_array());
        assert!(value["inbox_items"].is_array());
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventPayload {
    WorkspaceCreated {
        id: WorkspaceId,
        name: CompactString,
    },
    WorkspaceArchived {
        id: WorkspaceId,
    },
    AgentRegistered {
        id: AgentId,
        workspace: WorkspaceId,
        role: CompactString,
    },
    AgentStateChanged {
        id: AgentId,
        from: AgentState,
        to: AgentState,
    },
    InboxItemRaised {
        id: InboxItemId,
        urgency: Urgency,
    },
    InboxItemAcked {
        id: InboxItemId,
    },
}
