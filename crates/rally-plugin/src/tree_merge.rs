use std::collections::HashSet;

use crate::widgets::{AgentInfo, TreeNode, WorkspaceInfo};
use crate::ZellijPane;
use crate::ZellijTab;

/// Build the ordered list of visible tree nodes by merging daemon data (workspaces/agents)
/// with Zellij session data (tabs/panes).
///
/// If tab data is available, produces Tab → Pane/Agent hierarchy with agent overlay.
/// Otherwise falls back to Workspace → Agent from daemon snapshots only.
pub fn merge_tree(
    tabs: &[ZellijTab],
    panes: &[ZellijPane],
    workspaces: &[WorkspaceInfo],
    agents: &[AgentInfo],
    collapsed: &HashSet<String>,
    filter: &str,
    show_bare_terminals: bool,
) -> Vec<TreeNode> {
    let mut nodes = Vec::new();

    if !tabs.is_empty() {
        merge_tab_view(
            &mut nodes,
            tabs,
            panes,
            agents,
            collapsed,
            filter,
            show_bare_terminals,
        );
    } else {
        merge_workspace_view(&mut nodes, workspaces, agents, collapsed, filter);
    }

    nodes
}

fn merge_tab_view(
    nodes: &mut Vec<TreeNode>,
    tabs: &[ZellijTab],
    panes: &[ZellijPane],
    agents: &[AgentInfo],
    collapsed: &HashSet<String>,
    filter: &str,
    show_bare_terminals: bool,
) {
    for tab in tabs {
        nodes.push(TreeNode::Tab {
            position: tab.position,
            name: tab.name.clone(),
        });

        let tab_key = format!("tab:{}", tab.position);
        if collapsed.contains(&tab_key) {
            continue;
        }

        for pane in panes.iter().filter(|p| p.tab_position == tab.position) {
            if let Some(agent) = agents.iter().find(|a| a.pane_id == Some(pane.id)) {
                if agent_matches_filter(agent, filter) {
                    nodes.push(TreeNode::Agent {
                        id: agent.id.clone(),
                        workspace_id: agent.workspace_id.clone(),
                    });
                }
            } else if show_bare_terminals {
                nodes.push(TreeNode::Pane {
                    id: pane.id,
                    tab_position: pane.tab_position,
                });
            }
        }
    }
}

fn merge_workspace_view(
    nodes: &mut Vec<TreeNode>,
    workspaces: &[WorkspaceInfo],
    agents: &[AgentInfo],
    collapsed: &HashSet<String>,
    filter: &str,
) {
    for ws in workspaces {
        nodes.push(TreeNode::Workspace { id: ws.id.clone() });

        let workspace_agents: Vec<&AgentInfo> = agents
            .iter()
            .filter(|a| a.workspace_id == ws.id && agent_matches_filter(a, filter))
            .collect();

        let is_collapsed = collapsed.contains(&ws.id);
        let filter_forces_expand = !filter.is_empty() && !workspace_agents.is_empty();

        if !is_collapsed || filter_forces_expand {
            for agent in workspace_agents {
                nodes.push(TreeNode::Agent {
                    id: agent.id.clone(),
                    workspace_id: ws.id.clone(),
                });
            }
        }
    }
}

