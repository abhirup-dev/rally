use std::sync::Arc;
use std::time::Duration;

use rally_core::event::DomainEvent;
use rally_events::EventBus;
use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::services::RallyService;

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(250);

/// Spawn a task that pushes state snapshots to Zellij plugins via pipe
/// whenever a domain event fires, debounced to max 4 Hz.
pub fn spawn_pipe_pusher(service: Arc<RallyService>, event_bus: &EventBus) {
    let mut rx = event_bus.subscribe();

    tokio::spawn(async move {
        loop {
            // Wait for at least one event
            match rx.recv().await {
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    debug!(skipped = n, "pipe pusher lagged, catching up");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("event bus closed, pipe pusher exiting");
                    return;
                }
            }

            // Trailing-edge debounce: wait 250ms, drain any events that arrive
            tokio::time::sleep(DEBOUNCE_INTERVAL).await;
            drain_pending(&mut rx);

            // Push current snapshot to all active Zellij sessions
            push_snapshot(&service);
        }
    });
}

fn drain_pending(rx: &mut broadcast::Receiver<DomainEvent>) {
    loop {
        match rx.try_recv() {
            Ok(_) => continue,
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                debug!(skipped = n, "drained lagged events");
            }
            Err(_) => break,
        }
    }
}

fn push_snapshot(service: &RallyService) {
    let snapshot = service.state_snapshot();
    let json = match serde_json::to_string(&snapshot) {
        Ok(j) => j,
        Err(e) => {
            warn!(error = %e, "failed to serialize snapshot for pipe push");
            return;
        }
    };

    let sessions = rally_host_zellij::ZellijActions::list_sessions();
    if sessions.is_empty() {
        return;
    }

    debug!(
        session_count = sessions.len(),
        "pushing snapshot to Zellij sessions"
    );
    for session in &sessions {
        if let Err(e) = rally_host_zellij::ZellijActions::pipe_to_plugin(session, &json) {
            warn!(session, error = %e, "pipe push failed");
        }
    }
}
