use super::{truncate_chars, AgentInfo, InboxItemInfo, RenderCtx};
use zellij_widgets::prelude::*;

pub fn render_inbox_lines(ctx: &RenderCtx<'_>, expanded: bool) -> Vec<Line<'static>> {
    let signals = InboxSignals::from_ctx(ctx);
    if signals.total == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();

    let mut header_spans = vec![Span::styled(
        "Inbox",
        Style::default().add_modifier(Modifier::BOLD),
    )];
    header_spans.push(Span::raw(urgency_badge("high", signals.high)));
    header_spans.push(Span::raw(urgency_badge("medium", signals.medium)));
    lines.push(Line::from(header_spans));

    if expanded {
        render_detail(ctx, &mut lines);
    } else {
        lines.push(Line::from(vec![
            Span::raw(format!("  {} open · ", signals.total)),
            Span::styled(
                "press i for detail",
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]));
    }

    lines
}

struct InboxSignals {
    total: usize,
    high: usize,
    medium: usize,
}

impl InboxSignals {
    fn from_ctx(ctx: &RenderCtx<'_>) -> Self {
        let unacked: Vec<&InboxItemInfo> =
            ctx.inbox_items.iter().filter(|item| !item.acked).collect();

        if !unacked.is_empty() {
            return Self {
                total: unacked.len(),
                high: unacked
                    .iter()
                    .filter(|item| item.urgency.eq_ignore_ascii_case("high"))
                    .count(),
                medium: unacked
                    .iter()
                    .filter(|item| item.urgency.eq_ignore_ascii_case("medium"))
                    .count(),
            };
        }

        let attention_agents: Vec<&AgentInfo> = ctx
            .agents
            .iter()
            .filter(|agent| {
                agent.state == "attention_required" || agent.state == "waiting_for_input"
            })
            .collect();
        Self {
            total: attention_agents.len(),
            high: attention_agents
                .iter()
                .filter(|agent| agent.state == "attention_required")
                .count(),
            medium: attention_agents
                .iter()
                .filter(|agent| agent.state == "waiting_for_input")
                .count(),
        }
    }
}

fn render_detail(ctx: &RenderCtx<'_>, lines: &mut Vec<Line<'static>>) {
    if ctx.inbox_items.iter().any(|item| !item.acked) {
        for item in ctx.inbox_items.iter().filter(|item| !item.acked).take(5) {
            let agent = item
                .agent_id
                .as_deref()
                .and_then(|id| ctx.agents.iter().find(|a| a.id == id));
            let label = agent.map(|a| a.role.as_str()).unwrap_or("system");
            let message = item.message.as_deref().unwrap_or("attention required");
            lines.push(Line::from(format!(
                "  {} {:<8} {}",
                urgency_icon(&item.urgency),
                truncate_chars(label, 8),
                truncate_chars(message, ctx.cols.saturating_sub(13).max(1))
            )));
        }
        return;
    }

    for agent in ctx
        .agents
        .iter()
        .filter(|a| a.state == "attention_required" || a.state == "waiting_for_input")
        .take(5)
    {
        let icon = if agent.state == "attention_required" {
            "◉"
        } else {
            "⚠"
        };
        lines.push(Line::from(format!(
            "  {} {:<8} {}",
            icon,
            truncate_chars(&agent.role, 8),
            truncate_chars(&agent.state, ctx.cols.saturating_sub(13).max(1))
        )));
    }
}

fn urgency_badge(kind: &str, count: usize) -> String {
    if count == 0 {
        String::new()
    } else {
        format!(" {kind}:{count}")
    }
}

fn urgency_icon(urgency: &str) -> &'static str {
    match urgency.to_ascii_lowercase().as_str() {
        "high" => "◉",
        "medium" => "⚠",
        _ => "·",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::AgentInfo;

    fn agent(id: &str, role: &str, state: &str) -> AgentInfo {
        AgentInfo {
            id: id.to_string(),
            workspace_id: "w1".to_string(),
            role: role.to_string(),
            runtime: "cc".to_string(),
            state: state.to_string(),
            pane_session: None,
            pane_id: None,
        }
    }

    fn inbox(id: &str, agent_id: &str, urgency: &str) -> InboxItemInfo {
        InboxItemInfo {
            id: id.to_string(),
            agent_id: Some(agent_id.to_string()),
            urgency: urgency.to_string(),
            acked: false,
            raised_at: 1,
            message: Some("Apply changes?".to_string()),
        }
    }

    fn lines_text(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_summary_from_real_inbox_items() {
        let agents = vec![agent("a1", "review", "running")];
        let inbox_items = vec![inbox("i1", "a1", "High")];
        let ctx = RenderCtx {
            cols: 80,
            workspaces: &[],
            agents: &agents,
            inbox_items: &inbox_items,
            selected_agent_id: None,
            filter: None,
            status_message: None,
        };

        let lines = render_inbox_lines(&ctx, false);
        let text = lines_text(&lines);

        assert!(text.contains("Inbox"));
        assert!(text.contains("high:1"));
        assert!(text.contains("1 open"));
    }

    #[test]
    fn renders_detail_from_real_inbox_items() {
        let agents = vec![agent("a1", "review", "running")];
        let inbox_items = vec![inbox("i1", "a1", "High")];
        let ctx = RenderCtx {
            cols: 80,
            workspaces: &[],
            agents: &agents,
            inbox_items: &inbox_items,
            selected_agent_id: None,
            filter: None,
            status_message: None,
        };

        let lines = render_inbox_lines(&ctx, true);
        let text = lines_text(&lines);

        assert!(text.contains("review"));
        assert!(text.contains("Apply changes?"));
    }

    #[test]
    fn falls_back_to_attention_agents() {
        let agents = vec![agent("a1", "impl", "waiting_for_input")];
        let ctx = RenderCtx {
            cols: 80,
            workspaces: &[],
            agents: &agents,
            inbox_items: &[],
            selected_agent_id: None,
            filter: None,
            status_message: None,
        };

        let lines = render_inbox_lines(&ctx, true);
        let text = lines_text(&lines);

        assert!(text.contains("medium:1"));
        assert!(text.contains("impl"));
    }
}
