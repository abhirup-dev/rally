#![deny(unsafe_code)]

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use serde::Deserialize;
use theme::state_theme;
use widgets::{
    render_inbox_lines, render_status_lines, render_tree_lines, AgentInfo, InboxItemInfo,
    RenderCtx, TreeNode, WorkspaceInfo,
};
use zellij_tile::prelude::*;
use zellij_widgets::prelude::{Line, Modifier, Paragraph, PluginPane, Span, Style, Text};

use theme::palette;

mod theme;
mod tree_merge;
mod widgets;

/// Tracks which node in the tree is currently selected, by stable ID rather than by index so
/// that daemon pushes (which can reorder/resize the list) don't silently jump the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Selection {
    Workspace(String),
    Tab(usize),
    Pane(u32),
    Agent(String),
}

impl Selection {
    fn matches_node(&self, node: &TreeNode) -> bool {
        match (self, node) {
            (Selection::Workspace(id), TreeNode::Workspace { id: nid }) => id == nid,
            (Selection::Tab(pos), TreeNode::Tab { position, .. }) => pos == position,
            (Selection::Pane(id), TreeNode::Pane { id: nid, .. }) => id == nid,
            (Selection::Agent(id), TreeNode::Agent { id: nid, .. }) => id == nid,
            _ => false,
        }
    }
}

