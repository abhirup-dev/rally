use rally_core::event::DomainEvent;
use rally_core::ids::WorkspaceId;
use rally_core::ports::EventLog;
use rusqlite::Connection;
use tracing::{debug, warn};

use crate::convert::{event_to_stored, stored_to_event, ws_id_to_str, StoredEvent};
use crate::db::Store;
use crate::StoreError;

pub(crate) fn insert_event(conn: &Connection, event: &DomainEvent) -> Result<(), StoreError> {
    let stored = event_to_stored(event);
    debug!(
        kind = stored.kind,
        workspace_id = stored.workspace_id,
        at_ms = stored.at_ms,
        "appending domain event"
    );
    let payload = serde_json::to_string(&stored.payload)?;
    conn.execute(
        "INSERT INTO events (workspace_id, kind, payload_json, at_ms)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            stored.workspace_id,
            stored.kind,
            payload,
            stored.at_ms as i64
        ],
    )?;
    Ok(())
}

impl EventLog for Store {
    type Error = StoreError;

    fn append(&mut self, event: &DomainEvent) -> Result<(), Self::Error> {
        let conn = self.conn.lock().unwrap();
        insert_event(&conn, event)
    }

    fn list_for_workspace(&self, id: WorkspaceId) -> Result<Vec<DomainEvent>, Self::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT seq, workspace_id, kind, payload_json, at_ms
             FROM events WHERE workspace_id = ?1 ORDER BY seq",
        )?;
        let ws_str = ws_id_to_str(id);
        let rows = stmt.query_map([&ws_str], |row| {
            let _seq: i64 = row.get(0)?;
            let ws: String = row.get(1)?;
            let kind: String = row.get(2)?;
            let payload_json: String = row.get(3)?;
            let at_ms: i64 = row.get(4)?;
            Ok((ws, kind, payload_json, at_ms))
        })?;

        let mut events = Vec::new();
        for row in rows {
            let (_ws, kind, payload_json, at_ms) = row?;
            let payload: serde_json::Value =
                serde_json::from_str(&payload_json).unwrap_or(serde_json::Value::Null);
            let stored = StoredEvent {
                workspace_id: ws_id_to_str(id),
                kind,
                payload,
                at_ms: at_ms as u64,
            };
            match stored_to_event(&stored) {
                Ok(event) => events.push(event),
                Err(e) => warn!(error = %e, "skipping undeserializable event"),
            }
        }
        Ok(events)
    }
}
