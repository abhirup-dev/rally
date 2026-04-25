# Handoff: Sidebar Phases S4–S6 Complete — Dogfooding Ready

## Session Metadata
- Created: 2026-04-25 13:47:38
- Project: /Users/abhirupdas/Codes/Personal/rally
- Branch: main
- Session duration: ~4 hours

### Recent Commits (for context)
  - d15e153 chore: gate render_to_string behind #[cfg(test)]
  - 70da109 chore: add diagnostic logging to S4/T6/S5 critical paths
  - 8bd809a feat: capture pane ID from new-pane stdout, skip _attach shim
  - 9f6e632 phase S6: TreeMerge extraction, density mode, sidebar config
  - 4e411ff fix: pane menu Restart uses focus-pane-id before close-pane
  - 4569eda phase S5: floating action menu for bare terminal panes
  - ac3282d phase T6: subscribe TabUpdate/PaneUpdate, bare pane tree nodes
  - e630d08 phase S4: StateTheme + pane tinting + rename + focus action

## Handoff Chain

- **Continues from**: [2026-04-25-031934-sidebar-phases-s0-s6.md](./2026-04-25-031934-sidebar-phases-s0-s6.md)
  - Previous title: Sidebar Phases S0–S6 — Beads Issues Created, Ready to Implement
- **Supersedes**: None

## Current State Summary

The sidebar modernization is feature-complete through Phase S6 and ready for dogfooding. The plugin now has a full Tab→Pane→Agent tree from Zellij events, visual state encoding (glyphs, pane tinting, pane renaming), a floating action menu for bare terminals, density mode toggle, and configurable sidebar settings. The tree merge logic is extracted into a standalone tested module. Agent spawn now captures pane IDs directly from zellij stdout, eliminating the `_attach` shim roundtrip for pane correlation. Four epics closed this session: S4 (visual state), Tree Navigation, S5 (floating action menu), S6 (TreeMerge + config). All code is pushed. **76 tests passing across workspace, 30 in plugin alone.**

## Codebase Understanding

### Architecture Overview

Hexagonal / ports-and-adapters. Plugin has two data sources:
1. **Daemon snapshots** (workspaces, agents, inbox) — via `rally --json _plugin-state` + Zellij pipe push
2. **Zellij events** (tabs, panes, CWD) — via `EventType::TabUpdate` + `EventType::PaneUpdate` + `EventType::CwdChanged`

The plugin merges these client-side in `tree_merge::merge_tree()`. The daemon does NOT compute sidebar layout — it cannot see Zellij session topology.

### Critical Files

| File | Purpose | Relevance |
|------|---------|-----------|
| `crates/rally-plugin/src/main.rs` | Plugin entry point — load, update, render, key handling | Every sidebar feature touches this |
| `crates/rally-plugin/src/tree_merge.rs` | **NEW** — standalone tree merge: Tab→Pane→Agent hierarchy builder | Core data model, 8 unit tests |
| `crates/rally-plugin/src/theme.rs` | **NEW** — StateTheme: glyph/color/pane-bg mapping per agent state | Used by workspace_tree.rs + main.rs for pane tinting |
| `crates/rally-plugin/src/widgets/workspace_tree.rs` | Tree rendering: tab_line, pane_line, agent_line with connectors | Density mode, bare terminal rendering |
| `crates/rally-cli/src/pane_menu.rs` | **NEW** — `rally pane menu` interactive TUI (crossterm) | Split right/down/Restart actions |
| `crates/rally-host-zellij/src/actions.rs` | Zellij CLI wrappers | `new_pane()` now returns pane ID from stdout |

### Key Patterns Discovered

