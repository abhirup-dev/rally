# Handoff: Sidebar Phases S0–S3 Complete, Tree Navigation Epic Ready

## Session Metadata
- Created: 2026-04-25
- Project: /Users/abhirupdas/Codes/Personal/rally
- Branch: main
- Session duration: ~4 hours
- Continues from: .claude/handoffs/2026-04-25-031934-sidebar-phases-s0-s6.md

### Recent Commits
  - f503cfe phase S3: reactive state push via Zellij pipe + CWD tracking
  - e7610cb phase S2: agent context enrichment — cwd, branch, git discovery
  - ca6eba1 phase S1.6: plugin reads rally_cli_path from layout config
  - c0f3020 fix: plugin version gate accepts first snapshot at version 0
  - e1a0a7a phase S1.1–S1.4: zellij_widgets rendering replaces AnsiBuf
  - e7240f9 phase S0: clippy clean, ArcSwap snapshot, IPC hardening, unsafe_code denied

## Current State Summary

Sidebar phases S0 through S3 are complete and pushed to main. The sidebar plugin is live and functional with zellij_widgets rendering, reactive state push, and agent context (cwd/branch). A new epic (ral-5hsr) has been created for tree navigation but no code has been written for it yet.

## Phases Completed This Session

### Phase S0 — Cross-cutting prerequisites (6/6 tasks, epic ral-7dy ✓)
- Clippy clean across workspace (5 warnings fixed)
- `cargo xtask ci` verified (fmt + clippy + test)
- **ArcSwap<StateSnapshotView>** in RallyService — snapshot reads O(1), no SQLite on reads
- IPC per-request 5s timeout (server: spawn_blocking + timeout, client: tokio::time::timeout)
- IPC max payload 1MB via LinesCodec::new_with_max_length (prevents OOM)
- `#![deny(unsafe_code)]` on all 13 crates

### Phase S1 — Rendering foundation (7/7 tasks, epic ral-cis ✓)
- **zellij_widgets 0.1.3** replaces AnsiBuf — standalone ratatui-like library for Zellij plugins
- All 3 widgets (WorkspaceTree, InboxSummary, StatusBar) ported to styled Span/Line
- PluginPane<stdout> for production, PluginPane<Vec<u8>> for tests
- Deleted: SidebarWidget trait, AnsiBuf, HandleCtx, Key, Handled
- Plugin reads `rally_cli_path` from layout config
- S1.5 (background worker) closed as won't-fix: Zellij worker API string-only, 3x overhead
- **Version gate fix**: state_version changed to Option<u64> (first snapshot at version 0 accepted)
- **Loading text fix**: "Loading state..." instead of misleading "Grant permission"

### Phase S2 — Agent context enrichment (9/9 tasks, epic ral-75w ✓)
- Added cwd, project_root, branch fields to Agent entity
- Schema migration v3 adds columns to agents table
- **Fixed migration logic**: `version < N` replaces broken `(N-1..N).contains(&version)` range checks
- Made v2 migration idempotent for canonical_key (column may exist in updated v1 schema)
- Git discovery via `git rev-parse` in rally-daemon/src/git.rs
- RegisterAgent IPC now accepts optional cwd; CLI passes current dir
- AgentView in proto includes new fields with skip_serializing_if
- Plugin AgentInfo deserializes new fields; branch shown in sidebar as `[branch]`
- S2.6/S2.7 (metadata HashMap/IPC) closed as deferred — store already has metadata round-trip

### Phase S3 — Reactive state push (7/7 tasks, epic ral-6g6 ✓)
- **pipe_push task** in daemon: subscribes to EventBus, 250ms trailing-edge debounce, serializes ArcSwap snapshot, pipes to all active Zellij sessions
- rally-host-zellij gets `pipe_to_plugin()` and `list_sessions()` utilities
- Plugin pipe() is now primary data path (was already handling it correctly)
- Plugin timer reduced from 5s to 30s heartbeat fallback
- Plugin subscribes to EventType::CwdChanged, forwards to daemon via `rally agent update-cwd`
- UpdateAgentCwd IPC handler: updates cwd, re-runs git discovery, emits event → triggers push
- S3.7 (CWD polling fallback) closed as won't-fix: CWD is immutable per agent session

