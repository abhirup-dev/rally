use compact_str::CompactString;
use std::path::PathBuf;

use crate::agent::{AgentState, StateCause};
use crate::ids::{AgentId, HookId, InboxItemId, Timestamp, WorkspaceId};
use crate::inbox::{InboxKind, Urgency};
use crate::pane::PaneRef;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum DomainEvent {
    WorkspaceCreated {
        id: WorkspaceId,
        name: CompactString,
        repo: Option<PathBuf>,
        at: Timestamp,
    },
    WorkspaceArchived {
        id: WorkspaceId,
        at: Timestamp,
    },
    AgentRegistered {
        id: AgentId,
        workspace: WorkspaceId,
        role: CompactString,
        runtime: CompactString,
        at: Timestamp,
    },
    AgentAttachedPane {
        id: AgentId,
        pane_ref: PaneRef,
        at: Timestamp,
    },
    AgentStateChanged {
        id: AgentId,
        from: AgentState,
        to: AgentState,
        cause: StateCause,
        at: Timestamp,
    },
    AgentMetadataUpdated {
        id: AgentId,
        key: CompactString,
        value: serde_json::Value,
        at: Timestamp,
    },
    CaptureSnapshot {
        agent: AgentId,
        bytes_hash: [u8; 32],
        at: Timestamp,
    },
    InboxItemRaised {
        id: InboxItemId,
        agent: Option<AgentId>,
        urgency: Urgency,
        kind: InboxKind,
        at: Timestamp,
    },
    InboxItemAcked {
        id: InboxItemId,
        at: Timestamp,
    },
    HookFired {
        registration: HookId,
        event: CompactString,
        at: Timestamp,
    },
}

impl DomainEvent {
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
