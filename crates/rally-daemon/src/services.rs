use std::path::PathBuf;
use std::sync::Mutex;

use compact_str::CompactString;
use rally_core::agent::Agent;
use rally_core::event::DomainEvent;
use rally_core::ids::{AgentId, Timestamp, WorkspaceId};
use rally_core::ports::{AgentRepo, AliasRepo, Clock, IdGen, WorkspaceRepo};
use rally_core::workspace::Workspace;
use rally_events::EventBus;
use rally_proto::v1::{AgentView, WorkspaceView};
use rally_store::Store;
use tracing::{info, instrument};
use ulid::Ulid;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Timestamp::from_millis(ms)
    }
}

pub struct UlidIdGen;

impl IdGen for UlidIdGen {
    fn next_workspace_id(&self) -> WorkspaceId { WorkspaceId::new(Ulid::new()) }
    fn next_agent_id(&self) -> AgentId { AgentId::new(Ulid::new()) }
    fn next_pane_id(&self) -> rally_core::ids::PaneId { rally_core::ids::PaneId::new(Ulid::new()) }
    fn next_inbox_item_id(&self) -> rally_core::ids::InboxItemId { rally_core::ids::InboxItemId::new(Ulid::new()) }
    fn next_hook_id(&self) -> rally_core::ids::HookId { rally_core::ids::HookId::new(Ulid::new()) }
}

pub struct RallyService {
    pub store: Mutex<Store>,
    pub clock: SystemClock,
    pub id_gen: UlidIdGen,
    pub event_bus: EventBus,
}

impl RallyService {
    pub fn new(store: Store, event_bus: EventBus) -> Self {
        Self {
            store: Mutex::new(store),
            clock: SystemClock,
            id_gen: UlidIdGen,
            event_bus,
        }
    }

    #[instrument(skip(self), fields(name = %name))]
    pub fn create_workspace(
        &self,
        name: CompactString,
        repo: Option<PathBuf>,
    ) -> anyhow::Result<WorkspaceView> {
        let id = self.id_gen.next_workspace_id();
        let at = self.clock.now();
        let ws = Workspace::new(id, name.clone(), repo.clone(), at);
        let canonical_key = ws.canonical_key.clone();

        let event = DomainEvent::WorkspaceCreated {
            id,
            name: name.clone(),
            repo: repo.clone(),
            at,
        };

        let store = self.store.lock().unwrap();
        store.save_workspace_and_event(&ws, &event).map_err(|e| anyhow::anyhow!("{e}"))?;
        drop(store);

        self.event_bus.publish(event);
        info!(%id, %name, %canonical_key, "workspace created");

        Ok(WorkspaceView {
            id,
            name,
            canonical_key,
            repo,
            archived: false,
            created_at: at.as_millis(),
        })
    }

    pub fn list_workspaces(&self) -> anyhow::Result<Vec<WorkspaceView>> {
        let store = self.store.lock().unwrap();
        let list = WorkspaceRepo::list(&*store).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(list.iter().map(ws_to_view).collect())
    }

    pub fn get_workspace(&self, id: WorkspaceId) -> anyhow::Result<Option<WorkspaceView>> {
        let store = self.store.lock().unwrap();
        let ws = WorkspaceRepo::get(&*store, id).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(ws.as_ref().map(ws_to_view))
    }

    #[instrument(skip(self), fields(%workspace_id, %role, %runtime))]
    pub fn register_agent(
        &self,
        workspace_id: WorkspaceId,
        role: CompactString,
        runtime: CompactString,
    ) -> anyhow::Result<AgentView> {
        let id = self.id_gen.next_agent_id();
        let at = self.clock.now();
        let agent = Agent::new(id, workspace_id, role.clone(), runtime.clone(), at);

        let event = DomainEvent::AgentRegistered {
            id,
            workspace: workspace_id,
            role: role.clone(),
            runtime: runtime.clone(),
            at,
        };

        let store = self.store.lock().unwrap();
        store.save_agent_and_event(&agent, &event).map_err(|e| anyhow::anyhow!("{e}"))?;
        drop(store);

        self.event_bus.publish(event);
        info!(%id, %workspace_id, %role, %runtime, "agent registered");

        Ok(AgentView {
            id,
            workspace_id,
            role,
            runtime,
            state: rally_proto::v1::AgentState::Initializing,
            restart_count: 0,
            pane_session: None,
            pane_id: None,
            created_at: at.as_millis(),
        })
    }

