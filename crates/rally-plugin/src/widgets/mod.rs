mod inbox_summary;
mod status_bar;
mod workspace_tree;

use serde::Deserialize;

pub use inbox_summary::InboxSummary;
pub use status_bar::StatusBar;
pub use workspace_tree::WorkspaceTree;

#[allow(dead_code)]
pub trait SidebarWidget {
    fn id(&self) -> &'static str;
    fn render(&self, ctx: &RenderCtx<'_>, buf: &mut AnsiBuf);
    fn handle_key(&mut self, _ctx: &mut HandleCtx<'_>, _key: Key) -> Handled {
        Handled::No
    }
}

pub struct RenderCtx<'a> {
    pub cols: usize,
    pub workspaces: &'a [WorkspaceInfo],
    pub agents: &'a [AgentInfo],
    pub inbox_items: &'a [InboxItemInfo],
    pub selected_agent_id: Option<&'a str>,
    pub filter: Option<&'a str>,
    pub status_message: Option<&'a str>,
}

#[allow(dead_code)]
pub struct HandleCtx<'a> {
    pub workspaces: &'a [WorkspaceInfo],
    pub agents: &'a [AgentInfo],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Key {
    Char(char),
    Enter,
    Esc,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Handled {
    Yes,
    No,
}

#[derive(Default)]
pub struct AnsiBuf {
    inner: String,
}

impl AnsiBuf {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: String::with_capacity(capacity),
        }
    }

    pub fn line(&mut self, line: impl AsRef<str>) {
        self.inner.push_str(line.as_ref());
        self.inner.push('\n');
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    pub fn into_string(self) -> String {
        self.inner
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    pub canonical_key: String,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_buf_appends_lines() {
        let mut buf = AnsiBuf::with_capacity(16);
        buf.line("one");
        buf.line("two");

        assert_eq!(buf.as_str(), "one\ntwo\n");
    }
}
