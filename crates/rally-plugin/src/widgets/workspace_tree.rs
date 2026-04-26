use std::borrow::Cow;
use std::collections::HashSet;

use crate::theme::{bare_terminal_theme, palette, state_theme};
use crate::DensityMode;

use super::{truncate_chars, AgentInfo, TreeNode, WorkspaceInfo};
use std::collections::BTreeMap;
use std::path::PathBuf;
use zellij_widgets::prelude::*;

#[allow(clippy::too_many_arguments)]
pub fn render_tree_lines(
    workspaces: &[WorkspaceInfo],
    agents: &[AgentInfo],
    collapsed: &HashSet<String>,
    visible_nodes: &[TreeNode],
    selected: Option<&TreeNode>,
    pane_cwds: &BTreeMap<u32, PathBuf>,
    density: DensityMode,
    cols: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    for (i, node) in visible_nodes.iter().enumerate() {
        let is_selected = selected == Some(node);
        let next_is_parent_or_end = matches!(
            visible_nodes.get(i + 1),
            None | Some(TreeNode::Workspace { .. }) | Some(TreeNode::Tab { .. })
        );
        let line = match node {
            TreeNode::Workspace { id } => {
                workspace_line(id, workspaces, agents, collapsed, cols, is_selected)
            }
            TreeNode::Tab { name, position } => {
                let is_collapsed = collapsed.contains(&format!("tab:{position}"));
                tab_line(name, is_collapsed, cols, is_selected)
            }
            TreeNode::Pane { id, .. } => {
                let connector = if next_is_parent_or_end {
                    "└"
                } else {
                    "├"
                };
                let cwd = pane_cwds.get(id).and_then(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                });
                pane_line(*id, cwd.as_deref(), connector, cols, is_selected)
            }
            TreeNode::Agent { id, .. } => {
                let connector = if next_is_parent_or_end {
                    "└"
                } else {
                    "├"
                };
                agent_line(id, agents, connector, density, cols, is_selected)
            }
        };
        lines.push(line);
    }

    lines
}