- **Wasm API gating**: Plugin calls to `focus_terminal_pane`, `switch_session`, `set_pane_color`, `rename_terminal_pane`, `open_command_pane_floating` are wrapped in free functions with `#[cfg(not(test))]` blocks + `let _ = args;` to suppress unused warnings. Required because `host_run_plugin_command` is a wasm import that doesn't link on native test target.
- **Plugin logging**: No `tracing` crate available (wasm). Uses `eprintln!` which Zellij captures as plugin stderr. Key diagnostic points: stale snapshot rejection, PaneUpdate filter summary.
- **Collapse key format**: `collapsed: HashSet<String>` uses `"tab:{position}"` for tab collapse and raw workspace ID for workspace collapse.
- **DensityMode**: Enum in plugin — `Normal` (glyph + name + runtime + branch) vs `Compact` (glyph + name only). Toggle via `d` key. Readable from Zellij plugin config `sidebar_density = "compact"`.
- **Pane ID capture**: `zellij action new-pane` outputs `terminal_<id>` to stdout. `parse_pane_id()` extracts the u32. `BindPane` IPC sent directly from spawn flow — no more `_attach` env var roundtrip.

## Work Completed

### Epics Closed

| Epic | ID | Tasks |
|------|----|-------|
| Phase S4 — Visual state encoding | ral-dfw | 5/5 (S4.1–S4.5) |
| Sidebar Tree Navigation | ral-5hsr | 8/8 (T1–T6 + meta) |
| Phase S5 — Floating action menu | ral-uti | 5/5 (S5.1–S5.5) |
| Phase S6 — TreeMerge + config | ral-dul | 5/5 (S6.1–S6.5) |

### Individual Tasks Completed

- [x] **T5 (ral-0ms6)**: `f` key → `focus_terminal_pane` for agents, `switch_session("rally-{name}")` for workspaces
- [x] **T6 (ral-ivcb)**: Subscribe TabUpdate/PaneUpdate, store ZellijTab/ZellijPane, TreeNode::Tab+Pane, CWD tracking
- [x] **S4.1 (ral-tg5)**: StateTheme struct with corrected glyph/color mapping for all 8 agent states
- [x] **S4.2 (ral-s35)**: Sidebar rows use state_theme() for styled rendering
- [x] **S4.3 (ral-u5s)**: `set_pane_color` called on every snapshot for bound agent panes
- [x] **S4.4 (ral-lpa)**: `rename_terminal_pane` with state emoji prefix on every snapshot
- [x] **S4.5 (ral-3f65)**: `bare_terminal_theme()` stub for T6 bare pane nodes
- [x] **S5.1 (ral-4it)**: `rally pane menu --pane-id <id> --cwd <path>` — crossterm TUI
- [x] **S5.2 (ral-b1i)**: `a` key → `open_command_pane_floating()` with rally pane menu
- [x] **S5.3 (ral-csw)**: Split right/down via `zellij action new-pane --direction`, Restart via focus→close→new
- [x] **S5.4 (ral-m9r)**: Auto-close — process exit, Zellij cleans up floating pane
- [x] **S5.5 (ral-uj5)**: Closed as duplicate of T5
- [x] **S6.1 (ral-dul.1)**: Extracted `tree_merge.rs` with `merge_tree()` pure function
- [x] **S6.2 (ral-dul.2)**: Doc comment on StateSnapshotResponse: raw entities only
- [x] **S6.3 (ral-dul.3)**: 8 TreeMerge unit tests
- [x] **S6.4 (ral-gh3f)**: DensityMode enum, `d` key toggle, compact agent rendering
- [x] **S6.5 (ral-wkud)**: Sidebar config from Zellij plugin config map (density, show_bare_terminals, default_collapsed)
- [x] **ral-1fm**: Capture pane ID from new-pane stdout, skip _attach shim for bind
- [x] **ral-dz8d**: Closed as duplicate of T5
- [x] 17 stale Logging: issues closed (parent features covered)
- [x] Dead code cleanup: `render_to_string` gated to `#[cfg(test)]`

### Files Created

| File | Purpose |
|------|---------|
| `crates/rally-plugin/src/theme.rs` | StateTheme: glyph + style + pane_bg per agent state |
| `crates/rally-plugin/src/tree_merge.rs` | Standalone tree merge logic with 8 tests |
| `crates/rally-cli/src/pane_menu.rs` | Interactive floating action menu TUI |

### Files Modified

