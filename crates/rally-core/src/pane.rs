use compact_str::CompactString;

/// A reference to a live Zellij pane. Stored on an Agent after the _attach shim reports in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRef {
    pub session_name: CompactString,
    pub tab_index: u32,
    pub pane_id: u32,
}
