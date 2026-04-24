#![deny(unsafe_code)]

use std::collections::BTreeMap;

use serde::Deserialize;
use widgets::{
    render_inbox_lines, render_status_lines, render_workspace_lines, AgentInfo, InboxItemInfo,
    RenderCtx, WorkspaceInfo,
};
use zellij_tile::prelude::*;
use zellij_widgets::prelude::{Line, Modifier, Paragraph, PluginPane, Span, Style, Text};

mod widgets;

struct RallyPlugin {
    workspaces: Vec<WorkspaceInfo>,
    agents: Vec<AgentInfo>,
    inbox_items: Vec<InboxItemInfo>,
    state_version: Option<u64>,
    show_inbox_detail: bool,
    show_help: bool,
    filter_mode: bool,
    filter: String,
    selected_agent_id: Option<String>,
    status_message: Option<String>,
    ui_version: u64,
    permission_denied: bool,
    last_error: Option<String>,
    rally_cli_path: String,
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
            selected_agent_id: None,
            status_message: None,
            ui_version: 0,
            permission_denied: false,
            last_error: None,
            rally_cli_path: "rally".to_string(),
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
        ]);
        subscribe(&[
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
            EventType::Timer,
            EventType::Key,
        ]);

        // Layout config can override the CLI path (e.g. for dev builds).
        if let Some(cli_path) = config.get("rally_cli_path") {
            self.rally_cli_path = cli_path.clone();
        }
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.permission_denied = false;
                self.refresh_state();
                set_timeout(5.0);
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                self.permission_denied = true;
                true
            }
            Event::Timer(_) => {
                self.refresh_state();
                set_timeout(5.0);
                false
            }
            Event::Key(key) => {
                let changed = self.handle_key(key.bare_key);
                if changed {
                    self.ui_version = self.ui_version.saturating_add(1);
                }
                changed
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
        let cols = cols.min(40) as u16;
        let lines = self.build_lines(cols as usize);
        let text = Text::from(lines);
        let area = zellij_widgets::prelude::Geometry::new(rows, cols);

        let mut pane = PluginPane::new(std::io::stdout(), rows, cols);
        let _ = pane.draw(|frame| {
            frame.render_widget(Paragraph::new(text), area);
        });
    }
}

impl RallyPlugin {
    fn build_lines(&self, cols: usize) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        // Header
        lines.push(Line::from(Span::styled(
            " Rally ",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("─".repeat(cols)));

        // Error state
        if let Some(ref err) = self.last_error {
            lines.push(Line::from(Span::styled(
                format!("⚠ {}", truncate(err, cols.saturating_sub(2))),
                Style::default().fg(zellij_widgets::prelude::Color::Red),
            )));
            return lines;
        }

        // Loading / permission denied state
        if self.workspaces.is_empty() && self.state_version.is_none() {
            if self.permission_denied {
                lines.push(Line::from(Span::styled(
                    "Permission denied.",
                    Style::default().fg(zellij_widgets::prelude::Color::Red),
                )));
                lines.push(Line::from(Span::styled(
                    "Grant RunCommands to continue.",
                    Style::default().add_modifier(Modifier::DIM),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "Loading state…",
                    Style::default().add_modifier(Modifier::DIM),
                )));
            }
            return lines;
        }

        // Help screen
        if self.show_help {
            lines.extend(help_lines(cols));
            return lines;
        }

        // Normal render
        let ctx = RenderCtx {
            cols,
            workspaces: &self.workspaces,
            agents: &self.agents,
            inbox_items: &self.inbox_items,
            selected_agent_id: self.selected_agent_id.as_deref(),
            filter: (!self.filter.is_empty()).then_some(self.filter.as_str()),
            status_message: self.status_message.as_deref(),
        };
        lines.extend(render_workspace_lines(&ctx));
        lines.extend(render_inbox_lines(&ctx, self.show_inbox_detail));
        lines.extend(render_status_lines(&ctx));

        lines
    }

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
            return false;
        }

