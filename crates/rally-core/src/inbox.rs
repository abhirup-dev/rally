use compact_str::CompactString;

use crate::agent::AgentState;
use crate::ids::{AgentId, InboxItemId, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Urgency {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxKind {
    AgentStateChange { state: AgentState },
    CaptureMatch { pattern: CompactString },
    HookNotification { message: CompactString },
    IdleTimeout,
}

#[derive(Debug, Clone)]
pub struct InboxItem {
    pub id: InboxItemId,
    pub agent_id: Option<AgentId>,
    pub urgency: Urgency,
    pub kind: InboxKind,
    pub raised_at: Timestamp,
    pub acked: bool,
}

impl InboxItem {
    pub fn new(
        id: InboxItemId,
        agent_id: Option<AgentId>,
        urgency: Urgency,
        kind: InboxKind,
        raised_at: Timestamp,
    ) -> Self {
        Self {
            id,
            agent_id,
            urgency,
            kind,
            raised_at,
            acked: false,
        }
    }

    pub fn ack(&mut self) {
        self.acked = true;
    }
}
