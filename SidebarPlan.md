# Rally Sidebar Plan — Reactive, cmux-class Sidebar

> **Status**: Draft — awaiting feedback
> **Date**: 2025-04-25
> **Scope**: High-level phased plan; no implementation details

---

## 1. Competitive Landscape

What the three reference products do in their sidebars, and what Rally should
learn from each.

### 1.1 cmux (macOS native, libghostty)

| Feature | How it works |
|---|---|
| **Workspace metadata in sidebar** | Vertical tabs show git branch, PR status, CWD, listening ports, custom status |
| **`cmux set-status`** | Agents call a CLI to post status pills (label + icon + color) to sidebar |
| **`cmux set-progress`** | Agents post progress bars (0.0–1.0) rendered inline in sidebar |
| **`cmux log`** | Agents append severity-tagged log entries to sidebar history |
| **Notification rings** | Panes glow blue when attention needed; sidebar tabs show badges |
| **CWD tracking** | `--cwd` flag on workspace create; metadata surface from `CMUX_WORKSPACE_ID` env |
| **Agent control** | Socket API for create/destroy/focus/split/keystroke — full programmatic control |

**Key takeaway**: cmux has a *programmable metadata surface* — the sidebar is
not just a viewer, it is an API endpoint that agents and scripts push status
into.

### 1.2 Conductor (macOS app)

| Feature | How it works |
|---|---|
| **Agent identity bar** | Auto-detects runtime (Node, Python, Rust) and labels sidebar tabs |
| **Live state monitoring** | Bottom-panel indicators: thinking / working / idle / error |
| **Worktree isolation** | Sidebar manages agents in separate git worktrees |
| **Diff/PR review** | Dedicated area to inspect branch status and diffs before merge |
| **Parallel execution controls** | Spin up / monitor / terminate multiple agents from a central list |

**Key takeaway**: Conductor focuses on *identity and lifecycle* — the sidebar
answers "which agent is doing what in which worktree" at a glance.

### 1.3 Superset (superset.sh)

| Feature | How it works |
|---|---|
| **Unified workspace monitoring** | Central view of all running agent workspaces with status |
| **Visual status indicators** | Orange highlight for "needs attention"; distinct states for active/complete |
| **Built-in diff viewer** | Side-by-side syntax-highlighted comparisons before merge |
| **Workspace presets** | Save env configs to spin up standardized agent workspaces instantly |
| **One-click IDE handoff** | Open workspace in VS Code / Cursor / JetBrains from sidebar |

**Key takeaway**: Superset leans into *workflow presets and handoffs* — the
sidebar is a launchpad, not just a monitor.

### 1.4 Synthesis — What a "best-in-class" sidebar needs

```
┌───────────────────────────────────────────────────────────────────────┐
│  REACTIVE STATE     The sidebar reflects agent state within ~250ms    │
│  CWD / PROJECT      Shows where each agent is working, not just ID    │
│  AGENT-DRIVEN META  Agents push status, progress, logs to sidebar     │
│  VISUAL ENCODING    Color, icon, border, fill change with state       │
│  CONTROL PANEL      Focus, restart, fork, stop, permissions           │
│  GROUPED VIEWS      Group by project, branch, tag, state              │
│  NOTIFICATIONS      Badge, ring, or highlight for attention items     │
│  EXTENSIBLE         New sections without rebuilding the plugin        │
└───────────────────────────────────────────────────────────────────────┘
```

### 1.5 Current vs Target — Data Flow

**Current (v1): Plugin polls, renders hard-coded widgets**

```
┌───────────────────────────────────────────────────────────────────────┐
│ Zellij Session                                                        │
│                                                                       │
│  ┌────────────────────┐          ┌──────────────────────────────────┐ │
│  │ rally-plugin (WASM)│          │ agent panes (terminal)           │ │
│  │                    │          │  ┌───────┐ ┌───────┐ ┌───────┐   │ │
│  │  every 5s:         │          │  │ impl  │ │ tests │ │review │   │ │
│  │  run_command(      │          │  │       │ │       │ │       │   │ │
│  │   "rally",         │          │  └───────┘ └───────┘ └───────┘   │ │
│  │   "_plugin-state") │          │       ▲ no interaction from      │ │
│  │        │           │          │         sidebar                  │ │
│  │        ▼           │          └──────────────────────────────────┘ │
│  │  parse JSON        │                                               │
│  │  hard-coded render │                                               │
│  │  (AnsiBuf strings) │                                               │
│  └────────┬───────────┘                                               │
│           │ stdout ANSI                                               │
└───────────┼───────────────────────────────────────────────────────────┘
            ▼
      ┌──────────┐           ┌──────────────┐
      │ rallyd   │◀──────────│  rally CLI   │
      │ (daemon) │    IPC    │ _plugin-state│
      └──────────┘           └──────────────┘
```

**Target (v2): Daemon pushes, plugin renders + controls panes via Zellij CLI**

```
┌───────────────────────────────────────────────────────────────────────┐
│ Zellij Session                                                        │
│                                                                       │
│  ┌────────────────────┐          ┌──────────────────────────────────┐ │
│  │ rally-plugin (WASM)│          │ agent panes (terminal)           │ │
│  │                    │          │  ┌───────┐ ┌───────┐ ┌───────┐   │ │
│  │ on pipe message:   │─────────▶│  │🟢impl │ │🟡test │ │🔴rev  │   │ │
│  │  apply snapshot    │  zellij  │  │       │ │       │ │       │   │ │
│  │  render via Ratatui│  action  │  └───────┘ └───────┘ └───────┘   │ │
│  │                    │  calls:  │    ▲ set_pane_color              │ │
│  │ on keypress:       │   •focus │    ▲ rename_pane                 │ │
│  │  route action ─────┤   •rename│    ▲ focus_terminal_pane         │ │
│  │  via run_command() │   •color └──────────────────────────────────┘ │
│  │                    │   •close                                      │
│  └────────┬───────────┘                                               │
│           │ stdout ANSI                                               │
└───────────┼──────────────────────┬────────────────────────────────────┘
            ▼                      │
      ┌──────────┐                 │ zellij pipe --plugin (push)
      │ rallyd   │─────────────────┘
      │ (daemon) │  debounced at 4 Hz on state_version bump
      │          │◀──── hooks, capture, MCP, CLI ────
      └──────────┘
```

### 1.6 Zellij CLI as the Rendering/Control Surface

The sidebar plugin uses **Zellij's CLI and plugin API** to control agent panes.
The plugin itself is a WASM module that can only interact with the host through
Zellij's sanctioned commands. All agent pane control happens through these:

```
┌─────────────────── Sidebar Plugin (WASM) ───────────────────┐
│                                                             │
│  Rendering:     print ANSI/Ratatui to stdout                │
│                 (Zellij composites into plugin pane)        │
│                                                             │
│  Reading state: run_command(["rally", "_plugin-state"])     │
│                 pipe() handler for daemon push              │
│                                                             │
│  Controlling agent panes via Zellij SDK calls:              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ focus_terminal_pane(pane_id, client_id)               │  │
│  │ set_pane_color(pane_id, fg_hex, bg_hex)               │  │  ← NEW
│  │ rename_pane(pane_id, "⚠ impl-1")                      │  │  ← NEW
│  │ close_pane(pane_id)                                   │  │
│  │ switch_tab_to(tab_index)                              │  │
│  │ run_command(["rally", "agent", "restart", id])        │  │  ← NEW
│  │ run_command(["rally", "agent", "stop", id])           │  │  ← NEW
│  │ run_command(["rally", "agent", "spawn", ...])         │  │  ← NEW
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  NOT used:  direct PTY access, filesystem, network          │
│             (all gated behind Zellij permission system)     │
└─────────────────────────────────────────────────────────────┘
```

