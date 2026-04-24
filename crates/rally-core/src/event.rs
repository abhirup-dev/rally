use compact_str::CompactString;
use std::path::PathBuf;

use crate::agent::{AgentState, StateCause};
use crate::ids::{AgentId, HookId, InboxItemId, Timestamp, WorkspaceId};
use crate::inbox::{InboxKind, Urgency};
use crate::pane::PaneRef;

/// Events emitted by domain operations and persisted in the event log.
#[non_exhaustive]
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub enum DomainEvent {
    /// A new workspace was created.
    WorkspaceCreated {
        id: WorkspaceId,
        name: CompactString,
        repo: Option<PathBuf>,
        at: Timestamp,
    },
    /// A workspace was archived.
    WorkspaceArchived { id: WorkspaceId, at: Timestamp },
    /// An agent was registered in a workspace.
    AgentRegistered {
        id: AgentId,
        workspace: WorkspaceId,
        role: CompactString,
        runtime: CompactString,
        at: Timestamp,
    },
    /// The `_attach` shim bound a Zellij pane to an agent.
    AgentAttachedPane {
        id: AgentId,
        pane_ref: PaneRef,
        at: Timestamp,
    },
    /// Agent transitioned between lifecycle states.
    AgentStateChanged {
        id: AgentId,
        from: AgentState,
        to: AgentState,
        cause: StateCause,
        at: Timestamp,
    },
    /// A metadata key was set or updated on an agent.
    AgentMetadataUpdated {
        id: AgentId,
        key: CompactString,
        value: serde_json::Value,
        at: Timestamp,
    },
    /// A screen capture was taken from an agent's pane.
    CaptureSnapshot {
        agent: AgentId,
        bytes_hash: [u8; 32],
        at: Timestamp,
    },
    /// An inbox item was raised for human attention.
    InboxItemRaised {
        id: InboxItemId,
        agent: Option<AgentId>,
        urgency: Urgency,
        kind: InboxKind,
        at: Timestamp,
    },
    /// An inbox item was acknowledged.
    InboxItemAcked { id: InboxItemId, at: Timestamp },
    /// A hook was fired in response to an event.
    HookFired {
        registration: HookId,
        event: CompactString,
        at: Timestamp,
    },
}

impl DomainEvent {
    /// Extract the timestamp from any event variant.
    pub fn timestamp(&self) -> Timestamp {
        match self {
            DomainEvent::WorkspaceCreated { at, .. } => *at,
            DomainEvent::WorkspaceArchived { at, .. } => *at,
            DomainEvent::AgentRegistered { at, .. } => *at,
            DomainEvent::AgentAttachedPane { at, .. } => *at,
            DomainEvent::AgentStateChanged { at, .. } => *at,
            DomainEvent::AgentMetadataUpdated { at, .. } => *at,
            DomainEvent::CaptureSnapshot { at, .. } => *at,
            DomainEvent::InboxItemRaised { at, .. } => *at,
            DomainEvent::InboxItemAcked { at, .. } => *at,
            DomainEvent::HookFired { at, .. } => *at,
        }
    }
}
