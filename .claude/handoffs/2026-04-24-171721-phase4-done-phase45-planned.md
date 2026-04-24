# Handoff: Phase 4 Complete — Phase 4.5 Review Debt Planned

## Session Metadata
- Created: 2026-04-24 17:17:21
- Project: /Users/abhirupdas/Codes/Personal/rally
- Branch: main
- Session duration: ~3 hours (across multiple context windows)
- Continues from: 2026-04-24-011500-phase3-done.md

### Recent Commits
  - 5eb5303 planning: Phase 4.5 epic — review fixes, quality gates, testing debt
  - e1a8134 chore: cargo fmt --all
  - 474298e phase 4: zellij host integration + capture v1
  - 2314f84 planning: beads issues for Phases 4-9 with logging + cross-phase deps
  - 5f5c357 handoff: Phase 3 complete, all logging tasks closed

## Handoff Chain

- **Continues from**: `.claude/handoffs/2026-04-24-011500-phase3-done.md`
- **Supersedes**: None

## Current State Summary

**Phases 0–4 complete and pushed.** Rally can now create workspaces, register agents, spawn them into Zellij panes via `rally agent spawn --workspace <id> -- htop`, bind pane IDs back via the `_attach` shim, capture pane output via `rally capture snapshot/tail`, and launch standalone Zellij sessions with `rally up/down`. All 27 tests pass. Build is clean (fmt + clippy passing).

A code review identified 18 issues filed under **Phase 4.5 epic (ral-9rt)** covering: broken event deserialization, missing alias CLI wiring, CI quality gates, clippy compliance, and comprehensive testing debt. These are tracked but not yet implemented.

**Zellij 0.44.1 is installed** — provides `dump-screen --pane-id` and `focus-pane-with-id` which Phase 4 code relies on.

## Codebase Understanding

### Architecture (updated for Phase 4)

```
  rally (CLI)  ──unix socket──▶  rallyd (daemon)
       │                              │
       ├── clap v4 command tree       ├── RallyService (services.rs)
       ├── IpcClient (ipc_client.rs)  ├── IPC server (ipc.rs)
       ├── tracing_init (file log)    ├── tracing_init (file log)
       ├── autostart (spawn rallyd)   ├── EventBus (rally-events)
       │                              └── Store (rally-store, SQLite WAL)
       │
       ├── rally-host-zellij ◄── CLI owns all Zellij calls
       │     ├── SessionHandle
       │     ├── PluginBootstrap (env detection)
       │     ├── StandaloneBootstrap (rally up/down)
       │     └── ZellijActions (new-pane, dump-screen, focus, rename)
       │
       └── rally-capture
             ├── CaptureSource trait
             ├── DumpScreenSource (zellij --pane-id)
             └── LineIndexedRing
```

**Key design decision**: spawn_command is CLI-local only. The daemon never sees what command runs in a pane. The CLI calls `zellij action new-pane -- rally _attach <agent-id> <cmd>` after registering the agent. The `_attach` shim reads ZELLIJ_PANE_ID from env and reports it to the daemon via `BindPane` IPC.

### Critical Files (new in Phase 4)

| File | Purpose |
|------|---------|
| `crates/rally-host-zellij/src/session.rs` | SessionHandle, PluginBootstrap (env), StandaloneBootstrap (exec-in-place) |
| `crates/rally-host-zellij/src/actions.rs` | ZellijActions: new_pane, dump_screen --pane-id, focus_pane_with_id, rename_pane |
| `crates/rally-host-zellij/src/shim.rs` | PaneContext::from_env, correlation logging (2 tests) |
| `crates/rally-capture/src/source.rs` | CaptureSource trait, DumpScreenSource (polls zellij dump-screen) |
| `crates/rally-capture/src/ring.rs` | LineIndexedRing — fixed-capacity ring buffer (4 tests incl 10k stress) |
| `crates/rally-cli/src/main.rs` | Extended: agent spawn --, _attach, up, down, capture snapshot/tail, install-plugin, layout export |
| `crates/rally-store/src/db.rs` | save_workspace_and_event / save_agent_and_event atomic methods (ral-ieu fix) |
| `crates/rally-store/src/workspace.rs` | insert_workspace helper extracted for transaction use |
| `crates/rally-store/src/event_log.rs` | insert_event helper extracted; list_for_workspace STILL returns empty (ral-bzi) |
| `crates/rally-proto/src/v1/mod.rs` | Added: BindPane request, AgentView.pane_session + pane_id fields |
| `crates/rally-daemon/src/services.rs` | bind_pane method, atomic save methods, agent_to_view with pane fields |

### Key Patterns Discovered (Phase 4)

- **CLI-owned Zellij calls**: Daemon is Zellij-unaware. CLI detects session via `ZELLIJ_SESSION_NAME` env (PluginBootstrap) or creates one (StandaloneBootstrap). Pane spawning happens in CLI process.
- **_attach shim flow**: `rally _attach <id> <cmd>` → PaneContext::from_env → BindPane IPC → exec(cmd). The exec syscall replaces the rally process with the agent command.
- **rally up exec-in-place**: StandaloneBootstrap::up uses `CommandExt::exec` — replaces the CLI process with zellij. Does not return on success.
- **Zellij 0.44.1 required**: dump-screen --pane-id (0.44.0), focus-pane-with-id (0.44.1). Verified locally.
- **--session flag position**: Must come BEFORE `action` in zellij CLI: `zellij --session S action new-pane ...`