This means the plugin never calls `zellij action` from a shell — it uses the
in-process `zellij-tile` SDK equivalents. For operations that require the
rally daemon (restart, stop, spawn, metadata update), it shells out to the
`rally` CLI via `run_command()`, which talks to the daemon over the unix socket.

---

## 2. Zellij Plugin API — What Is and Isn't Possible

### 2.1 What the Zellij plugin SDK supports ✅

| Capability | API / mechanism |
|---|---|
| **Full ANSI rendering** | Plugin `render()` prints ANSI to stdout; Zellij composites it |
| **Ratatui integration** | `zellij_widgets` crate bridges Ratatui `Buffer` → Zellij pane |
| **Rich colors (fg/bg)** | ANSI 256-color and truecolor escape codes work inside the plugin pane |
| **`set_pane_color(pane_id, fg, bg)`** | Programmatically set fg/bg of *other* panes at runtime |
| **Subscribe to events** | `ModeUpdate`, `PaneUpdate`, `TabUpdate`, `Timer`, `Key` events |
| **`run_command()`** | Execute external commands from plugin (e.g., `rally` CLI calls) |
| **Background workers** | Offload heavy tasks (search, network) off the render thread |
| **`focus_terminal_pane(id)`** | Focus a specific pane from plugin code |
| **`rename_pane(id, name)`** | Rename pane titles dynamically |
| **`close_pane(id)`** | Close panes programmatically |
| **`switch_tab_to(idx)`** | Switch tabs from plugin |
| **Borderless mode** | Plugin pane can run borderless to feel like a native sidebar |
| **Pipe messages** | `zellij pipe --plugin` sends arbitrary payloads to the plugin |
| **Permission system** | Granular capability requests (RunCommands, ReadApplicationState, etc.) |

### 2.2 What Zellij does NOT support (hard limitations) ❌

| Limitation | Impact on Rally | Workaround |
|---|---|---|
| **No per-pane border *color*** | Cannot make a failing agent's pane border turn red | Sidebar renders colored status glyphs; `set_pane_color` changes interior bg/fg as a proxy |
| **No "notification ring" on panes** | Cannot glow/pulse a pane border like cmux does | Sidebar badge + `rename_pane` with emoji prefix (e.g., `⚠ impl-1`) + macOS `terminal-notifier` |
| **No arbitrary DOM / HTML** | Cannot embed rich widgets or canvas elements | Plugin renders TUI via ANSI / Ratatui; rich UIs stay external |
| **Single-threaded WASM** | Heavy render logic in plugin blocks UI | Offload to background workers; keep plugin as thin renderer |
| **No direct PTY read from plugin** | Plugin cannot read another pane's terminal buffer directly | Daemon captures via `dump-screen` or PTY ownership; pushes data to plugin via pipe |
| **No dynamic WASM loading** | Cannot load third-party sidebar widgets as WASM modules at runtime | Extensions register with daemon; daemon sends declarative view models; plugin renders them |
| **No native scroll / overflow** | Plugin pane does not auto-scroll; must manage scroll state manually | Implement virtual scroll in plugin code |
| **No mouse events (limited)** | Mouse handling is basic; no click-on-row in sidebar | Keyboard-driven selection (j/k/Enter); mouse support is best-effort |

### 2.3 Verdict: Is Zellij enough?

**Yes, for 90% of what cmux/Conductor/Superset do.** The missing 10% is:
- Per-pane border coloring (cmux has native control; Zellij themes are global)
- Notification rings (cmux is a native macOS app with AppKit drawing)

Rally's workarounds for these are sufficient for a terminal-native product:
- State-driven interior colors via `set_pane_color` API
- Pane title prefixing with state emoji
- macOS notifications via `terminal-notifier`
- TUI rendering via Ratatui in the plugin

**No additional UI library is needed.** The combination of Zellij's plugin API +
ANSI/Ratatui rendering + daemon-pushed state is the right stack.

---

## 3. Gap Analysis — Current Architecture vs. Desired Features

### Feature 1: Active CWD display

| Aspect | Current state | Gap |
|---|---|---|
| `rally agent spawn --cwd` | ✅ Passes `--cwd` to Zellij `new-pane` | — |
| CWD persisted on agent entity | ❌ Not stored in `rally-core::Agent` | **Must persist** |
| CWD in `AgentView` (proto) | ❌ Not in `AgentView` struct | **Must add field** |
| CWD in plugin `AgentInfo` | ❌ Not deserialized | **Must add field** |
| CWD rendered in sidebar | ❌ Not rendered | **Must add to widget** |

### Feature 2: Reactive CWD tracking on session switch

| Aspect | Current state | Gap |
|---|---|---|
| Detect CWD change in running pane | ❌ No mechanism | **Need:** capture-based CWD detection (parse OSC 7/`\e]7;…` escape, or `readlink /proc/<pid>/cwd`) |
| Update agent CWD in daemon | ❌ No CWD update path | **Need:** `UpdateAgentCwd` event or metadata update |
| Push updated CWD to sidebar | ✅ Existing snapshot push pipeline works if CWD is in the view | — (piggybacks on existing infra) |

### Feature 3: Sidebar polls agent session for status

| Aspect | Current state | Gap |
|---|---|---|
| Plugin polls daemon | ✅ `refresh_state()` calls `rally _plugin-state` | — |
| Poll interval | ⚠️ 5s timer — too slow for "reactive" | **Must reduce** to ~1–2s or switch to daemon-push |
| Daemon pushes state changes | ✅ `zellij pipe` mechanism exists | ⚠️ Only used on pipe events, not proactively pushed on every state bump |
| Agent state machine | ✅ Full state machine in `rally-core` | — |
| Hook-driven state updates | ❌ Phase 5 (not yet built) | **Blocked on Phase 5** `rally-hooks` |

### Feature 4: Agent status → sidebar visual style

| Aspect | Current state | Gap |
|---|---|---|
| State glyphs (`●◐◉○✕✗⧗`) | ✅ Rendered in `WorkspaceTree` | — |
| State-based row coloring | ❌ All rows same color | **Must add** ANSI color per state |
| State-based pane color | ❌ Never calls `set_pane_color` | **Must add** API call from plugin |
| Ratatui rendering | ❌ Currently raw ANSI string building | **Migrate** to Ratatui for richer styling |

### Feature 5: Sidebar as control panel

| Aspect | Current state | Gap |
|---|---|---|
| Focus action (`f` key) | ⚠️ Sets feedback message; does not actually focus | **Must call** `focus_terminal_pane` |
| Restart action (`r` key) | ⚠️ Sets feedback message; does not restart | **Must route** through daemon IPC |
| Stop action | ❌ Not bound | **Must add** key binding |
| Fork/clone session | ❌ No concept | **Must design** fork semantics (new agent, same CWD, new worktree) |
| Permission change | ❌ No concept | **Must design** what "permissions" means for agents |
| Spawn from sidebar | ⚠️ `s` key exists but is feedback-only | **Must wire** to daemon spawn flow |

### Forward-looking features (future scope — not MVP)

