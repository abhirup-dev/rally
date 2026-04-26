mod inbox_summary;
mod status_bar;
mod workspace_tree;

use serde::Deserialize;

pub use inbox_summary::render_inbox_lines;
pub use status_bar::render_status_lines;
pub use workspace_tree::render_tree_lines;

pub struct RenderCtx<'a> {
    pub cols: usize,
    pub agents: &'a [AgentInfo],
    pub inbox_items: &'a [InboxItemInfo],
    pub filter: Option<&'a str>,
    pub status_message: Option<&'a str>,
}

/// A node in the visible (collapse-aware, filter-aware) tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeNode {
    Workspace { id: String },
    Tab { position: usize, name: String },
    Pane { id: u32, tab_position: usize },
    Agent { id: String, workspace_id: String },
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
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
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

/// Truncate `value` to fit within `max` terminal columns (display width, not char count).
/// Uses unicode-width so CJK and wide glyphs are measured correctly.
pub fn truncate_display(value: &str, max: usize) -> String {
    use unicode_width::UnicodeWidthChar;

    let total: usize = value.chars().filter_map(|c| c.width()).sum();
    if total <= max {
        return value.to_string();
    }
    let budget = max.saturating_sub(1); // reserve 1 col for '…'
    let mut width = 0;
    let mut truncated = String::new();
    for c in value.chars() {
        let cw = c.width().unwrap_or(0);
        if width + cw > budget {
            break;
        }
        width += cw;
        truncated.push(c);
    }
    truncated.push('…');
    truncated
}
