use crate::theme::palette;

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
    lines.push(Line::from(""));

    if let Some(message) = ctx.status_message {
        lines.push(Line::from(Span::styled(
            truncate_chars(message, width),
            Style::default().fg(palette::GOLD),
        )));
    }
    if let Some(filter) = ctx.filter {
        lines.push(Line::from(vec![
            Span::styled("/", Style::default().fg(palette::IRIS)),
            Span::styled(
                truncate_chars(filter, width.saturating_sub(1)),
                Style::default().fg(palette::TEXT),
            ),
        ]));
    }

    lines.push(merged_footer(total, running, attention, width));

    lines
}

fn merged_footer(total: usize, running: usize, attention: usize, cols: usize) -> Line<'static> {
    let mut left_spans = vec![Span::styled(
        format!(" {total} agents"),
        Style::default().fg(palette::SUBTLE),
    )];
    if running > 0 {
        left_spans.push(Span::styled(
            format!(" {running}●"),
            Style::default().fg(palette::PINE),
        ));
    }
    if attention > 0 {
        left_spans.push(Span::styled(
            format!(" {attention}◉"),
            Style::default().fg(palette::GOLD),
        ));
    }

    let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
    let right = "? help ";
    let right_width = right.chars().count();
    let fill = cols.saturating_sub(left_width + right_width);

    left_spans.push(Span::raw(" ".repeat(fill)));
    left_spans.push(Span::styled(right, Style::default().fg(palette::MUTED)));

    Line::from(left_spans)
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
    fn renders_summary_counts_and_hint() {
        let agents = vec![
            agent("running"),
            agent("waiting_for_input"),
            agent("completed"),
        ];
        let ctx = RenderCtx {
            cols: 40,
            agents: &agents,
            inbox_items: &[],
            filter: None,
            status_message: None,
        };

        let lines = render_status_lines(&ctx);
        let text = lines_text(&lines);

        assert!(text.contains("3 agents"));
        assert!(text.contains("1●"));
        assert!(text.contains("1◉"));
        assert!(text.contains("? help"));
    }
}
