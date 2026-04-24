use crate::ids::AgentId;

/// How output is captured from an agent pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// One-shot dump of the current screen contents.
    Snapshot,
    /// A bounded sliding window of recent lines.
    Window,
    /// Continuous stream from the pane.
    Stream,
    /// Fan-in from multiple agents, tagged by source.
    Group,
}

/// A reference to stored capture data (body on disk / ring buffer).
#[derive(Debug, Clone)]
pub struct CaptureRef {
    /// Which agent produced this capture.
    pub agent: AgentId,
    /// Capture strategy used.
    pub mode: CaptureMode,
    /// SHA-256 of the captured bytes for deduplication.
    pub bytes_hash: [u8; 32],
}
