use std::path::PathBuf;

use compact_str::CompactString;
use rally_core::ids::{Timestamp, WorkspaceId};
use rally_core::ports::WorkspaceRepo;
use rally_core::workspace::Workspace;
use rusqlite::Connection;

use crate::convert::{str_to_ws_id, ws_id_to_str};
use crate::db::Store;
use crate::StoreError;

pub(crate) fn insert_workspace(conn: &Connection, ws: &Workspace) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO workspaces (id, name, canonical_key, repo, created_at, archived)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           name          = excluded.name,
           canonical_key = excluded.canonical_key,
           repo          = excluded.repo,
           archived      = excluded.archived",
        rusqlite::params![
            ws_id_to_str(ws.id),
            ws.name.as_str(),
            ws.canonical_key.as_str(),
            ws.repo.as_ref().map(|p| p.to_string_lossy().into_owned()),
            ws.created_at.as_millis() as i64,
            ws.archived as i64,
        ],
    )?;
    Ok(())
}

impl WorkspaceRepo for Store {
    type Error = StoreError;

    fn get(&self, id: WorkspaceId) -> Result<Option<Workspace>, Self::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, canonical_key, repo, created_at, archived FROM workspaces WHERE id = ?1",
        )?;
        let mut rows = stmt.query([ws_id_to_str(id)])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_workspace(row)?))
        } else {
            Ok(None)
        }
    }

    fn list(&self) -> Result<Vec<Workspace>, Self::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, canonical_key, repo, created_at, archived FROM workspaces ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(row_to_workspace(r))
        })?;
        rows.map(|r| r?).collect()
    }

    fn save(&mut self, ws: &Workspace) -> Result<(), Self::Error> {
        let conn = self.conn.lock().unwrap();
        insert_workspace(&conn, ws)
    }
}

fn row_to_workspace(row: &rusqlite::Row<'_>) -> Result<Workspace, StoreError> {
    let id_str: String        = row.get(0)?;
    let name: String          = row.get(1)?;
    let canonical_key: String = row.get(2)?;
    let repo: Option<String>  = row.get(3)?;
    let at_ms: i64            = row.get(4)?;
    let archived: i64         = row.get(5)?;

    Ok(Workspace {
        id: str_to_ws_id(&id_str)?,
        name: CompactString::from(name),
        canonical_key: CompactString::from(canonical_key),
        repo: repo.map(PathBuf::from),
        created_at: Timestamp::from_millis(at_ms as u64),
        archived: archived != 0,
    })
}