### Decisions Made

| Decision | Rationale |
|----------|-----------|
| spawn_command stays CLI-local, not in domain model | Hexagonal arch: rally-core has zero Zellij. Command is opaque to daemon. If respawn needed later, use Agent.metadata HashMap. |
| CLI owns all Zellij calls (not daemon) | Daemon doesn't need Zellij in Phase 4. Plugin sidebar (Phase 7) will add daemon→Zellij path. |
| StandaloneBootstrap::up uses exec() | User expects to land inside the session immediately. Two-step "spawn + attach" is worse UX. |
| Reverted spawn_command from Agent model + migration v3 | User feedback: "what are all these AgentModel changes about?" — over-engineering for Phase 4. |
| ral-gva (integration tests) deferred | Manual gate verification first. E2E tests gated on RALLY_E2E_ZELLIJ=1. |

## Work Completed

### Phase 4 (ral-2zv) — All 10 subtasks closed

| Task | What was built |
|------|---------------|
| ral-ieu (P1 bug) | Atomic save_workspace_and_event / save_agent_and_event via SQLite transaction; insert_* helpers extracted |
| ral-ufa | SessionHandle + PluginBootstrap (env) + StandaloneBootstrap (exec-in-place up, force-delete down) |
| ral-s4p | ZellijActions: new_pane, dump_screen --pane-id, focus_pane_with_id, rename_pane (zellij 0.44.1) |
| ral-821 | PluginBootstrap::detect reads ZELLIJ_SESSION_NAME, 2 negative PaneContext tests |
| ral-vzk | rally up (exec into zellij session), rally down (delete-session --force) |
| ral-mxk | rally _attach shim: PaneContext::from_env → BindPane IPC → exec replacement |
| ral-8gl | Shim correlation logging: log_attach_correlation at crosspoint, log_attach_env_missing |
| ral-khz | DumpScreenSource + LineIndexedRing + CaptureSource trait; 4 ring buffer tests |
| ral-336 | rally capture snapshot/tail --follow wired to real pane_id from AgentView |
| ral-c3m | BUNDLED_LAYOUT_KDL const, rally layout export, install-plugin placeholder |

### Phase 4.5 Planning (ral-9rt) — 18 issues filed

Code review identified issues in 3 categories:
- **P1 bugs** (3): EventLog returns empty, alias CLI not wired, clippy missing_docs
- **P2 tasks** (7): xtask ci, StateSnapshot projections, CLI test assertions, BindPane integration test, fake zellij tests, vacuous crash test, transition table tests
- **P3 tasks** (8): private domain fields, sync mutex, IPC negatives, config tests, ring edge cases, shim env safety

## Pending Work

### Phase 4 Gate (NOT YET VERIFIED)

The PLAN says "two manual runs, both green":
1. Plugin mode: `rally workspace new demo && rally agent spawn --workspace <id> --role test -- htop` → pane opens, `rally capture tail <agent-id>` streams
2. Standalone mode: `rally up demo` → session created, `rally down demo` → cleaned up

Neither has been manually tested yet.

## Immediate Next Steps

1. **Manual gate testing** — Run both Phase 4 gate scenarios inside a real Zellij session
2. **Fix P1 bugs from Phase 4.5** — Start with ral-bzi (EventLog returns empty), ral-d9z (alias wiring), ral-yje (clippy)
3. **Implement xtask ci** (ral-66h) — fmt + clippy + test + deny
4. **Begin Phase 5** — Claude Code hooks + Inbox + Config doctor (ral-lu5)

### Open Issues Summary

```
bd stats output:
- Phase 4.5 (ral-9rt): 18 open issues (3 P1, 7 P2, 8 P3)
- Phase 5 (ral-lu5): blocked on Phase 4 epic (now closed)
- Phases 6-9: planned, all blocked on Phase 5
```

## Important Context

1. **All code pushed** — `git status` should show clean. Branch: main.
2. **27 tests pass** — `cargo test --workspace`. No test failures.
3. **Zellij 0.44.1 installed** — `zellij --version` confirms. Required for --pane-id flag.
4. **deny.toml is strict** — add deps one crate at a time, build after each.
5. **RALLY_DAEMON_SOCKET_PATH** env var is the key integration seam.
6. **No Zellij SDK dependency** — all Zellij calls go through `std::process::Command` to the `zellij` binary.
7. **EventLog::list_for_workspace returns empty** — this is a known P1 bug (ral-bzi). Don't trust event replay until fixed.
8. **Alias CLI is dead code** — RallyService has set_alias/resolve_alias but no proto/CLI wiring (ral-d9z).
9. **Phase 4.5 epic**: `bd show ral-9rt` for full issue list.
10. **Phase roadmap**: `bd show ral-lu5` (Phase 5), `bd show ral-3qa` (Phase 6), etc.

---

**Security Reminder**: No secrets in this handoff. Remote is SSH-based with key reference only.