fn workspace_line(
    ws_id: &str,
    workspaces: &[WorkspaceInfo],
    agents: &[AgentInfo],
    collapsed: &HashSet<String>,
    cols: usize,
    selected: bool,
) -> Line<'static> {
    let name = workspaces
        .iter()
        .find(|w| w.id == ws_id)
        .map(|w| w.name.clone())
        .unwrap_or_else(|| ws_id.to_string());

    let has_agents = agents.iter().any(|a| a.workspace_id == ws_id);
    let (glyph, glyph_color): (&'static str, Color) = if !has_agents {
        ("◇", palette::MUTED)
    } else if collapsed.contains(ws_id) {
        ("▸", palette::IRIS)
    } else {
        ("▾", palette::IRIS)
    };

    let max_name = cols.saturating_sub(3).max(1);
    let spans = vec![
        Span::styled(
            glyph,
            Style::default()
                .fg(glyph_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            truncate_chars(&name, max_name),
            Style::default()
                .fg(palette::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    styled_line(spans, selected, cols)
}

fn agent_line(
    agent_id: &str,
    agents: &[AgentInfo],
    connector: &str,
    density: DensityMode,
    cols: usize,
    selected: bool,
) -> Line<'static> {
    let Some(agent) = agents.iter().find(|a| a.id == agent_id) else {
        return Line::from(Span::styled(
            format!(" {connector} (unknown)"),
            Style::default().fg(palette::MUTED),
        ));
    };

    let theme = state_theme(&agent.state);
    let glyph = theme.glyph;
    let glyph_style = theme.style;

    let suffix = match density {
        DensityMode::Compact => String::new(),
        DensityMode::Normal => {
            let branch_tag = agent
                .branch
                .as_deref()
                .map(|b| format!(" [{b}]"))
                .unwrap_or_default();
            format!(" ({}){}", agent.runtime, branch_tag)
        }
    };

    let prefix_len = 5usize;
    let max_role = cols
        .saturating_sub(prefix_len + suffix.chars().count())
        .max(1);

    let spans = vec![
        Span::styled(
            format!(" {connector} "),
            Style::default().fg(palette::MUTED),
        ),
        Span::styled(glyph, glyph_style),
        Span::raw(" "),
        Span::styled(
            truncate_chars(&agent.role, max_role),
            Style::default().fg(palette::TEXT),
        ),
        Span::styled(suffix, Style::default().fg(palette::SUBTLE)),
    ];

    styled_line(spans, selected, cols)
}

fn tab_line(name: &str, is_collapsed: bool, cols: usize, selected: bool) -> Line<'static> {
    let glyph = if is_collapsed { "▸" } else { "▾" };
    let max_name = cols.saturating_sub(3).max(1);
    let spans = vec![
        Span::styled(
            glyph,
            Style::default()
                .fg(palette::FOAM)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            truncate_chars(name, max_name),
            Style::default()
                .fg(palette::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    styled_line(spans, selected, cols)
}

fn pane_line(
    pane_id: u32,
    cwd_name: Option<&str>,
    connector: &str,
    cols: usize,
    selected: bool,
) -> Line<'static> {
    let theme = bare_terminal_theme();
    let label = cwd_name.unwrap_or("terminal");
    let suffix = format!(" p:{pane_id}");

    let prefix_len = 5usize;
    let max_label = cols
        .saturating_sub(prefix_len + suffix.chars().count())
        .max(1);

    let spans = vec![
        Span::styled(
            format!(" {connector} "),
            Style::default().fg(palette::MUTED),
        ),
        Span::styled(theme.glyph, theme.style),
        Span::raw(" "),
        Span::styled(
            truncate_chars(label, max_label),
            Style::default().fg(palette::TEXT),
        ),
        Span::styled(suffix, Style::default().fg(palette::SUBTLE)),
    ];

    styled_line(spans, selected, cols)
}

fn styled_line(spans: Vec<Span<'static>>, selected: bool, cols: usize) -> Line<'static> {
    if !selected {
        return Line::from(spans);
    }
    let sel_bg = palette::SURFACE;
    let content_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let pad = cols.saturating_sub(content_width);

    let mut styled: Vec<Span<'static>> = spans
        .into_iter()
        .map(|s| {
            Span::styled(
                Cow::<'static, str>::Owned(s.content.into_owned()),
                s.style.bg(sel_bg),
            )
        })
        .collect();

    if pad > 0 {
        styled.push(Span::styled(
            " ".repeat(pad),
            Style::default().bg(sel_bg),
        ));
    }

    Line::from(styled)
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
    fn renders_expanded_workspace_with_connectors() {
        let workspaces = vec![workspace("w1", "api"), workspace("w2", "web")];
        let agents = vec![
            agent("w1", "impl", "running"),
            agent("w1", "review", "attention_required"),
            agent("w2", "ops", "idle"),
        ];
        let collapsed = HashSet::new();
        let visible_nodes = vec![
            TreeNode::Workspace {
                id: "w1".to_string(),
            },
            TreeNode::Agent {
                id: "w1-impl".to_string(),
                workspace_id: "w1".to_string(),
            },
            TreeNode::Agent {
                id: "w1-review".to_string(),
                workspace_id: "w1".to_string(),
            },
            TreeNode::Workspace {
                id: "w2".to_string(),
            },
            TreeNode::Agent {
                id: "w2-ops".to_string(),
                workspace_id: "w2".to_string(),
            },
        ];

        let lines = render_tree_lines(&workspaces, &agents, &collapsed, &visible_nodes, None, &BTreeMap::new(), DensityMode::Normal, 40);
        let text = lines_text(&lines);

        assert!(text.contains("▾ api"), "expanded workspace glyph");
        assert!(text.contains("├"), "non-last connector");
        assert!(text.contains("└"), "last connector");
        assert!(text.contains("impl"));
        assert!(text.contains("review"));
        assert!(text.contains("▾ web"));
        assert!(text.contains("ops"));
    }

    #[test]
    fn renders_collapsed_workspace_without_agents() {
        let workspaces = vec![workspace("w1", "api")];
        let agents = vec![agent("w1", "impl", "running")];
        let mut collapsed = HashSet::new();
        collapsed.insert("w1".to_string());
        let visible_nodes = vec![TreeNode::Workspace {
            id: "w1".to_string(),
        }];

        let lines = render_tree_lines(&workspaces, &agents, &collapsed, &visible_nodes, None, &BTreeMap::new(), DensityMode::Normal, 40);
        let text = lines_text(&lines);

        assert!(text.contains("▸ api"), "collapsed workspace glyph");
        assert!(!text.contains("impl"), "agents hidden when collapsed");
    }

    #[test]
    fn renders_empty_workspace_with_diamond() {
        let workspaces = vec![workspace("w1", "api")];
        let agents: Vec<AgentInfo> = vec![];
        let collapsed = HashSet::new();
        let visible_nodes = vec![TreeNode::Workspace {
            id: "w1".to_string(),
        }];

        let lines = render_tree_lines(&workspaces, &agents, &collapsed, &visible_nodes, None, &BTreeMap::new(), DensityMode::Normal, 40);
        let text = lines_text(&lines);

        assert!(text.contains("◇ api"), "empty workspace uses diamond");
    }

    #[test]
    fn selection_applies_surface_bg() {
        let workspaces = vec![workspace("w1", "api")];
        let agents = vec![agent("w1", "impl", "running")];
        let collapsed = HashSet::new();
        let visible_nodes = vec![
            TreeNode::Workspace {
                id: "w1".to_string(),
            },
            TreeNode::Agent {
                id: "w1-impl".to_string(),
                workspace_id: "w1".to_string(),
            },
        ];
        let selected = &visible_nodes[1];

        let lines = render_tree_lines(
            &workspaces,
            &agents,
            &collapsed,
            &visible_nodes,
            Some(selected),
            &BTreeMap::new(),
            DensityMode::Normal,
            40,
        );

        let selected_line = &lines[1];
        let has_bg = selected_line
            .spans
            .iter()
            .any(|s| s.style.bg == Some(palette::SURFACE));
        assert!(has_bg, "selected node gets Surface bg tint");
    }

    #[test]
    fn truncates_on_char_boundaries() {
        use super::super::truncate_chars;
        assert_eq!(truncate_chars("abcd", 3), "ab…");
        assert_eq!(truncate_chars("⚠abcd", 3), "⚠a…");
    }
}
