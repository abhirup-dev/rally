use compact_str::CompactString;
use thiserror::Error;

use crate::ids::AgentId;

/// An intent that an agent or external caller wants to execute against the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    FocusPane {
        agent: AgentId,
    },
    RenamePane {
        agent: AgentId,
        name: CompactString,
    },
    ColorPane {
        agent: AgentId,
        color: CompactString,
    },
    SendInput {
        agent: AgentId,
        text: CompactString,
    },
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("intent {intent:?} not permitted: {reason}")]
    Denied {
        intent: String,
        reason: CompactString,
    },
}

/// Validate an intent against the current policy.
/// Returns `Ok(())` if allowed, `Err(PolicyError::Denied)` otherwise.
pub fn validate(intent: &Intent) -> Result<(), PolicyError> {
    match intent {
        // All read/focus intents are always allowed.
        Intent::FocusPane { .. } => Ok(()),

        // Rename/color are informational and safe.
        Intent::RenamePane { .. } | Intent::ColorPane { .. } => Ok(()),

        // Sending input is a control action — allowed by default but
        // the daemon may override this gate with a config flag.
        Intent::SendInput { .. } => Ok(()),
    }
}
