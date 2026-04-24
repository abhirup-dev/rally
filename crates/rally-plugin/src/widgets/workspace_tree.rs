use super::{state_glyph, truncate_chars, AgentInfo, RenderCtx};
use zellij_widgets::prelude::*;

pub fn render_workspace_lines(ctx: &RenderCtx<'_>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    for workspace in ctx.workspaces {
        let max_name = ctx.cols.saturating_sub(2).max(1);
        lines.push(Line::from(vec![
            Span::styled(
                "◆",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                truncate_chars(&workspace.name, max_name),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));

        let mut count = 0usize;
        for agent in ctx
            .agents
            .iter()
            .filter(|a| a.workspace_id == workspace.id)
            .filter(|a| matches_filter(a, ctx.filter))
        {
            count += 1;
            lines.push(render_agent_line(
                agent,
                ctx.cols,
                ctx.selected_agent_id == Some(agent.id.as_str()),
            ));
        }

        if count == 0 {
            lines.push(Line::from(Span::styled(
                "  no agents",
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
    }

    lines
}

fn render_agent_line(agent: &AgentInfo, cols: usize, selected: bool) -> Line<'static> {
    let pane = agent.pane_id.map(|p| format!(" p:{p}")).unwrap_or_default();
    let suffix = format!(" ({}){}", agent.runtime, pane);
    let max_role = cols
        .saturating_sub(6)
        .saturating_sub(suffix.chars().count())
        .max(1);

    let (glyph, glyph_style) = state_glyph(&agent.state);

    let cursor = if selected {
        Span::styled(">", Style::default().add_modifier(Modifier::REVERSED))
    } else {
        Span::raw(" ")
    };

    Line::from(vec![
        Span::raw(" "),
        cursor,
        Span::raw(" "),
        Span::styled(glyph, glyph_style),
        Span::raw(" "),
        Span::raw(truncate_chars(&agent.role, max_role)),
        Span::styled(suffix, Style::default().add_modifier(Modifier::DIM)),
    ])
}

fn matches_filter(agent: &AgentInfo, filter: Option<&str>) -> bool {
    let Some(filter) = filter.filter(|f| !f.is_empty()) else {
        return true;
    };
    agent.role.contains(filter)
        || agent.runtime.contains(filter)
        || agent.state.contains(filter)
        || agent.id.contains(filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::WorkspaceInfo;

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

    fn lines_contain(lines: &[Line<'_>], needle: &str) -> bool {
        lines.iter().any(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            text.contains(needle)
        })
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

        let lines = render_workspace_lines(&ctx);

        assert!(lines_contain(&lines, "api"));
        assert!(lines_contain(&lines, "impl"));
        assert!(lines_contain(&lines, "web"));
        assert!(lines_contain(&lines, "review"));
        assert!(lines_contain(&lines, "◉"));
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

        let lines = render_workspace_lines(&ctx);

        assert!(lines_contain(&lines, "no agents"));
    }

    #[test]
    fn truncates_on_char_boundaries() {
        use super::super::truncate_chars;
        assert_eq!(truncate_chars("abcd", 3), "ab…");
        assert_eq!(truncate_chars("⚠abcd", 3), "⚠a…");
    }
}
