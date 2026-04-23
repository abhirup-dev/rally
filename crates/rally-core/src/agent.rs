use compact_str::CompactString;
use thiserror::Error;

use crate::ids::{AgentId, PaneId, Timestamp, WorkspaceId};
use crate::pane::PaneRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Triggers that drive the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentTrigger {
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

/// Why a state transition happened (stored in DomainEvent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateCause {
    Started,
    IdleTimeout,
    InputReceived,
    HookEvent(CompactString),
    CaptureRuleMatch(CompactString),
    Acknowledged,
    Manual,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("invalid transition: {trigger:?} in state {state:?}")]
pub struct InvalidTransition {
    pub state: AgentState,
    pub trigger: AgentTrigger,
}

/// Pure state machine — no IO, no allocation on the hot path.
pub fn transition(
    state: AgentState,
    trigger: &AgentTrigger,
) -> Result<AgentState, InvalidTransition> {
    use AgentState::*;
    use AgentTrigger::*;

    let next = match (state, trigger) {
        (Initializing, Started) => Running,

        (Running, IdleTimeout) => Idle,
        (Running, HookWaitingForInput) => WaitingForInput,
        (Running, CaptureRuleAttention) => AttentionRequired,
        (Running, HookCompleted) => Completed,
        (Running, HookFailed) => Failed,
        (Running, StopRequested) => Stopped,

        (Idle, InputReceived) => Running,
        (Idle, HookWaitingForInput) => WaitingForInput,
        (Idle, StopRequested) => Stopped,

        (WaitingForInput, InputResolved) => Running,
        (WaitingForInput, CaptureRuleAttention) => AttentionRequired,
        (WaitingForInput, StopRequested) => Stopped,

        (AttentionRequired, Acknowledged) => Running,
        (AttentionRequired, StopRequested) => Stopped,

        (Stopped, Restarted) => Initializing,
        (Failed, Restarted) => Initializing,

        _ => {
            return Err(InvalidTransition {
                state,
                trigger: trigger.clone(),
            })
        }
    };

    Ok(next)
}

/// An agent registered in a workspace.
#[derive(Debug, Clone)]
pub struct Agent {
    pub id: AgentId,
    pub workspace_id: WorkspaceId,
    pub role: CompactString,
    pub runtime: CompactString,
    pub state: AgentState,
    pub pane_ref: Option<PaneRef>,
    pub pane_id: Option<PaneId>,
    pub restart_count: u32,
    pub metadata: std::collections::HashMap<CompactString, serde_json::Value>,
    pub created_at: Timestamp,
}

impl Agent {
    pub fn new(
        id: AgentId,
        workspace_id: WorkspaceId,
        role: CompactString,
        runtime: CompactString,
        created_at: Timestamp,
    ) -> Self {
        Self {
            id,
            workspace_id,
            role,
            runtime,
            state: AgentState::Initializing,
            pane_ref: None,
            pane_id: None,
            restart_count: 0,
            metadata: std::collections::HashMap::new(),
            created_at,
        }
    }

    /// Apply a trigger, mutating state on success.
    pub fn apply(&mut self, trigger: &AgentTrigger) -> Result<AgentState, InvalidTransition> {
        let next = transition(self.state, trigger)?;
        self.state = next;
        Ok(next)
    }
}
