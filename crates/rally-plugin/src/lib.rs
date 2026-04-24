#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::Deserialize;
use zellij_tile::prelude::*;

#[derive(Default)]
struct RallyPlugin {
    workspaces: Vec<WorkspaceInfo>,
    agents: Vec<AgentInfo>,
    last_error: Option<String>,
    rows: usize,
    cols: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkspaceInfo {
    id: String,
    name: String,
    canonical_key: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentInfo {
    id: String,
    workspace_id: String,
    role: String,
    runtime: String,
    state: String,
    #[serde(default)]
    pane_session: Option<String>,
    #[serde(default)]
    pane_id: Option<u32>,
}

register_plugin!(RallyPlugin);

impl ZellijPlugin for RallyPlugin {
    fn load(&mut self, _config: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ReadApplicationState,
        ]);
        subscribe(&[
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
            EventType::Timer,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.refresh_state();
                set_timeout(5.0);
                true
            }
            Event::Timer(_) => {
                self.refresh_state();
                set_timeout(5.0);
                true
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                let cmd_type = context.get("type").map(|s| s.as_str()).unwrap_or("");
                if exit_code.is_some_and(|c| c != 0) {
                    self.last_error = Some(String::from_utf8_lossy(&stderr).trim().to_string());
                    return true;
                }
                match cmd_type {
                    "workspace_list" => {
                        if let Ok(parsed) = serde_json::from_slice::<WorkspaceListResponse>(&stdout)
                        {
                            self.workspaces = parsed.items;
                            self.last_error = None;
                        }
                    }
                    "agent_list" => {
                        if let Ok(parsed) = serde_json::from_slice::<AgentListResponse>(&stdout) {
                            self.agents = parsed.items;
                            self.last_error = None;
                        }
                    }
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.rows = rows;
        self.cols = cols;

        println!("\x1b[1m Rally \x1b[0m");
        println!("{}", "─".repeat(cols.min(40)));

        if let Some(ref err) = self.last_error {
            println!("\x1b[31m⚠ {}\x1b[0m", truncate(err, cols - 2));
            return;
        }

        if self.workspaces.is_empty() {
            println!("\x1b[2mno workspaces\x1b[0m");
            println!();
            println!("\x1b[2mrally workspace new --name <n>\x1b[0m");
            return;
        }

        for ws in &self.workspaces {
            println!("\x1b[1;34m◆\x1b[0m \x1b[1m{}\x1b[0m", ws.name);
            let ws_agents: Vec<&AgentInfo> = self
                .agents
                .iter()
                .filter(|a| a.workspace_id == ws.id)
                .collect();
            if ws_agents.is_empty() {
                println!("  \x1b[2mno agents\x1b[0m");
            } else {
                for agent in &ws_agents {
                    let glyph = state_glyph(&agent.state);
                    let pane = agent.pane_id.map(|p| format!(" p:{p}")).unwrap_or_default();
                    println!(
                        "  {} \x1b[0m{}\x1b[2m ({}){}\x1b[0m",
                        glyph, agent.role, agent.runtime, pane
                    );
                }
            }
        }

        let total = self.agents.len();
        let running = self.agents.iter().filter(|a| a.state == "running").count();
        let attn = self
            .agents
            .iter()
            .filter(|a| a.state == "attention_required" || a.state == "waiting_for_input")
            .count();

        println!("{}", "─".repeat(cols.min(40)));
        print!("{total} agents");
        if running > 0 {
            print!(" \x1b[32m{running}↑\x1b[0m");
        }
        if attn > 0 {
            print!(" \x1b[33m{attn}⚠\x1b[0m");
        }
        println!();
    }
}

impl RallyPlugin {
    fn refresh_state(&self) {
        let mut ws_ctx = BTreeMap::new();
        ws_ctx.insert("type".to_string(), "workspace_list".to_string());
        run_command(&["rally", "--json", "workspace", "ls"], ws_ctx);

        let mut agent_ctx = BTreeMap::new();
        agent_ctx.insert("type".to_string(), "agent_list".to_string());
        run_command(&["rally", "--json", "agent", "ls"], agent_ctx);
    }
}

#[derive(Deserialize)]
struct WorkspaceListResponse {
    #[serde(default)]
    items: Vec<WorkspaceInfo>,
}

#[derive(Deserialize)]
struct AgentListResponse {
    #[serde(default)]
    items: Vec<AgentInfo>,
}

fn state_glyph(state: &str) -> &'static str {
    match state {
        "initializing" => "\x1b[2m○\x1b[0m",
        "running" => "\x1b[32m●\x1b[0m",
        "idle" => "\x1b[33m◐\x1b[0m",
        "waiting_for_input" => "\x1b[33m⧗\x1b[0m",
        "attention_required" => "\x1b[31m⚠\x1b[0m",
        "completed" => "\x1b[32m✓\x1b[0m",
        "failed" => "\x1b[31m✗\x1b[0m",
        "stopped" => "\x1b[2m◻\x1b[0m",
        _ => "?",
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