| File | Changes | Rationale |
|------|---------|-----------|
| `crates/rally-plugin/src/main.rs` | TreeNode Tab/Pane variants, Selection Tab/Pane, Tab/PaneUpdate handlers, collapse generalization, handle_focus with native API, handle_action_menu, density toggle, sidebar config, wasm API gating, diagnostic logging | Core of S4/T6/S5/S6 |
| `crates/rally-plugin/src/widgets/workspace_tree.rs` | tab_line, pane_line renderers, density-aware agent_line, connector logic | Tree rendering for new node types |
| `crates/rally-plugin/src/widgets/mod.rs` | Removed old state_glyph, TreeNode extended | Replaced by theme.rs |
| `crates/rally-host-zellij/src/actions.rs` | new_pane returns u32 pane ID, parse_pane_id(), warn logging | Pane ID capture feature |
| `crates/rally-cli/src/main.rs` | Pane subcommand, spawn flow uses captured pane ID for BindPane | S5.1 + ral-1fm |
| `crates/rally-cli/Cargo.toml` | Added crossterm dep | For pane_menu TUI |
| `crates/rally-plugin/src/snapshots/*.snap` | Updated golden ANSI snapshots | Glyph changes from S4.1 |

### Decisions Made

| Decision | Options Considered | Rationale |
|----------|-------------------|-----------|
| `a` → action menu, `K` → ack inbox | Keep `a` for ack, use Enter for action | `a` is more ergonomic for frequent action; ack is less frequent |
| Floating pane shells out to `zellij action` CLI | IPC back to plugin, native plugin API | Simple, no plumbing; zellij binary always in PATH inside a session |
| Plugin-side tree merge (not daemon) | Daemon-side SidebarProjection | Daemon can't see Zellij tabs/panes; plugin has both data sources |
| `focus_terminal_pane` native API for agent focus | `run_command("zellij action focus-pane-id")` | No subprocess, no PATH dependency, works from wasm |
| `switch_session(rally-{name})` for workspace focus | run_command to attach | Native API, session naming convention confirmed in CLI code |
| S6 redesigned to plugin-side TreeMerge | Original daemon-side projection | T6 introduced client-side Zellij events, making daemon projection obsolete |
| Density mode as plugin config + runtime toggle | Config-only, runtime-only | Both: config sets default, `d` key overrides at runtime |

## Pending Work

## Immediate Next Steps

**CRITICAL: Build and install the wasm plugin before testing:**
```bash
cargo build -p rally-plugin --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/rally-plugin.wasm ~/.config/rally/rally.wasm
```

#### QA Flow 1: Basic Tree Rendering
1. Start a rally session: `rally up <workspace-name>`
2. Open multiple tabs (Ctrl+T in Zellij)
3. Open multiple panes in different tabs
4. **Verify**: Sidebar shows Tab→Pane hierarchy with correct tree connectors (├──, └──)
5. **Verify**: Each tab shows ▼ (expanded) or ▶ (collapsed) glyph in cyan
6. **Verify**: Bare panes show ▪ glyph with CWD basename and pane ID (e.g. `▪ rally p:5`)

#### QA Flow 2: Navigation (j/k/h/l)
1. Press `j`/`k` — selection moves through visible tree nodes (wraps around)
2. Select a tab → press `h` — tab collapses, children hidden
3. Press `l` — tab expands, shows children
4. Press `l` again on expanded tab — selection descends to first child
5. Select a bare pane → press `h` — moves to parent tab and collapses it
6. **Verify**: Selection highlight (REVERSED style) tracks correctly through all node types