        self.state_version = Some(snapshot.version);
        self.workspaces = snapshot.workspaces;
        self.agents = snapshot.agents;
        self.inbox_items = snapshot.inbox_items;
        self.ensure_selection();
        self.last_error = None;
        true
    }

    fn refresh_state(&self) {
        let mut ctx = BTreeMap::new();
        ctx.insert("type".to_string(), "state_snapshot".to_string());
        run_command(&[&self.rally_cli_path, "--json", "_plugin-state"], ctx);
    }

    fn render_to_string(&self, rows: usize, cols: usize) -> String {
        let cols = cols.min(40);
        let rows = rows as u16;
        let cols_u16 = cols as u16;
        let lines = self.build_lines(cols);
        let text = Text::from(lines);
        let area = zellij_widgets::prelude::Geometry::new(rows, cols_u16);

        let mut buf = Vec::new();
        let mut pane = PluginPane::new(&mut buf, rows, cols_u16);
        let _ = pane.draw(|frame| {
            frame.render_widget(Paragraph::new(text), area);
        });
        String::from_utf8_lossy(&buf).into_owned()
    }

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
            BareKey::Char('f') => self.action_feedback("focus"),
            BareKey::Char('a') => self.action_feedback("ack"),
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

    fn action_feedback(&mut self, action: &str) -> bool {
        let target = self
            .selected_agent_id
            .as_deref()
            .unwrap_or("no selected agent");
        self.status_message = Some(format!("{action}: {target}"));
        true
    }

    fn ensure_selection(&mut self) {
        let visible = self.visible_agent_ids();
        if visible.is_empty() {
            self.selected_agent_id = None;
            return;
        }
        if self
            .selected_agent_id
            .as_ref()
            .is_some_and(|selected| visible.iter().any(|id| id == selected))
        {
            return;
        }
        self.selected_agent_id = visible.first().cloned();
    }

    fn move_selection(&mut self, delta: isize) {
        let visible = self.visible_agent_ids();
        if visible.is_empty() {
            self.selected_agent_id = None;
            return;
        }

        let current = self
            .selected_agent_id
            .as_ref()
            .and_then(|selected| visible.iter().position(|id| id == selected))
            .unwrap_or(0);
        let next = (current as isize + delta).rem_euclid(visible.len() as isize) as usize;
        self.selected_agent_id = Some(visible[next].clone());
    }

    fn visible_agent_ids(&self) -> Vec<String> {
        self.agents
            .iter()
            .filter(|agent| {
                self.filter.is_empty()
                    || agent.role.contains(&self.filter)
                    || agent.runtime.contains(&self.filter)
                    || agent.state.contains(&self.filter)
                    || agent.id.contains(&self.filter)
            })
            .map(|agent| agent.id.clone())
            .collect()
    }
}

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

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

fn help_lines(width: usize) -> Vec<Line<'static>> {
    let _ = width;
    vec![
        Line::from(Span::styled(
            "Keys",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("j/k      move selection"),
        Line::from("f        focus selected agent"),
        Line::from("a        ack selected item"),
        Line::from("r        restart selected agent"),
        Line::from("s        spawn wizard"),
        Line::from("/        filter agents"),
        Line::from("i        inbox detail"),
        Line::from("Esc      close mode"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn keyboard_moves_selection_and_filter() {
        let mut plugin = RallyPlugin::default();
        plugin.apply_snapshot_bytes(
            br#"{
                "kind":"state_snapshot",
                "version":1,
                "workspaces":[{"id":"w1","name":"api","canonical_key":"api"}],
                "agents":[
                    {"id":"a1","workspace_id":"w1","role":"impl","runtime":"cc","state":"running"},
                    {"id":"a2","workspace_id":"w1","role":"review","runtime":"cc","state":"idle"}
                ],
                "inbox_items":[]
            }"#,
        );

        assert_eq!(plugin.selected_agent_id.as_deref(), Some("a1"));
        assert!(plugin.handle_key(BareKey::Char('j')));
        assert_eq!(plugin.selected_agent_id.as_deref(), Some("a2"));
        assert!(plugin.handle_key(BareKey::Char('/')));
        assert!(plugin.handle_key(BareKey::Char('i')));
        assert!(plugin.handle_key(BareKey::Char('m')));
        assert!(plugin.handle_key(BareKey::Char('p')));
        assert!(plugin.handle_key(BareKey::Char('l')));
        assert!(plugin.handle_key(BareKey::Enter));
        assert_eq!(plugin.filter, "impl");
        assert_eq!(plugin.selected_agent_id.as_deref(), Some("a1"));
    }

    #[test]
    fn keyboard_action_sets_feedback() {
        let mut plugin = RallyPlugin {
            selected_agent_id: Some("a1".to_string()),
            ..Default::default()
        };

        assert!(plugin.handle_key(BareKey::Char('f')));

        assert_eq!(plugin.status_message.as_deref(), Some("focus: a1"));
    }

    #[test]
    fn render_body_contains_widget_output() {
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

        assert!(body.contains("api"));
        assert!(body.contains("impl"));
        assert!(body.contains("1 agents"));
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