fn node_to_selection(node: &TreeNode) -> Selection {
    match node {
        TreeNode::Workspace { id } => Selection::Workspace(id.clone()),
        TreeNode::Tab { position, .. } => Selection::Tab(*position),
        TreeNode::Pane { id, .. } => Selection::Pane(*id),
        TreeNode::Agent { id, .. } => Selection::Agent(id.clone()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DensityMode {
    Normal,
    Compact,
}

/// Minimal tab info kept from Zellij's TabUpdate event.
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ZellijTab {
    position: usize,
    name: String,
    active: bool,
}

/// Minimal pane info kept from Zellij's PaneUpdate event.
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ZellijPane {
    id: u32,
    tab_position: usize,
    is_plugin: bool,
    is_floating: bool,
    is_selectable: bool,
    title: String,
}

struct RallyPlugin {
    workspaces: Vec<WorkspaceInfo>,
    agents: Vec<AgentInfo>,
    inbox_items: Vec<InboxItemInfo>,
    state_version: Option<u64>,
    show_inbox_detail: bool,
    show_help: bool,
    filter_mode: bool,
    filter: String,
    /// Stable-key selection — never a raw index.
    selection: Option<Selection>,
    /// Node IDs that the user has explicitly collapsed (workspace IDs, tab keys).
    collapsed: HashSet<String>,
    /// First visible tree-row index (updated each render() to keep selection in view).
    scroll_offset: usize,
    status_message: Option<String>,
    ui_version: u64,
    permission_denied: bool,
    last_error: Option<String>,
    rally_cli_path: String,
    density: DensityMode,
    show_bare_terminals: bool,
    default_collapsed: bool,
    /// Tabs in the current Zellij session (from TabUpdate events).
    tabs: Vec<ZellijTab>,
    /// Tiled, selectable, non-plugin panes in the current session (from PaneUpdate events).
    panes: Vec<ZellijPane>,
    /// CWD for each terminal pane (from CwdChanged events). Key = pane_id.
    pane_cwds: BTreeMap<u32, PathBuf>,
}

impl Default for RallyPlugin {
    fn default() -> Self {
        Self {
            workspaces: Vec::new(),
            agents: Vec::new(),
            inbox_items: Vec::new(),
            state_version: None,
            show_inbox_detail: false,
            show_help: false,
            filter_mode: false,
            filter: String::new(),
            selection: None,
            collapsed: HashSet::new(),
            scroll_offset: 0,
            status_message: None,
            ui_version: 0,
            permission_denied: false,
            last_error: None,
            rally_cli_path: "rally".to_string(),
            density: DensityMode::Normal,
            show_bare_terminals: true,
            default_collapsed: false,
            tabs: Vec::new(),
            panes: Vec::new(),
            pane_cwds: BTreeMap::new(),
        }
    }
}

#[cfg(not(test))]
register_plugin!(RallyPlugin);

impl ZellijPlugin for RallyPlugin {
    fn load(&mut self, config: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ReadApplicationState,
            // ChangeApplicationState is required for focus_terminal_pane,
            // set_pane_color (S4.3), rename_terminal_pane (S4.4), and
            // switch_session (workspace focus). Without it, those calls are
            // silently denied — see zellij_exports.rs permission table.
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
            EventType::Timer,
            EventType::Key,
            EventType::CwdChanged,
            EventType::TabUpdate,
            EventType::PaneUpdate,
        ]);

        if let Some(cli_path) = config.get("rally_cli_path") {
            self.rally_cli_path = cli_path.clone();
        }
        if config.get("sidebar_density").map(|s| s.as_str()) == Some("compact") {
            self.density = DensityMode::Compact;
        }
        if config.get("show_bare_terminals").map(|s| s.as_str()) == Some("false") {
            self.show_bare_terminals = false;
        }
        if config.get("default_collapsed").map(|s| s.as_str()) == Some("true") {
            self.default_collapsed = true;
        }
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.permission_denied = false;
                self.refresh_state();
                set_timeout(30.0);
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                self.permission_denied = true;
                true
            }
            Event::Timer(_) => {
                self.refresh_state();
                set_timeout(30.0);
                false
            }
            Event::Key(key) => {
                let changed = self.handle_key(key.bare_key);
                if changed {
                    self.ui_version = self.ui_version.saturating_add(1);
                }
                changed
            }
            Event::TabUpdate(tab_infos) => {
                self.tabs = tab_infos
                    .into_iter()
                    .map(|t| ZellijTab {
                        position: t.position,
                        name: t.name,
                        active: t.active,
                    })
                    .collect();
                self.tabs.sort_by_key(|t| t.position);
                true
            }
            Event::PaneUpdate(manifest) => {
                let total_reported: usize = manifest.panes.values().map(|v| v.len()).sum();
                self.panes = manifest
                    .panes
                    .into_iter()
                    .flat_map(|(tab_pos, pane_infos)| {
                        pane_infos.into_iter().filter_map(move |p| {
                            if p.is_plugin || p.is_floating || p.is_suppressed || !p.is_selectable
                            {
                                return None;
                            }
                            Some(ZellijPane {
                                id: p.id,
                                tab_position: tab_pos,
                                is_plugin: p.is_plugin,
                                is_floating: p.is_floating,
                                is_selectable: p.is_selectable,
                                title: p.title,
                            })
                        })
                    })
                    .collect();
                self.panes.sort_by_key(|p| (p.tab_position, p.id));
                eprintln!(
                    "[rally-plugin] PaneUpdate: kept {}/{} panes (filtered plugin/floating/suppressed)",
                    self.panes.len(),
                    total_reported
                );
                self.ensure_selection();
                true
            }
            Event::CwdChanged(PaneId::Terminal(pane_id), new_cwd, _clients) => {
                self.handle_cwd_changed(pane_id, new_cwd);
                false
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                let cmd_type = context.get("type").map(|s| s.as_str()).unwrap_or("");
                if exit_code.is_some_and(|c| c != 0) {
                    self.last_error = Some(String::from_utf8_lossy(&stderr).trim().to_string());
                    return true;
                }
                self.last_error = None;
                match cmd_type {
                    "state_snapshot" => self.apply_snapshot_bytes(&stdout),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        match pipe_message.payload {
            Some(payload) => self.apply_snapshot_bytes(payload.as_bytes()),
            None => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let rows = rows as u16;
        let cols = cols as u16;

        self.update_scroll(rows as usize, cols as usize);

        let lines = self.build_lines(rows as usize, cols as usize);
        let text = Text::from(lines);
        let area = zellij_widgets::prelude::Geometry::new(rows, cols);

        let mut pane = PluginPane::new(std::io::stdout(), rows, cols);
        let _ = pane.draw(|frame| {
            frame.render_widget(Paragraph::new(text), area);
        });
    }
}

impl RallyPlugin {
    // ── State management ──────────────────────────────────────────────────

    fn apply_snapshot_bytes(&mut self, bytes: &[u8]) -> bool {
        match serde_json::from_slice::<StateSnapshotResponse>(bytes) {
            Ok(snapshot) => self.apply_snapshot(snapshot),
            Err(err) => {
                self.last_error = Some(format!("state snapshot parse failed: {err}"));
                true
            }
        }
    }

    fn apply_snapshot(&mut self, snapshot: StateSnapshotResponse) -> bool {
        if self.state_version.is_some_and(|v| snapshot.version <= v) {
            eprintln!(
                "[rally-plugin] dropping stale snapshot v{} (current: v{})",
                snapshot.version,
                self.state_version.unwrap_or(0)
            );
            return false;
        }

        self.state_version = Some(snapshot.version);
        self.workspaces = snapshot.workspaces;
        self.agents = snapshot.agents;
        self.inbox_items = snapshot.inbox_items;
        self.ensure_selection();
        self.last_error = None;

        // S4.3 + S4.4: update every bound pane's color tint and name to reflect current state.
        for agent in &self.agents {
            if let Some(pane_id) = agent.pane_id {
                let theme = state_theme(&agent.state);
                zellij_set_pane_color(pane_id, theme.pane_bg);
                zellij_rename_pane(pane_id, &format!("{} {}", theme.glyph, agent.role));
            }
        }

        true
    }

    fn refresh_state(&self) {
        let mut ctx = BTreeMap::new();
        ctx.insert("type".to_string(), "state_snapshot".to_string());
        run_command(&[&self.rally_cli_path, "--json", "_plugin-state"], ctx);
    }

    fn handle_cwd_changed(&mut self, pane_id: u32, new_cwd: std::path::PathBuf) {
        // Track CWD for all terminal panes (used by action menu for bare terminals).
        self.pane_cwds.insert(pane_id, new_cwd.clone());

        let agent = self.agents.iter().find(|a| a.pane_id == Some(pane_id));
        if let Some(agent) = agent {
            let mut ctx = BTreeMap::new();
            ctx.insert("type".to_string(), "update_cwd".to_string());
            run_command(
                &[
                    &self.rally_cli_path,
                    "--json",
                    "agent",
                    "update-cwd",
                    "--agent",
                    &agent.id,
                    "--cwd",
                    &new_cwd.to_string_lossy(),
                ],
                ctx,
            );
        }
    }

    // ── Tree model ────────────────────────────────────────────────────────

    fn visible_tree_nodes(&self) -> Vec<TreeNode> {
        tree_merge::merge_tree(
            &self.tabs,
            &self.panes,
            &self.workspaces,
            &self.agents,
            &self.collapsed,
            &self.filter,
            self.show_bare_terminals,
        )
    }

    /// Index of the selected node in the current visible list (O(n) but n is tiny).
    fn selected_index(&self) -> Option<usize> {
        let sel = self.selection.as_ref()?;
        self.visible_tree_nodes()
            .iter()
            .position(|node| sel.matches_node(node))
    }

    /// Clamp/reset selection to a valid node after the list changes (snapshot or filter).
    fn ensure_selection(&mut self) {
        let nodes = self.visible_tree_nodes();
        if nodes.is_empty() {
            self.selection = None;
            return;
        }
        let is_valid = self
            .selection
            .as_ref()
            .is_some_and(|sel| nodes.iter().any(|n| sel.matches_node(n)));
        if !is_valid {
            self.selection = Some(node_to_selection(&nodes[0]));
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let nodes = self.visible_tree_nodes();
        if nodes.is_empty() {
            self.selection = None;
            return;
        }
        let current = self
            .selection
            .as_ref()
            .and_then(|sel| nodes.iter().position(|n| sel.matches_node(n)))
            .unwrap_or(0);
        let next = (current as isize + delta).rem_euclid(nodes.len() as isize) as usize;
        self.selection = Some(node_to_selection(&nodes[next]));
    }

    /// h — collapse parent node, or move to parent if already a leaf.
    fn handle_collapse(&mut self) {
        match self.selection.clone() {
            Some(Selection::Tab(pos)) => {
                self.collapsed.insert(format!("tab:{pos}"));
            }
            Some(Selection::Workspace(ws_id)) => {
                self.collapsed.insert(ws_id);
            }
            Some(Selection::Pane(pane_id)) => {
                // Move to parent tab and collapse it.
                if let Some(pane) = self.panes.iter().find(|p| p.id == pane_id) {
                    let tab_pos = pane.tab_position;
                    self.selection = Some(Selection::Tab(tab_pos));
                    self.collapsed.insert(format!("tab:{tab_pos}"));
                }
            }
            Some(Selection::Agent(agent_id)) => {
                // Move to parent: tab (if in tab view) or workspace (if in daemon view).
                let agent = self.agents.iter().find(|a| a.id == agent_id);
                if let Some(agent) = agent {
                    if !self.tabs.is_empty() {
                        // Tab view: find parent tab via agent pane_id.
                        if let Some(pane) = agent
                            .pane_id
                            .and_then(|pid| self.panes.iter().find(|p| p.id == pid))
                        {
                            let tab_pos = pane.tab_position;
                            self.selection = Some(Selection::Tab(tab_pos));
                            self.collapsed.insert(format!("tab:{tab_pos}"));
                        }
                    } else {
                        let ws_id = agent.workspace_id.clone();
                        self.selection = Some(Selection::Workspace(ws_id.clone()));
                        self.collapsed.insert(ws_id);
                    }
                }
            }
            None => {}
        }
    }

    /// l — expand node if collapsed, else descend to first child.
    fn handle_expand(&mut self) {
        match self.selection.clone() {
            Some(Selection::Tab(pos)) => {
                let key = format!("tab:{pos}");
                if self.collapsed.remove(&key) {
                    // Was collapsed; expanding is the action.
                } else {
                    // Already expanded: descend into first visible child.
                    let nodes = self.visible_tree_nodes();
                    let first_child = nodes.iter().find(|n| match n {
                        TreeNode::Pane { tab_position, .. } if *tab_position == pos => true,
                        TreeNode::Agent { id, .. } => self
                            .agents
                            .iter()
                            .find(|a| a.id == *id)
                            .and_then(|a| a.pane_id)
                            .and_then(|pid| self.panes.iter().find(|p| p.id == pid))
                            .is_some_and(|p| p.tab_position == pos),
                        _ => false,
                    });
                    if let Some(child) = first_child {
                        self.selection = Some(node_to_selection(child));
                    }
                }
            }
            Some(Selection::Workspace(ws_id)) => {
                if self.collapsed.remove(&ws_id) {
                    // Was collapsed; expanding is the action.
                } else {
                    let filter = &self.filter;
                    let first_child = self
                        .agents
                        .iter()
                        .find(|a| {
                            a.workspace_id == ws_id
                                && tree_merge::agent_matches_filter(a, filter)
                        })
                        .map(|a| a.id.clone());
                    if let Some(agent_id) = first_child {
                        self.selection = Some(Selection::Agent(agent_id));
                    }
                }
            }
            Some(Selection::Agent(_) | Selection::Pane(_)) => {
                // Leaf nodes — nothing to expand.
            }
            None => {}
        }
    }

    // ── Scroll ────────────────────────────────────────────────────────────

    /// Adjust scroll_offset so the selected node stays in the visible tree window.
    fn update_scroll(&mut self, total_rows: usize, cols: usize) {
        let ctx = RenderCtx {
            cols,
            agents: &self.agents,
            inbox_items: &self.inbox_items,
            filter: (!self.filter.is_empty()).then_some(self.filter.as_str()),
            status_message: self.status_message.as_deref(),
        };
        let inbox_count = render_inbox_lines(&ctx, self.show_inbox_detail).len();
        let status_count = render_status_lines(&ctx).len();
        let chrome = 3 + inbox_count + status_count;
        let tree_rows = total_rows.saturating_sub(chrome).max(1);

        if let Some(idx) = self.selected_index() {
            if idx < self.scroll_offset {
                self.scroll_offset = idx;
            } else if idx >= self.scroll_offset + tree_rows {
                self.scroll_offset = idx.saturating_sub(tree_rows - 1);
            }
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────

    fn build_lines(&self, rows: usize, cols: usize) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Rally",
            Style::default()
                .fg(palette::ROSE)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        if let Some(ref err) = self.last_error {
            let msg = truncate(err, cols.saturating_sub(4)).to_string();
            lines.push(Line::from(vec![
                Span::styled(
                    " ! ",
                    Style::default()
                        .fg(palette::LOVE)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(msg, Style::default().fg(palette::LOVE)),
            ]));
            return lines;
        }

        if self.workspaces.is_empty() && self.state_version.is_none() {
            if self.permission_denied {
                lines.push(Line::from(Span::styled(
                    "  Permission denied.",
                    Style::default().fg(palette::LOVE),
                )));
                lines.push(Line::from(Span::styled(
                    "  Grant RunCommands to continue.",
                    Style::default().fg(palette::SUBTLE),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "  Loading state...",
                    Style::default().fg(palette::SUBTLE),
                )));
            }
            return lines;
        }

        if self.show_help {
            lines.extend(help_lines(cols));
            return lines;
        }

        // Build status + inbox lines first so we know how many rows the tree gets.
        let ctx = RenderCtx {
            cols,
            agents: &self.agents,
            inbox_items: &self.inbox_items,
            filter: (!self.filter.is_empty()).then_some(self.filter.as_str()),
            status_message: self.status_message.as_deref(),
        };
        let inbox_lines = render_inbox_lines(&ctx, self.show_inbox_detail);
        let status_lines = render_status_lines(&ctx);

        let chrome = 3 + inbox_lines.len() + status_lines.len();
        let tree_rows = rows.saturating_sub(chrome).max(1);

        // Tree.
        let visible_nodes = self.visible_tree_nodes();
        let selected_node = self
            .selection
            .as_ref()
            .and_then(|sel| visible_nodes.iter().find(|n| sel.matches_node(n)));

        let all_tree_lines = render_tree_lines(
            &self.workspaces,
            &self.agents,
            &self.collapsed,
            &visible_nodes,
            selected_node,
            &self.pane_cwds,
            self.density,
            cols,
        );

        lines.extend(
            all_tree_lines
                .into_iter()
                .skip(self.scroll_offset)
                .take(tree_rows),
        );
        lines.extend(inbox_lines);
        lines.extend(status_lines);

        lines
    }

    #[cfg(test)]
    fn render_to_string(&self, rows: usize, cols: usize) -> String {
        let cols = cols.min(40);
        let rows_u16 = rows as u16;
        let cols_u16 = cols as u16;
        let lines = self.build_lines(rows, cols);
        let text = Text::from(lines);
        let area = zellij_widgets::prelude::Geometry::new(rows_u16, cols_u16);

        let mut buf = Vec::new();
        let mut pane = PluginPane::new(&mut buf, rows_u16, cols_u16);
        let _ = pane.draw(|frame| {
            frame.render_widget(Paragraph::new(text), area);
        });
        String::from_utf8_lossy(&buf).into_owned()
    }

    // ── Key handling ──────────────────────────────────────────────────────

    fn handle_key(&mut self, key: BareKey) -> bool {
        if self.filter_mode {
            return self.handle_filter_key(key);
        }

        match key {
            BareKey::Char('j') | BareKey::Down => {
                self.move_selection(1);
                true
            }
            BareKey::Char('k') | BareKey::Up => {
                self.move_selection(-1);
                true
            }
            BareKey::Char('h') | BareKey::Left => {
                self.handle_collapse();
                true
            }
            BareKey::Char('l') | BareKey::Right => {
                self.handle_expand();
                true
            }
            BareKey::Char('d') => {
                self.density = match self.density {
                    DensityMode::Normal => DensityMode::Compact,
                    DensityMode::Compact => DensityMode::Normal,
                };
                true
            }
            BareKey::Char('i') => {
                self.show_inbox_detail = !self.show_inbox_detail;
                true
            }
            BareKey::Char('?') => {
                self.show_help = !self.show_help;
                true
            }
            BareKey::Char('/') => {
                self.filter_mode = true;
                self.status_message = Some("filter mode: type query, Enter to apply".into());
                true
            }
            BareKey::Char('f') => self.handle_focus(),
            BareKey::Char('a') => self.handle_action_menu(),
            BareKey::Char('K') => self.action_feedback("ack"),
            BareKey::Char('r') => self.action_feedback("restart"),
            BareKey::Char('s') => self.action_feedback("spawn"),
            BareKey::Esc => {
                let changed = self.show_inbox_detail
                    || self.show_help
                    || self.filter_mode
                    || self.status_message.is_some();
                self.show_inbox_detail = false;
                self.show_help = false;
                self.filter_mode = false;
                self.status_message = None;
                changed
            }
            _ => false,
        }
    }

    fn handle_filter_key(&mut self, key: BareKey) -> bool {
        match key {
            BareKey::Char(c) => {
                self.filter.push(c);
                self.ensure_selection();
                true
            }
            BareKey::Backspace => {
                self.filter.pop();
                self.ensure_selection();
                true
            }
            BareKey::Enter => {
                self.filter_mode = false;
                self.status_message = None;
                true
            }
            BareKey::Esc => {
                self.filter_mode = false;
                self.filter.clear();
                self.status_message = None;
                self.ensure_selection();
                true
            }
            _ => false,
        }
    }

    fn handle_focus(&mut self) -> bool {
        match self.selection.clone() {
            Some(Selection::Pane(pane_id)) => {
                zellij_focus_pane(pane_id);
                self.status_message = None;
                true
            }
            Some(Selection::Agent(id)) => {
                if let Some(agent) = self.agents.iter().find(|a| a.id == id) {
                    if let Some(pane_id) = agent.pane_id {
                        zellij_focus_pane(pane_id);
                        self.status_message = None;
                    } else {
                        self.status_message = Some("agent has no pane bound yet".into());
                    }
                }
                true
            }
            Some(Selection::Tab(_)) => {
                // Tab nodes can't be focused.
                false
            }
            Some(Selection::Workspace(id)) => {
                if let Some(ws) = self.workspaces.iter().find(|w| w.id == id) {
                    zellij_switch_session(&format!("rally-{}", ws.name));
                    self.status_message = None;
                }
                true
            }
            None => false,
        }
    }

    fn handle_action_menu(&mut self) -> bool {
        let (pane_id, cwd) = match &self.selection {
            Some(Selection::Pane(id)) => (*id, self.pane_cwds.get(id).cloned()),
            Some(Selection::Agent(agent_id)) => {
                if let Some(agent) = self.agents.iter().find(|a| a.id == *agent_id) {
                    if let Some(pid) = agent.pane_id {
                        (pid, self.pane_cwds.get(&pid).cloned())
                    } else {
                        self.status_message = Some("agent has no pane bound yet".into());
                        return true;
                    }
                } else {
                    return false;
                }
            }
            _ => {
                self.status_message = Some("select a pane to open action menu".into());
                return true;
            }
        };

        let mut args = vec![
            &self.rally_cli_path as &str,
            "pane",
            "menu",
            "--pane-id",
        ];
        let pane_id_str = pane_id.to_string();
        args.push(&pane_id_str);

        let cwd_string;
        if let Some(ref cwd_path) = cwd {
            cwd_string = cwd_path.to_string_lossy().into_owned();
            args.push("--cwd");
            args.push(&cwd_string);
        }

        zellij_open_floating_command(&args);
        self.status_message = None;
        true
    }

    fn action_feedback(&mut self, action: &str) -> bool {
        let target = match &self.selection {
            Some(Selection::Agent(id)) => id.clone(),
            Some(Selection::Pane(id)) => format!("pane:{id}"),
            Some(Selection::Tab(pos)) => format!("tab:{pos}"),
            Some(Selection::Workspace(id)) => format!("workspace:{id}"),
            None => "nothing selected".to_string(),
        };
        self.status_message = Some(format!("{action}: {target}"));
        true
    }
}

/// Raw entity snapshot from the daemon. Contains only domain entities — NO sidebar
/// projection or layout data. The plugin merges this with Zellij TabUpdate/PaneUpdate
/// events client-side (see `tree_merge` module). The daemon cannot compute tab-based
/// grouping because it never sees Zellij events.
#[derive(Deserialize)]
struct StateSnapshotResponse {
    version: u64,
    #[serde(default)]
    workspaces: Vec<WorkspaceInfo>,
    #[serde(default)]
    agents: Vec<AgentInfo>,
    #[serde(default)]
    inbox_items: Vec<InboxItemInfo>,
}

/// Wrap `focus_terminal_pane` so tests (native target) compile without the wasm import.
fn zellij_focus_pane(pane_id: u32) {
    #[cfg(not(test))]
    focus_terminal_pane(pane_id, false, false);
    let _ = pane_id;
}

/// Wrap `switch_session` so tests (native target) compile without the wasm import.
fn zellij_switch_session(session: &str) {
    #[cfg(not(test))]
    switch_session(Some(session));
    let _ = session;
}

/// Apply a subtle bg tint to a terminal pane to reflect agent state (S4.3).
/// `pane_bg` is a hex string (e.g. `"#001800"`) or `None` to clear the tint.
fn zellij_set_pane_color(pane_id: u32, pane_bg: Option<&str>) {
    #[cfg(not(test))]
    set_pane_color(
        PaneId::Terminal(pane_id),
        None,
        pane_bg.map(str::to_owned),
    );
    let _ = (pane_id, pane_bg);
}

/// Spawn a floating command pane (S5.2: action menu).
fn zellij_open_floating_command(args: &[&str]) {
    #[cfg(not(test))]
    {
        let cmd = CommandToRun {
            path: args.first().copied().unwrap_or("rally").into(),
            args: args.iter().skip(1).map(|s| s.to_string()).collect(),
            cwd: None,
        };
        open_command_pane_floating(cmd, None, BTreeMap::new());
    }
    let _ = args;
}

/// Rename a terminal pane with a state-emoji prefix (S4.4).
fn zellij_rename_pane(pane_id: u32, name: &str) {
    #[cfg(not(test))]
    rename_terminal_pane(pane_id, name);
    let _ = (pane_id, name);
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

fn help_lines(width: usize) -> Vec<Line<'static>> {
    use theme::palette;

    let heading = Style::default()
        .fg(palette::IRIS)
        .add_modifier(Modifier::BOLD);
    let key = Style::default().fg(palette::FOAM);
    let desc = Style::default().fg(palette::TEXT);
    let dim = Style::default().fg(palette::MUTED);

    let entries: &[(&str, &str)] = &[
        ("j k", "move selection"),
        ("h l", "collapse / expand"),
        ("d", "toggle density"),
        ("f", "focus pane"),
        ("a", "action menu"),
        ("K", "acknowledge"),
        ("r", "restart agent"),
        ("s", "spawn wizard"),
        ("/", "filter agents"),
        ("i", "inbox detail"),
        ("Esc", "close overlay"),
    ];

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(" Keybindings", heading)),
        Line::from(""),
    ];

    for (k, d) in entries {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<14}", k), key),
            Span::styled(*d, desc),
        ]));
    }

    lines.push(Line::from(""));

    let hint = "Esc to close ";
    let fill = width.saturating_sub(hint.len());
    lines.push(Line::from(vec![
        Span::raw(" ".repeat(fill)),
        Span::styled(hint, dim),
    ]));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_snapshot(plugin: &mut RallyPlugin, workspaces: &str, agents: &str) {
        let payload = format!(
            r#"{{"kind":"state_snapshot","version":1,"workspaces":[{workspaces}],"agents":[{agents}],"inbox_items":[]}}"#
        );
        plugin.apply_snapshot_bytes(payload.as_bytes());
    }

    #[test]
    fn applies_newer_state_snapshot() {
        let mut plugin = RallyPlugin::default();
        let changed = plugin.apply_snapshot_bytes(
            br#"{
                "kind":"state_snapshot",
                "version":1,
                "workspaces":[{"id":"w1","name":"api","canonical_key":"api"}],
                "agents":[{"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running"}],
                "inbox_items":[]
            }"#,
        );

        assert!(changed);
        assert_eq!(plugin.state_version, Some(1));
        assert_eq!(plugin.workspaces.len(), 1);
        assert_eq!(plugin.agents.len(), 1);
    }

    #[test]
    fn accepts_first_snapshot_at_version_zero() {
        let mut plugin = RallyPlugin::default();
        let changed = plugin.apply_snapshot_bytes(
            br#"{
                "kind":"state_snapshot",
                "version":0,
                "workspaces":[{"id":"w1","name":"api","canonical_key":"api"}],
                "agents":[],
                "inbox_items":[]
            }"#,
        );

        assert!(changed);
        assert_eq!(plugin.state_version, Some(0));
        assert_eq!(plugin.workspaces.len(), 1);
    }

    #[test]
    fn ignores_stale_state_snapshot() {
        let mut plugin = RallyPlugin {
            state_version: Some(2),
            ..Default::default()
        };
        let changed = plugin.apply_snapshot_bytes(
            br#"{
                "kind":"state_snapshot",
                "version":2,
                "workspaces":[{"id":"w1","name":"api","canonical_key":"api"}],
                "agents":[],
                "inbox_items":[]
            }"#,
        );

        assert!(!changed);
        assert!(plugin.workspaces.is_empty());
    }

    #[test]
    fn initial_selection_is_first_workspace() {
        let mut plugin = RallyPlugin::default();
        load_snapshot(
            &mut plugin,
            r#"{"id":"w1","name":"api","canonical_key":"api"}"#,
            r#"{"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running"}"#,
        );
        // After snapshot the tree is [Workspace(w1), Agent(a1)].
        // ensure_selection picks the first node = workspace.
        assert_eq!(
            plugin.selection,
            Some(Selection::Workspace("w1".to_string()))
        );
    }

    #[test]
    fn jk_navigation_moves_through_tree() {
        let mut plugin = RallyPlugin::default();
        load_snapshot(
            &mut plugin,
            r#"{"id":"w1","name":"api","canonical_key":"api"}"#,
            r#"{"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running"},
               {"id":"a2","workspace_id":"w1","role":"review","runtime":"cc","state":"idle"}"#,
        );

        // Visible: [Workspace(w1), Agent(a1), Agent(a2)]
        assert_eq!(
            plugin.selection,
            Some(Selection::Workspace("w1".to_string()))
        );

        plugin.handle_key(BareKey::Char('j'));
        assert_eq!(plugin.selection, Some(Selection::Agent("a1".to_string())));

        plugin.handle_key(BareKey::Char('j'));
        assert_eq!(plugin.selection, Some(Selection::Agent("a2".to_string())));

        // Wraps around.
        plugin.handle_key(BareKey::Char('j'));
        assert_eq!(
            plugin.selection,
            Some(Selection::Workspace("w1".to_string()))
        );

        plugin.handle_key(BareKey::Char('k'));
        assert_eq!(plugin.selection, Some(Selection::Agent("a2".to_string())));
    }

    #[test]
    fn hl_collapse_expand_workspace() {
        let mut plugin = RallyPlugin::default();
        load_snapshot(
            &mut plugin,
            r#"{"id":"w1","name":"api","canonical_key":"api"}"#,
            r#"{"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running"}"#,
        );

        // Start at workspace node.
        assert_eq!(
            plugin.selection,
            Some(Selection::Workspace("w1".to_string()))
        );
        assert!(!plugin.collapsed.contains("w1"));

        // h collapses.
        plugin.handle_key(BareKey::Char('h'));
        assert!(plugin.collapsed.contains("w1"));

        // l expands.
        plugin.handle_key(BareKey::Char('l'));
        assert!(!plugin.collapsed.contains("w1"));

        // l again descends to first child.
        plugin.handle_key(BareKey::Char('l'));
        assert_eq!(plugin.selection, Some(Selection::Agent("a1".to_string())));

        // h from agent moves to parent workspace and collapses.
        plugin.handle_key(BareKey::Char('h'));
        assert_eq!(
            plugin.selection,
            Some(Selection::Workspace("w1".to_string()))
        );
        assert!(plugin.collapsed.contains("w1"));
    }

    #[test]
    fn filter_auto_expands_collapsed_workspace() {
        let mut plugin = RallyPlugin::default();
        load_snapshot(
            &mut plugin,
            r#"{"id":"w1","name":"api","canonical_key":"api"}"#,
            r#"{"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running"}"#,
        );
        plugin.collapsed.insert("w1".to_string());
        plugin.filter = "impl".to_string();

        let nodes = plugin.visible_tree_nodes();
        // Even though w1 is collapsed, the filter matches an agent inside it → auto-expand.
        assert_eq!(nodes.len(), 2);
        assert!(matches!(&nodes[1], TreeNode::Agent { id, .. } if id == "a1"));
    }

    #[test]
    fn keyboard_filter_mode() {
        let mut plugin = RallyPlugin::default();
        load_snapshot(
            &mut plugin,
            r#"{"id":"w1","name":"api","canonical_key":"api"}"#,
            r#"{"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running"},
               {"id":"a2","workspace_id":"w1","role":"review","runtime":"cc","state":"idle"}"#,
        );

        // Enter filter mode and type "impl".
        assert!(plugin.handle_key(BareKey::Char('/')));
        for c in "impl".chars() {
            plugin.handle_key(BareKey::Char(c));
        }
        plugin.handle_key(BareKey::Enter);

        assert_eq!(plugin.filter, "impl");
        // Selection should land on the workspace (first visible node) or the matching agent.
        let nodes = plugin.visible_tree_nodes();
        assert!(nodes
            .iter()
            .any(|n| matches!(n, TreeNode::Agent { id, .. } if id == "a1")));
        assert!(!nodes
            .iter()
            .any(|n| matches!(n, TreeNode::Agent { id, .. } if id == "a2")));
    }

    #[test]
    fn focus_agent_without_pane_shows_status() {
        let mut plugin = RallyPlugin::default();
        load_snapshot(
            &mut plugin,
            r#"{"id":"w1","name":"api","canonical_key":"api"}"#,
            r#"{"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running"}"#,
        );
        // Select the agent (which has no pane_id in the test payload).
        plugin.selection = Some(Selection::Agent("a1".to_string()));
        assert!(plugin.handle_key(BareKey::Char('f')));
        assert_eq!(
            plugin.status_message.as_deref(),
            Some("agent has no pane bound yet")
        );
    }

    #[test]
    fn focus_agent_with_pane_clears_status() {
        let mut plugin = RallyPlugin::default();
        // Agent with explicit pane_id.
        let payload = r#"{
            "kind":"state_snapshot","version":1,
            "workspaces":[{"id":"w1","name":"api","canonical_key":"api"}],
            "agents":[{"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running","pane_id":7}],
            "inbox_items":[]
        }"#;
        plugin.apply_snapshot_bytes(payload.as_bytes());
        plugin.selection = Some(Selection::Agent("a1".to_string()));
        plugin.status_message = Some("old message".into());
        assert!(plugin.handle_key(BareKey::Char('f')));
        // zellij_focus_pane is a no-op in tests; status_message should be cleared.
        assert_eq!(plugin.status_message, None);
    }

    #[test]
    fn focus_workspace_clears_status() {
        let mut plugin = RallyPlugin::default();
        load_snapshot(
            &mut plugin,
            r#"{"id":"w1","name":"api","canonical_key":"api"}"#,
            r#""#,
        );
        plugin.selection = Some(Selection::Workspace("w1".to_string()));
        plugin.status_message = Some("old message".into());
        assert!(plugin.handle_key(BareKey::Char('f')));
        // zellij_switch_session is a no-op in tests; status_message should be cleared.
        assert_eq!(plugin.status_message, None);
    }

    #[test]
    fn render_body_contains_tree_output() {
        let mut plugin = RallyPlugin::default();
        plugin.apply_snapshot_bytes(
            br#"{
                "kind":"state_snapshot",
                "version":1,
                "workspaces":[{"id":"w1","name":"api","canonical_key":"api"}],
                "agents":[{"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running"}],
                "inbox_items":[]
            }"#,
        );

        let body = plugin.render_to_string(50, 40);

        assert!(body.contains("api"), "workspace name visible");
        assert!(body.contains("impl"), "agent role visible");
        assert!(body.contains("1 agents"), "status bar visible");
    }

    #[test]
    fn golden_ansi_snapshot_matrix() {
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);

        settings.bind(|| {
            for agent_count in [0usize, 1, 5, 50] {
                for inbox_count in [0usize, 3, 20] {
                    let plugin = snapshot_plugin(agent_count, inbox_count);
                    let body = plugin.render_to_string(80, 72);
                    assert_file_snapshot(
                        &format!("sidebar_{agent_count}_agents_{inbox_count}_inbox"),
                        &body,
                        line!(),
                    );
                }
            }
        });
    }

    fn assert_file_snapshot(name: &str, body: &str, line: u32) {
        insta::_macro_support::assert_snapshot(
            insta::_macro_support::SnapshotValue::FileText {
                name: Some(name.into()),
                content: body,
            },
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")),
            "golden_ansi_snapshot_matrix",
            module_path!(),
            file!(),
            line,
            "body",
        )
        .unwrap();
    }

    fn snapshot_plugin(agent_count: usize, inbox_count: usize) -> RallyPlugin {
        let workspaces = r#"
            {"id":"w1","name":"api-service","canonical_key":"api-service"},
            {"id":"w2","name":"web-client","canonical_key":"web-client"}
        "#;
        let agents = (0..agent_count)
            .map(|idx| {
                let workspace_id = if idx % 2 == 0 { "w1" } else { "w2" };
                let state = match idx % 5 {
                    0 => "running",
                    1 => "idle",
                    2 => "waiting_for_input",
                    3 => "attention_required",
                    _ => "completed",
                };
                format!(
                    r#"{{"id":"a{idx}","workspace_id":"{workspace_id}","role":"agent-{idx}","runtime":"cc","state":"{state}","pane_id":{}}}"#,
                    idx + 10
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let inbox_items = (0..inbox_count)
            .map(|idx| {
                let agent_id = if agent_count == 0 {
                    "null".to_string()
                } else {
                    format!(r#""a{}""#, idx % agent_count)
                };
                let urgency = if idx % 2 == 0 { "high" } else { "medium" };
                format!(
                    r#"{{"id":"i{idx}","agent_id":{agent_id},"urgency":"{urgency}","acked":false,"raised_at":{idx},"message":"message {idx}"}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let payload = format!(
            r#"{{
                "kind":"state_snapshot",
                "version":1,
                "workspaces":[{workspaces}],
                "agents":[{agents}],
                "inbox_items":[{inbox_items}]
            }}"#
        );
        let mut plugin = RallyPlugin::default();
        plugin.apply_snapshot_bytes(payload.as_bytes());
        plugin
    }
}
