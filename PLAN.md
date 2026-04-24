# Rally — Terminal-Native Multi-Agent Orchestrator (Zellij + Rust)

> Name: **`rally`** — committed. Config paths: `~/.config/rally/`, socket:
> `$XDG_RUNTIME_DIR/rally/rally.sock`, env prefix: `RALLY_*`.

---

## 0. Context

### Why this is being built
The user lives in a terminal + Neovim + Claude Code workflow and wants cmux /
Conductor-style **parallel agent orchestration** without leaving the shell.
Existing options are either macOS-native GUIs (cmux), browser-first products
(Conductor, Superset), or ad-hoc tmux scripts. None provide:

- First-class **CLI** as the public integration surface (instead of GUI-only).
- A **structured capture API** rather than `tmux capture-pane` scraping.
- Reactive **inbox / attention routing** with durable state.
- **MCP tool surface** so agents can orchestrate *other* agents.

Zellij is picked as the host because (a) it has a real **plugin API** (WASM +
`zellij-tile`), (b) it has a **CLI pipe** protocol (0.40+) that is the right
primitive for a sidebar plugin talking to an external daemon, and (c) it has
supported "run command" plugin actions so plugins can shell out safely.

### Intended outcome
After Phase 4 the user can spawn Claude Code agents into labeled panes from the
shell and see them in a live sidebar with state badges. After Phase 6 external
automation + MCP is real. After Phase 7 the product feels "done" as a daily
driver and we switch to dogfooding: **subsequent phases are implemented using
rally to coordinate the agents that build rally** (self-hosting milestone).

---

## 1. Requirements Traceability

Each section from `multi_agent_zellij_requirements.md` → where it is addressed
in this plan:

| Req section | Addressed by |
|---|---|
| §6.1 Zellij plugin surface | Phase 7 (`rally-plugin`), §10 sidebar mockup |
| §6.2 Core orchestrator | Phase 1–2 (`rally-core`, `rally-store`) |
| §6.3 CLI surface | Phase 3 (`rally-cli`), §9 command tree |
| §6.4 Capture API | Phase 4 v1 (polling) → Phase 8 v2 (PTY ownership) |
| §6.5 Notifications / inbox | Phase 5 (`rally-inbox`), §11 inbox mockup |
| §6.6 Extension + integration | Phase 5 hooks, Phase 6 MCP, Phase 8 extensions |
| §7.1–7.3 Workspace/Agent/Pane models | Phase 1 domain model + §7 state machine |
| §7.4 Sidebar | Phase 7, §10 |
| §7.6 Capture modes (snapshot, window, stream, group) | Phase 4 + Phase 8 |
| §7.7 CLI design principles | §9 (predictable nouns/verbs, JSON out, stable IDs) |
| §7.8 Claude Code hooks | Phase 5 (`rally-hooks`) |
| §7.9 Pane modification by agents | Phase 5 (intent API, policy layer) |
| §7.10 MCP access | Phase 6 (`rally-mcp`, reuses core services) |
| §7.11 Extension model | Phase 8 (typed plugin SDK + event subscribers) |
| §8 Layer boundaries | §4 crate layout (hexagonal) |
| §9 Testability | §13 testing strategy (each phase has exit gate) |
| §10 Core entities | §5 data model |
| §11 Operational resilience | §12 failure model; supervisor + retries |
| §13 MVP scope | Phases 1–5 (= MVP), §14 phased breakdown |
| §15 Risks | §16 risk register |

---

## 2. Feasibility Analysis (one pass per feature)

The requirements are ambitious. Below is an honest feasibility verdict per
capability with mitigations for the thin spots.

