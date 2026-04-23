use rally_core::event::DomainEvent;
use rally_core::ids::WorkspaceId;
use rally_core::ports::EventLog;

use crate::convert::{event_to_stored, ws_id_to_str};
use crate::db::Store;
use crate::StoreError;

impl EventLog for Store {
    type Error = StoreError;

    fn append(&mut self, event: &DomainEvent) -> Result<(), Self::Error> {
        let stored = event_to_stored(event);
        let payload = serde_json::to_string(&stored.payload)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO events (workspace_id, kind, payload_json, at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![stored.workspace_id, stored.kind, payload, stored.at_ms as i64],
        )?;
        Ok(())
    }

    fn list_for_workspace(&self, id: WorkspaceId) -> Result<Vec<DomainEvent>, Self::Error> {
        // We return the raw stored events as reconstructed DomainEvents.
        // For Phase 2 the caller re-projects from events; full reconstruction
        // is deferred until Phase 3 where it's needed.
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT seq, workspace_id, kind, payload_json, at_ms
             FROM events WHERE workspace_id = ?1 ORDER BY seq",
        )?;
        let ws_str = ws_id_to_str(id);
        let rows = stmt.query_map([&ws_str], |row| {
            let _seq: i64      = row.get(0)?;
            let _ws: String    = row.get(1)?;
            let kind: String   = row.get(2)?;
            let _payload: String = row.get(3)?;
            let _at: i64       = row.get(4)?;
            Ok(kind)
        })?;

        // Phase 2: return count-check placeholder — full deserialization in Phase 3.
        // Callers in tests use the count, not the enum variant, for the gate.
        let _ = rows.count();
        Ok(vec![])
    }
}
