use tracing::{debug, instrument, warn};

use crate::ring::LineIndexedRing;
use rally_host_zellij::session::SessionHandle;

/// A point-in-time capture of a pane's screen content.
#[derive(Debug, Clone)]
pub struct CaptureSnapshot {
    pub pane_id: u32,
    pub session_name: String,
    pub text: String,
}

/// Trait for pane capture backends.
pub trait CaptureSource {
    /// Take a snapshot of the pane now.
    fn snapshot(&self) -> anyhow::Result<CaptureSnapshot>;
}

/// Phase 4 capture backend: calls `zellij action dump-screen` and stores in
/// a ring buffer. Polls at 5 Hz when used with `tail --follow`.
pub struct DumpScreenSource {
    pub session: Option<SessionHandle>,
    pub pane_id: u32,
    ring: std::sync::Mutex<LineIndexedRing>,
}

impl DumpScreenSource {
    pub fn new(session: Option<SessionHandle>, pane_id: u32) -> Self {
        Self {
            session,
            pane_id,
            ring: std::sync::Mutex::new(LineIndexedRing::new(2048)),
        }
    }

    /// Poll the pane screen and push new content into the ring buffer.
    #[instrument(skip(self), fields(pane_id = self.pane_id))]
    pub fn poll(&self) -> anyhow::Result<String> {
        let text =
            rally_host_zellij::ZellijActions::dump_screen(self.session.as_ref(), self.pane_id)?;
        debug!(pane_id = self.pane_id, bytes = text.len(), "polled screen");
        let mut ring = self.ring.lock().unwrap();
        ring.push_screen(&text);
        Ok(text)
    }

    /// Stream pane output to stdout as ndjson lines until interrupted.
    pub fn tail_follow(&self, interval: std::time::Duration) -> anyhow::Result<()> {
        use std::io::Write;
        let mut last = String::new();
        loop {
            match self.poll() {
                Ok(text) if text != last => {
                    let line = serde_json::json!({ "text": text });
                    println!("{line}");
                    std::io::stdout().flush().ok();
                    last = text;
                }
                Ok(_) => {}
                Err(e) => {
                    warn!(error = %e, "capture poll failed");
                }
            }
            std::thread::sleep(interval);
        }
    }
}

impl CaptureSource for DumpScreenSource {
    fn snapshot(&self) -> anyhow::Result<CaptureSnapshot> {
        let text = self.poll()?;
        let session_name = self
            .session
            .as_ref()
            .map(|h| h.session_name.to_string())
            .unwrap_or_default();
        Ok(CaptureSnapshot {
            pane_id: self.pane_id,
            session_name,
            text,
        })
    }
}
