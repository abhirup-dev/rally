use super::{truncate_chars, RenderCtx};
use zellij_widgets::prelude::*;

pub fn render_status_lines(ctx: &RenderCtx<'_>) -> Vec<Line<'static>> {
    let width = ctx.cols.max(1);
    let total = ctx.agents.len();
    let running = ctx.agents.iter().filter(|a| a.state == "running").count();
    let attention = ctx
        .agents
        .iter()
        .filter(|a| a.state == "attention_required" || a.state == "waiting_for_input")
        .count();

    let mut lines = Vec::new();
    lines.push(Line::from("─".repeat(width)));

    if let Some(message) = ctx.status_message {
        lines.push(Line::from(Span::styled(
            truncate_chars(message, width),
            Style::default().fg(Color::Yellow),
        )));
    }
    if let Some(filter) = ctx.filter {
        lines.push(Line::from(format!(
            "/{}",
            truncate_chars(filter, width.saturating_sub(1))
        )));
    }

    lines.push(summary_line(total, running, attention));
    lines.push(Line::from(Span::styled(
        truncate_chars(
            "[N]ext [j/k]move [f]ocus [a]ck [s]pawn [/]filter [?]help",
            width,
        ),
        Style::default().add_modifier(Modifier::DIM),
    )));

    lines
}

fn summary_line(total: usize, running: usize, attention: usize) -> Line<'static> {
    let mut spans = vec![Span::raw(format!("{total} agents"))];
    if running > 0 {
        spans.push(Span::styled(
            format!(" {running}●"),
            Style::default().fg(Color::Green),
        ));
    }
    if attention > 0 {
        spans.push(Span::styled(
            format!(" {attention}⚠"),
            Style::default().fg(Color::Yellow),
        ));
    }
    Line::from(spans)
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
            cwd: None,
            project_root: None,
            branch: None,
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
    fn renders_summary_counts_and_hints() {
        let agents = vec![
            agent("running"),
            agent("waiting_for_input"),
            agent("completed"),
        ];
        let ctx = RenderCtx {
            cols: 80,
            agents: &agents,
            inbox_items: &[],
            filter: None,
            status_message: None,
        };

        let lines = render_status_lines(&ctx);
        let text = lines_text(&lines);

        assert!(text.contains("3 agents"));
        assert!(text.contains("1●"));
        assert!(text.contains("1⚠"));
        assert!(text.contains("[N]ext"));
    }
}
