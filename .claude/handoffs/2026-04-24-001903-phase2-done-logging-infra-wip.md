# Handoff: Rally Phase 0-2 Done, Logging Infra WIP

## Session Metadata
- Created: 2026-04-24 00:19:03
- Project: /Users/abhirupdas/Codes/Personal/rally
- Branch: main
- Session duration: ~2 hours

### Recent Commits (for context)
  - 8aaa1a2 phase 2: rally-store — SQLite WAL persistence layer
  - a4e25c9 phase 1: pure domain model in rally-core + rally-proto + rally-test-utils
  - 88f6b76 phase 0: scaffold 14-crate Cargo workspace
  - 61bb3bc bd init: initialize beads issue tracking
  - 2e0a035 Initial commit

## Handoff Chain

- **Continues from**: None (fresh start)
- **Supersedes**: None

## Current State Summary

Rally is a terminal-native multi-agent orchestrator (Rust + Zellij). Phases 0-2 are complete and pushed to `origin/main` at `github.com/abhirup-dev/rally`. Phase 0 scaffolded a 14-crate Cargo workspace. Phase 1 implemented the pure domain model (state machine, entities, events, ports). Phase 2 added SQLite WAL persistence. All 10 tests pass (6 proptests at 200k cases + 4 crash-restart integration tests).

Work was in progress on a **logging/tracing instrumentation** sprint when the session ended. The beads task list has been reshaped per advisor guidance — low-value tasks closed, high-value crosspoint logging tasks created. `tracing` deps were added to workspace Cargo.toml and per-crate Cargo.tomls, but **no instrumentation code has been written yet** — the commit is uncommitted.

## Codebase Understanding

### Architecture Overview

Hexagonal / ports-and-adapters. `rally-core` has zero IO, zero async, zero Zellij — pure domain model. Side-effects live behind traits (`ports.rs`) implemented by outer crates. Events are the source of truth (append-only `DomainEvent` enum). `rally-store` implements persistence on SQLite WAL. `rally-proto` has serde wire types. `rally-test-utils` provides `InMemoryRepo`, `FakeClock`, `FakeIdGen`.

### Critical Files

