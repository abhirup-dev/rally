/// Conversions between domain types and their SQLite-storable representations.
use rally_core::agent::AgentState;
use rally_core::event::DomainEvent;
use rally_core::ids::{AgentId, HookId, InboxItemId, WorkspaceId};
use rally_core::inbox::Urgency;
use rally_core::pane::PaneRef;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::StoreError;

// ── ID helpers ───────────────────────────────────────────────────────────────

pub fn ws_id_to_str(id: WorkspaceId) -> String {
    id.inner().to_string()
}

pub fn agent_id_to_str(id: AgentId) -> String {
    id.inner().to_string()
}

pub fn inbox_id_to_str(id: InboxItemId) -> String {
    id.inner().to_string()
}

pub fn hook_id_to_str(id: HookId) -> String {
    id.inner().to_string()
}

fn parse_ulid(s: &str) -> Result<Ulid, StoreError> {
    s.parse::<Ulid>().map_err(|e| {
        StoreError::NotFound(format!("invalid ulid '{s}': {e}"))
    })
}

pub fn str_to_ws_id(s: &str) -> Result<WorkspaceId, StoreError> {
    Ok(WorkspaceId::new(parse_ulid(s)?))
}

pub fn str_to_agent_id(s: &str) -> Result<AgentId, StoreError> {
    Ok(AgentId::new(parse_ulid(s)?))
}

#[allow(dead_code)]
pub fn str_to_inbox_id(s: &str) -> Result<InboxItemId, StoreError> {
    Ok(InboxItemId::new(parse_ulid(s)?))
}

#[allow(dead_code)]
pub fn str_to_hook_id(s: &str) -> Result<HookId, StoreError> {
    Ok(HookId::new(parse_ulid(s)?))
}

// ── AgentState ↔ string ──────────────────────────────────────────────────────

pub fn state_to_str(s: AgentState) -> &'static str {
    match s {
        AgentState::Initializing     => "initializing",
        AgentState::Running          => "running",
        AgentState::Idle             => "idle",
        AgentState::WaitingForInput  => "waiting_for_input",
        AgentState::AttentionRequired => "attention_required",
        AgentState::Completed        => "completed",
        AgentState::Failed           => "failed",
        AgentState::Stopped          => "stopped",
    }
}

pub fn str_to_state(s: &str) -> Result<AgentState, StoreError> {
    match s {
        "initializing"      => Ok(AgentState::Initializing),
        "running"           => Ok(AgentState::Running),
        "idle"              => Ok(AgentState::Idle),
        "waiting_for_input" => Ok(AgentState::WaitingForInput),
        "attention_required"=> Ok(AgentState::AttentionRequired),
        "completed"         => Ok(AgentState::Completed),
        "failed"            => Ok(AgentState::Failed),
        "stopped"           => Ok(AgentState::Stopped),
        other => Err(StoreError::NotFound(format!("unknown state: {other}"))),
    }
}

// ── DomainEvent ↔ StoredEvent ────────────────────────────────────────────────

/// Serialisable envelope stored in the `events` table.
#[derive(Debug, Serialize, Deserialize)]
pub struct StoredEvent {
    pub workspace_id: String,
    pub kind: String,
    pub payload: serde_json::Value,
    pub at_ms: u64,
}

