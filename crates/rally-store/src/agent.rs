use std::collections::HashMap;

use compact_str::CompactString;
use rally_core::agent::Agent;
use rally_core::ids::{AgentId, Timestamp, WorkspaceId};
use rally_core::ports::AgentRepo;
use rusqlite::Connection;

use crate::convert::{
    agent_id_to_str, pane_ref_from_row, state_to_str, str_to_agent_id, str_to_state, str_to_ws_id,
    ws_id_to_str,
};
use crate::db::Store;
use crate::StoreError;

pub(crate) fn insert_agent(conn: &Connection, agent: &Agent) -> Result<(), StoreError> {
    let metadata = serde_json::to_string(&agent.metadata)?;
    let pane = agent.pane_ref.as_ref();
    conn.execute(
        "INSERT INTO agents
           (id, workspace_id, role, runtime, state,
            pane_session, pane_tab_index, pane_id,
            restart_count, cwd, project_root, branch,
            metadata_json, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
         ON CONFLICT(id) DO UPDATE SET
           state           = excluded.state,
           pane_session    = excluded.pane_session,
           pane_tab_index  = excluded.pane_tab_index,
           pane_id         = excluded.pane_id,
           restart_count   = excluded.restart_count,
           cwd             = excluded.cwd,
           project_root    = excluded.project_root,
           branch          = excluded.branch,
           metadata_json   = excluded.metadata_json",
        rusqlite::params![
            agent_id_to_str(agent.id),
            ws_id_to_str(agent.workspace_id),
            agent.role.as_str(),
            agent.runtime.as_str(),
            state_to_str(agent.state),
            pane.map(|p| p.session_name.as_str()),
            pane.map(|p| p.tab_index),
            pane.map(|p| p.pane_id),
            agent.restart_count,
            agent.cwd.as_ref().map(|p| p.to_string_lossy().into_owned()),
            agent
                .project_root
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            agent.branch.as_ref().map(|b| b.as_str()),
            metadata,
            agent.created_at.as_millis() as i64,
        ],
    )?;
    Ok(())
}

impl AgentRepo for Store {
    type Error = StoreError;

    fn get(&self, id: AgentId) -> Result<Option<Agent>, Self::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT id, workspace_id, role, runtime, state,
                    pane_session, pane_tab_index, pane_id,
                    restart_count, cwd, project_root, branch,
                    metadata_json, created_at
             FROM agents WHERE id = ?1",
        )?;
        let mut rows = stmt.query([agent_id_to_str(id)])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_agent(row)?))
        } else {
            Ok(None)
        }
    }

    fn list_by_workspace(&self, workspace_id: WorkspaceId) -> Result<Vec<Agent>, Self::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT id, workspace_id, role, runtime, state,
                    pane_session, pane_tab_index, pane_id,
                    restart_count, cwd, project_root, branch,
                    metadata_json, created_at
             FROM agents WHERE workspace_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map([ws_id_to_str(workspace_id)], |r| Ok(row_to_agent(r)))?;
        rows.map(|r| r?).collect()
    }

    fn save(&mut self, agent: &Agent) -> Result<(), Self::Error> {
        let conn = self.conn.lock().unwrap();
        insert_agent(&conn, agent)
    }
}

fn row_to_agent(row: &rusqlite::Row<'_>) -> Result<Agent, StoreError> {
    let id_str: String = row.get(0)?;
    let ws_str: String = row.get(1)?;
    let role: String = row.get(2)?;
    let runtime: String = row.get(3)?;
    let state_str: String = row.get(4)?;
    let pane_session: Option<String> = row.get(5)?;
    let pane_tab: Option<u32> = row.get(6)?;
    let pane_id_col: Option<u32> = row.get(7)?;
    let restart_count: u32 = row.get(8)?;
    let cwd: Option<String> = row.get(9)?;
    let project_root: Option<String> = row.get(10)?;
    let branch: Option<String> = row.get(11)?;
    let metadata_json: String = row.get(12)?;
    let at_ms: i64 = row.get(13)?;

    let metadata: HashMap<CompactString, serde_json::Value> = serde_json::from_str(&metadata_json)?;

    Ok(Agent {
        id: str_to_agent_id(&id_str)?,
        workspace_id: str_to_ws_id(&ws_str)?,
        role: CompactString::from(role),
        runtime: CompactString::from(runtime),
        state: str_to_state(&state_str)?,
        pane_ref: pane_ref_from_row(pane_session, pane_tab, pane_id_col),
        pane_id: None,
        restart_count,
        cwd: cwd.map(std::path::PathBuf::from),
        project_root: project_root.map(std::path::PathBuf::from),
        branch: branch.map(CompactString::from),
        metadata,
        created_at: Timestamp::from_millis(at_ms as u64),
    })
}
