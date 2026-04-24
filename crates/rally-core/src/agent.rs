use std::path::PathBuf;

use compact_str::CompactString;
use thiserror::Error;
use tracing::warn;

use crate::ids::{AgentId, PaneId, Timestamp, WorkspaceId};
use crate::pane::PaneRef;

/// Lifecycle states an agent can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentState {
    /// Agent created, not yet started.
    Initializing,
    /// Actively executing.
    Running,
    /// No input activity for the idle timeout duration.
    Idle,
    /// Blocked on user input.
    WaitingForInput,
    /// A capture rule or hook flagged this agent for human review.
    AttentionRequired,
    /// Finished successfully.
    Completed,
    /// Exited with an error.
    Failed,
    /// Manually stopped.
    Stopped,
}

/// Triggers that drive the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentTrigger {
    /// Process started executing.
    Started,
    /// No activity for the configured idle period.
    IdleTimeout,
    /// New input detected in the pane.
    InputReceived,
    /// Hook signalled that the agent needs user input.
    HookWaitingForInput,
    /// A capture rule matched, requesting human attention.
    CaptureRuleAttention,
    /// Previously-needed input has been provided.
    InputResolved,
    /// Human acknowledged an attention-required state.
    Acknowledged,
    /// Hook reported successful completion.
    HookCompleted,
    /// Hook reported failure.
    HookFailed,
    /// User or system requested a graceful stop.
    StopRequested,
    /// Agent is being restarted from a terminal state.
    Restarted,
}

/// Why a state transition happened (stored in DomainEvent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateCause {
    /// Process launched.
    Started,
    /// Idle timer expired.
    IdleTimeout,
    /// Pane input detected.
    InputReceived,
    /// Hook-driven event with a description.
    HookEvent(CompactString),
    /// Capture rule matched with the pattern name.
    CaptureRuleMatch(CompactString),
    /// Human acknowledged attention state.
    Acknowledged,
    /// Operator-initiated transition.
    Manual,
}

/// Error returned when a trigger is not valid in the current state.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("invalid transition: {trigger:?} in state {state:?}")]
pub struct InvalidTransition {
    /// State the agent was in when the trigger was attempted.
    pub state: AgentState,
    /// The trigger that was rejected.
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
            warn!(
                state = ?state,
                trigger = ?trigger,
                "invalid agent state transition"
            );
            return Err(InvalidTransition {
                state,
                trigger: trigger.clone(),
            });
        }
    };

    Ok(next)
}

/// An agent registered in a workspace.
#[derive(Debug, Clone)]
pub struct Agent {
    /// Unique identifier.
    pub id: AgentId,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Human-readable role label (e.g. "impl", "review").
    pub role: CompactString,
    /// Runtime type (e.g. "claude-code", "zsh").
    pub runtime: CompactString,
    /// Current lifecycle state.
    pub state: AgentState,
    /// Zellij pane binding, set after `_attach` reports in.
    pub pane_ref: Option<PaneRef>,
    /// Logical pane ID (domain-level, distinct from Zellij numeric ID).
    pub pane_id: Option<PaneId>,
    /// How many times this agent has been restarted.
    pub restart_count: u32,
    /// Current working directory of the agent's process.
    pub cwd: Option<PathBuf>,
    /// Root of the git repository (or project dir) containing cwd.
    pub project_root: Option<PathBuf>,
    /// Current git branch name, derived from project_root.
    pub branch: Option<CompactString>,
    /// Arbitrary key-value metadata.
    pub metadata: std::collections::HashMap<CompactString, serde_json::Value>,
    /// When this agent was first registered.
    pub created_at: Timestamp,
}

impl Agent {
    /// Create a new agent in the `Initializing` state.
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
            cwd: None,
            project_root: None,
            branch: None,
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
