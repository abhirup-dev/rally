mod inbox_summary;
mod status_bar;
mod workspace_tree;

use serde::Deserialize;
use zellij_widgets::prelude::*;

pub use inbox_summary::render_inbox_lines;
pub use status_bar::render_status_lines;
pub use workspace_tree::render_workspace_lines;

pub struct RenderCtx<'a> {
    pub cols: usize,
    pub workspaces: &'a [WorkspaceInfo],
    pub agents: &'a [AgentInfo],
    pub inbox_items: &'a [InboxItemInfo],
    pub selected_agent_id: Option<&'a str>,
    pub filter: Option<&'a str>,
    pub status_message: Option<&'a str>,
}

// Fields are deserialized from daemon JSON; not all are read by the rendering code yet.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    pub canonical_key: String,
}

// Fields are deserialized from daemon JSON; not all are read by the rendering code yet.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub workspace_id: String,
    pub role: String,
    pub runtime: String,
    pub state: String,
    #[serde(default)]
    pub pane_session: Option<String>,
    #[serde(default)]
    pub pane_id: Option<u32>,
}

// Fields are deserialized from daemon JSON; not all are read by the rendering code yet.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct InboxItemInfo {
    pub id: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    pub urgency: String,
    #[serde(default)]
    pub acked: bool,
    #[serde(default)]
    pub raised_at: u64,
    #[serde(default)]
    pub message: Option<String>,
}

pub fn state_glyph(state: &str) -> (&'static str, Style) {
    match state {
        "running" => ("●", Style::default().fg(Color::Green)),
        "idle" => ("◐", Style::default().fg(Color::Yellow)),
        "attention_required" => ("◉", Style::default().fg(Color::Red)),
        "completed" => ("○", Style::default().fg(Color::Green)),
        "stopped" => ("✕", Style::default().add_modifier(Modifier::DIM)),
        "failed" => ("✗", Style::default().fg(Color::Red)),
        "initializing" => ("⧗", Style::default().add_modifier(Modifier::DIM)),
        "waiting_for_input" => ("⚠", Style::default().fg(Color::Yellow)),
        _ => ("?", Style::default()),
    }
}

pub fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let keep = max.saturating_sub(1);
    let mut truncated: String = value.chars().take(keep).collect();
    truncated.push('…');
    truncated
}
