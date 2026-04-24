use super::{AnsiBuf, RenderCtx, SidebarWidget};

#[derive(Default)]
pub struct StatusBar;

impl SidebarWidget for StatusBar {
    fn id(&self) -> &'static str {
        "status_bar"
    }

    fn render(&self, ctx: &RenderCtx<'_>, buf: &mut AnsiBuf) {
        let width = ctx.cols.max(1);
        let total = ctx.agents.len();
        let running = ctx
            .agents
            .iter()
            .filter(|agent| agent.state == "running")
            .count();
        let attention = ctx
            .agents
            .iter()
            .filter(|agent| {
                agent.state == "attention_required" || agent.state == "waiting_for_input"
            })
            .count();

        buf.line("─".repeat(width));
        if let Some(message) = ctx.status_message {
            buf.line(format!("\x1b[33m{}\x1b[0m", truncate_chars(message, width)));
        }
        if let Some(filter) = ctx.filter {
            buf.line(format!(
                "/{}",
                truncate_chars(filter, width.saturating_sub(1))
            ));
        }
        buf.line(summary_line(total, running, attention));
        buf.line(truncate_chars(
            "\x1b[2m[N]ext [j/k]move [f]ocus [a]ck [s]pawn [/]filter [?]help\x1b[0m",
            width,
        ));
    }
}

fn summary_line(total: usize, running: usize, attention: usize) -> String {
    let mut line = format!("{total} agents");
    if running > 0 {
        line.push_str(&format!(" \x1b[32m{running}●\x1b[0m"));
    }
    if attention > 0 {
        line.push_str(&format!(" \x1b[33m{attention}⚠\x1b[0m"));
    }
    line
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
    use crate::widgets::AgentInfo;

    fn agent(state: &str) -> AgentInfo {
        AgentInfo {
            id: state.to_string(),
            workspace_id: "w1".to_string(),
            role: "impl".to_string(),
            runtime: "cc".to_string(),
            state: state.to_string(),
            pane_session: None,
            pane_id: None,
        }
    }

    #[test]
    fn renders_summary_counts_and_hints() {
        let agents = vec![
            agent("running"),
            agent("waiting_for_input"),
            agent("completed"),
        ];
        let ctx = RenderCtx {
            cols: 80,
            workspaces: &[],
            agents: &agents,
            inbox_items: &[],
            selected_agent_id: None,
            filter: None,
            status_message: None,
        };
        let mut buf = AnsiBuf::default();

        StatusBar.render(&ctx, &mut buf);

        assert!(buf.as_str().contains("3 agents"));
        assert!(buf.as_str().contains("1●"));
        assert!(buf.as_str().contains("1⚠"));
        assert!(buf.as_str().contains("[N]ext"));
    }
}