pub fn event_to_stored(e: &DomainEvent) -> StoredEvent {
    use serde_json::json;

    match e {
        DomainEvent::WorkspaceCreated { id, name, repo, at } => StoredEvent {
            workspace_id: ws_id_to_str(*id),
            kind: "WorkspaceCreated".into(),
            payload: json!({ "id": ws_id_to_str(*id), "name": name.as_str(),
                             "repo": repo.as_ref().map(|p| p.to_string_lossy().into_owned()) }),
            at_ms: at.as_millis(),
        },
        DomainEvent::WorkspaceArchived { id, at } => StoredEvent {
            workspace_id: ws_id_to_str(*id),
            kind: "WorkspaceArchived".into(),
            payload: json!({ "id": ws_id_to_str(*id) }),
            at_ms: at.as_millis(),
        },
        DomainEvent::AgentRegistered { id, workspace, role, runtime, at } => StoredEvent {
            workspace_id: ws_id_to_str(*workspace),
            kind: "AgentRegistered".into(),
            payload: json!({ "id": agent_id_to_str(*id), "workspace": ws_id_to_str(*workspace),
                             "role": role.as_str(), "runtime": runtime.as_str() }),
            at_ms: at.as_millis(),
        },
        DomainEvent::AgentAttachedPane { id, pane_ref, at } => StoredEvent {
            workspace_id: "".into(),
            kind: "AgentAttachedPane".into(),
            payload: json!({ "id": agent_id_to_str(*id),
                             "session": pane_ref.session_name.as_str(),
                             "tab": pane_ref.tab_index, "pane": pane_ref.pane_id }),
            at_ms: at.as_millis(),
        },
        DomainEvent::AgentStateChanged { id, from, to, cause, at } => StoredEvent {
            workspace_id: "".into(),
            kind: "AgentStateChanged".into(),
            payload: json!({ "id": agent_id_to_str(*id),
                             "from": state_to_str(*from), "to": state_to_str(*to),
                             "cause": format!("{cause:?}") }),
            at_ms: at.as_millis(),
        },
        DomainEvent::AgentMetadataUpdated { id, key, value, at } => StoredEvent {
            workspace_id: "".into(),
            kind: "AgentMetadataUpdated".into(),
            payload: json!({ "id": agent_id_to_str(*id), "key": key.as_str(), "value": value }),
            at_ms: at.as_millis(),
        },
        DomainEvent::CaptureSnapshot { agent, bytes_hash, at } => StoredEvent {
            workspace_id: "".into(),
            kind: "CaptureSnapshot".into(),
            payload: json!({ "agent": agent_id_to_str(*agent),
                             "hash": hex::encode_bytes_hash(bytes_hash) }),
            at_ms: at.as_millis(),
        },
        DomainEvent::InboxItemRaised { id, agent, urgency, kind, at } => StoredEvent {
            workspace_id: "".into(),
            kind: "InboxItemRaised".into(),
            payload: json!({ "id": inbox_id_to_str(*id),
                             "agent": agent.map(|a| agent_id_to_str(a)),
                             "urgency": urgency_to_str(*urgency),
                             "kind": format!("{kind:?}") }),
            at_ms: at.as_millis(),
        },
        DomainEvent::InboxItemAcked { id, at } => StoredEvent {
            workspace_id: "".into(),
            kind: "InboxItemAcked".into(),
            payload: json!({ "id": inbox_id_to_str(*id) }),
            at_ms: at.as_millis(),
        },
        DomainEvent::HookFired { registration, event, at } => StoredEvent {
            workspace_id: "".into(),
            kind: "HookFired".into(),
            payload: json!({ "registration": hook_id_to_str(*registration),
                             "event": event.as_str() }),
            at_ms: at.as_millis(),
        },
        // Forward-compat: unknown variants from future DomainEvent additions.
        _ => StoredEvent {
            workspace_id: "".into(),
            kind: "Unknown".into(),
            payload: serde_json::Value::Null,
            at_ms: 0,
        },
    }
}

fn urgency_to_str(u: Urgency) -> &'static str {
    match u {
        Urgency::Low    => "low",
        Urgency::Medium => "medium",
        Urgency::High   => "high",
    }
}

mod hex {
    pub fn encode_bytes_hash(bytes: &[u8; 32]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

// ── PaneRef helpers ──────────────────────────────────────────────────────────

pub fn pane_ref_from_row(
    session: Option<String>,
    tab: Option<u32>,
    pane_id: Option<u32>,
) -> Option<PaneRef> {
    match (session, tab, pane_id) {
        (Some(s), Some(t), Some(p)) => Some(PaneRef {
            session_name: s.into(),
            tab_index: t,
            pane_id: p,
        }),
        _ => None,
    }
}