| Feature | Current support | Gap |
|---|---|---|
| **Progress bars** (cmux `set-progress`) | ❌ | Need agent metadata field + sidebar renderer |
| **Agent summary/status line** | ❌ | Need agent `summary` field from hooks/capture |
| **Git branch display** | ❌ | Need daemon-side git discovery or agent metadata |
| **Token/cost tracking** | ❌ | Need hook-driven token counter in agent metadata |
| **Group by project/cwd** | ❌ (groups by workspace only) | Need SidebarV2 grouping projection |
| **Drag-reorder / custom layout** | ❌ | Needs config-driven section ordering |
| **Agent log preview** | ❌ | Need capture tail in sidebar (truncated last N lines) |

---

## 4. What Is Definitely NOT Possible (and alternatives)

### 4.1 True per-pane border coloring
Zellij does not expose border color as a per-pane API. Themes are global.

**Alternative**: Use `set_pane_color(pane_id, fg, bg)` to tint the *interior*
of agent panes based on state (red bg tint for Failed, green for Running,
yellow for WaitingForInput). Combined with emoji-prefixed pane titles via
`rename_pane`.

### 4.2 Notification rings / pane glow
Zellij panes cannot glow or pulse. This is a native GUI feature.

**Alternative**: Three-pronged:
1. Sidebar badge with unread count + urgency coloring
2. `rename_pane` with state emoji prefix (visible in Zellij tab bar)
3. macOS `terminal-notifier` for desktop-level alerts (already in Phase 5)

### 4.3 Real-time per-keystroke CWD tracking
No terminal API provides per-keystroke CWD change notifications.

**Alternative**: Poll-based detection (every 1–2s) via:
- OSC 7 escape sequence parsing from capture stream (many shells emit this)
- `/proc/<pid>/cwd` readlink (Linux)
- `lsof -p <pid> | grep cwd` (macOS, slower)
- Or: agent hook emits CWD on tool use (Claude Code hooks already fire on directory changes)

### 4.4 Arbitrary third-party sidebar widgets (WASM hot-load)
Zellij does not support loading multiple WASM modules into one plugin pane.

**Alternative**: Rally's existing plan is correct — extensions register with
the daemon, which publishes declarative view models. The plugin renders them
using built-in renderers (`tree`, `list`, `summary`, `status_bar`). No WASM
hot-loading needed.

---

## 5. Phased Sidebar Roadmap

> Each phase has concrete deliverables that stack. Sub-item dependencies
> within each phase are documented. Hooks and MCP are **not** dependencies —
> they enrich the sidebar later but are not required for any phase to ship.

### Phase S0 — Cross-cutting prerequisites

> **Goal**: Fix foundational daemon and IPC issues that are not sidebar-specific
> but have direct cross-impact on sidebar quality and every downstream consumer
> (CLI, MCP, plugin). Doing these first means S1–S6 build on solid ground.

**Deliverables:**

```
S0.1  Fix all clippy warnings across workspace                     │
       cargo clippy --workspace --all-targets -- -D warnings        │
       └─ depends on: nothing                                       │
S0.2  Enforce quality gate: fmt → clippy → test                    │
       CI or pre-commit hook that gates on all three                 │
       └─ depends on: S0.1                                         │
S0.3  Snapshot as real projection — daemon-side foundation          │
       Move GetStateSnapshot from "query SQLite on demand" to       │
       ArcSwap<StateSnapshot> updated on domain event publish        │
       └─ depends on: nothing (rally-events ArcSwap already exists) │
S0.4  IPC per-request timeout                                      │
       Add 5s timeout on all CLI→daemon IPC calls so hung daemon    │
       doesn't freeze any consumer (CLI, plugin run_command, menu)   │
       └─ depends on: nothing                                       │
S0.5  IPC max payload size                                         │
       Add max frame size check on IPC socket reads                  │
       └─ depends on: nothing (parallel with S0.4)                  │
S0.6  #![deny(unsafe_code)] on all crates that don't already have it│
       └─ depends on: nothing                                       │
                                                                    │
Milestone: quality gate green, snapshot reads are O(1),             │
           IPC is hardened, Rust hygiene enforced everywhere         │
```

**Testing focus:**
- Unit: `ArcSwap<StateSnapshot>` is updated when `EventBus` publishes domain events
- Unit: IPC timeout fires after 5s on a stalled connection (mock slow handler)
- Unit: IPC rejects payload > max frame size with typed error
- Integration: `cargo clippy --workspace -- -D warnings` passes
- Integration: `GetStateSnapshot` IPC call returns in <10ms (reads ArcSwap, no SQLite)

**Codebase impact:**
| File / module | Impact | Detail |
|---|---|---|
| `crates/rally-events/src/lib.rs` | **Modify** | Wire `ArcSwap<StateSnapshot>` to update on every domain event publish |
| `crates/rally-daemon/src/services/agent.rs` | **Modify** | `GetStateSnapshot` reads `ArcSwap` instead of querying SQLite |
| `crates/rally-daemon/src/ipc/` | **Modify** | Add `tokio::time::timeout(5s)` wrapper around request handlers |
| `crates/rally-daemon/src/ipc/` | **Modify** | Add max frame size check on socket reads |
| All crates | **Modify** | `#![deny(unsafe_code)]` + clippy fixes |

**Critique.md items addressed:**
- **§5 (Snapshots not real projections)**: S0.3 is the foundational fix. Once the
  daemon maintains a live `ArcSwap<StateSnapshot>`, ALL consumers (sidebar plugin,
  CLI, MCP) get cheap O(1) reads. This unblocks S3's reactive push.
- **§4 (IPC framing)**: S0.4 and S0.5 add per-request timeout and max payload size.
  These protect every consumer, not just the sidebar's floating menu.
- **§8 (Clippy)**: S0.1 cleans up existing warnings. S0.2 prevents new ones.
- **§9 (Rust hygiene)**: S0.6 enforces `deny(unsafe_code)` everywhere.

**Why S0**: These are "pay once, benefit everywhere" fixes. Without S0.3, the S3
push mechanism would be pushing stale SQLite query results. Without S0.4, the S5
floating menu could hang on a slow daemon. Doing these first is cheaper than
patching them into each sidebar phase.

---

### Phase S1 — Rendering foundation (Ratatui + background workers)

> **Goal**: Replace the raw ANSI string builder with Ratatui via `zellij_widgets`.
> Move CLI calls and JSON parsing off the render thread.
> This is the foundation everything else builds on.

**Deliverables:**

```
S1.1  Add zellij_widgets + ratatui deps to rally-plugin  ─────────┐
S1.2  Implement RatatuiRenderer that replaces AnsiBuf             │
       └─ depends on: S1.1                                        │
S1.3  Port WorkspaceTree widget to Ratatui styled spans           │
       └─ depends on: S1.2                                        │
S1.4  Port InboxSummary + StatusBar to Ratatui                    │
       └─ depends on: S1.2                                        │
S1.5  Add background worker for run_command/pipe + JSON parse     │
       └─ depends on: nothing (parallel with S1.2-S1.4)           │
S1.6  Query session env vars (RALLY_SOCKET_PATH) at plugin load   │
       └─ depends on: nothing (parallel)                           │
S1.7  Golden snapshot tests updated for Ratatui output             │
       └─ depends on: S1.3, S1.4                                  │
                                                                   │
Milestone: sidebar renders identically via Ratatui, CLI calls     │
           are non-blocking                                        │
```

**Zellij 0.44 features adopted:**
- Background workers (move JSON parse off render thread)
- Query environment variables (dynamic plugin config)