    pub fn bind_pane(
        &self,
        agent_id: AgentId,
        session_name: CompactString,
        tab_index: u32,
        pane_id: u32,
    ) -> anyhow::Result<()> {
        use rally_core::pane::PaneRef;
        let store = self.store.lock().unwrap();
        let agent = AgentRepo::get(&*store, agent_id)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .ok_or_else(|| anyhow::anyhow!("agent {agent_id} not found"))?;
        let pane_ref = PaneRef { session_name: session_name.clone(), tab_index, pane_id };
        let mut updated = agent;
        updated.pane_ref = Some(pane_ref.clone());
        let event = DomainEvent::AgentAttachedPane {
            id: agent_id,
            pane_ref,
            at: self.clock.now(),
        };
        store.save_agent_and_event(&updated, &event).map_err(|e| anyhow::anyhow!("{e}"))?;
        drop(store);
        self.event_bus.publish(event);
        info!(%agent_id, %session_name, pane_id, "pane bound to agent");
        Ok(())
    }

    pub fn get_agent(&self, id: AgentId) -> anyhow::Result<Option<AgentView>> {
        let store = self.store.lock().unwrap();
        let agent = AgentRepo::get(&*store, id).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(agent.as_ref().map(agent_to_view))
    }

    pub fn list_agents(&self, workspace_id: Option<WorkspaceId>) -> anyhow::Result<Vec<AgentView>> {
        let store = self.store.lock().unwrap();
        let agents = if let Some(ws_id) = workspace_id {
            AgentRepo::list_by_workspace(&*store, ws_id).map_err(|e| anyhow::anyhow!("{e}"))?
        } else {
            let workspaces = WorkspaceRepo::list(&*store).map_err(|e| anyhow::anyhow!("{e}"))?;
            let mut all = Vec::new();
            for ws in &workspaces {
                let ws_agents = AgentRepo::list_by_workspace(&*store, ws.id)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                all.extend(ws_agents);
            }
            all
        };
        Ok(agents.iter().map(agent_to_view).collect())
    }

    pub fn set_alias(&self, alias: &str, workspace_id: WorkspaceId) -> anyhow::Result<()> {
        let mut store = self.store.lock().unwrap();
        AliasRepo::set_alias(&mut *store, alias, workspace_id)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        info!(%alias, %workspace_id, "alias set");
        Ok(())
    }

    pub fn resolve_alias(&self, alias: &str) -> anyhow::Result<Option<WorkspaceId>> {
        let store = self.store.lock().unwrap();
        AliasRepo::resolve(&*store, alias).map_err(|e| anyhow::anyhow!("{e}"))
    }
}

fn ws_to_view(ws: &Workspace) -> WorkspaceView {
    WorkspaceView {
        id: ws.id,
        name: ws.name.clone(),
        canonical_key: ws.canonical_key.clone(),
        repo: ws.repo.clone(),
        archived: ws.archived,
        created_at: ws.created_at.as_millis(),
    }
}

fn agent_to_view(a: &Agent) -> AgentView {
    AgentView {
        id: a.id,
        workspace_id: a.workspace_id,
        role: a.role.clone(),
        runtime: a.runtime.clone(),
        state: a.state.into(),
        restart_count: a.restart_count,
        pane_session: a.pane_ref.as_ref().map(|p| p.session_name.clone()),
        pane_id: a.pane_ref.as_ref().map(|p| p.pane_id),
        created_at: a.created_at.as_millis(),
    }
}