| File | Purpose | Relevance |
|------|---------|-----------|
| `PLAN.md` | Full 1400-line implementation plan | Source of truth for all design decisions. Index it with context-mode `ctx_index` at session start. |
| `crates/rally-core/src/agent.rs` | AgentState machine + Agent entity | Core domain logic. 8 states, 11 triggers, pure `transition()` fn. |
| `crates/rally-core/src/event.rs` | DomainEvent enum (#[non_exhaustive]) | 10 variants. Source of truth for all state changes. |
| `crates/rally-core/src/ports.rs` | Trait ports (WorkspaceRepo, AgentRepo, EventLog, Clock, IdGen) | Contract between core and outer crates. |
| `crates/rally-store/src/db.rs` | Store::open, WAL config, migrations | Hand-rolled PRAGMA user_version migrations. |
| `crates/rally-store/tests/crash_restart.rs` | Phase 2 gate tests | 4 tests: reopen survival, append-only events, rollback safety. |
| `deny.toml` | Dependency firewall | Enforces: rusqlite only in rally-store, tokio banned from core/proto, etc. |

### Key Patterns Discovered

- IDs are `Ulid` wrapped in newtypes with a `id_newtype!` macro. Optional `serde` feature flag on rally-core enables Serialize/Deserialize for proto layer.
- `Timestamp` is a newtype over `u64` (unix ms), not `SystemTime`, for `Copy` + total ordering.
- `DomainEvent` is `#[non_exhaustive]` — the store's `event_to_stored()` has a wildcard arm for forward compat.
- Store uses `Mutex<Connection>` (not r2d2 pool yet) — single-writer pattern matches the plan's "single writer task" architecture.
- `EventLog::list_for_workspace` returns `Vec<DomainEvent>` but Phase 2 doesn't reconstruct events from JSON — just verifies row counts. Full deserialization deferred to Phase 3.

## Work Completed

### Tasks Finished

- [x] Phase 0 — Workspace Skeleton (ral-7lu, 5 children — all closed)
- [x] Phase 1 — Core Domain (ral-np3, 9 children — all closed)
- [x] Phase 2 — Persistence (ral-bku, 4 children — all closed)
- [x] Reshaped logging task list per advisor guidance (closed noise tasks, created high-value ones)

### Files Modified (uncommitted)

| File | Changes | Rationale |
|------|---------|-----------|
| `Cargo.toml` | Added `tracing`, `tracing-subscriber`, `tracing-appender` to workspace deps | Logging infra setup |
| `crates/rally-core/Cargo.toml` | Added `tracing = { workspace = true }` | Enable instrumentation in core |
| `crates/rally-store/Cargo.toml` | Added `tracing = { workspace = true }` | Enable instrumentation in store |

### Decisions Made

| Decision | Options Considered | Rationale |
|----------|-------------------|-----------|
| `Timestamp` as `u64` not `SystemTime` | SystemTime, chrono::DateTime, u64 ms | Copy + Ord + no heap allocation, converts cleanly to/from SQLite INTEGER |
| `Mutex<Connection>` not r2d2 pool | r2d2, single mutex, async sqlx | Single-writer pattern per plan §12; r2d2 adds deps, not needed until daemon |
| `serde` as optional feature on rally-core | Always-on serde, no serde in core | Keeps core serde-free by default (plan §5.1) while letting proto enable it |
| Closed entity constructor logging tasks | Log everything vs log crosspoints | Advisor: entity constructors have no IO/failure modes; SQLite events table IS the audit log |
| Reshaped logging to focus on crosspoints | Blanket CRUD logging vs targeted | IPC boundary, shim correlation, hook execution, supervisor events — these are where bugs hide |

## Pending Work

### Immediate Next Steps

1. **Implement remaining logging tasks** (uncommitted deps already added):
   - `ral-hzg` — tracing deps done, need to verify `cargo deny check` passes
   - `ral-kmc` — Add `warn!` on `InvalidTransition` in `transition()` fn
   - `ral-gct` — Add `info!` on `Store::open` and `migrate()`
   - `ral-55d` — Add `debug!` on `EventLog::append`
   - `ral-60v` — Create tracing-init module with per-module file appenders (Targets filter + rolling files)
   - `ral-mdx` — Document log targets in CLAUDE.md
2. **Commit the logging work** and close the tasks
3. **Start Phase 3** — Daemon + CLI skeleton + config (ral-fq5 epic, 11 children)

### Blockers/Open Questions

- None blocking

### Deferred Items

- `ral-mul` (IPC boundary logging) — deferred to Phase 3 when daemon IPC exists
- `ral-8gl` (shim pane correlation logging) — deferred to Phase 4 when _attach shim exists
- Full `EventLog::list_for_workspace` deserialization — deferred to Phase 3

## Context for Resuming Agent

### Important Context

1. **Use `bd` for all task tracking** — not TodoWrite/TaskCreate. Run `bd ready` to see available work. Run `bd prime` after compaction/new session for full command reference.
2. **Index PLAN.md with context-mode** at session start: `ctx_index` with path to PLAN.md, source "Rally PLAN.md". Then use `ctx_search` to look up specs before implementing.
3. **The deny.toml firewall is strict**: `multiple-versions = "deny"`. Adding a new dep can fail if it pulls a second version of an existing transitive dep. Always run `cargo deny check` after adding deps.
4. **Logging architecture per advisor**: Use `tracing` spans (not just events), structured fields (not format strings), per-module file routing via `tracing_subscriber::filter::Targets` + multiple Layers. Each binary process is its own subscriber. `WorkerGuard` must live for process lifetime. WASM plugin can NOT use tracing-subscriber — route via zellij pipe.
5. **Git remote**: `origin` → `git@github.com:abhirup-dev/rally.git` (via SSH alias `github-me` in `~/.ssh/config`, identity `~/.ssh/id_abhirupdev`). gitconfig for `~/Codes/Personal/` uses `~/.gitconfig_me` which rewrites github.com to github-me host.
6. **Phase 3 is next** — daemon + CLI skeleton + config. Check `bd ready` after closing logging tasks to see ral-fq5 children unblocked.

### Assumptions Made

- `tracing` crate is sync/no-tokio — passes deny.toml firewall (verify after adding)
- Per-module file appenders use `tracing-appender::rolling` — not yet verified this works with `Targets` filter
- WASM plugin (rally-plugin) will NOT get a tracing subscriber — logs route through daemon

### Potential Gotchas

- `#[non_exhaustive]` on `DomainEvent` means pattern matches in external crates need a wildcard arm (already handled in `event_to_stored`)
- `rally-core` has `#![warn(missing_docs)]` — every public item needs a doc comment or you get warnings. Existing warnings are accepted for now.
- The `compact_str` crate is at 0.8 (0.9 available but not upgraded to avoid breakage)
- `rusqlite` with `bundled` feature compiles sqlite3 from source — first build after adding deps takes longer

## Environment State

### Tools/Services Used

- `bd` (beads) for issue tracking — `.beads/` directory in project root
- `rtk` (Rust Token Killer) — transparent CLI proxy via Claude Code hooks
- `cargo deny` for dependency auditing
- `proptest` for property-based testing (200k cases per property)

### Active Processes

- None (no daemon or servers running yet)

### Environment Variables

- `RALLY_LOG` — will control tracing verbosity (not yet implemented)
- `ZELLIJ_PANE_ID`, `ZELLIJ_SESSION_NAME` — used by _attach shim (Phase 4)

## Related Resources

- `PLAN.md` — full implementation plan (index with ctx_index)
- `multi_agent_zellij_requirements.md` — original requirements doc
- `deny.toml` — dependency firewall rules
- `.github/workflows/ci.yml` — CI matrix (fmt/clippy/test/deny)

---

**Security Reminder**: No secrets in this handoff. Remote is SSH-based with key reference only.
