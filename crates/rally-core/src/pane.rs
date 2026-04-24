use compact_str::CompactString;

/// A reference to a live Zellij pane. Stored on an Agent after the `_attach` shim reports in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRef {
    /// Zellij session name (e.g. "rally-myproject").
    pub session_name: CompactString,
    /// Tab index within the session.
    pub tab_index: u32,
    /// Numeric pane ID within the tab.
    pub pane_id: u32,
}
