use compact_str::CompactString;
use thiserror::Error;

use crate::ids::AgentId;

/// An intent that an agent or external caller wants to execute against the host.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Intent {
    /// Focus a specific agent's pane.
    FocusPane { agent: AgentId },
    /// Rename an agent's pane tab.
    RenamePane { agent: AgentId, name: CompactString },
    /// Change an agent's pane border color.
    ColorPane {
        agent: AgentId,
        color: CompactString,
    },
    /// Send text input to an agent's pane.
    SendInput { agent: AgentId, text: CompactString },
}

/// Error when an intent is denied by policy.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PolicyError {
    /// The intent was not permitted.
    #[error("intent {intent:?} not permitted: {reason}")]
    Denied {
        /// Description of the denied intent.
        intent: String,
        /// Why it was denied.
        reason: CompactString,
    },
}

/// Validate an intent against the current policy.
/// Returns `Ok(())` if allowed, `Err(PolicyError::Denied)` otherwise.
pub fn validate(intent: &Intent) -> Result<(), PolicyError> {
    match intent {
        Intent::FocusPane { .. } => Ok(()),
        Intent::RenamePane { .. } | Intent::ColorPane { .. } => Ok(()),
        Intent::SendInput { .. } => Ok(()),
    }
}
