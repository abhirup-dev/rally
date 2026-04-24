use super::{AgentInfo, AnsiBuf, RenderCtx, SidebarWidget, WorkspaceInfo};

#[derive(Default)]
pub struct WorkspaceTree;

impl SidebarWidget for WorkspaceTree {
    fn id(&self) -> &'static str {
        "workspace_tree"
    }

    fn render(&self, ctx: &RenderCtx<'_>, buf: &mut AnsiBuf) {
        for workspace in ctx.workspaces {
            render_workspace_header(buf, workspace, ctx.cols);

            let mut count = 0usize;
            for agent in ctx
                .agents
                .iter()
                .filter(|agent| agent.workspace_id == workspace.id)
                .filter(|agent| matches_filter(agent, ctx.filter))
            {
                count += 1;
                render_agent_row(
                    buf,
                    agent,
                    ctx.cols,
                    ctx.selected_agent_id == Some(agent.id.as_str()),
                );
            }

            if count == 0 {
                buf.line("  \x1b[2mno agents\x1b[0m");
            }
        }
    }
}

fn render_workspace_header(buf: &mut AnsiBuf, workspace: &WorkspaceInfo, cols: usize) {
    let max_name = cols.saturating_sub(2).max(1);
    buf.line(format!(
        "\x1b[1;34m◆\x1b[0m \x1b[1m{}\x1b[0m",
        truncate_chars(&workspace.name, max_name)
    ));
}

fn render_agent_row(buf: &mut AnsiBuf, agent: &AgentInfo, cols: usize, selected: bool) {
    let pane = agent.pane_id.map(|p| format!(" p:{p}")).unwrap_or_default();
    let suffix = format!(" ({}){}", agent.runtime, pane);
    let max_role = cols
        .saturating_sub(6)
        .saturating_sub(display_len(&suffix))
        .max(1);
    let cursor = if selected { "\x1b[7m>\x1b[0m" } else { " " };

    buf.line(format!(
        " {} {} {}\x1b[2m{}\x1b[0m",
        cursor,
        state_glyph(&agent.state),
        truncate_chars(&agent.role, max_role),
        suffix
    ));
}

fn matches_filter(agent: &AgentInfo, filter: Option<&str>) -> bool {
    let Some(filter) = filter.filter(|filter| !filter.is_empty()) else {
        return true;
    };
    agent.role.contains(filter)
        || agent.runtime.contains(filter)
        || agent.state.contains(filter)
        || agent.id.contains(filter)
}

fn state_glyph(state: &str) -> &'static str {
    match state {
        "running" => "\x1b[32m●\x1b[0m",
        "idle" => "\x1b[33m◐\x1b[0m",
        "attention_required" => "\x1b[31m◉\x1b[0m",
        "completed" => "\x1b[32m○\x1b[0m",
        "stopped" => "\x1b[2m✕\x1b[0m",
        "failed" => "\x1b[31m✗\x1b[0m",
        "initializing" => "\x1b[2m⧗\x1b[0m",
        "waiting_for_input" => "\x1b[33m⚠\x1b[0m",
        _ => "?",
    }
}

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }

    let keep = max.saturating_sub(1);
    let mut truncated: String = value.chars().take(keep).collect();
    truncated.push('…');
    truncated
}

fn display_len(value: &str) -> usize {
    value.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(id: &str, name: &str) -> WorkspaceInfo {
        WorkspaceInfo {
            id: id.to_string(),
            name: name.to_string(),
            canonical_key: name.to_string(),
        }
    }

    fn agent(workspace_id: &str, role: &str, state: &str) -> AgentInfo {
        AgentInfo {
            id: format!("{workspace_id}-{role}"),
            workspace_id: workspace_id.to_string(),
            role: role.to_string(),
            runtime: "cc".to_string(),
            state: state.to_string(),
            pane_session: None,
            pane_id: Some(7),
        }
    }

    #[test]
    fn renders_workspace_tree_grouped_by_workspace() {
        let workspaces = vec![workspace("w1", "api"), workspace("w2", "web")];
        let agents = vec![
            agent("w1", "impl", "running"),
            agent("w2", "review", "attention_required"),
        ];
        let ctx = RenderCtx {
            cols: 40,
            workspaces: &workspaces,
            agents: &agents,
            inbox_items: &[],
            selected_agent_id: Some("w2-review"),
            filter: None,
            status_message: None,
        };
        let mut buf = AnsiBuf::default();

        WorkspaceTree.render(&ctx, &mut buf);

        assert!(buf.as_str().contains("api"));
        assert!(buf.as_str().contains("impl"));
        assert!(buf.as_str().contains("web"));
        assert!(buf.as_str().contains("review"));
        assert!(buf.as_str().contains("◉"));
    }

    #[test]
    fn renders_empty_workspace_placeholder() {
        let workspaces = vec![workspace("w1", "api")];
        let ctx = RenderCtx {
            cols: 40,
            workspaces: &workspaces,
            agents: &[],
            inbox_items: &[],
            selected_agent_id: None,
            filter: None,
            status_message: None,
        };
        let mut buf = AnsiBuf::default();

        WorkspaceTree.render(&ctx, &mut buf);

        assert!(buf.as_str().contains("no agents"));
    }

    #[test]
    fn truncates_on_char_boundaries() {
        assert_eq!(truncate_chars("abcd", 3), "ab…");
        assert_eq!(truncate_chars("⚠abcd", 3), "⚠a…");
    }
}
