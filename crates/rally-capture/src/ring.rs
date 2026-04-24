use bytes::Bytes;

/// A fixed-capacity ring buffer of captured output lines.
///
/// Lines are stored as `Bytes` slices (zero-copy subranges of the capture
/// buffer). When the ring is full the oldest line is dropped.
pub struct LineIndexedRing {
    lines: std::collections::VecDeque<Bytes>,
    capacity: usize,
}

impl LineIndexedRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: std::collections::VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a raw screen dump, splitting on newlines.
    pub fn push_screen(&mut self, raw: &str) {
        for line in raw.lines() {
            if self.lines.len() == self.capacity {
                self.lines.pop_front();
            }
            self.lines
                .push_back(Bytes::copy_from_slice(line.as_bytes()));
        }
    }

    /// Return all buffered lines as UTF-8 strings (best-effort).
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines
            .iter()
            .filter_map(|b| std::str::from_utf8(b).ok())
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Drain all lines, returning them as a single newline-joined string.
    pub fn snapshot_text(&self) -> String {
        self.lines().collect::<Vec<_>>().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_capacity_evicts_oldest() {
        let mut ring = LineIndexedRing::new(3);
        ring.push_screen("a\nb\nc\nd");
        let lines: Vec<_> = ring.lines().collect();
        assert_eq!(lines, vec!["b", "c", "d"]);
    }

    #[test]
    fn snapshot_text_joins_lines() {
        let mut ring = LineIndexedRing::new(10);
        ring.push_screen("hello\nworld");
        assert_eq!(ring.snapshot_text(), "hello\nworld");
    }

    #[test]
    fn sustained_input_retains_last_n() {
        let cap = 1024;
        let mut ring = LineIndexedRing::new(cap);
        let total = 10_000;
        for i in 0..total {
            ring.push_screen(&format!("line-{i}"));
        }
        assert_eq!(ring.len(), cap);
        let lines: Vec<_> = ring.lines().collect();
        assert_eq!(lines.first().unwrap(), &format!("line-{}", total - cap));
        assert_eq!(lines.last().unwrap(), &format!("line-{}", total - 1));
    }

    #[test]
    fn empty_input_does_nothing() {
        let mut ring = LineIndexedRing::new(10);
        ring.push_screen("");
        assert!(ring.is_empty());
    }
}