| Capability | Feasibility | Notes / mitigation |
|---|---|---|
| Workspace + agent model (pure domain) | **Trivial** | Pure Rust, no host deps. |
| CLI surface (clap + JSON out) | **Trivial** | `clap v4` derive, `serde_json`. |
| Persistent daemon + IPC | **Easy** | `tokio` + `interprocess` unix socket + length-prefixed JSON/MsgPack frames. |
| Run as plugin in user's zellij | **Easy** | `zellij-tile` WASM plugin; rally installs/updates the `.wasm` + prints a KDL snippet. |
| Run as standalone zellij instance | **Easy** | `rally up` calls `zellij --session rally-<id> --layout <ship-ped.kdl>`; daemon supervises the child. |
| Spawn agent into a Zellij pane | **Easy** | `zellij action new-pane --cwd X -- <cmd>` (works in both modes; standalone passes `--session`). |
| Pane/agent correlation | **Medium** | Zellij doesn't expose stable pane IDs to external callers. Workaround: wrap agent command in `rally _attach <id>` which registers the shell PID → pane mapping via `ZELLIJ_PANE_ID` env var. (See §8.) |
| Sidebar plugin (WASM) | **Easy** | `zellij-tile` API is stable enough; render returns ANSI string. |
| Sidebar ↔ daemon comms | **Medium** | WASM plugin is sandboxed. Uses Zellij's `run_command` host call to invoke `rally` CLI, which talks to the daemon. Reverse direction uses `zellij pipe --plugin` from the daemon. |
| Structured capture (snapshot) | **Easy v1** | `zellij action dump-screen`. |
| Structured capture (stream / window) | **Medium v2** | Either poll `dump-screen` + diff, or own the PTY via `portable-pty` with the agent running inside our process tree (preferred). |
| Notifications + inbox (durable) | **Easy** | SQLite (`rusqlite` or `sqlx`) with WAL. |
| Attention routing (blocked / idle detection) | **Medium** | Pattern rules over capture stream + Claude Code hooks. `vte` crate to parse ANSI to cells for heuristics. |
| Claude Code hooks | **Easy** | Claude Code supports user-configured event hooks (PreTool / PostTool / Notification / Stop). Hook script = tiny shell wrapper that calls `rally agent emit …`. |
| Pane modification by agents (safe intent API) | **Medium** | Typed intent surface with policy gate; the daemon validates and translates to Zellij actions. |
| MCP server | **Easy** | `rmcp` official Rust SDK. Thin adapter over the same core services the CLI uses. |
| Worktree isolation | **Easy** | Delegates to `worktrunk` CLI (user's existing tool); rally stores the worktree path returned by worktrunk. |
| Blazing-fast text ingestion at scale | **Hard but tractable** | See §6 capture design — ring buffers on `bytes::Bytes`, lock-free snapshot via `arc-swap`, bounded broadcast for live subscribers, SQLite WAL only for summaries. |
| Reactive sidebar under load | **Medium** | Plugin must not re-render on every byte. Render-on-diff + coalesced "state version" counter pattern; see §10. |
| Resumable sessions | **Medium** | SQLite snapshot of workspace state + on-resume reconcile against live Zellij sessions. |
| Extensibility (without sprawl) | **Hard by nature** | Addressed by typed event bus + narrow plugin trait surfaces (see §15). |

**Bottom line**: everything in the requirements is feasible. The two design-sensitive
areas are (1) **pane ↔ agent correlation** under Zellij's external CLI, and (2)
**capture at scale**. Both have concrete approaches below.

---

## 3. Guiding Design Principles

1. **Hexagonal / ports-and-adapters.** `rally-core` has zero IO, zero async,
   zero Zellij. All side-effects live behind traits implemented by outer crates.
2. **Events are the source of truth.** Every state change is an immutable
   domain event; current state is a projection. Makes replay, audit, and
   plugin subscription trivial.
3. **Two-layer concurrency.** Hot path = lock-free (arc-swap snapshot, broadcast
   channels). Cold path = `tokio::sync::RwLock` on the command side.
4. **Wire protocol is versioned and stable.** `rally-proto` is a separate crate
   so the CLI, plugin, MCP, and hook scripts can evolve without breaking.
5. **Sync core, async edges.** The core state machine is plain Rust; only the
   daemon / IO adapters use `tokio`.
6. **Fail-soft.** A misbehaving hook, plugin, or MCP client must not crash the
   daemon; supervised tasks + bounded queues + timeouts everywhere.
7. **CLI is the lowest common denominator.** If a feature is not reachable
   from the CLI it does not exist. Plugin + MCP are *views* over the CLI surface.
8. **Zero `unwrap` / `expect` in library code.** Typed errors (`thiserror`)
   per crate, `anyhow` only at the binary boundary.

---

## 3.5 Deployment Modes (important)

Rally must run in **both** of the following modes. The design is shaped so the
daemon + core don't care which one is active — only the Zellij adapter does.

### Mode A — Plugin-in-existing-zellij
The user is already living in their own Zellij session. They install the
sidebar plugin (or invoke `rally up` which registers it) and the daemon
autostarts. Rally commands target the running Zellij via its standard CLI.

```
 ┌─────────────── existing zellij session (user-managed) ─────────────────┐
 │  ┌─tab: work─────────────────────────────┐ ┌─tab: rally───────────┐    │
 │  │ agent pane │ agent pane │ agent pane  │ │ sidebar plugin pane  │    │
 │  │  (zellij)  │  (zellij)  │  (zellij)   │ │ (rally-plugin.wasm)  │    │
 │  └────────────┴────────────┴─────────────┘ └──────────────────────┘    │
 └──────────┬──────────────────────────┬────────────────────────┬─────────┘
            │ zellij action …          │  zellij pipe --plugin  │ CLI from
            ▼                          ▼                        │ anywhere
     ┌────────────────────────────────────────────────────────┐ │
     │                   rally-daemon (user scope)             │◀┘
     └────────────────────────────────────────────────────────┘
```

### Mode B — Standalone managed zellij
The user types `rally up` (or `rally workspace open <WS>`). Rally spawns and
owns a **dedicated Zellij session** with a specific session name (`rally-<WS>`)
and a rally-specific layout KDL that has the sidebar plugin pinned in place.
This gives users a zero-config out-of-the-box experience identical to cmux's
"open an app".

```
   $ rally up api-service
          │
          ▼
   rally-daemon ──spawns──▶  zellij attach -c rally-api-service \
                                   --layout <ship-ped KDL>
                                        │
                                        ▼
              ┌──────────────────────── rally-owned session ─────────┐
              │ layout pre-loaded with sidebar pinned left,          │
              │ agent panes spawned on demand to the right           │
              └─────────────────────────────────────────────────────┘
```

### What differs between the modes

| Concern | Plugin mode | Standalone mode |
|---|---|---|
| Zellij lifecycle | User-owned | Rally-owned (`rally up` / `rally down`) |
| Session discovery | Sidebar lives in whatever session it was launched from; rally stores `ZELLIJ_SESSION_NAME` on first contact | Deterministic session name `rally-<workspace-id>` |
| Layout | User installs plugin; rally provides an optional KDL snippet to import | Rally ships a complete layout; user never edits KDL |
| Crash recovery | If user quits zellij, rally state persists; `rally up` can reopen | Rally supervises the zellij process; on unexpected exit, it reports and offers auto-respawn |
| Pane spawning | `zellij action new-pane` against the active session | `zellij action new-pane --session rally-<WS>` explicit |
| Sidebar distribution | User installs plugin binary at a known path; rally writes/manages it | Same binary bundled into rally's share dir |

### Design implication — ONE adapter, two bootstrappers
`rally-host-zellij` stays a single adapter that takes a session handle.
Above it sit two **thin** bootstrappers:

```
 rally-host-zellij::SessionHandle { session_name, detected_via, owned: bool }
          ▲                                    ▲
          │                                    │
 ┌─────────────────┐                 ┌───────────────────┐
 │ PluginBootstrap │                 │ StandaloneBootstrap│
 │ detects via env │                 │ spawns zellij with │
 │ ZELLIJ_SESSION  │                 │ rally layout KDL   │
 └─────────────────┘                 └───────────────────┘
```

This is the only place "plugin vs standalone" leaks. Everything above it is
identical. That keeps the code path for both modes short and test-shared.

### Two new CLI verbs

- `rally up [<workspace>]` — standalone: spawns zellij with rally layout.
  Idempotent: if already up, attaches.
- `rally down [<workspace>]` — graceful teardown of a standalone session.
- `rally install-plugin` — plugin mode: copies the WASM, prints a KDL snippet
  to add to the user's layouts, offers to patch their config.
- `rally layout export` — emits the KDL for the current workspace so power
  users can hand-edit.

### Feasibility notes
Both modes use only public Zellij CLI surfaces:
- `zellij --session <name>` / `zellij attach -c <name>` — creating or attaching
  a session is standard.
- `--layout <path-or-kdl>` — Zellij accepts layout files via CLI.
- Plugins can be referenced in a layout with `plugin location="file:..."`.
- `zellij pipe --plugin file:<path>` — routes messages to the plugin.

No private APIs, no patches to zellij. This is intentionally boring.

### Bundled layout (shipped, also overrideable)

```
 // $XDG_DATA_HOME/rally/layouts/default.kdl
 layout {
   pane size=28 borderless=true {
     plugin location="file:~/.local/share/rally/plugins/rally-sidebar.wasm" {
       daemon_socket "/run/user/1000/rally/rally.sock"
     }
   }
   pane split_direction="vertical" {
     // agent panes appear here on demand
   }
   tab_template {
     default_tab_template
   }
 }
```

---

## 4. Top-Level Architecture

```
                              ┌─────────────────────────────────────────┐
                              │                USER                      │
                              │  shell / nvim / Claude Code / Raycast    │
                              └──┬──────────────┬───────────────┬───────┘
                                 │              │               │
                   CLI invocations│    keybind   │    MCP tool   │   hook script
                                 ▼              ▼    calls       ▼
     ┌──────────┐   clap    ┌─────────────────────────────────────────┐
     │  rally   │──────────▶│                rally-cli                 │
     │  (bin)   │◀──────────│   output: human TTY  /  --json / --ndjson│
     └────┬─────┘           └───────────────────┬──────────────────────┘
          │                                     │  unix socket (length-prefixed MsgPack)
          ▼                                     ▼
┌───────────────────────────────────────────────────────────────────────┐
│                           rally-daemon                                 │
│  ┌─────────────┐   commands    ┌─────────────────────────────────┐    │
│  │  IPC server │──────────────▶│       Core Service Layer         │    │
│  │ (unix sock) │               │  workspace / agent / pane / …    │    │
│  └─────┬───────┘◀──────────────└──────────────┬──────────────────┘    │
│        │       events                         ▼                        │
│  ┌─────▼───────┐       ┌────────────────────────────────────────────┐ │
│  │  Event bus  │◀─────▶│          rally-core  (pure domain)          │ │
│  │ (broadcast) │       │  entities · state-machine · projections     │ │
│  └─────┬───────┘       └────────────────────────────────────────────┘ │
│        │                               │                               │
│        │ subscribe                     ▼  repo port                    │
│        ▼                      ┌──────────────────────┐                 │
│  ┌─────────────┐              │   rally-store        │                 │
│  │ Subsystems  │              │  SQLite (WAL) +      │                 │
│  │  · inbox    │              │  in-memory caches    │                 │
│  │  · capture  │              └──────────────────────┘                 │
│  │  · hooks    │                                                       │
│  │  · mcp      │         ┌───────────────────────────────────┐         │
│  │  · zellij   │────────▶│   rally-host-zellij (adapter)     │         │
│  │    host     │         │  · zellij action new-pane         │         │
│  │             │         │  · zellij pipe --plugin rally     │         │
│  │             │         │  · zellij action dump-screen      │         │
│  │             │         │  · PTY owner (phase 8)            │         │
│  └─────────────┘         └──────────────────┬────────────────┘         │
└──────────────────────────────────────┬──────┴─────────────────────────┘
                                       │
                  ┌────────────────────┼─────────────────────┐
                  ▼                    ▼                     ▼
         ┌─────────────────┐    ┌────────────┐       ┌───────────────┐
         │  zellij session │    │  zellij    │       │ claude-code   │
         │  (terminal)     │    │  plugin    │──────▶│ hook scripts  │
         │   panes ×N      │◀───│  sidebar   │       │  (shell)      │
         └─────────────────┘    │  (WASM)    │       └───────────────┘
                                └────────────┘
```

### Why a daemon (and not just a library)?

| Concern | Why daemon wins |
|---|---|
| State outlives zellij session | Sidebar sessions die/resume; daemon is the truth store. |
| Many clients (CLI, plugin, MCP, hooks, Raycast) | All talk to one place. No double-writer race. |
| Event fan-out | One bus, cheap broadcast subscribers. Plugin can't host this safely. |
| PTY ownership later | Daemon must own file descriptors that plugins/CLIs can't. |
| Crash isolation | A bad hook kills its task, not the state. |

The daemon binds a unix socket at `${XDG_RUNTIME_DIR:-/tmp}/rally/rally.sock`
and is spawned on-demand by the CLI (first call autostarts it, same pattern
as `dockerd`, `nvim --listen`, etc.).

---

## 5. Crate Layout (Cargo Workspace)

```
rally/
├── Cargo.toml                      # [workspace] with resolver = "2"
├── rust-toolchain.toml             # pin stable
├── deny.toml                       # cargo-deny policy
├── .cargo/config.toml              # target dir, rustflags
├── crates/
│   ├── rally-core/                 # domain model. no IO. no async. no tokio.  [ral-np3 ✓]
│   │   ├── src/
│   │   │   ├── ids.rs              # WorkspaceId, AgentId, PaneId (ulid)       [ral-np3.1 ✓]
│   │   │   ├── workspace.rs
│   │   │   ├── agent.rs            # state machine                             [ral-np3.2 ✓, ral-np3.3 ✓]
│   │   │   ├── pane.rs
│   │   │   ├── event.rs            # domain events (enum, #[non_exhaustive])   [ral-np3.6 ✓]
│   │   │   ├── inbox.rs
│   │   │   ├── capture.rs          # Capture* types (not IO)                   [ral-np3.5 ✓]
│   │   │   ├── policy.rs           # intent validation                         [ral-np3.5 ✓]
│   │   │   └── ports.rs            # repo + clock + id-gen traits              [ral-np3.4 ✓]
│   │   └── tests/                  # property tests on state machine           [ral-np3.9 ✓]
│   ├── rally-proto/                # wire types. serde. versioned.             [ral-np3.7 ✓]
│   │   ├── src/
│   │   │   ├── v1/                 # rpc requests, responses, events
│   │   │   └── lib.rs
│   ├── rally-store/                # SQLite adapter implementing ports.rs      [ral-bku ✓]
│   ├── rally-events/               # tokio broadcast-bus wrapper + watch snapshots  [ral-fq5.10]
│   ├── rally-capture/              # CaptureSource trait + impls (dump-screen, PTY) [ral-khz ✓]
│   ├── rally-host-zellij/          # zellij action / pipe adapters               [ral-ufa ✓, ral-s4p ✓, ral-mxk ✓]
│   ├── rally-hooks/                # claude-code hook runner + schemas            [ral-qdw]
│   ├── rally-inbox/                # rule engine over event stream → InboxItems   [ral-kj1]
│   ├── rally-config/               # JSONC config parsing, schema gen, config doctor  [ral-fq5.2, ral-fq5.3]
│   ├── rally-daemon/               # [[bin]] rallyd                            [ral-fq5.1, ral-fq5.4, ral-fq5.5]
│   ├── rally-cli/                  # [[bin]] rally  (the user-facing binary)   [ral-fq5.6, ral-fq5.7]
│   ├── rally-mcp/                  # [[bin]] rally-mcp  (MCP stdio/HTTP/streamable server
│   ├── rally-plugin/               # cdylib → wasm32-wasi, zellij plugin         [ral-l4t ✓]
│   └── rally-test-utils/           # fake clock, in-memory repo, ipc harness  [ral-np3.8 ✓, ral-fq5.8]
└── xtask/                          # cargo xtask for build/release workflows   [ral-7lu.1 ✓]
```

### Dependency direction (must never cycle)

```
       rally-plugin ──┐
                      ▼
 rally-cli ──▶ rally-proto ──▶ rally-core
                      ▲
 rally-mcp ───────────┤
                      │
 rally-daemon ────────┼──▶ rally-store ─▶ rally-core
                      ├──▶ rally-events ─▶ rally-core
                      ├──▶ rally-capture ─▶ rally-core
                      ├──▶ rally-hooks ──▶ rally-core
                      ├──▶ rally-inbox ──▶ rally-core
                      └──▶ rally-host-zellij ─▶ rally-core
```

`rally-core` depends on **nothing** in this workspace. It is reusable and
testable in isolation, per §8 of the requirements.

### 5.1 Module Responsibility Contract (one-to-one mapping to requirements §8)

Each requirement layer maps to exactly one crate, each crate owns exactly
one kind of decision, and the "allowed deps" column is enforced by
`cargo deny`'s `[bans]` list — a crate that shouldn't depend on another
**cannot compile** if someone forgets.

| Requirements layer (§8) | Crate | Owns (the single decision) | Must NOT own | Allowed deps |
|---|---|---|---|---|
| 8.1 Core domain | `rally-core` `[ral-np3 ✓]` | Entities, state machine, policy, events, projections | IO, async, Zellij, SQLite, serde wire | — |
| (wire contract) | `rally-proto` `[ral-np3.7 ✓]` | On-wire request/response/event types, versioning | Business rules | `rally-core` (only for id types) |
| (durable store) | `rally-store` `[ral-bku ✓]` | Event log + projection persistence | Business rules, host knowledge | `rally-core`, `rusqlite` |
| (event bus) | `rally-events` `[ral-fq5.10]` | Fan-out topology, backpressure, subscriptions | Persistence, domain logic | `rally-core`, `tokio` |
| 8.2 Host integration | `rally-host-zellij` `[ral-ufa ✓, ral-s4p ✓, ral-mxk ✓, ral-821 ✓, ral-vzk ✓]` | Translating domain intents ↔ Zellij actions/pipes | Business rules, inbox rules | `rally-core`, `rally-proto`, process spawn |
| 8.5 Capture | `rally-capture` `[ral-khz ✓]` | Source traits, ring buffer, subscription semantics | Rendering, inbox rules | `rally-core`, `bytes`, `vte` |
| 8.6 Integration (hooks) | `rally-hooks` `[ral-qdw]` | Hook schema, hook→domain-event normalization | Sidebar, Zellij specifics | `rally-core`, `rally-proto` |
| 8.6 Integration (MCP) | `rally-mcp` `[ral-5q5, ral-8qj, ral-9j7, ral-cce, ral-mtp, ral-nkd]` | Mapping MCP tools → core services | Owning state, new business rules | `rally-proto`, `rmcp` |
| 8.7 Notifications | `rally-inbox` `[ral-kj1]` | Event→InboxItem rule engine, urgency | Rendering | `rally-core`, `rally-events` |
| 8.3 CLI | `rally-cli` `[ral-fq5.6, ral-fq5.7]` | Command parsing, output format, selection DSL | Business rules, Zellij calls | `rally-proto`, `rally-config`, IPC client |
| (config) | `rally-config` `[ral-fq5.2, ral-fq5.3]` | JSONC parsing, schema gen, config doctor, layer merge | Business rules, IO, Zellij | `serde_json`, `json_comments`, `schemars`, `jsonschema` |
| 8.4 Plugin UI | `rally-plugin` | Rendering a snapshot, keyboard handling | Source of truth | `zellij-tile`, `rally-proto` (read-only) |
| (orchestrator) | `rally-daemon` `[ral-fq5.1, ral-fq5.4, ral-fq5.5]` | Wiring, supervision, IPC entry, services | Any business rule that belongs to core | all of the above |

### 5.2 The three contracts that connect the modules

Everything crosses a module boundary through exactly one of three typed
contracts — so new modules slot in without edits elsewhere:

```
     ┌───────────────────────────────────────────────────────────────┐
     │                                                               │
     │   Contract A: ports::* traits  (rally-core)                   │
     │   ───────────────────────────────                             │
     │   pub trait WorkspaceRepo { … }                               │
     │   pub trait Clock         { … }                               │
     │   pub trait HostSession   { … }    ◀── host-zellij impls this │
     │   pub trait CaptureSource { … }    ◀── rally-capture impls    │
     │                                                               │
     │   → lets us swap SQLite / in-memory, Zellij / fake-host,      │
     │     dump-screen / PTY, and test core without any IO           │
     │                                                               │
     ├───────────────────────────────────────────────────────────────┤
     │                                                               │
     │   Contract B: DomainEvent  (rally-core::event)                │
     │   ───────────────────────────────                             │
     │   #[non_exhaustive] pub enum DomainEvent { … }                │
     │                                                               │
     │   → inbox rules, capture detectors, MCP notifications,        │
     │     external extensions all subscribe to this ONE stream      │
     │                                                               │
     ├───────────────────────────────────────────────────────────────┤
     │                                                               │
     │   Contract C: proto v1  (rally-proto)                         │
     │   ───────────────────────────────                             │
     │   Request / Response / EventEnvelope MsgPack schema           │
     │                                                               │
     │   → CLI, plugin, MCP, hook scripts, external SDKs, and        │
     │     even other languages bind through this.                   │
     │                                                               │
     └───────────────────────────────────────────────────────────────┘
```

### 5.3 Dependency firewall (CI-enforced)

`deny.toml`:

```toml
[bans]
multiple-versions = "deny"

[[bans.deny]]
name = "rusqlite"
wrappers = ["rally-store"]   # no other crate may depend on sqlite

[[bans.deny]]
name = "tokio"
wrappers = [                  # core and proto are tokio-free
  "rally-events", "rally-capture", "rally-host-zellij",
  "rally-hooks",  "rally-inbox",   "rally-daemon",
  "rally-cli",    "rally-mcp"
]

[[bans.deny]]
name = "zellij-tile"
wrappers = ["rally-plugin"]   # only the plugin crate knows zellij-tile

[[bans.deny]]
name = "rmcp"
wrappers = ["rally-mcp"]      # MCP crate is the only one that knows MCP
```

This is the part that turns "clean separation" from a comment in a wiki into
a compile-time invariant. Forgetting the rule = CI red.

### 5.4 Extension shape follows the same contracts

External extensions (phase 8+) don't get a new surface. They subscribe via
Contract B (domain events) over Contract C (proto wire) and call back in
through the same services the CLI uses. That means:

- A new "Slack notifier" is a 200-line binary that reads events and posts.
- A new "cargo-watch detector" is a subscriber that emits `AttentionRequired`.
- A new "Raycast extension" is just a CLI script.

None of them fork rally's core or need plugin-level coupling.

---

## 6. Rust Tooling & Library Choices

Blazing-fast + memory-efficient + great at high-volume text rendering guided
every pick. Justifications in the right column.

| Concern | Pick | Why |
|---|---|---|
| Async runtime | **`tokio`** (multi-thread) | Mature, zero-cost `Send` futures, first-class `broadcast` / `watch` / `mpsc`. |
| CLI parsing | **`clap` v4 derive** | Stable, subcommand friendly, shell completions gratis. |
| Serialization (public JSON) | **`serde` + `serde_json`** | Standard; required for `--json` output. |
| Serialization (IPC wire) | **`serde` + `rmp-serde`** (MessagePack) | ~3–5× smaller than JSON, ~5× faster, zero schema pain. |
| Wire framing | **`tokio-util::codec` LengthDelimitedCodec** | Well-tested, trivial, composes with tokio AsyncRead/Write. |
| Unix socket / IPC | **`interprocess`** (local-socket) | Cross-platform, uniform API (we'll also run on Linux). |
| Persistence | **`rusqlite`** + `r2d2_sqlite` pool, WAL | Small, embedded, zero ops. Consider `sqlx` if we later want async migrations; not on the hot path. |
| Domain ids | **`ulid`** (monotonic, sortable) | 26-char base32, sorts by time — perfect for event logs. |
| Event bus | `tokio::sync::broadcast` + **`arc-swap`** for latest-state snapshots | Broadcast for live tail; arc-swap for lock-free "read current snapshot" path hit by the plugin. |
| Zero-copy byte buffers | **`bytes::Bytes` / `BytesMut`** | Ref-counted, slicable without copy — ideal for streaming PTY output to N subscribers. |
| Ring buffer for pane output | Custom `LineIndexedRing` over `BytesMut` + `Vec<LineSpan>` | Fixed memory ceiling per agent; O(1) append; O(log N) "last N lines" lookups. |
| ANSI parsing (for state detection) | **`vte`** | Xterm-correct, no allocs on fast path. |
| PTY ownership (phase 8) | **`portable-pty`** | Cross-platform, battle-tested by wezterm. |
| MCP server | **`rmcp`** (official Anthropic Rust SDK) | First-party; same surface as CLI via shared service trait. |
| HTTP (if MCP-over-HTTP) | `axum` | Default stack, minimal cost. |
| Structured logging | **`tracing`** + `tracing-subscriber` + `tracing-appender` | Async-friendly, span-based, works under tokio. `[ral-hzg ✓, ral-60v ✓, ral-c11 ✓, ral-kmc ✓, ral-gct ✓, ral-6ml ✓, ral-55d ✓]` |
| Error types | **`thiserror`** in libs, `anyhow` at bin edges | Typed recovery vs ergonomic top-level. |
| Process supervision | **`tokio::task` JoinSet** + custom `Supervisor` | Named tasks, restart policies, panic capture. |
| Command spawning | **`tokio::process::Command`** | Non-blocking, `CommandExt` for extras. |
| Testing (CLI) | `assert_cmd` + `predicates` + `insta` | Command integration + snapshot. |
| Testing (property) | `proptest` | State-machine invariants. |
| Testing (mocks) | `mockall` for port traits | Clean, generates stubs. |
| Benchmarks | `criterion` | Capture ingest + render path. |
| Lint / formatting | `rustfmt`, `clippy -D warnings`, `cargo-deny`, `cargo-audit`, `cargo-machete` | In CI. |
| Plugin build | **`zellij-tile`** + `cargo build --target wasm32-wasi --release` | Standard zellij plugin toolchain. |
| Task runner | **`cargo xtask`** | No external build system. |
| Binary size / startup | `strip = "symbols"`, `lto = "thin"`, `codegen-units = 1` in release profile | CLI startup latency matters. |
| Compact strings | `compact_str` for display labels | Tag/label heavy; avoids heap allocs for short strings. |
| Config (JSONC) | `serde_json` + **`json_comments`** | Strip JSONC comments before parse; zero custom parser. |
| Config schema gen | **`schemars`** | Derives JSON Schema from Rust config struct at build time. |
| Config validation | **`jsonschema`** | Validates user config against schema; human errors with line numbers. |

### What we explicitly do **not** use
- `async-std` — redundant with tokio.
- `tonic`/gRPC — overkill for one-box IPC; MsgPack over unix socket is lighter and zero-ceremony.
- Global `lazy_static!` mutables — use `OnceLock` or dependency-injected services.
- Sync mutexes in hot paths — RwLock + arc-swap only.

---

## 7. Data Model & State Machines

### Entities (as in §10 of the requirements) `[ral-np3.1 ✓, ral-np3.3 ✓, ral-np3.5 ✓]`

```
 Workspace ──1:N─▶ Agent ──1:1─▶ Pane(zellij-ref)
     │               │
     │               └──▶ CaptureStream
     │
     ├──1:N─▶ Event(append-only)
     ├──1:N─▶ InboxItem
     ├──0:N─▶ Worktree (git)
     └──0:N─▶ HookRegistration
```

All ids are `Ulid` wrapped in newtypes (`WorkspaceId(Ulid)`). Wrapping gives
type safety and prevents "accidentally pass AgentId where PaneId expected"
bugs that plague cmux/tmux wrappers.

### Agent state machine `[ral-np3.2 ✓]`

```
             ┌───────────────────────────────────────────────────┐
             │                                                   │
             ▼                                                   │
     ┌──────────────┐                                            │
     │ Initializing │                                            │
     └──────┬───────┘                                            │
            │ started                                            │
            ▼                                                    │
     ┌──────────────┐      idle-timeout       ┌──────────────┐   │
     │   Running    │────────────────────────▶│     Idle     │   │
     └──┬─────┬─────┘                         └───────┬──────┘   │
        │     │◀──────────────── input ───────────────┘          │
        │     │                                                  │
        │     │ hook: waiting_for_input                          │
        │     ▼                                                  │
        │  ┌──────────────────┐                                  │
        │  │ WaitingForInput  │─────── input resolved ───────────┤
        │  └─────────┬────────┘                                  │
        │            │ capture-rule: attention                   │
        │            ▼                                           │
        │      ┌──────────────────┐                              │
        │      │ AttentionRequired│────── acknowledged ──────────┤
        │      └────────┬─────────┘                              │
        │               │                                        │
        │ hook: completed/failed                                 │
        ▼               ▼                                        │
  ┌──────────┐   ┌──────────┐    ┌──────────┐                    │
  │Completed │   │  Failed  │    │ Stopped  │────── restarted ───┘
  └──────────┘   └──────────┘    └──────────┘
```

Implemented as an enum + a `transition(&self, event) -> Result<Self, InvalidTransition>`
pure function in `rally-core`. Unit-tested with `proptest` — every random
event sequence either transitions or returns `InvalidTransition`; never panics,
never loses state.

### Events (append-only log) `[ral-np3.6 ✓, ral-bku.3 ✓]`

```rust
#[non_exhaustive]
pub enum DomainEvent {
    WorkspaceCreated { id: WorkspaceId, name: CompactString, repo: Option<PathBuf>, at: Timestamp },
    WorkspaceArchived { id: WorkspaceId, at: Timestamp },
    AgentRegistered { id: AgentId, workspace: WorkspaceId, role: CompactString, runtime: CompactString, at: Timestamp },
    AgentAttachedPane { id: AgentId, pane_ref: PaneRef, at: Timestamp },
    AgentStateChanged { id: AgentId, from: AgentState, to: AgentState, cause: StateCause, at: Timestamp },
    AgentMetadataUpdated { id: AgentId, key: CompactString, value: serde_json::Value, at: Timestamp },
    CaptureSnapshot { agent: AgentId, bytes_hash: [u8; 32], at: Timestamp },   // body stored on disk
    InboxItemRaised { id: InboxItemId, agent: Option<AgentId>, urgency: Urgency, kind: InboxKind, at: Timestamp },
    InboxItemAcked { id: InboxItemId, at: Timestamp },
    HookFired { registration: HookId, event: CompactString, at: Timestamp },
}
```

`#[non_exhaustive]` preserves forward-compat. Plugin + MCP clients handle
unknown variants gracefully.

### Projections (read models)

- `WorkspaceOverview` — cached by workspace id, rebuilt on relevant events.
- `AgentBadge` — the 12-character cell the sidebar renders per agent.
- `InboxView` — paginated, filtered.

Projections live in `rally-core` (pure), are **updated inside the event
handler** (no async), and snapshotted with `arc-swap` so read paths (sidebar,
MCP `list_agents`) are lock-free.

---

## 8. Pane ↔ Agent Correlation (the one tricky bit) `[ral-8gl]`

Zellij's external CLI does not return the pane id it just created. It also
does not let plugins poll "which pane was last spawned?" reliably. Strategy:

```
   ┌──────────────────────────────────────────────────────────────────┐
   │ user runs:                                                       │
   │   $ rally agent spawn --workspace api --role impl --runtime cc   │
   │                                                                  │
   │ 1. CLI → daemon: create agent A1, state=Initializing             │
   │ 2. daemon → zellij: `zellij action new-pane -- rally _attach A1` │
   │ 3. pane starts the shim `rally _attach A1` (hidden subcommand)   │
   │ 4. shim reads $ZELLIJ_PANE_ID (+ session id + tab id)            │
   │    and sends it to the daemon over the unix socket               │
   │ 5. daemon records PaneRef{session, tab, pane_id} on A1           │
   │ 6. shim execs (or forks+exec) the real agent command             │
   └──────────────────────────────────────────────────────────────────┘
```

This is the **only** reliable way to correlate externally, and it's how iTerm
integrations and cmux's Swift bridge effectively work. `ZELLIJ_PANE_ID` and
`ZELLIJ_SESSION_NAME` are provided by Zellij to every pane's environment.

Benefits:
- Correlation is exact; no fragile "last pane created" heuristic.
- The shim is also where we install the agent-side hook env (`CLAUDE_HOOKS_PATH`
  etc.), so Claude Code events flow automatically.
- In Phase 8, the shim becomes the PTY subordinate — same entry point evolves.

### When we own the PTY (Phase 8)

```
 ┌──────────────────────────┐     ┌──────────────────────────┐
 │ rally-daemon             │     │ zellij pane              │
 │                          │     │                          │
 │  portable-pty master ────┼─┬──▶│ rally _attach --pty A1   │──▶ agent
 │  ring buffer │           │ │   │  (just does dup2 into    │
 │  subscribers │           │ │   │   master fd and execs)   │
 │              ▼           │ │   └──────────────────────────┘
 │    broadcast::Sender     │ │
 │    arc-swap<Snapshot>    │ │     plugin / mcp / cli can subscribe without
 │                          │ │     touching the pane
 └──────────────────────────┘ │
                              ▼ (tee)
                       SQLite capture log
```

This is the "blazing fast rendering of large text volumes" win: one producer,
many zero-copy consumers on `bytes::Bytes` slices, bounded memory.

---

## 9. CLI Surface (§7.7)

### Noun-verb command tree

```
 rally
  ├── workspace
  │     ├── new     [--repo PATH] [--name NAME] [--worktree]
  │     ├── ls      [--json] [--filter status=active]
  │     ├── show    <WS>
  │     ├── focus   <WS>
  │     ├── archive <WS>
  │     └── resume  <WS>
  ├── agent
  │     ├── spawn   --workspace <WS> --role <R> --runtime <claude|codex|shell> [-- CMD...]
  │     ├── ls      [--workspace <WS>] [--state running,idle] [--json]
  │     ├── show    <AGENT>
  │     ├── focus   <AGENT>
  │     ├── stop    <AGENT> [--force]
  │     ├── restart <AGENT>
  │     ├── set     <AGENT> <KEY>=<VAL> ...          # metadata
  │     └── emit    <AGENT> <EVENT> [--payload JSON] # from hooks
  ├── pane
  │     ├── ls
  │     ├── focus   <PANE|AGENT>
  │     └── color   <PANE|AGENT> <NAMED|#HEX>        # intent (policy gate)
  ├── capture
  │     ├── snapshot <AGENT>                          [--format ansi|text|json]
  │     ├── tail     <AGENT> [--since DUR] [--follow] [--lines N]
  │     └── group    <WS|TAG>                         # multiplex multiple agents
  ├── inbox
  │     ├── ls       [--unread] [--urgency high]
  │     ├── show     <ITEM>
  │     ├── ack      <ITEM|--all>
  │     └── watch                                     # ndjson stream
  ├── hook
  │     ├── install  claude-code
  │     ├── ls
  │     └── test     <HOOK>
  ├── mcp
  │     └── serve    [--stdio|--http PORT]            # runs rally-mcp
  ├── session
  │     ├── status
  │     └── restart-daemon
  ├── _attach         (hidden: used by the shim)
  └── _plugin-pipe    (hidden: used by sidebar plugin)
```

### Design invariants
- Every subcommand supports `--json` and exits with structured output.
- Every list command supports `--filter KEY=VAL` and `--sort KEY`.
- Every id is a ulid or a unique prefix (like `git` does with sha prefixes);
  `rally agent show 01HX…` or `rally agent show api/impl-2` (workspace/role tag).
- `watch` commands emit ndjson for pipelines: `rally inbox watch | jq 'select(.urgency=="high")'`.
- Human output uses `comfy-table` with boxes; `NO_COLOR` and `--no-color` respected.

### Completion
`clap_complete` generates zsh/bash/fish on `rally completions <shell>`.

---

## 10. Sidebar UI (§7.4) — cmux / Conductor-inspired

Visual inspiration: cmux vertical tabs with notification rings, Conductor's
task list, Superset's workspace tiles. Zellij's plugin pane is a vertical
strip rendered as an ANSI string. Design choice: the sidebar is **narrow
(28 cols default)**, **glanceable**, and uses Unicode box-drawing + badges.

### Default layout (sidebar + tabs bar)

```
 ┌───────Tabs───────────────────────────────────────────────────────────────┐
 │ [ api-service ] [ mobile-app ] [ +  ]                                    │
 ├───────────────────────────┬──────────────────────────────────────────────┤
 │                           │                                              │
 │ ╭ RALLY ───────── ⚙ ⌕ ╮   │                                              │
 │ │                    │    │                                              │
 │ │ ▾ api-service   ●3 │    │                                              │
 │ │  ● impl-1   ▶ 04:12│    │                                              │
 │ │  ◐ tests-2  … idle │    │          agent-1 pane content                │
 │ │  ◉ review-1 ⚠ wait │    │          (real terminal, not plugin)         │
 │ │                    │    │                                              │
 │ │ ▾ mobile-app    ●0 │    │                                              │
 │ │  ● build-1  ▶ 12:03│    │                                              │
 │ │  ○ lint-1   ✓ done │    │                                              │
 │ │  ✕ ship-1   ✗ fail │    │                                              │
 │ │                    │    │                                              │
 │ │ ── INBOX ───── 4 ──│    │                                              │
 │ │ ⚠ impl-1 needs y/n │    │                                              │
 │ │ ⚠ review-1 blocked │    │                                              │
 │ │ ✓ lint-1 finished  │    │                                              │
 │ │ ✗ ship-1 failed    │    │                                              │
 │ ╰────────────────────╯    │                                              │
 │                           │                                              │
 └───────────────────────────┴──────────────────────────────────────────────┘
 [N]ext  [j/k]move  [f]ocus  [a]ck  [s]poawn  [/] filter  [?] help
```

### State glyph vocabulary

```
 ●  running         ◐  idle                 ◉  attention required
 ○  completed ok    ✕  stopped              ✗  failed
 ⧗  initializing    ?  unknown              ⚠  waiting for input
```

Badges on the right: `▶ MM:SS` (runtime), `●N` (unread inbox count for group),
`↺N` (restart count).

### Alternate "dense" mode (expandable)

```
 ╭ ws:api-service · branch:feat/auth-mfa · dirty:3 ──── 4 needs ⚠ ─╮
 │ ● impl-1   cc    ▶ 04:12   tokens: 18.3k   tool: Edit           │
 │ ◉ review-1 cc    ⚠ waiting "Apply changes? (y/n)"  ← FOCUS      │
 │ ◐ tests-2  sh    … 02:30   pytest -q tests/                     │
 ╰─────────────────────────────────────────────────────────────────╯
```

### Inbox detail view (pressed `i`)

```
 ╭ INBOX ─────── filter: unread|high ─── 4 items ──────────────────╮
 │                                                                 │
 │ ⚠  16:04  review-1  WaitingForInput                             │
 │          "Apply changes to 3 files? (y/n)"                      │
 │                                                   [f] focus     │
 │                                                   [a] ack       │
 │ ────────────────────────────────────────────────────────────── │
 │ ✗  15:58  ship-1    Failed                                      │
 │          exit=1  "npm run build: Type error TS2322"             │
 │                                                   [l] logs      │
 │                                                   [r] restart   │
 │ ────────────────────────────────────────────────────────────── │
 ╰─────────────────────────────────────────────────────────────────╯
```

### Rendering strategy (how we make this fast)

Because Zellij plugins re-render on every `update()`, naive string building
is wasteful. The plugin instead:

```
 State snapshot (arc-swap)         render_cache
 ┌─────────────┐                   ┌────────────┐
 │ version: 47 │──── if changed ──▶│ full string│──▶ return to zellij
 └─────────────┘    else skip      │ version:47 │
                                   └────────────┘
```

- The daemon maintains a monotonically increasing `state_version` per projection.
- The plugin pulls (via `zellij pipe` initiated by the daemon) `{version, snapshot}`.
- If `version == cached`, `update()` returns `false` (no repaint).
- Per-agent rows are rendered into a `SmallVec<[Row; 32]>` allocated once.
- Glyphs are const `&'static str`.
- Render output is built into a `String` with `String::with_capacity(8*1024)` reused.

Result: idle sidebar = zero allocations, 0 bytes sent to Zellij. Busy sidebar
(50 agents) = one allocation per diff, bounded by the state-version tick rate
(~4 Hz is plenty for human glanceability).

### Extensibility hooks in the sidebar

Even in Phase 7 the sidebar is built around a `Widget` trait so later phases
can drop in new sections without rewriting:

```rust
pub trait SidebarWidget {
    fn id(&self) -> &str;
    fn render(&self, ctx: &RenderCtx, buf: &mut AnsiBuf);
    fn handle_key(&mut self, ctx: &mut HandleCtx, key: Key) -> Handled;
}

// shipped: WorkspaceTree, InboxSummary, StatusBar
// plus a registry so plugins of rally (not zellij!) can add widgets later
// by registering via the daemon rather than editing the plugin binary.
```

The daemon can tell the plugin "here are the active widgets and their state";
the plugin is a **dumb renderer** of what the daemon publishes. This is the
key extensibility shape — the source of truth stays in the daemon, UI is
pluggable.

---

## 11. Inbox + Notifications (§7.5)

Inbox items are **durable** (SQLite) and generated by rules over the event
stream:

```
 events ──▶ rally-inbox::RuleSet ──▶ InboxItem ──▶ subscribers
                │                         │
                │                         ├─▶ plugin sidebar
                │                         ├─▶ `rally inbox watch`
                │                         └─▶ external sink (macOS terminal-notifier,
                │                             Slack webhook, Raycast push)
                │
                └── rules include: "agent entered WaitingForInput",
                    "agent.Failed", "capture matched /Apply changes/",
                    "hook.Notification arrived", "timeout: idle > 10m"
```

Rules are typed enums in Phase 5, declarative DSL (KDL, same as zellij) in
Phase 8. Decay/TTL so resolved items don't bloat the store.

---

## 12. Failure Model & Resilience

- Every subsystem = named supervised task. Panic → tracing error → restart with
  exponential backoff (max 3 in 60s before the subsystem is parked and reported).
- Bounded queues everywhere (`tokio::mpsc` capacity=512). Backpressure returns
  typed errors rather than silently dropping or unbounded-growing.
- IPC server uses per-connection tasks with 30s idle timeout.
- Hooks are spawned with 5s default wall-clock timeout and 64 KiB stdout cap.
- Plugin failures (zellij reports plugin panic) just mean the sidebar is gone;
  the daemon keeps running.
- SQLite writes wrapped in a single writer task (no multi-writer contention);
  reads go through the pool.
- All tasks observe a shared `CancellationToken`; `rally session restart-daemon`
  is clean (SIGTERM → cancel → drain → exit in <1s).

---

## 13. Testing Strategy (Autonomous Gates)

Each phase has an exit gate. CI must pass the gate before the next phase starts.
Every layer is testable without live UI where practical (§9 of the requirements).

### Per-layer test style

| Layer | Test style | Tooling |
|---|---|---|
| `rally-core` | unit + property tests on state machine; zero IO | `proptest`, `insta` for projections `[ral-np3.9 ✓]` |
| `rally-store` | integration tests against temp sqlite files | `tempfile`, fixtures `[ral-bku.4 ✓]` |
| `rally-events` | broadcast fan-out & backpressure | tokio-test, loom for shared state `[ral-fq5.10]` |
| `rally-daemon` | in-process harness: spawn daemon on ephemeral socket | `rally-test-utils::DaemonHarness` `[ral-fq5.8]` |
| `rally-cli` | black-box end-to-end against a daemon harness | `assert_cmd`, `insta` snapshots of --json output `[ral-fq5.11]` |
| `rally-capture` | fake source emits scripted bytes; assert on ring buffer + subscribers | `tokio-test` |
| `rally-host-zellij` | contract tests guarded by `#[ignore]` unless `RALLY_E2E_ZELLIJ=1` set; runs against real zellij in CI nightly | headless zellij |
| `rally-plugin` | pure-Rust renderer tested against golden ANSI output | `insta` |
| `rally-mcp` | run server on stdio, feed a scripted MCP client | `rmcp` client |
| `rally-hooks` | invoke hook binary with fake env, assert resulting events | `assert_cmd` |

### Exit gate per phase
1. All tests green (`cargo test --all --all-features`).
2. Clippy clean (`cargo clippy --all-targets -- -D warnings`).
3. `cargo deny check` + `cargo audit` clean.
4. Documented public API (`cargo doc --no-deps` warns as errors).
5. At least one new **end-to-end** test exercising the phase's headline feature.
6. Benchmark regression check on capture throughput (≥ 2 GB/s ingest, ≥ 50 MB/s
   sustained to 4 subscribers) after Phase 8.

### Nightly CI
- `cargo test --release`, fuzz for 2 minutes per fuzz target (`cargo-fuzz` on
  the proto decoder).
- Run the **self-hosted** Phase 5+ workflow: spawn rally agents that run the
  test suite in a sandbox workspace and post results to inbox.

---

## 14. Phased Implementation Breakdown

### Phase 0 — Workspace skeleton (0.5 day) `[ral-7lu ✓]`
**Goal:** the empty car runs.
- `cargo new --workspace`, lay out crates in §5. `[ral-7lu.3 ✓]`
- `rust-toolchain.toml`, `deny.toml`, `.cargo/config.toml`, `xtask/`. `[ral-7lu.2 ✓, ral-7lu.1 ✓]`
- GitHub Actions (or local `just ci`) running fmt/clippy/test matrix. `[ral-7lu.5 ✓]`
- Release profile tuned (`lto="thin"`, `codegen-units=1`, `strip="symbols"`). `[ral-7lu.4 ✓]`

**Gate:** `cargo build --workspace` + empty test suites pass.

---

### Phase 1 — Core domain (2 days) ★ `[ral-np3 ✓]`
**Goal:** pure, testable domain model.
- Ids, entities, `DomainEvent`, `AgentState` machine. `[ral-np3.1 ✓, ral-np3.2 ✓, ral-np3.3 ✓, ral-np3.6 ✓]`
- `ports::WorkspaceRepo`, `AgentRepo`, `Clock`, `IdGen`. `[ral-np3.4 ✓]`
- `InMemoryRepo` in `rally-test-utils`. `[ral-np3.8 ✓]`
- **Property tests**: "no event sequence causes panic or lost state",
  "projection == fold(events, initial)". `[ral-np3.9 ✓]`
- `rally-proto v1`: requests/responses/events as Rust types, `serde`. `[ral-np3.7 ✓]`
- `InboxItem`, `CaptureRef`, policy types. `[ral-np3.5 ✓]`

**Gate:** state-machine property tests at ≥ 200k cases each. Zero IO/async in core.

---

### Phase 2 — Persistence (1.5 days) `[ral-bku ✓]`
**Goal:** durability without complexity.
- `rally-store` implements `WorkspaceRepo`/`AgentRepo`/`EventLog` on SQLite WAL. `[ral-bku.2 ✓]`
- Event sourcing: append-only `events` table, projection caches on bump. `[ral-bku.3 ✓]`
- Migration framework (`refinery` or hand-rolled `PRAGMA user_version`). `[ral-bku.1 ✓]`

**Gate:** crash-restart test (kill mid-write, reopen, invariants hold). `[ral-bku.4 ✓]`

---

### Phase 3 — Daemon + CLI skeleton + config (3 days) ★ `[ral-fq5 ✓]`
**Goal:** first usable tool. No zellij yet.
- `rally-config` crate: JSONC parsing (`json_comments` + `serde_json`), typed
  `RallyConfig` struct, layered merge (defaults → file → env → CLI flags). `[ral-fq5.3 ✓, ral-fq5.2 ✓ · log: ral-anw, ral-0v7 ✓]`
- `rallyd` binary: reads config, binds unix socket IPC, MsgPack frames. `[ral-fq5.1 ✓ · log: ral-2l9 ✓, ral-mul ✓]`
- `rally-cli` with `workspace`/`agent ls/show`/`session status` commands. `[ral-fq5.6 ✓, ral-fq5.7 ✓ · log: ral-ibs]`
- `--json` everywhere. `tracing` logs to `~/.local/state/rally/rallyd.log`. `[ral-fq5.7 ✓]`
- Daemon autostarts on first client call (double-fork, pid file). `[ral-fq5.5 ✓ · log: ral-s4a ✓]`
- `rally-test-utils::DaemonHarness` used by CLI integration tests. `[ral-fq5.8 ✓, ral-fq5.11 ✓ · log: ral-d0n]`
- Session naming: auto-generated canonical key (`<repo>-<branch>-<id>-<ts>`)
  with optional user alias (`rally workspace alias <key> <alias>`). `[ral-fq5.9 ✓ · log: ral-cxv]`
- `rally-events` tokio broadcast bus + arc-swap snapshot. `[ral-fq5.10 ✓ · log: ral-da1]`
- `rallyd` core service layer (WorkspaceService, AgentService). `[ral-fq5.4 ✓ · log: ral-wgv]`

**Gate:** can create workspaces (with auto key + alias), register agents,
list with `rally agent ls --json | jq`. Config file is loaded and layered.

---

### Phase 4 — Zellij host integration + capture v1 (4 days) ★ `[ral-2zv ✓]`
**Goal:** actually spawn agents in panes and read their output, in **both**
deployment modes.
- `rally-host-zellij::SessionHandle` abstraction over `session_name`. `[ral-ufa ✓ · log: ral-x5t]`
- `PluginBootstrap`: detects `ZELLIJ_SESSION_NAME` env on first contact, caches it. `[ral-821 ✓ · log: ral-bna]`
- `StandaloneBootstrap`: `rally up` spawns `zellij attach -c rally-<id>
  --layout <bundled.kdl>`; daemon supervises the child and tears it down on
  `rally down`. `[ral-vzk ✓ · log: ral-l03]`
- `zellij action` wrappers (`new-pane`, `focus-pane`, `dump-screen`,
  `rename-pane`, `move-pane`). All take an optional `--session` arg. `[ral-s4p ✓ · log: ral-dpd]`
- `rally _attach` shim: captures `ZELLIJ_PANE_ID`, reports to daemon, execs
  the real command. `[ral-mxk ✓, ral-8gl ✓ · log: ral-axk]`
- `rally-capture` v1: `DumpScreenSource` polls at 5 Hz; diffs into ring buffer. `[ral-khz ✓ · log: ral-arg]`
- `rally capture snapshot` + `rally capture tail --follow` (ndjson) working. `[ral-336 ✓ · log: ral-ke9]`
- `rally install-plugin` / `rally layout export`. `[ral-c3m ✓ · log: ral-59g]`
- Phase 4 integration tests (both deployment modes). `[ral-gva ✓ · log: ral-aeq]`

**Gate:** two manual runs, both green:
- Plugin mode (inside an already-running zellij):
  `rally workspace new demo && rally agent spawn --workspace demo -- htop`
  opens a pane in the current session; `rally capture tail` streams.
- Standalone mode (clean shell): `rally up demo` creates a rally-owned zellij
  session with the sidebar pinned; `rally down demo` cleans up.

### 🎉 Self-hosting milestone candidate
At the end of Phase 4 we already have "spawn shell in named pane + observe
output". Phases 5 onwards **can and should be built by the user driving rally
itself**, e.g.:

```
 $ rally workspace new rally-dev --repo .
 $ rally agent spawn --workspace rally-dev --role impl \
     --runtime claude-code -- claude 'implement phase-5 inbox rules'
 $ rally agent spawn --workspace rally-dev --role tests \
     -- cargo watch -x 'test -p rally-inbox'
 $ rally agent spawn --workspace rally-dev --role review \
     --runtime claude-code -- claude 'review the diff so far'
```

This gives the project a dogfooding loop months before anyone else uses it.

---

### Phase 4.5 — Review fixes, quality gates, and correctness debt `[ral-9rt]`
**Goal:** address P1 bugs and testing gaps discovered during Phase 4 code review. Not in the original plan — emerged from advisor review.
- Fix `EventLog::list_for_workspace` discarding events. `[ral-bzi ✓, ral-dcb ✓]`
- Wire alias CLI command + proto request (infrastructure existed, not exposed). `[ral-d9z ✓, ral-dey ✓]`
- Fix clippy `-D warnings` blocking CI. `[ral-m2z ✓, ral-yje ✓]`
- Fake zellij binary + `TestWorld` harness for CLI integration tests. `[ral-9rt.5 ✓]`
- CLI integration tests assert list contents, not just response kind. `[ral-9rt.3 ✓]`
- Agent spawn constructs correct zellij argv. `[ral-c2r ✓]`
- `_attach` + `BindPane` IPC updates agent `pane_id` end-to-end. `[ral-khd ✓, ral-9rt.4 ✓]`
- Alias end-to-end test: set alias, resolve in workspace show and agent spawn. `[ral-9rt.2 ✓]`
- `cargo xtask ci` with real quality gates. `[ral-roc ✓, ral-66h]`
- `EventBus StateSnapshot` maintaining workspace/agent projections. `[ral-p35]`
- Capture pane ID from `new-pane` stdout instead of `_attach` shim. `[ral-1fm]`
- Fix Zellij plugin missing `_start` export. `[ral-65y ✓]`

**Remaining open:** `ral-66h` (xtask ci), `ral-p35` (EventBus projections), `ral-1fm` (pane-id capture), plus lower-priority P3 items.

---

### Phase 5 — Claude Code hooks + Inbox + Config doctor (4 days) ★ `[ral-lu5]`
**Goal:** "the sidebar item that tells me impl-1 is blocked" + config hackability.
- `rally hook install claude-code` writes settings to user `settings.json`
  that call `rally agent emit "$AGENT_ID" "$EVENT" --payload …`. `[ral-0xg · log: ral-2yw]`
- Hook events: PreToolUse, PostToolUse, Notification, Stop → map onto domain
  events: `AgentStateChanged{to: WaitingForInput}`, etc. `[ral-qdw · log: ral-93r]`
- `rally-inbox` rule engine: typed rules over event stream → `InboxItem`.
  Inbox rules are **config-driven** — users edit `config.jsonc` `"inbox.rules"`
  to add custom triggers without touching code. `[ral-kj1 · log: ral-2yb]`
- `rally inbox ls/watch/ack/show`. `[ral-u80 · log: ral-bu9]`
- Capture pattern rules: "if stdout matches `/^Error:/` set attention". `[ral-c6u · log: ral-ekj]`
- **Config doctor**: `rally config doctor` validates config against JSON Schema,
  checks binary versions (zellij, worktrunk), daemon health, hook installation. `[ral-7lg · log: ral-2es]`
- **Schema generation**: `cargo xtask schema` produces `rally-config.schema.json`
  from the `RallyConfig` struct via `schemars`. Published alongside releases for
  editor autocompletion. `[ral-2n2 · log: ral-kmn]`
- macOS desktop notifications via `terminal-notifier` as a built-in notification
  sink (configured in `config.jsonc` `"notifications.sinks"`). `[ral-wwt · log: ral-thi]`
- Phase 5 integration tests. `[ral-wjb · log: ral-o2c]`

**Gate:** a spawned Claude Code agent that asks a y/n question raises an
inbox item visible via CLI within 200 ms. `rally config doctor` reports all-green
on a correctly configured machine.

---

### Phase 6 — MCP server (3 days) `[ral-3qa]`
**Goal:** agents can query and affect other agents via all MCP transports.
- `rally-mcp` binary supporting **all three MCP transports**:
  - **stdio** (default, `rally mcp serve --stdio`): claude-desktop, Claude Code. `[ral-5q5 · log: ral-9jy]`
  - **SSE/HTTP** (`rally mcp serve --http :8377`): remote agents, browser clients. `[ral-9j7 · log: ral-dgw]`
  - **streamable-HTTP** (`rally mcp serve --streamable-http :8377`): modern MCP
    clients that support the newest transport. `[ral-cce · log: ral-dsa]`
- Transport picked by flag; a single `McpRouter` struct with pluggable IO backends.
- Tools: `list_workspaces`, `list_agents`, `get_agent`, `capture_snapshot`,
  `tail_capture`, `list_inbox`, `ack_inbox`, `emit_agent_event`,
  `request_focus`, `update_agent_metadata`. `[ral-8qj · log: ral-ph0]`
- Control tools: `emit_agent_event`, `request_focus`, `ack_inbox`, `update_agent_metadata`. `[ral-mtp · log: ral-741]`
- Authorization scopes: read-only by default; control actions require an
  env flag or an explicit `allow_control=true` launch argument. `[ral-nkd · log: ral-csb]`
- Published as a claude_desktop_config.json snippet in docs. `[ral-pld · log: ral-slo]`

**Gate:** an MCP client session can list and tail agents; control tools gated
by policy pass unit + integration tests.

---

### Phase 7 — Zellij plugin sidebar (4 days) ★ `[ral-l4t ✓]`
**Goal:** the visible product.
- `rally-plugin` cdylib → `wasm32-wasi` using `zellij-tile`. `[ral-rbv ✓ · log: ral-zvl]`
- State pulled: daemon pushes via `zellij pipe --plugin file:rally.wasm`
  on every bumped `state_version`. `[ral-3cc ✓ · log: ral-n9x]`
- `SidebarWidget` trait + built-ins: WorkspaceTree, InboxSummary, StatusBar. `[ral-uge ✓ · log: ral-9b4]`
- InboxSummary widget + inbox detail view. `[ral-d4c ✓ · log: ral-cbj]`
- StatusBar widget. `[ral-h0k ✓ · log: ral-z29]`
- Keyboard: `j/k` nav, `f` focus, `a` ack, `r` restart, `s` spawn wizard,
  `/` filter, `?` help, `i` inbox detail, `Esc` back. `[ral-nvb ✓ · log: ral-1r4]`
- Render cache + version gate (§10). `[ral-cgs ✓ · log: ral-s2x]`
- Golden ANSI snapshots (insta tests). `[ral-8zf ✓ · log: ral-4q9]`

**Gate:** visual acceptance — the mockup in §10 matches what a user sees with
5 agents across 2 workspaces. Golden ANSI snapshots in `insta`.

---

### Phase 8 — Advanced capture + extensions (4 days) `[ral-zbs]`
**Goal:** scale + openness.
- Swap `DumpScreenSource` for `PtySource` using `portable-pty` (§8 diagram). `[ral-zbs.1]`
- Subscriber model: `broadcast::Sender<Bytes>` with backpressure. `[ral-zbs.2]`
- `rally capture group` — one stream fans in multiple agents with source tags. `[ral-zbs.3 · log: ral-zbs.10]`
- KDL-based declarative rules for inbox/capture (same flavor as zellij). `[ral-zbs.4 · log: ral-zbs.11]`
- External notification sinks (macOS `terminal-notifier`, Slack webhook). `[ral-zbs.5 · log: ral-zbs.12]`
- Typed extension SDK: `rally-ext` crate lets users write out-of-tree
  subscribers that connect to the daemon via the stable wire protocol. `[ral-zbs.6 · log: ral-zbs.13]`

**Gate:** `criterion` benchmark: sustained 50 MB/s agent output, 4 subscribers,
p99 plugin render < 8 ms; zero allocations on the ingest fast path. `[ral-zbs.7 · log: ral-zbs.14]`

---

### Phase 9 — Polish, docs, packaging (ongoing) `[ral-62b]`
- Homebrew formula, cargo binstall metadata, precompiled darwin/linux binaries.
- `rally doctor` consolidation (already in Phase 5, extend with network checks,
  MCP client smoke, plugin version alignment).
- User guide with the cmux-style screenshots above (now real).
- Config schema published to SchemaStore for VS Code + Neovim auto-discovery.
- `rally config init` — generates a commented `config.jsonc` from current
  defaults, so users have a hackable starting point.
- `rally config diff` — shows what differs from defaults (for debugging).
- Post-MVP from §14 of the requirements (richer worktree templates, advanced
  triage) gated on real use.

---

## 15. Extensibility Shape (addressing §7.11 + §15.4)

```
    ┌──────────────────────────────────────────────────────────┐
    │                    rally-daemon                           │
    │                                                           │
    │   ┌──────────────┐     ┌──────────────┐                   │
    │   │ event bus    │────▶│ subscription │◀── typed trait    │
    │   │ (broadcast)  │     │ registry     │    contract,      │
    │   └──────────────┘     └──────┬───────┘    not ad-hoc     │
    │                               │                           │
    │                   ┌───────────┼──────────┐                │
    │                   ▼           ▼          ▼                │
    │           ┌──────────┐ ┌────────────┐ ┌──────────┐        │
    │           │  inbox   │ │  capture   │ │ external │        │
    │           │  rules   │ │  detectors │ │   ext    │        │
    │           └──────────┘ └────────────┘ └────┬─────┘        │
    └─────────────────────────────────────────────┼─────────────┘
                                                  │ stable wire proto
                                                  ▼
                                     ┌─────────────────────┐
                                     │  third-party ext    │
                                     │  (any language)     │
                                     └─────────────────────┘
```

Every extension surface — rule, detector, notification sink, external process
— subscribes through the **same versioned event wire** and calls back through
the **same CLI/MCP/IPC surface**. No parallel "control path" emerges.

---

## 16. Risk Register (addressing §15)

| Risk | Phase most at risk | Mitigation |
|---|---|---|
| UI/orchestration coupling (§15.1) | 7 | Plugin is a dumb renderer; projections live in daemon. Sidebar widget registry. |
| CLI inconsistency (§15.2) | 3 | Design noun-verb tree up front; CLI integration tests assert structure; `--json` mandatory. |
| Capture API under-specification (§15.3) | 4, 8 | Capture types + rules in `rally-core`, not in ad-hoc CLI flags. |
| Extension sprawl (§15.4) | 5, 8 | Single event bus + wire schema; no bespoke side-channels allowed. |
| Zellij leakage into core (§15.5) | 1→4 | Host port trait; `rally-core` literally has no `zellij` string. |
| Zellij plugin API breakage between versions | 7+ | Pin `zellij-tile` version; runtime check of `zellij --version`; `rally doctor`. |
| SQLite contention under heavy event rate | 5, 8 | Single writer task; async-flush buffer; WAL. |
| Agent process lifecycle confusion | 4, 8 | Shim owns the child; daemon supervises the shim. |
| Hook script timeouts blocking daemon | 5 | Hooks run in spawned tasks with timeouts; never block the event loop. |
| MCP as a second parallel product (§15.4 echo) | 6 | MCP tools are generated from the same service trait the CLI uses. |

---

## 17. Rust Best-Practices Checklist (enforced in CI)

- `#![deny(unsafe_code)]` on all crates except `rally-capture::pty` if absolutely required (gated behind a feature, audited).
- `#![warn(missing_docs)]` on public crates; `cargo doc` is treated as a build.
- No `unwrap`/`expect` outside of tests and `main`. `clippy::unwrap_used` + `clippy::expect_used` at error level.
- All public functions return `Result<_, thiserror enum>`; top-level converts to `anyhow::Error` + exit code.
- `#[non_exhaustive]` on all public enums/structs in `rally-proto` and `rally-core`.
- No `lazy_static!` or global mutables; use `OnceLock`/injection.
- Newtypes for all ids (no raw `String` ids across crate boundaries).
- `impl<T: ?Sized>` and `dyn Trait` used on cold paths; generics on hot paths.
- Every `tokio::spawn` is either supervised or documented as fire-and-forget.
- Every channel has a bounded capacity and a documented backpressure policy.
- `tracing` spans at the daemon entry points; `#[instrument]` on service methods.
- Benchmarks in `rally-capture`, `rally-events`; regression budget in CI.
- Public types implement `Debug` (hand-rolled to avoid leaking secrets),
  `Clone` when cheap, `Send + Sync` when crossing task boundaries.
- `cargo-machete`, `cargo-udeps` run periodically to trim deps.

---

## 18. Critical Files & Entry Points (when we implement)

These are the files that will be created / focal during implementation:

```
crates/rally-core/src/agent.rs                 # state machine
crates/rally-core/src/event.rs                 # DomainEvent enum (stable surface)
crates/rally-core/src/ports.rs                 # trait ports (repo, clock, idgen)
crates/rally-proto/src/v1/mod.rs               # wire types
crates/rally-store/src/sqlite.rs               # WAL + migrations
crates/rally-daemon/src/main.rs                # tokio runtime + supervisor
crates/rally-daemon/src/ipc.rs                 # unix socket server (tokio-util codec)
crates/rally-daemon/src/services/agent.rs      # AgentService — shared by CLI + MCP
crates/rally-cli/src/main.rs                   # clap derive tree
crates/rally-cli/src/commands/agent.rs         # one file per noun
crates/rally-host-zellij/src/actions.rs        # `zellij action …` wrappers
crates/rally-host-zellij/src/shim.rs           # `rally _attach` logic
crates/rally-capture/src/ring.rs               # LineIndexedRing over BytesMut
crates/rally-capture/src/source/dump_screen.rs # phase 4
crates/rally-capture/src/source/pty.rs         # phase 8
crates/rally-inbox/src/rules.rs                # phase 5
crates/rally-hooks/src/claude_code.rs          # phase 5
crates/rally-mcp/src/server.rs                 # phase 6
crates/rally-config/src/lib.rs                 # RallyConfig, layered merge (phase 3)
crates/rally-config/src/doctor.rs              # DoctorCheck trait + checks (phase 5)
crates/rally-config/src/schema.rs              # schemars derive (phase 5)
crates/rally-plugin/src/lib.rs                 # phase 7 (wasm entry)
crates/rally-plugin/src/widgets/mod.rs         # phase 7 (SidebarWidget)
```

No code is being written now — plan mode.

---

## 19. Verification Plan (how we'll prove the plan works)

After each phase:

1. **Unit + property tests** green: `cargo test --workspace`.
2. **Black-box e2e** for the phase's headline feature, e.g. after Phase 4:
   ```
   $ rally workspace new demo --repo /tmp/demo
   $ rally agent spawn --workspace demo --role worker -- yes hello
   $ rally capture tail <agent> --lines 50 | head -5
   $ rally agent stop <agent>
   ```
3. **Performance smoke** (Phase 4+): `rally-capture` bench shows bounded memory
   and target throughput on synthetic 100 MB/s ingest.
4. **Self-hosted task loop** (Phase 5+): drive a workspace whose agents run
   `cargo test` on a feature branch and report via inbox; green = gate passed.
5. **Interop smoke** (Phase 6): `rmcp` sample client lists and tails an agent.
6. **Plugin render golden** (Phase 7): `insta` snapshots of sidebar for 0/1/5/50
   agents across 0/3/20 inbox items.

---

## 20. Open Questions — ALL RESOLVED

| # | Question | Resolution |
|---|---|---|
| 1 | Tool name | **`rally`** — committed. All paths, env vars, config use this. |
| 2 | MCP transport | **All transports**: stdio (default), SSE/HTTP, streamable-HTTP. See Phase 6. |
| 3 | macOS notifications | **Yes**, via `terminal-notifier` shell-out as a built-in notification sink. Phase 5 inbox → sink. |
| 4 | Worktree policy | **Delegate to `worktrunk`**. Rally calls the worktrunk CLI for worktree creation/cleanup — no homebrew git logic. Rally stores the path worktrunk returns. |
| 5 | Session naming & persistence | **Auto-generated + user-editable alias**. See §20.1 below. |

### 20.1 Session Naming Strategy (resolved) `[ral-fq5.9 · log: ral-cxv]`

Every session gets an **auto-generated canonical key** on creation:

```
 <sanitized-repo-path>-<branch>-<tree-id>-<YYYYMMDDTHHmmss>
  e.g.  rally-feat-auth-mfa-01JQ-20260423T143012
```

This key is:
- Immutable once created (used as the durable storage key in SQLite, log dirs,
  zellij session name `rally-<key>`).
- Globally unique + time-sortable (safe for concurrent users on same machine).

Users may **also set an alias** at any time:

```
 $ rally workspace alias rally-feat-auth-mfa-01JQ-20260423T143012 auth-work
 $ rally workspace focus auth-work        # alias works everywhere
 $ rally agent ls --workspace auth-work   # alias works everywhere
```

Internally, aliases map to canonical keys in a simple `aliases` SQLite table.
A canonical key is always valid; an alias is a convenience pointer.

```
 ┌────────────────────────────────────────────────────────────────┐
 │   Storage & addressing:                                        │
 │                                                                │
 │   rally-feat-auth-mfa-01JQ-20260423T143012   ← canonical key  │
 │            ▲                                                   │
 │            │  alias                                            │
 │     "auth-work"   ← user-editable, optional, unique           │
 │                                                                │
 │   Both are valid in:                                           │
 │     rally workspace focus <KEY-or-ALIAS>                       │
 │     rally agent spawn --workspace <KEY-or-ALIAS>               │
 │     rally capture tail --workspace <KEY-or-ALIAS>              │
 │     zellij session name = "rally-<KEY>"                        │
 └────────────────────────────────────────────────────────────────┘
```

Alias resolution is done once at the CLI/IPC boundary and never leaks into
domain logic — `rally-core` only deals with `WorkspaceId(Ulid)`.

---

## 21. Configuration: JSONC Schema + Config Doctor `[ral-fq5.2, ral-fq5.3 · log: ral-anw, ral-0v7]`

Rally uses a **single JSONC config file** at `~/.config/rally/config.jsonc`
(XDG-aware, `RALLY_CONFIG` env override). JSONC allows comments so users can
annotate their setup.

### Config structure (schema-first)

A JSON Schema (`rally-config.schema.json`) ships alongside the binary and
is published to the repo so editors (VS Code, Neovim with SchemaStore) get
autocompletion + validation for free.

```jsonc
// ~/.config/rally/config.jsonc
{
  // "$schema": "https://raw.githubusercontent.com/.../rally-config.schema.json",

  // Daemon
  "daemon": {
    "socket_path": null,              // null = auto (XDG_RUNTIME_DIR)
    "log_level": "info",              // trace|debug|info|warn|error
    "log_file": null                  // null = ~/.local/state/rally/rallyd.log
  },

  // Zellij host
  "zellij": {
    "binary": "zellij",              // or absolute path
    "default_layout": null,          // null = bundled; path = override
    "plugin_path": null              // null = ~/.local/share/rally/plugins/
  },

  // Worktree (delegates to worktrunk)
  "worktree": {
    "backend": "worktrunk",          // only supported backend for now
    "worktrunk_binary": "wt"         // or absolute path
  },

  // Notifications
  "notifications": {
    "sinks": [
      { "type": "terminal-notifier" },
      // { "type": "slack-webhook", "url": "https://hooks.slack.com/..." },
      // { "type": "script", "path": "~/.config/rally/hooks/notify.sh" }
    ]
  },

  // MCP server defaults
  "mcp": {
    "default_transport": "stdio",    // stdio|http|streamable-http
    "http_port": 8377,
    "allow_control": false           // read-only by default
  },

  // Capture
  "capture": {
    "ring_buffer_mb": 16,            // per-agent, default 16 MB
    "poll_hz": 5,                    // dump-screen polling (phase 4)
    "snapshot_format": "ansi"        // ansi|text|json
  },

  // Inbox rules (declarative, extensible)
  "inbox": {
    "rules": [
      { "on": "agent.state == 'WaitingForInput'", "urgency": "high" },
      { "on": "agent.state == 'Failed'",          "urgency": "high" },
      { "on": "agent.state == 'Completed'",        "urgency": "low"  },
      { "on": "agent.idle_seconds > 600",          "urgency": "medium" }
    ]
  }
}
```

### rally config doctor

`rally config doctor` validates the config and the environment:

```
 $ rally config doctor

 ✓ Config file          ~/.config/rally/config.jsonc
 ✓ Schema validation    valid against rally-config.schema.json v1
 ✓ Zellij binary        /opt/homebrew/bin/zellij (0.41.2)
 ✓ Zellij plugin        ~/.local/share/rally/plugins/rally-sidebar.wasm (v0.3.0)
 ✓ Worktrunk binary     /opt/homebrew/bin/wt (0.8.1)
 ✓ Daemon socket        /tmp/rally/rally.sock (listening, pid 41023)
 ✓ SQLite store         ~/.local/share/rally/state.db (WAL, 2.3 MB)
 ✗ terminal-notifier    not found — install via `brew install terminal-notifier`
 ⚠ MCP allow_control    false (control tools will be read-only)
 ✓ Claude Code hooks    ~/.config/claude/settings.json (rally hooks installed)

 7/8 checks passed · 1 error · 1 warning
```

### Implementation notes
- **Parsing**: `serde_json` with `json_comments` crate to strip JSONC comments
  before deserialization into typed `RallyConfig` struct.
- **Schema generation**: `schemars` crate derives JSON Schema from the config
  struct at build time (`cargo xtask schema > rally-config.schema.json`).
- **Validation**: `jsonschema` crate validates user file against the schema;
  errors are human-readable with line numbers.
- **Layering**: defaults → config file → env vars (`RALLY_DAEMON_LOG_LEVEL`) →
  CLI flags. Standard precedence. Each layer is a partial struct merged with
  `Option` fields.
- **Doctor checks**: each check is a `DoctorCheck` trait impl returning
  `Ok(msg)` / `Warn(msg)` / `Err(msg)`. New checks are added by implementing
  the trait — extensible by design.

### New crate

```
crates/rally-config/             # config parsing, schema gen, doctor
├── src/
│   ├── lib.rs                   # RallyConfig, merge layers
│   ├── schema.rs                # schemars derive
│   ├── doctor.rs                # DoctorCheck trait + built-in checks
│   └── jsonc.rs                 # strip comments, parse
```

`rally-config` is depended on by `rally-daemon` and `rally-cli` only; core
remains config-free (config is injected as concrete values into service
constructors).
