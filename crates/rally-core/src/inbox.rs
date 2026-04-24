use compact_str::CompactString;

use crate::agent::AgentState;
use crate::ids::{AgentId, InboxItemId, Timestamp};

/// Priority level for inbox items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Urgency {
    /// Informational, no action needed.
    Low,
    /// Should be reviewed soon.
    Medium,
    /// Requires immediate attention.
    High,
}

/// What triggered the inbox item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxKind {
    /// An agent changed state.
    AgentStateChange {
        /// The state that triggered the notification.
        state: AgentState,
    },
    /// A capture rule matched pane output.
    CaptureMatch {
        /// The pattern that matched.
        pattern: CompactString,
    },
    /// A hook sent a notification.
    HookNotification {
        /// Human-readable message from the hook.
        message: CompactString,
    },
    /// An agent has been idle too long.
    IdleTimeout,
}

/// A notification queued for human review.
#[derive(Debug, Clone)]
pub struct InboxItem {
    /// Unique identifier.
    pub id: InboxItemId,
    /// Agent that triggered this item, if any.
    pub agent_id: Option<AgentId>,
    /// How urgent this item is.
    pub urgency: Urgency,
    /// What caused this notification.
    pub kind: InboxKind,
    /// When the item was raised.
    pub raised_at: Timestamp,
    /// Whether a human has acknowledged this item.
    pub acked: bool,
}

impl InboxItem {
    /// Create a new unacknowledged inbox item.
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

    /// Mark this item as acknowledged.
    pub fn ack(&mut self) {
        self.acked = true;
    }
}