#### QA Flow 3: Focus Action (f key)
1. Select a bare terminal pane → press `f`
2. **Verify**: Zellij focus switches to that pane (cursor moves there)
3. Select a tab node → press `f`
4. **Verify**: Nothing happens (tabs can't be focused, no error)

#### QA Flow 4: Action Menu (a key)
1. Select a bare pane → press `a`
2. **Verify**: Floating pane opens with rally pane menu showing:
   - "Split right" / "Split down" / "Restart"
   - CWD displayed correctly
3. Navigate with `j`/`k`, press Enter on "Split right"
4. **Verify**: A new pane appears to the right of the target pane, in the same CWD
5. Press `a` again → select "Split down"
6. **Verify**: A new pane appears below
7. Press `a` → select "Restart"
8. **Verify**: Target pane closes, new pane opens in same CWD
9. Press `Esc` in menu → **Verify**: Menu closes, no action taken

#### QA Flow 5: Density Mode (d key)
1. Press `d` — tree switches to compact mode
2. **Verify**: Agent rows show only glyph + role name (no `(cc)` runtime, no `[branch]` tag)
3. Press `d` again — back to normal mode
4. **Verify**: Full info restored: `● impl (cc) [main]`

#### QA Flow 6: Visual State Encoding (S4)
1. Spawn agents with different states (if agents are onboarded)
2. **Verify glyphs**: ◐ initializing, ● running, ○ idle, ⧗ waiting, ◉ attention, ✓ completed, ✗ failed, ✕ stopped
3. **Verify colors**: Green (running/completed), Yellow (initializing/waiting), Red (attention/failed), Gray (idle/stopped)
4. **Verify pane tinting**: Running agent pane gets subtle green bg, failed gets red bg
5. **Verify pane naming**: Pane title shows state glyph prefix (e.g. "● impl")

#### QA Flow 7: Filter Mode (/)
1. Press `/` — filter mode activates
2. Type "impl" → press Enter
3. **Verify**: Only agents matching "impl" visible; non-matching agent panes disappear
4. Press `Esc` → filter clears, all nodes return

#### QA Flow 8: Agent Spawn with Pane ID Capture
1. Run: `rally agent spawn --workspace <ws> --role test --runtime cc -- zsh`
2. **Verify**: Pane opens, agent is bound (check `rally agent show <id>` shows pane_id)
3. **Verify**: No `_attach` shim in the pane command (just `zsh`)

#### QA Flow 9: Sidebar Config
1. Edit layout KDL to add config:
   ```kdl
   plugin location="file:~/.config/rally/rally.wasm" {
       skip_plugin_cache true
       sidebar_density "compact"
       show_bare_terminals "false"
       default_collapsed "true"
   }
   ```
2. Restart session
3. **Verify**: Starts in compact mode, bare terminals hidden, tabs collapsed

#### QA Flow 10: Edge Cases
1. Close all panes in a tab → **Verify**: Tab still appears (no crash), pane list empty
2. Open 50+ panes → **Verify**: Scroll works, selection stays in view
3. Rapid tab/pane open/close → **Verify**: No stale pane IDs, tree updates reactively
4. Plugin permission denied → **Verify**: Shows "Permission denied. Grant RunCommands to continue."
5. Daemon not running → **Verify**: Tab/pane tree still works (daemon data absent, Zellij events still flow)

### Blockers/Open Questions

- [ ] **Restart action timing**: focus-pane-id → close-pane → new-pane is three sequential zellij CLI calls. If focus-pane-id targets a stale pane (already closed), the whole sequence fails silently. Needs dogfooding to assess reliability.
- [ ] **PaneUpdate frequency**: How often does Zellij fire PaneUpdate? If too frequent, the eprintln filter summary could spam stderr. May need to throttle or remove.
- [ ] **Tab index for BindPane**: When capturing pane ID via stdout, `tab_index: 0` is hardcoded. This is wrong if the agent spawns in a non-first tab. Needs investigation.

### Deferred Items

- **Agent restart/stop CLI commands**: Deferred until agents are onboarded. Proto/daemon already support `AgentTrigger::StopRequested` and `AgentTrigger::Restarted`.
- **S6 daemon-side projection**: Redesigned away. See ral-7qo6 for future requirements.
- **Phase 5 (hooks/inbox)**: Orthogonal to sidebar. 13 logging review issues remain open.
- **Phase 6 (MCP)**: Orthogonal to sidebar.

## Context for Resuming Agent

## Important Context

1. **The plugin is wasm32-wasip1**. It cannot use `tracing`, cannot link native libraries, and all Zellij API calls (`focus_terminal_pane`, `switch_session`, etc.) must be gated behind `#[cfg(not(test))]` wrappers to compile in native test mode.

2. **Two data sources, one tree**. Daemon provides workspaces/agents/inbox. Zellij events provide tabs/panes/CWD. `tree_merge::merge_tree()` combines them. If tabs are available, the tree is Tab→Pane/Agent. If not (before TabUpdate fires), it falls back to Workspace→Agent.

3. **Session naming convention**: Each workspace runs in a Zellij session named `rally-{workspace_name}`. Confirmed in CLI code at `main.rs:380`. The `switch_session` call for workspace focus relies on this.

4. **Golden ANSI snapshots**: The `golden_ansi_snapshot_matrix` test renders the sidebar at various agent/inbox counts and compares against committed `.snap` files. Any rendering change (glyph, color, layout) requires `cargo insta accept` to update them.

5. **Beads memory**: Two design decisions saved via `bd remember`:
   - S5 design: `a` → action menu, `K` → ack; floating pane shells to zellij CLI
   - Plugin version gate: `state_version` must be `Option<u64>` (daemon EventBus version starts at 0)

### Assumptions Made

- Zellij 0.44+ is installed (required for `focus-pane-id`, `new-pane` stdout pane ID, `set_pane_color`)
- `rally` binary is in PATH when the plugin runs `run_command` or `open_command_pane_floating`
- Each workspace = one Zellij session, sidebar plugin runs inside that session
- Bare terminal panes are: `!is_plugin && !is_floating && !is_suppressed && is_selectable`
- `close-pane` closes the focused pane (no pane-id variant in zellij 0.44 CLI)

### Potential Gotchas

- **Stale pane IDs**: If a pane is closed externally, `focus_terminal_pane(stale_id)` may be a no-op. The action menu's Restart flow (focus→close→new) could fail silently if the target pane was already gone.
- **BindPane tab_index hardcoded to 0**: In `rally agent spawn`, the `BindPane` request sends `tab_index: 0`. If the agent spawns in tab 2, the daemon has wrong tab info. This was pre-existing but becomes more visible now that tabs are shown in the sidebar.
- **PaneUpdate includes the sidebar plugin pane**: The filter `is_plugin` removes it, but if Zellij reports the plugin as a terminal for some reason, it would appear as a bare pane.
- **`open_command_pane_floating` returns `Option<PaneId>`**: Currently ignored. If the floating pane fails to spawn (e.g., rally binary not found), there's no feedback to the user.

## Environment State

### Tools/Services Used

- Zellij 0.44.x (required for new-pane stdout, focus-pane-id, set_pane_color APIs)
- rallyd (daemon, unix socket IPC)
- rally (CLI binary)
- cargo + wasm32-wasip1 target for plugin builds

### Build Commands

```bash
# Regular build (excludes plugin):
cargo build

# Plugin build:
cargo build -p rally-plugin --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/rally-plugin.wasm ~/.config/rally/rally.wasm

# Test:
cargo test --workspace  # 76 tests
cargo test -p rally-plugin  # 30 tests (includes tree_merge)

# Dev layout (skip_plugin_cache for fresh wasm):
PATH="$PWD/target/debug:$PATH" zellij -n layouts/sidebar-dev.kdl
```

### Environment Variables

- `RALLY_LOG` — log level for daemon/CLI (e.g., `RALLY_LOG=debug`)
- `ZELLIJ_SESSION_NAME` — set by Zellij, used for session detection

## Related Resources

- `SidebarPlan.md` — original phased roadmap (S0–S6)
- `SidebarV2.md` — competitive analysis (cmux, Conductor, Superset)
- `.claude/handoffs/2026-04-25-031934-sidebar-phases-s0-s6.md` — previous handoff (beads issues created)
- `bd stats` — project statistics
- `bd ready` — available work for next session

---

**Security Reminder**: Before finalizing, run `validate_handoff.py` to check for accidental secret exposure.
