use rally_core::ids::WorkspaceId;
use rally_core::ports::AliasRepo;

use crate::convert::{str_to_ws_id, ws_id_to_str};
use crate::db::Store;
use crate::StoreError;

impl AliasRepo for Store {
    type Error = StoreError;

    fn resolve(&self, alias: &str) -> Result<Option<WorkspaceId>, Self::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached("SELECT workspace_id FROM aliases WHERE alias = ?1")?;
        let mut rows = stmt.query([alias])?;
        if let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            Ok(Some(str_to_ws_id(&id_str)?))
        } else {
            Ok(None)
        }
    }

    fn set_alias(&mut self, alias: &str, workspace_id: WorkspaceId) -> Result<(), Self::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO aliases (alias, workspace_id) VALUES (?1, ?2)
             ON CONFLICT(alias) DO UPDATE SET workspace_id = excluded.workspace_id",
            rusqlite::params![alias, ws_id_to_str(workspace_id)],
        )?;
        Ok(())
    }

    fn remove_alias(&mut self, alias: &str) -> Result<(), Self::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM aliases WHERE alias = ?1", [alias])?;
        Ok(())
    }

    fn list_aliases(&self) -> Result<Vec<(String, WorkspaceId)>, Self::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare_cached("SELECT alias, workspace_id FROM aliases ORDER BY alias")?;
        let rows = stmt.query_map([], |r| {
            let alias: String = r.get(0)?;
            let id_str: String = r.get(1)?;
            Ok((alias, id_str))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (alias, id_str) = row?;
            result.push((alias, str_to_ws_id(&id_str)?));
        }
        Ok(result)
    }
}
