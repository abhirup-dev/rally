# Handoff: Logging Sprint Done, Phase 3 Starting

## Session Metadata
- Created: 2026-04-24 00:35:00
- Project: /Users/abhirupdas/Codes/Personal/rally
- Branch: main
- Session duration: ~30 minutes
- Continues from: 2026-04-24-001903-phase2-done-logging-infra-wip.md

### Recent Commits
  - c21517e logging sprint: instrument rally-core, rally-store, binary tracing init
  - f54048a wip: add tracing deps to workspace + handoff document
  - 8aaa1a2 phase 2: rally-store — SQLite WAL persistence layer

## Current State Summary

Phases 0–2 complete. Logging sprint complete and **pushed to origin/main**. All 10 workspace tests pass. Phase 3 (Daemon + CLI + Config) is claimed and about to start — all 11 subtasks are `in_progress`, assigned to Abhirup Das.

## Work Completed This Session

### Logging Tasks Closed (6)
- `ral-hzg` — tracing deps verified in workspace + rally-core + rally-store
- `ral-kmc` — `warn!` on `InvalidTransition` in `rally-core/src/agent.rs:89` (state + trigger fields)
- `ral-gct` — `info!/debug!/error!` on `Store::open`, `configure()`, `migrate()` in `rally-store/src/db.rs`
- `ral-55d` — `debug!` on `EventLog::append` in `rally-store/src/event_log.rs` (kind, workspace_id, at_ms)
- `ral-60v` — Per-binary tracing subscriber: `rally-daemon/src/tracing_init.rs` + `rally-cli/src/tracing_init.rs`. Daily rolling file appenders to `~/.local/state/rally/logs/`, `RALLY_LOG` env-filter, `WorkerGuard`. **Note**: implements per-binary file logging (not per-module Targets-filtered routing as originally spec'd — simplified to avoid complexity).
- `ral-mdx` — Logging section in `CLAUDE.md`: `RALLY_LOG` examples, file locations, targets

### Other Housekeeping
- `ral-mul` (IPC boundary logging) → **superseded by `ral-2l9`** which covers same scope within Phase 3 context
- Created 8 Phase 3 logging sub-tasks (one per implementation task, each blocked on its parent):
  `ral-2l9`, `ral-anw`, `ral-0v7`, `ral-wgv`, `ral-s4a`, `ral-ibs`, `ral-da1`, `ral-cxv`, `ral-d0n`
- Each logging task requires **advisor consultation** scoped to only the logic just implemented

## Phase 3 Implementation Plan (Advisor-Reviewed)

Advisor reviewed the Phase 3 approach and provided sequencing + 4 traps to watch:

### Sequencing (tightest-constraint-first)
1. `rally-config` — pure, no runtime deps
2. `rally-events` — tokio broadcast + arc-swap
3. Session naming in `rally-core` (canonical key generator) + `rally-store` migration for `aliases` table
4. `rally-proto` — add `RequestEnvelope { request_id, payload }` wrapper
5. `rallyd` IPC server (tokio::net::UnixListener, not interprocess — simpler, avoids dep)
6. `rallyd` services (WorkspaceService, AgentService)
7. `rallyd` autostart (double-fork + pid file)
8. `rally-cli` clap tree + IPC client
9. `DaemonHarness` in `rally-test-utils`
10. Integration tests (assert_cmd proving the gate)

### Advisor Traps Flagged
1. **ral-mul vs ral-2l9 duplication** — RESOLVED: superseded ral-mul → ral-2l9
2. **deny.toml `multiple-versions = "deny"`** — add deps per-crate, build after each, not all at once
3. **RequestEnvelope needed** — current `Request` has no `request_id` or `client_pid`. Must add `RequestEnvelope` to `rally-proto` BEFORE building IPC server. `client_pid`: self-reported by client (not from socket peer credentials — avoids platform complexity for a single-user tool)
4. **CLI error visibility** — `tracing_init` sends to file only. Every CLI error path needs explicit `eprintln!` (or `--json`-aware stderr writer) so user isn't left staring at silence. Don't let errors disappear into the file-only seam.

### Other Advisor Notes
- `DaemonHarness` must `Child::kill()` on `Drop` — or tests leak
- Gate test: `rally agent ls --json | jq` piped to `serde_json::from_slice` as an `assert_cmd` test
- Use `tokio::net::UnixListener` (not `interprocess`) — simpler, avoids a dep, peer_cred via `UCred` on Linux (on macOS client self-reports pid)
- `cargo-deny` not installed locally — CI will verify

## Decisions Made This Session

| Decision | Rationale |
|----------|-----------|
| Per-binary (not per-module) file logging | Simpler; per-module routing via Targets filter adds complexity without proportional value at current scale |
| Supersede ral-mul → ral-2l9 | ral-2l9 explicitly covers IPC boundary logging with full Phase 3 context |
| Use tokio::net::UnixListener not interprocess | Simpler, avoids extra dep, avoids deny.toml dup-version risk |
| Self-reported client_pid not socket peer credentials | Single-user tool; avoids macOS/Linux platform divergence |

## Critical Files (new this session)

| File | Purpose |
|------|---------|
| `crates/rally-daemon/src/tracing_init.rs` | Daemon tracing subscriber: daily rolling file appender, RALLY_LOG env-filter |
| `crates/rally-cli/src/tracing_init.rs` | CLI tracing subscriber: same pattern, warn default |
| `CLAUDE.md` (Logging section) | RALLY_LOG examples, log file locations, log targets per module |

## Context for Resuming Agent

1. **Phase 3 is actively claimed** — all 11 subtasks in_progress, assigned. Start with `rally-config` (ral-fq5.3 + ral-fq5.2).
2. **Index PLAN.md with context-mode** at session start for design spec lookup.
3. **deny.toml is strict** — add deps one crate at a time, build after each.
4. **RequestEnvelope** must be added to `rally-proto` before the IPC server (step 4 in sequence).
5. **ral-8gl** (shim pane correlation logging) remains open — Phase 4 task.
6. **Git remote**: `origin` → `git@github.com:abhirup-dev/rally.git` (SSH alias `github-me`).
7. **cargo-deny not installed** — `cargo deny check` won't run locally; CI handles it.
8. After each implementation task, consult advisor for its paired logging task.

---

**Security Reminder**: No secrets in this handoff. Remote is SSH-based with key reference only.