**Why S1 first**: Ratatui changes the entire rendering surface. Doing it later
would mean rewriting every widget twice. Background workers prevent UI stalls
that would mask bugs in later phases.

**Note on `_attach` shim**: The `_attach` shim is NOT addressed in S1. It is
not just a pane ID retrieval mechanism — it bootstraps runtime context (env,
hooks, pane registration). Replacing it requires a proper launch-spec and
lifecycle design, which is tracked separately. `new-pane` already returns the
pane ID without `--blocking` (`--blocking` blocks until the pane *closes*, not
until it's created). Pane ID correlation for sidebar display already works via
the existing `BindPane` IPC.

**Testing focus:**
- Unit: `RatatuiRenderer` produces expected styled spans for known `AgentInfo` data
- Unit: `StateTheme` glyph/color lookups return correct values for each `AgentState`
- Unit: Background worker message passing (mock `run_command` → verify JSON parse result delivered)
- Integration: golden snapshot tests (S1.7) — render full sidebar with fixture data, compare against `.golden` files

**Codebase impact:**
| File / module | Impact | Detail |
|---|---|---|
| `crates/rally-plugin/src/widgets/mod.rs` | **Rewrite** | `SidebarWidget` trait changes from `fn render(&self, buf: &mut AnsiBuf)` to Ratatui `Widget` trait |
| `crates/rally-plugin/src/ansi_buf.rs` | **Delete** | Entire `AnsiBuf` module removed, replaced by Ratatui `Frame` |
| `crates/rally-plugin/src/widgets/workspace_tree.rs` | **Rewrite** | Port from `AnsiBuf::line()` calls to `ratatui::widgets::List` with styled items |
| `crates/rally-plugin/src/widgets/inbox_summary.rs` | **Rewrite** | Port to Ratatui styled text |
| `crates/rally-plugin/src/widgets/status_bar.rs` | **Rewrite** | Port to Ratatui layout |
| `crates/rally-plugin/src/main.rs` | **Modify** | `render()` switches from `print!()` to Ratatui `terminal.draw()`. Add background worker wiring |
| `crates/rally-plugin/tests/` | **Rewrite** | Golden snapshots regenerated for Ratatui output format |

**Critique.md incorporation:**
- **§8 (Clippy)**: Since plugin rendering is being rewritten from scratch, enforce
  `#![deny(clippy::all, clippy::pedantic)]` on `rally-plugin` from day 1. No clippy
  debt carried forward from the AnsiBuf era.
- **§9 (Rust hygiene)**: The new `RatatuiRenderer` and background worker use typed errors
  (not `anyhow`) at the plugin boundary. Add `#![deny(unsafe_code)]` to `rally-plugin`.

---

### Phase S2 — Agent context enrichment

> **Goal**: The daemon knows everything about each agent/session that the
> sidebar needs to display: CWD, project root, git branch.

**Deliverables:**

```
S2.1  Add `cwd: Option<PathBuf>` to rally-core Agent entity  ─────┐
S2.2  Add `project_root: Option<PathBuf>` to Agent                 │
       └─ depends on: S2.1                                         │
S2.3  Add `branch: Option<String>` to Agent                        │
       └─ depends on: S2.2                                         │
S2.4  Daemon-side git discovery: resolve repo root + branch        │
       from CWD at spawn time                                      │
       └─ depends on: S2.1                                         │
S2.5  Add cwd, project_root, branch to AgentView in rally-proto    │
       └─ depends on: S2.1, S2.2, S2.3                             │
S2.6  Add metadata: HashMap<String, Value> to AgentView            │
       └─ depends on: nothing (parallel, forward-looking field)     │
S2.7  Wire UpdateAgentMetadata IPC request                         │
       └─ depends on: S2.6                                         │
S2.8  Update plugin AgentInfo to deserialize new fields            │
       └─ depends on: S2.5                                         │
S2.9  Render CWD + branch in sidebar agent rows (Ratatui)          │
       └─ depends on: S1 (Ratatui), S2.8                           │
                                                                    │
Milestone: sidebar shows CWD + git branch for each agent/session   │
```

**Depends on**: Phase S1 (Ratatui rendering for S2.9)

**Testing focus:**
- Unit: `Agent` entity roundtrips cwd/project_root/branch through `Store` (SQLite)
- Unit: git discovery resolves known repo path → correct root + branch
- Unit: git discovery returns `None` for non-repo directory
- Unit: `AgentView` serialization includes new fields (serde round-trip test)
- Unit: `UpdateAgentMetadata` IPC request correctly merges metadata map
- Integration: spawn agent with `--cwd /tmp/repo` → `rally agent list --json` shows cwd + branch

**Codebase impact:**
| File / module | Impact | Detail |
|---|---|---|
| `crates/rally-core/src/agent.rs` | **Modify** | Add `cwd`, `project_root`, `branch` fields to `Agent` struct |
| `crates/rally-store/src/lib.rs` | **Modify** | Schema migration: add cwd/project_root/branch columns to agents table |
| `crates/rally-proto/src/v1/mod.rs` | **Modify** | Add fields + `metadata: HashMap` to `AgentView` and `AgentInfo` |
| `crates/rally-daemon/src/services/agent.rs` | **Modify** | Populate new fields at spawn time; add `UpdateAgentMetadata` handler |
| `crates/rally-plugin/src/state.rs` | **Modify** | Deserialize new `AgentInfo` fields from snapshot JSON |
| `crates/rally-plugin/src/widgets/workspace_tree.rs` | **Modify** | Render CWD + branch in agent rows |

**Critique.md incorporation:**
- **§6 (Event sourcing)**: New fields (`cwd`, `branch`) are mutable agent state. Ensure
  changes emit `AgentMetadataUpdated` domain events, keeping the event log useful
  for later replay/audit. Don't just silently mutate the table. These events also
  serve as the trigger for S3's reactive push — without them, the sidebar won't
  know when CWD/branch changes.
- *(§5 snapshot projection foundation is handled in S0.3)*

---

### Phase S3 — Reactive state push

> **Goal**: Sidebar updates within ~250ms of state change, not on a 5s timer.

**Deliverables:**

```
S3.1  Daemon EventBus subscriber that triggers pipe push on        │
       every state_version bump                                     │
       └─ depends on: rally-events crate (already built)            │
S3.2  Debounce pipe push to max 4 Hz (250ms floor)                 │
       └─ depends on: S3.1                                         │
S3.3  Plugin switches to push-primary: pipe() is main data path    │
       (pipe messages are routed to background worker for parse)    │
       └─ depends on: S3.1, S1.5                                   │
S3.4  Plugin timer reduced to 30s heartbeat fallback               │
       └─ depends on: S3.3                                         │
S3.5  Plugin subscribes to Event::CwdChanged(PaneId, PathBuf)      │
       Primary CWD tracking path — Zellij fires this when           │
       a terminal pane's CWD changes (via OSC 7)                    │
       → plugin forwards to daemon via IPC (UpdateAgentCwd)         │
       └─ depends on: S2.1 (cwd field exists)                       │
S3.6  CWD change received → update agent entity → re-run git       │
       discovery (branch/root) → bump version → auto-push          │
       └─ depends on: S3.5, S3.1                                   │
S3.7  (Optional) Daemon-side CWD polling fallback: every 5s,       │
       check /proc/pid/cwd or lsof -p on macOS for panes where     │
       Event::CwdChanged is not firing (SSH sessions, subshells     │
       that don't emit OSC 7)                                       │
       └─ depends on: S3.6 (same update path)                      │
                                                                    │
Milestone: sidebar reacts to state changes in <250ms,              │
           CWD updates are live via native Zellij events            │
```

**Depends on**: Phase S2 (CWD field for S3.5-S3.7)

**CWD tracking strategy**: Zellij's plugin API fires
`Event::CwdChanged(PaneId, PathBuf, Vec<ClientId>)` when a terminal pane's CWD
changes. This is the **primary path** — zero polling, instant, native. Daemon-side
polling (S3.7) is an **optional fallback** for edge cases where OSC 7 is not
emitted (SSH sessions, some subshells). Most agent runtimes (Claude Code, Codex,
etc.) run in shells that emit OSC 7 properly.

**Testing focus:**
- Unit: EventBus subscriber fires callback on `state_version` bump
- Unit: debounce logic — rapid bumps (10 in 50ms) result in ≤4 pipe pushes
- Unit: `Event::CwdChanged` handler forwards correct pane_id + path to daemon
- Unit: CWD fallback polling detects change from `/a` to `/b` and emits update
- Unit: CWD fallback polling handles dead process gracefully (no panic)
- Integration: modify agent state → verify plugin receives pipe message within 500ms
- Integration: plugin fallback — if pipe push stops, 30s heartbeat still refreshes sidebar

**Codebase impact:**
| File / module | Impact | Detail |
|---|---|---|
| `crates/rally-daemon/src/daemon.rs` | **Modify** | Add EventBus subscriber task that pushes to plugin via `zellij pipe` |
| `crates/rally-plugin/src/main.rs` | **Modify** | Subscribe to `Event::CwdChanged`, forward to daemon. `pipe()` becomes primary data handler; `timer()` reduced to 30s heartbeat |
| `crates/rally-daemon/src/services/agent.rs` | **Modify** | Add `UpdateAgentCwd` IPC handler. Optionally add fallback CWD polling task |
| `crates/rally-events/src/lib.rs` | **Modify** | Ensure `ArcSwap` snapshot updates trigger subscriber notifications |

**Critique.md incorporation:**
- **§5 (Snapshots not real projections)**: This is the phase where polling dies. The
  daemon must build a real in-memory projection (via `ArcSwap<StateSnapshot>`) that is
  updated on every domain event, then pushed to the plugin. The plugin never reconstructs
  state from SQLite — it consumes the pre-built projection.
- **§3 (Coarse locking)**: The pipe push path must NOT hold the `Store` Mutex. The push
  reads from the `ArcSwap` snapshot (lock-free), serializes, and sends.
- **§1 (Capture polling)**: The CWD fallback polling task (S3.7) should follow the
  "long-lived worker" pattern — one supervised task per agent, not a new process
  spawn per poll cycle. But the primary path (S3.5) avoids polling entirely.

---

### Phase S4 — Visual state encoding

> **Goal**: Sidebar rows and agent panes visually reflect state through
> hardcoded color/glyph mapping. Keep it simple.

**Deliverables:**

```
S4.1  Hardcoded StateTheme map in plugin code:                     │
       AgentState → { glyph, fg_color, bg_color }                  │
       └─ depends on: S1 (Ratatui for styled spans)                │
S4.2  Sidebar rows render with state-specific colors               │
       (green=running, yellow=waiting, red=failed, grey=idle)       │
       └─ depends on: S4.1                                         │
S4.3  Plugin calls set_pane_color(pane_id, fg, bg) on state change │
       └─ depends on: S4.1, pane_id available (already done)        │
S4.4  Plugin calls rename_pane(pane_id, "● impl-1") with emoji     │
       └─ depends on: S4.1                                         │
S4.5  Non-agent sessions (plain terminals) get a neutral style     │
       └─ depends on: S4.1                                         │
                                                                    │
Milestone: state is visually obvious at a glance — in sidebar      │
           AND on agent pane titles/colors                          │
```

**Depends on**: Phase S1 (Ratatui), Phase S3 (reactive push so colors update promptly)

**Note**: StateTheme is hardcoded for now. Config-driven theming is future scope.

**Testing focus:**
- Unit: `StateTheme` map returns correct glyph + colors for every `AgentState` variant
- Unit: non-agent sessions (plain terminals) get neutral fallback style
- Unit: `set_pane_color` call generated with correct hex values per state
- Unit: `rename_pane` call includes correct emoji prefix
- Integration: golden snapshot tests — sidebar with mixed agent states renders correct colors

**Codebase impact:**
| File / module | Impact | Detail |
|---|---|---|
| `crates/rally-plugin/src/theme.rs` | **New file** | `StateTheme` struct with hardcoded state→style map |
| `crates/rally-plugin/src/widgets/workspace_tree.rs` | **Modify** | Agent rows use `StateTheme` for styled rendering |
| `crates/rally-plugin/src/main.rs` | **Modify** | On state change, call `set_pane_color` + `rename_pane` via Zellij SDK |

---

### Phase S5 — Floating action window (control panel MVP)

> **Goal**: User presses Enter on a selected sidebar item → Zellij native
> floating pane opens with available actions. Works for both agent sessions
> and plain terminal sessions.

**Deliverables:**

```
S5.1  rally CLI: `rally pane menu <pane_id>` interactive TUI       │
       Shows contextual actions based on session type:               │
       • Agent session:  Focus | Restart | Stop | View logs         │
       • Terminal session: Restart shell (same CWD) | Focus         │
       └─ depends on: nothing (CLI-side, independent)                │
S5.2  Plugin: on Enter key, spawn floating pane via Zellij API     │
       running `rally pane menu <pane_id>`                          │
       └─ depends on: S5.1                                         │
S5.3  `rally pane menu` executes chosen action:                    │
       • Restart → rally agent restart <id> (agent) or              │
                   respawn shell in same cwd (terminal)              │
       • Stop → rally agent stop <id>                               │
       • Focus → runs `zellij action focus-terminal-pane <id>`      │
       └─ depends on: S5.1                                         │
S5.4  Floating pane auto-closes after action completes             │
       └─ depends on: S5.2                                         │
S5.5  Plugin: `f` key still does direct focus_terminal_pane()      │
       (shortcut, no floating window needed)                        │
       └─ depends on: nothing (instantaneous SDK call)              │
                                                                    │
Milestone: user can restart, stop, focus agents from sidebar       │
           via a clean floating action menu                         │
```

**Depends on**: Phase S1 (Ratatui + background workers), Phase S4 (visual
feedback after action completes)

**Architecture note**: The floating menu is intentionally simple — a `rally`
CLI TUI running in a Zellij floating pane. This avoids complex UI logic in the
WASM plugin. Future scope: action registry in config that declares available
actions per session type.

**Testing focus:**
- Unit: `rally pane menu` renders correct options for agent session type
- Unit: `rally pane menu` renders correct options for terminal session type
- Unit: Restart action calls `rally agent restart <id>` and exits cleanly
- Unit: Terminal restart spawns shell with same CWD
- Integration: Plugin spawns floating pane → menu renders → user selects → action executes → pane closes
- Integration: `f` key shortcut focuses pane without floating window

**Codebase impact:**
| File / module | Impact | Detail |
|---|---|---|
| `crates/rally-cli/src/commands/pane.rs` | **New file** | `rally pane menu <pane_id>` subcommand with interactive TUI |
| `crates/rally-cli/src/commands/mod.rs` | **Modify** | Register `pane` subcommand group |
| `crates/rally-plugin/src/main.rs` | **Modify** | Enter key handler spawns floating pane via `open_floating_pane()` |
| `crates/rally-daemon/src/services/agent.rs` | **Modify** | Ensure `restart` and `stop` IPC handlers exist and work end-to-end |

**Critique.md incorporation:**
- *(§4 IPC timeout is handled in S0.4 — the floating menu benefits from it
  automatically since all CLI→daemon calls are covered.)*

---

### Phase S6 — Plugin-side tree merge and configurable layout

> **Goal**: Formalize the plugin's hybrid tree construction and add user-facing
> configuration. The plugin merges two data sources client-side — daemon entities
> and Zellij session topology. No daemon-side projection needed.

**Design decision** (ral-7qo6): The daemon cannot compute tab-based grouping
because it never sees `TabUpdate`/`PaneUpdate` events. The plugin already merges
both sources in `visible_tree_nodes()`. S6 formalizes this.

```
Data ownership:

  ┌─────────────┐     raw entities     ┌──────────────────┐
  │   Daemon     │ ──────────────────→ │                  │
  │ workspaces   │   (pipe / snapshot)  │    Plugin        │
  │ agents       │                      │   TreeMerge      │
  │ inbox_items  │                      │                  │
  └─────────────┘                      │   merges both    │
                                        │   → builds tree  │
  ┌─────────────┐   Zellij events      │   → renders      │
  │   Zellij     │ ──────────────────→ │                  │
  │ TabUpdate    │   (plugin API)       │                  │
  │ PaneUpdate   │                      │                  │
  │ CwdChanged   │                      │                  │
  └─────────────┘                      └──────────────────┘

  Primary view: Tab → Pane/Agent (when TabUpdate fires)
  Fallback view: Workspace → Agent (daemon-only, before events arrive)
```

**Deliverables:**

```
S6.1  Extract TreeMerge module from RallyPlugin                    │
       Move visible_tree_nodes() into standalone, testable          │
       tree_merge.rs as a pure function:                            │
       fn build_tree(tabs, panes, workspaces, agents,               │
                     collapsed, filter) -> Vec<TreeNode>            │
       └─ depends on: nothing (pure refactor)                       │
S6.2  Daemon snapshot unchanged — raw entities only                 │
       Document: StateSnapshotResponse stays { version,             │
       workspaces, agents, inbox_items }. No sidebar field.         │
       CLI/MCP/tests get raw entities. Plugin does the merge.       │
       └─ depends on: nothing (documentation + guardrail)           │
S6.3  TreeMerge test suite                                         │
       Comprehensive unit tests for the extracted function:          │
       • Tab→Pane→Agent hierarchy with correct ordering             │
       • Agent overlays pane when agent.pane_id matches             │
       • Fallback to Workspace→Agent when no tabs                   │
       • Collapsed tabs hide children                               │
       • Filter affects agents but not bare panes                   │
       • Empty state (no tabs, no workspaces)                       │
       └─ depends on: S6.1                                         │
S6.4  Density mode: "compact" (glyph + name) vs "normal"           │
       (glyph + name + cwd + branch). Toggle with `d` key.         │
       └─ depends on: S6.1, S2 (context fields to display)          │
S6.5  config.jsonc schema for sidebar settings                     │
       sidebar.density, sidebar.show_bare_terminals,                │
       sidebar.default_collapsed — parsed at plugin load            │
       └─ depends on: S6.4                                         │
                                                                    │
Milestone: tree construction is formalized, tested, and             │
           configurable. No daemon-side projection needed.          │
```

**Depends on**: Phase S1 (Ratatui), Phase S2 (context fields for density mode)

**Note**: Grouping is automatic — Tab-based (primary) when Zellij events
are available, Workspace-based (fallback) when they're not. No GroupBy config
needed; the data source determines the hierarchy.

**Testing focus:**
- Unit: `build_tree()` produces correct Tab→Pane→Agent hierarchy from mixed inputs
- Unit: agent with `pane_id` matching a pane replaces that pane's node
- Unit: fallback to Workspace→Agent when `tabs` is empty
- Unit: collapsed set hides tab children; filter narrows agents
- Unit: density "compact" renders glyph + name only
- Unit: config.jsonc schema validates sidebar fields
- Integration: plugin renders tab tree with live Zellij events
- Integration: CLI `rally --json _plugin-state` returns raw entities (no regression)

**Codebase impact:**
| File / module | Impact | Detail |
|---|---|---|
| `crates/rally-plugin/src/tree_merge.rs` | **New file** | Standalone `build_tree()` pure function extracted from `visible_tree_nodes()` |
| `crates/rally-plugin/src/main.rs` | **Modify** | `visible_tree_nodes()` delegates to `tree_merge::build_tree()`. Add density toggle key. |
| `crates/rally-plugin/src/widgets/workspace_tree.rs` | **Modify** | Agent line rendering respects density mode |
| `crates/rally-config/src/lib.rs` | **Modify** | Add `SidebarConfig` struct with density/show_bare_terminals/default_collapsed |

**Critique.md incorporation:**
- ~~§5 (daemon-side projection)~~: No longer applies. The projection is plugin-side.
  The daemon's `ArcSwap<StateSnapshot>` from S0/S3 still serves raw entities efficiently.
- **§10 (Runtime spine)**: The plugin's `build_tree()` is the sidebar's projection function.
  It's a pure function (no IO, no state mutation) that runs on every render — easy to
  test, easy to reason about. No supervised daemon task needed for this.

---

### Future Scope (not in MVP)

Items that the architecture must support but are not implemented in the
initial sidebar build:

| Feature | Requires | Why deferred |
|---|---|---|
| Fork/clone session | Spawn with captured metadata from existing agent | Semantics need more design; architecture supports it (fork = capture → stop → respawn) |
| Progress bars | `metadata.progress` field + sidebar renderer | Needs hooks to populate; hardcode-able later |
| Agent summary/status line | `metadata.summary` from hooks/capture | Needs hooks |
| Token/cost tracking | Hook-driven counter | Needs hooks |
| Capture preview in sidebar | Scrollback reading (Zellij 0.44) | Complex; Ratatui scroll widget needed |
| Agent log panel | Scrollable sub-view in sidebar | Complex UI; deferred |
| Custom status pills | cmux-style `rally agent set` CLI | Needs MCP for agent-driven updates |
| Permission levels | Agent property affecting spawn flags | Design needed |
| Config-driven action registry | Declares actions per session type in config | Floating menu MVP is sufficient |
| `_attach` lifecycle redesign | Launch-spec + proper pane context bootstrapping | Not a rendering concern; needs spawn-flow design |
| Stacked/floating pane management | Zellij 0.42+ stacking API | Too complex for MVP |
| Viewport text highlighting | Zellij 0.44 highlight API | Nice-to-have |
| Config hot-reload | Zellij 0.44 config propagation | Simple to add post-MVP |

---

## 6. Architecture Support Assessment

### What the current architecture handles well ✅

1. **Event-sourced state** — Every state change is a `DomainEvent`. Sidebar
   projection is just another projection over the event stream. No new pattern
   needed.

2. **Plugin as dumb renderer** — The plan already says the plugin should be a
   thin renderer of daemon-published state. Ratatui (S1) doubles down on this.

3. **Pipe-based push** — `zellij pipe --plugin` is already wired. Moving from
   poll to push is a configuration change, not an architectural one.

4. **Pane correlation** — `BindPane` IPC already gives the daemon the `pane_id`
   for each agent. `new-pane` returns pane ID natively (no `--blocking` needed).

5. **Config-driven behavior** — `rally-config` with JSONC + JSON Schema
   already exists. Adding `sidebar.*` section is straightforward.

### What needs incremental improvement ⚠️

| Gap | Fix | Phase | Effort |
|---|---|---|---|
| Clippy warnings across workspace | Fix + enforce gate | **S0** | Small |
| Snapshot rebuilt from SQLite per request | ArcSwap projection | **S0** | Medium |
| No IPC timeout or payload limits | Per-request timeout + max frame | **S0** | Small |
| Plugin renders raw ANSI, not Ratatui | Migrate to Ratatui via `zellij_widgets` | **S1** | Medium |
| CLI calls block render thread | Background workers | **S1** | Medium |
| `_attach` lifecycle bootstraps too much | Needs launch-spec + lifecycle redesign | **Future** | Medium |
| `AgentView` too thin (no cwd/branch) | Add fields to proto + core | **S2** | Small |
| No agent CWD persistence | Add field to core `Agent` entity | **S2** | Small |
| Plugin polls at 5s | Daemon push + debounce | **S3** | Medium |
| Actions are feedback-only | Floating action window via `rally pane menu` | **S5** | Medium |
| Tree merge logic is inline in main.rs | Extract to testable `TreeMerge` module | **S6** | Small |

### What requires new architectural concepts 🔴

| Concept | Why it's new | Risk | Phase |
|---|---|---|---|
| **ArcSwap snapshot projection** | Replaces on-demand SQLite query with live projection | Low — ArcSwap already exists | **S0** |
| **Ratatui in plugin** | Different rendering approach; `zellij_widgets` dep | Low — well-documented | **S1** |
| **Daemon-side CWD polling** | No existing subsystem polls process state | Low — supervised task | **S3** |
| **Floating action menu** | New `rally pane menu` TUI + floating pane spawn | Low — simple TUI | **S5** |
| **Declarative sidebar view model** | SidebarV2 proposed but not built | Medium — schema design | **S6** |

---

## 7. Dependency Map

```
Phase S0 (cross-cutting: clippy, ArcSwap projection, IPC hardening)
         │
         ▼
Phase S1 (Ratatui + bg workers)
         │
         ├──────────────────────────────────┐
         ▼                                  │
Phase S2 (agent context: cwd, branch)       │
         │                                  │
         ├──────────────────┐               │
         ▼                  │               │
Phase S3 (reactive push)    │               │
         │                  │               │
         ▼                  │               │
Phase S4 (visual encoding)  │               │
         │                  │               │
         ▼                  ▼               │
Phase S5 (floating action   Phase S6        │
         window)            (grouped views) │
                            │               │
                            ▼               │
                      Future scope ◀────────┘

S0 is cross-cutting — benefits CLI, MCP, and plugin equally.
No external dependencies on hooks or MCP.
```

---

## 8. Zellij 0.44 Audit — Unused Native Capabilities

Rally's current plugin (`rally-plugin`) was written against an older Zellij API
surface. Zellij 0.42–0.44 added significant plugin capabilities that Rally
does not yet use. This section critiques the current implementation against
what is now natively available.

### 8.1 Capabilities Rally should adopt

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                         Zellij 0.44 Feature Audit                                │
├──────────────────────────────┬───────────┬───────────────────────────┬───────────┤
│ Feature                      │ Rally?    │ Impact if adopted         │ Phase     │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Background workers           │ ❌ unused  │ Move JSON parse to worker │ S1        │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Query session env vars       │ ❌ unused  │ Detect RALLY_WORKSPACE   │ S1        │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Event::CwdChanged            │ ❌ unused  │ Primary CWD tracking —   │ S3        │
│                              │           │ zero-poll, native push   │           │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ set_pane_color(id, fg, bg)   │ ❌ unused  │ State-driven pane tinting │ S4        │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ rename_pane(id, name)        │ ❌ unused  │ Emoji state prefix on     │ S4        │
│                              │           │ agent pane titles         │           │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Floating pane coordinates    │ ❌ unused  │ Spawn floating action menu│ S5        │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Read pane scrollback (ANSI)  │ ❌ unused  │ Replaces dump-screen poll │ Future    │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Viewport text highlighting   │ ❌ unused  │ Highlight errors/matches  │ Future    │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Config propagation to plugin │ ❌ unused  │ Hot-reload sidebar config │ Future    │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Stacked pane management      │ ❌ unused  │ Stack related agents      │ Future    │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Hide/show panes by ID        │ ❌ unused  │ Collapse agent panes      │ Future    │
├──────────────────────────────┼───────────┼───────────────────────────┼───────────┤
│ Open pane relative to plugin │ ❌ unused  │ Spawn next to sidebar     │ Future    │
└──────────────────────────────┴───────────┴───────────────────────────┴───────────┘
```

### 8.2 Critique of current implementation

**What it does well:**
- Clean separation of plugin from daemon (correct architectural choice)
- State version gating (avoids redundant re-renders)
- Pipe handler for daemon push (forward-looking, just not the primary path yet)
- Golden ANSI snapshot tests (good test coverage pattern)

**What it gets wrong / leaves on the table:**

1. **Polling instead of push-primary.**
   The plugin polls every 5s via `run_command`. Zellij 0.44's pipe mechanism
   is already wired in `fn pipe()` but isn't the primary data path. The daemon
   should push on every state bump; the timer should be a 30s heartbeat fallback.

2. **No background workers.**
   The `run_command` call and JSON parse happen synchronously. Zellij provides
   background workers specifically to avoid blocking the render thread. Large
   snapshots (50+ agents) will stall the UI.

3. **No pane interaction at all.**
   The plugin knows `pane_id` for each agent (it's in `AgentInfo`) but never
   calls `focus_terminal_pane`, `set_pane_color`, `rename_pane`, or `close_pane`.
   All sidebar actions (`f`, `r`, `s`, `a`) only set a `status_message` string.
   This is the single biggest gap vs. cmux/Conductor.

4. **Raw ANSI string building instead of Ratatui.**
   `AnsiBuf` is a `String` wrapper with a `line()` method. No layout engine,
   no styled spans, no reusable components. Ratatui via `zellij_widgets` would
   give: proper layout constraints, styled text spans, built-in list/table
   widgets, and automatic terminal capability handling.

5. **No env var query.**
   The plugin hard-codes `run_command(["rally", "_plugin-state"])`. With 0.44,
   it could query `RALLY_SOCKET_PATH` or `RALLY_WORKSPACE` from the session
   environment to configure itself dynamically.

6. **No config propagation.**
   If the user changes sidebar config (density, grouping, theme), the plugin
   must be restarted. Zellij 0.44 can propagate config changes to running
   plugins — Rally should use this for hot-reload.

7. **No scrollback reading.**
   Zellij 0.44 lets plugins read other panes' scrollback with ANSI styling.
   This could replace the `rally-capture` dump-screen polling for sidebar
   preview purposes (last N lines of agent output shown in sidebar).

8. **No floating/stacked pane use.**
   Rally spawns all agent panes flat. Zellij 0.42+ supports stacked panes
   (group related agents) and floating panes (detail overlays). Rally should
   use `open_pane_relative_to_plugin` to spawn agents next to the sidebar.

9. **`_attach` shim needs lifecycle redesign.**
   `new-pane` already returns pane IDs (no `--blocking` needed). However,
   `_attach` does more than capture pane IDs — it bootstraps runtime context,
   env, and hooks. Replacing it requires a proper launch-spec design, tracked
   separately from the sidebar roadmap.

### 8.3 Adoption priority (mapped to phases)

```
S1 — MUST HAVE (rendering foundation):
  ✦ Background workers for CLI calls + JSON parse     ← fixes UI stalls
  ✦ Query env vars at plugin load                     ← dynamic config

S3 — MUST HAVE (reactive push):
  ✦ Push-primary via pipe (stop 5s polling)            ← <250ms reactivity

S4 — MUST HAVE (visual encoding):
  ✦ set_pane_color on agent state change               ← state-driven panes
  ✦ rename_pane with state emoji                       ← visible in tab bar

S5 — MUST HAVE (control panel):
  ✦ Floating pane API for action menu                  ← `rally pane menu`
  ✦ focus_terminal_pane for quick focus                ← `f` key shortcut

Future scope:
  ✦ Config propagation for hot-reload
  ✦ Read pane scrollback for capture preview
  ✦ Stacked panes for related agents
  ✦ Viewport text highlighting for error markers
  ✦ open_pane_relative_to_plugin for sidebar-adjacent spawning
```

---

## 9. Phase Roadmap — Visual Overview

```
    ┌─────────────────────────────────────────────────────────────────┐
    │                      SIDEBAR ROADMAP                            │
    │                                                                 │
    │  ┌──────────────────────────────────────┐                       │
    │  │ S0: Cross-cutting Prerequisites     │                       │
    │  │  • Clippy + quality gate            │                       │
    │  │  • ArcSwap snapshot projection      │                       │
    │  │  • IPC timeout + max payload        │                       │
    │  │  • deny(unsafe_code) everywhere     │                       │
    │  └──────────┬───────────────────────────┘                       │
    │             │                                                   │
    │             ▼                                                   │
    │  ┌──────────────────────────────────────┐                       │
    │  │ S1: Rendering Foundation             │                       │
    │  │  • Ratatui via zellij_widgets        │                       │
    │  │  • Background workers                │                       │
    │  │  • Query session env vars            │                       │
    │  └──────────┬───────────────────────────┘                       │
    │             │                                                   │
    │             ▼                                                   │
    │  ┌──────────────────────────────────────┐                       │
    │  │ S2: Agent Context Enrichment         │                       │
    │  │  • cwd, project_root, branch         │                       │
    │  │  • metadata map on AgentView         │                       │
    │  │  • Render CWD + branch in sidebar    │                       │
    │  └──────────┬───────────┬───────────────┘                       │
    │             │           │                                       │
    │             ▼           │                                       │
    │  ┌─────────────────────┐│                                       │
    │  │ S3: Reactive Push   ││                                       │
    │  │  • daemon→plugin    ││                                       │
    │  │    pipe push @4Hz   ││                                       │
    │  │  • CWD polling      ││                                       │
    │  └──────────┬──────────┘│                                       │
    │             │           │                                       │
    │             ▼           │                                       │
    │  ┌─────────────────────┐│                                       │
    │  │ S4: Visual State    ││                                       │
    │  │  • set_pane_color   ││                                       │
    │  │  • rename_pane      ││                                       │
    │  │  • hardcoded theme  ││                                       │
    │  └──────────┬──────────┘│                                       │
    │             │           │                                       │
    │             ▼           ▼                                       │
    │  ┌─────────────────┐ ┌─────────────────────┐                    │
    │  │ S5: Floating    │ │ S6: Plugin-side     │                    │
    │  │  Action Window  │ │  Tree Merge         │                    │
    │  │ • rally pane    │ │  • extract build_   │                    │
    │  │   menu          │ │    tree() module    │                    │
    │  │ • restart/stop  │ │  • density modes    │                    │
    │  │ • focus shortcut│ │  • config.jsonc     │                    │
    │  └─────────────────┘ └──────────┬──────────┘                    │
    │                                 │                               │
    │                                 ▼                               │
    │                        ┌─────────────────┐                      │
    │                        │  Future Scope   │                      │
    │                        │ • fork/clone    │                      │
    │                        │ • progress bars │                      │
    │                        │ • action reg.   │                      │
    │                        │ • scrollback    │                      │
    │                        │ • stacked panes │                      │
    │                        └─────────────────┘                      │
    │                                                                 │
    │  No external deps on hooks/MCP.                                 │
    │  Hooks/MCP enrich later but are NOT blockers.                   │
    └─────────────────────────────────────────────────────────────────┘
```

### Floating action window flow (Phase S5 detail)

```
User presses Enter on selected sidebar item
    │
    ├── Is it an agent session?
    │     │
    │     ▼ YES
    │   ┌─────────────────────────────────────────┐
    │   │ Plugin spawns floating pane:             │
    │   │   open_command_pane_floating(            │
    │   │     "rally", ["pane", "menu", pane_id])  │
    │   │                                          │
    │   │   ┌────────────────────────────────────┐ │
    │   │   │ rally pane menu (floating TUI)     │ │
    │   │   │                                    │ │
    │   │   │   ▸ Focus pane                     │ │
    │   │   │     Restart agent                  │ │
    │   │   │     Stop agent                     │ │
    │   │   │     View logs                      │ │
    │   │   │                                    │ │
    │   │   │ Selection → executes action        │ │
    │   │   │ Floating pane auto-closes          │ │
    │   │   └────────────────────────────────────┘ │
    │   └─────────────────────────────────────────┘
    │
    └── Is it a plain terminal session?
          │
          ▼ YES
        ┌─────────────────────────────────────────┐
        │ Plugin spawns floating pane:             │
        │   open_command_pane_floating(            │
        │     "rally", ["pane", "menu", pane_id])  │
        │                                          │
        │   ┌────────────────────────────────────┐ │
        │   │ rally pane menu (floating TUI)     │ │
        │   │                                    │ │
        │   │   ▸ Focus pane                     │ │
        │   │     Restart shell (same CWD)       │ │
        │   │                                    │ │
        │   └────────────────────────────────────┘ │
        └─────────────────────────────────────────┘

User presses 'f' → direct focus (no floating window):
    │
    ▼
┌────────────────────────────────────┐
│ Plugin: handle_key('f')            │
│  pane_id = 12                      │
│  → focus_terminal_pane(12)         │ ← Zellij SDK, in-process
└────────────────────────────────────┘
```

---

## 10. Summary

| Question | Answer |
|---|---|
| Can Zellij do agent-status-driven pane colors? | **Yes** — via `set_pane_color(pane_id, fg, bg)` for interior; no border color |
| Is a UI library needed beyond Zellij? | **No** — Ratatui inside the plugin is sufficient |
| Does the current architecture support reactive sidebar? | **Mostly yes** — event bus + pipe push exist; need wiring + faster push |
| What's definitely impossible? | Per-pane border color, notification rings |
| Biggest gap? | Raw ANSI rendering, 5s polling, no pane interaction, no CWD persistence |
| Does Rally use Zellij 0.44 capabilities? | **No** — 12 major APIs available but unused (see §8) |
| What's the MVP control panel? | Floating action window via `rally pane menu` — works for agents AND terminals |
| Are hooks/MCP blockers? | **No** — they enrich the sidebar later but all phases ship without them |
| How many phases? | 7 phases: S0 (cross-cutting) + S1–S6 (sidebar-specific) + future scope |
| What does S0 buy? | Quality gate, O(1) snapshot reads, IPC hardening — benefits all consumers, not just sidebar |
