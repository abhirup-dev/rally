use std::sync::Arc;

use arc_swap::ArcSwap;
use rally_core::event::DomainEvent;
use tokio::sync::broadcast;
const DEFAULT_CAPACITY: usize = 512;

/// Monotonically increasing version counter for snapshot change detection.
pub type StateVersion = u64;

/// A snapshot of the latest domain state, published via `arc-swap` for
/// lock-free reads (sidebar plugin, MCP list queries).
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    pub version: StateVersion,
    pub events_since_last: Vec<DomainEvent>,
}

impl Default for StateSnapshot {
    fn default() -> Self {
        Self {
            version: 0,
            events_since_last: Vec::new(),
        }
    }
}

/// The event bus — fan-out domain events to subscribers + maintain a
/// lock-free latest-state snapshot.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<DomainEvent>,
    snapshot: Arc<ArcSwap<StateSnapshot>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            snapshot: Arc::new(ArcSwap::from_pointee(StateSnapshot::default())),
        }
    }

    /// Publish a domain event. Returns the number of active subscribers
    /// that received the event.
    pub fn publish(&self, event: DomainEvent) -> usize {
        // Bump the snapshot
        let old = self.snapshot.load();
        let new_version = old.version + 1;
        self.snapshot.store(Arc::new(StateSnapshot {
            version: new_version,
            events_since_last: vec![event.clone()],
        }));

        // Broadcast to live subscribers
        match self.tx.send(event) {
            Ok(n) => n,
            Err(_) => {
                // No active receivers — not an error, just no one listening
                0
            }
        }
    }

    /// Subscribe to live domain events.
    /// Lag detection belongs at the recv() call site, not here — a freshly
    /// created receiver has no meaningful backlog.
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }

    /// Read the latest state snapshot (lock-free).
    pub fn snapshot(&self) -> Arc<StateSnapshot> {
        self.snapshot.load_full()
    }

    /// Current state version (lock-free).
    pub fn version(&self) -> StateVersion {
        self.snapshot.load().version
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compact_str::CompactString;
    use rally_core::ids::{Timestamp, WorkspaceId};
    use ulid::Ulid;

    fn test_event() -> DomainEvent {
        DomainEvent::WorkspaceCreated {
            id: WorkspaceId::new(Ulid::new()),
            name: CompactString::from("test"),
            repo: None,
            at: Timestamp::from_millis(1000),
        }
    }

    #[test]
    fn publish_without_subscribers() {
        let bus = EventBus::new();
        let count = bus.publish(test_event());
        assert_eq!(count, 0);
        assert_eq!(bus.version(), 1);
    }

    #[tokio::test]
    async fn publish_with_subscriber() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        let count = bus.publish(test_event());
        assert_eq!(count, 1);

        let received = rx.recv().await.unwrap();
        assert!(matches!(received, DomainEvent::WorkspaceCreated { .. }));
    }

    #[test]
    fn snapshot_version_increments() {
        let bus = EventBus::new();
        assert_eq!(bus.version(), 0);
        bus.publish(test_event());
        assert_eq!(bus.version(), 1);
        bus.publish(test_event());
        assert_eq!(bus.version(), 2);
    }

    #[test]
    fn snapshot_is_lock_free() {
        let bus = EventBus::new();
        let snap1 = bus.snapshot();
        bus.publish(test_event());
        let snap2 = bus.snapshot();
        // snap1 is still valid (old Arc), snap2 has new version
        assert_eq!(snap1.version, 0);
        assert_eq!(snap2.version, 1);
    }
}