pub fn agent_matches_filter(agent: &AgentInfo, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    agent.role.contains(filter)
        || agent.runtime.contains(filter)
        || agent.state.contains(filter)
        || agent.id.contains(filter)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(pos: usize, name: &str) -> ZellijTab {
        ZellijTab {
            position: pos,
            name: name.to_string(),
            active: pos == 0,
        }
    }

    fn pane(id: u32, tab_pos: usize) -> ZellijPane {
        ZellijPane {
            id,
            tab_position: tab_pos,
            is_plugin: false,
            is_floating: false,
            is_selectable: true,
            title: format!("pane-{id}"),
        }
    }

    fn workspace(id: &str, name: &str) -> WorkspaceInfo {
        WorkspaceInfo {
            id: id.to_string(),
            name: name.to_string(),
            canonical_key: name.to_string(),
        }
    }

    fn agent(id: &str, ws_id: &str, role: &str, pane_id: Option<u32>) -> AgentInfo {
        AgentInfo {
            id: id.to_string(),
            workspace_id: ws_id.to_string(),
            role: role.to_string(),
            runtime: "cc".to_string(),
            state: "running".to_string(),
            pane_session: None,
            pane_id,
            cwd: None,
            project_root: None,
            branch: None,
        }
    }

    #[test]
    fn tab_view_with_bare_panes() {
        let tabs = vec![tab(0, "Tab 1")];
        let panes = vec![pane(10, 0), pane(11, 0)];
        let nodes = merge_tree(&tabs, &panes, &[], &[], &HashSet::new(), "", true);

        assert_eq!(nodes.len(), 3); // Tab + 2 Panes
        assert!(matches!(&nodes[0], TreeNode::Tab { position: 0, .. }));
        assert!(matches!(&nodes[1], TreeNode::Pane { id: 10, .. }));
        assert!(matches!(&nodes[2], TreeNode::Pane { id: 11, .. }));
    }

    #[test]
    fn agent_overlays_onto_pane() {
        let tabs = vec![tab(0, "Tab 1")];
        let panes = vec![pane(10, 0), pane(11, 0)];
        let agents = vec![agent("a1", "w1", "impl", Some(10))];
        let nodes = merge_tree(&tabs, &panes, &[], &agents, &HashSet::new(), "", true);

        assert_eq!(nodes.len(), 3); // Tab + Agent(overlay) + Pane(bare)
        assert!(matches!(&nodes[1], TreeNode::Agent { id, .. } if id == "a1"));
        assert!(matches!(&nodes[2], TreeNode::Pane { id: 11, .. }));
    }

    #[test]
    fn collapsed_tab_hides_children() {
        let tabs = vec![tab(0, "Tab 1"), tab(1, "Tab 2")];
        let panes = vec![pane(10, 0), pane(11, 1)];
        let mut collapsed = HashSet::new();
        collapsed.insert("tab:0".to_string());
        let nodes = merge_tree(&tabs, &panes, &[], &[], &collapsed, "", true);

        // Tab 0 collapsed (no children), Tab 1 expanded
        assert_eq!(nodes.len(), 3); // Tab0 + Tab1 + Pane11
        assert!(matches!(&nodes[0], TreeNode::Tab { position: 0, .. }));
        assert!(matches!(&nodes[1], TreeNode::Tab { position: 1, .. }));
        assert!(matches!(&nodes[2], TreeNode::Pane { id: 11, .. }));
    }

    #[test]
    fn multi_tab_ordering() {
        let tabs = vec![tab(0, "main"), tab(1, "tests"), tab(2, "logs")];
        let panes = vec![pane(1, 0), pane(2, 1), pane(3, 1), pane(4, 2)];
        let nodes = merge_tree(&tabs, &panes, &[], &[], &HashSet::new(), "", true);

        assert_eq!(nodes.len(), 7); // 3 tabs + 4 panes
        // Verify tab→pane ordering
        assert!(matches!(&nodes[0], TreeNode::Tab { name, .. } if name == "main"));
        assert!(matches!(&nodes[1], TreeNode::Pane { id: 1, .. }));
        assert!(matches!(&nodes[2], TreeNode::Tab { name, .. } if name == "tests"));
        assert!(matches!(&nodes[3], TreeNode::Pane { id: 2, .. }));
        assert!(matches!(&nodes[4], TreeNode::Pane { id: 3, .. }));
        assert!(matches!(&nodes[5], TreeNode::Tab { name, .. } if name == "logs"));
        assert!(matches!(&nodes[6], TreeNode::Pane { id: 4, .. }));
    }

    #[test]
    fn fallback_workspace_view_when_no_tabs() {
        let workspaces = vec![workspace("w1", "api")];
        let agents = vec![agent("a1", "w1", "impl", None)];
        let nodes = merge_tree(&[], &[], &workspaces, &agents, &HashSet::new(), "", true);

        assert_eq!(nodes.len(), 2);
        assert!(matches!(&nodes[0], TreeNode::Workspace { id } if id == "w1"));
        assert!(matches!(&nodes[1], TreeNode::Agent { id, .. } if id == "a1"));
    }

    #[test]
    fn filter_hides_non_matching_agents() {
        let tabs = vec![tab(0, "Tab 1")];
        let panes = vec![pane(10, 0), pane(11, 0)];
        let agents = vec![
            agent("a1", "w1", "impl", Some(10)),
            agent("a2", "w1", "review", Some(11)),
        ];
        let nodes = merge_tree(&tabs, &panes, &[], &agents, &HashSet::new(), "impl", true);

        // Only a1 matches filter; a2's pane is agent-bound but filtered out entirely.
        assert_eq!(nodes.len(), 2);
        assert!(matches!(&nodes[0], TreeNode::Tab { position: 0, .. }));
        assert!(matches!(&nodes[1], TreeNode::Agent { id, .. } if id == "a1"));
    }

    #[test]
    fn filter_auto_expands_collapsed_workspace() {
        let workspaces = vec![workspace("w1", "api")];
        let agents = vec![agent("a1", "w1", "impl", None)];
        let mut collapsed = HashSet::new();
        collapsed.insert("w1".to_string());
        let nodes = merge_tree(&[], &[], &workspaces, &agents, &collapsed, "impl", true);

        // Filter forces expand even though workspace is collapsed
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn empty_state() {
        let nodes = merge_tree(&[], &[], &[], &[], &HashSet::new(), "", true);
        assert!(nodes.is_empty());
    }
}
