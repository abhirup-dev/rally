use super::{AgentInfo, AnsiBuf, InboxItemInfo, RenderCtx, SidebarWidget};

#[derive(Default)]
pub struct InboxSummary {
    expanded: bool,
}

impl InboxSummary {
    pub fn new(expanded: bool) -> Self {
        Self { expanded }
    }
}

impl SidebarWidget for InboxSummary {
    fn id(&self) -> &'static str {
        "inbox_summary"
    }

    fn render(&self, ctx: &RenderCtx<'_>, buf: &mut AnsiBuf) {
        let signals = InboxSignals::from_ctx(ctx);
        if signals.total == 0 {
            return;
        }

        buf.line(format!(
            "\x1b[1mInbox\x1b[0m {}{}",
            urgency_badge("high", signals.high),
            urgency_badge("medium", signals.medium)
        ));

        if self.expanded {
            render_detail(ctx, buf);
        } else {
            buf.line(format!(
                "  {} open · \x1b[2mpress i for detail\x1b[0m",
                signals.total
            ));
        }
    }
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

fn render_detail(ctx: &RenderCtx<'_>, buf: &mut AnsiBuf) {
    if ctx.inbox_items.iter().any(|item| !item.acked) {
        for item in ctx.inbox_items.iter().filter(|item| !item.acked).take(5) {
            let agent = item
                .agent_id
                .as_deref()
                .and_then(|id| ctx.agents.iter().find(|agent| agent.id == id));
            let label = agent.map(|agent| agent.role.as_str()).unwrap_or("system");
            let message = item.message.as_deref().unwrap_or("attention required");
            buf.line(format!(
                "  {} {:<8} {}",
                urgency_icon(&item.urgency),
                truncate_chars(label, 8),
                truncate_chars(message, ctx.cols.saturating_sub(13).max(1))
            ));
        }
        return;
    }

    for agent in ctx
        .agents
        .iter()
        .filter(|agent| agent.state == "attention_required" || agent.state == "waiting_for_input")
        .take(5)
    {
        let icon = if agent.state == "attention_required" {
            "◉"
        } else {
            "⚠"
        };
        buf.line(format!(
            "  {} {:<8} {}",
            icon,
            truncate_chars(&agent.role, 8),
            truncate_chars(&agent.state, ctx.cols.saturating_sub(13).max(1))
        ));
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

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }

    let keep = max.saturating_sub(1);
    let mut truncated: String = value.chars().take(keep).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{AgentInfo, WorkspaceInfo};

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
        let mut buf = AnsiBuf::default();

        InboxSummary::default().render(&ctx, &mut buf);

        assert!(buf.as_str().contains("Inbox"));
        assert!(buf.as_str().contains("high:1"));
        assert!(buf.as_str().contains("1 open"));
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
        let mut buf = AnsiBuf::default();

        InboxSummary::new(true).render(&ctx, &mut buf);

        assert!(buf.as_str().contains("review"));
        assert!(buf.as_str().contains("Apply changes?"));
    }

    #[test]
    fn falls_back_to_attention_agents() {
        let workspaces = vec![WorkspaceInfo {
            id: "w1".to_string(),
            name: "api".to_string(),
            canonical_key: "api".to_string(),
        }];
        let agents = vec![agent("a1", "impl", "waiting_for_input")];
        let ctx = RenderCtx {
            cols: 80,
            workspaces: &workspaces,
            agents: &agents,
            inbox_items: &[],
            selected_agent_id: None,
            filter: None,
            status_message: None,
        };
        let mut buf = AnsiBuf::default();

        InboxSummary::new(true).render(&ctx, &mut buf);

        assert!(buf.as_str().contains("medium:1"));
        assert!(buf.as_str().contains("impl"));
    }
}