### Bug Fixes
- Plugin version gate: `state_version: Option<u64>` fixes version-0 rejection
- Migration logic: `version < N` instead of range checks
- v2 migration idempotent for canonical_key

## Pending Work

### Immediate Next: Tree Navigation Epic (ral-5hsr)
The sidebar currently has flat agent-only navigation. The new epic rewrites it as a collapsible tree:

```
◆ api-service
├── Tab 1
│   ├── ● impl (cc) [main] p:1
│   └── ◐ review (cc) [main] p:2
◆ web-client
└── Tab 1
    └── ⧗ impl (cc) p:3
```

**8 sub-tasks created, dependency chain wired:**
- ral-i72z T1: TreeNode enum + collapsed state tracking
- ral-gknj T2: List widget rendering with tree connectors (depends on T1)
- ral-wdjn T3: j/k navigation through visible tree nodes (depends on T2)
- ral-9wnk T4: h/l expand/collapse tree nodes (depends on T3)
- ral-0ms6 T5: Wire focus action per node type (depends on T3)
- ral-ivcb T6: Subscribe to TabUpdate/PaneUpdate for tab+bare pane discovery (depends on T4)
- ral-o7sy Original navigation issue (folded into epic)
- ral-dz8d Focus action wiring (folded into epic)

**Key design decisions:**
- Uses zellij_widgets `List` widget with `ListState` for selection + scrolling
- Tree hierarchy: Workspace → Tab → Pane → Agent (maps 1:1 to Zellij Session → Tab → Pane)
- **No SQLite impact** — purely plugin-side rendering change
- PaneRef already stores (session_name, tab_index, pane_id) — the full tree coordinate
- Bare panes discovered via Zellij Event::TabUpdate + Event::PaneUpdate (T6)
- Session ↔ Workspace is 1:1 (`rally up` creates `rally-{workspace}` session names)

**Key caveat confirmed by advisor:**
- PaneInfo from Zellij does NOT include CWD — bare terminal CWDs need CwdChanged events
- Agents between RegisterAgent and BindPane have no pane_ref — need "Unbound" group
- Focus action differs per node type: workspace→attach session, tab→go-to-tab, pane→focus-pane-id

### Other Ready Work
- Phase S4 (ral-dfw): Visual state encoding — blocked on S3 (now unblocked)
- Phase 4.5 epic (ral-9rt): Review fixes, quality gates — P1 but mostly cleanup
- Logging tasks: ~10 P2 tasks, many may be stale after S1 rewrite

## Codebase Understanding

### New Files This Session
| File | Purpose |
|------|---------|
| crates/rally-daemon/src/git.rs | Git discovery: rev-parse for repo root + branch |
| crates/rally-daemon/src/pipe_push.rs | Daemon→plugin pipe push with 250ms debounce |

### Key Patterns Discovered This Session
- **zellij_widgets** (not ratatui): Standalone library, Widget trait, PluginPane<W: Write>, Frame, Buffer. API similar to ratatui but purpose-built for Zellij WASM plugins
- **Plugin version gate**: state_version must be Option<u64> because daemon EventBus starts at version 0 for pre-existing data
- **Migration ordering**: Use `version < N` not `(N-1..N).contains()` — range checks skip migrations for fresh DBs
- **CWD is immutable per agent session**: Set once at spawn, doesn't change. CwdChanged events are for Zellij shells, not rally agent runtimes
- **Zellij permissions**: Pre-grant via ~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl — dialog doesn't render in narrow panes

### Test Coverage
- 55 workspace tests + 14 plugin tests = 69 total, all passing
- CI: `cargo xtask ci` (fmt + clippy + test) green
- Golden snapshots: 12 insta file snapshots for sidebar rendering matrix

## Environment State
- Daemon: not currently running (was killed during testing)
- WASM plugin: deployed at ~/.config/rally/rally.wasm (S3 version)
- Zellij permissions: pre-granted for RunCommands + ReadApplicationState
- No active Zellij sessions

### How to Resume
```bash
# Start daemon
./target/debug/rallyd &

# Launch sidebar
PATH="$PWD/target/debug:$PATH" zellij -n layouts/sidebar-dev.kdl

# Start tree navigation work
bd ready  # ral-i72z (T1) should be first
bd update ral-i72z --claim
```
