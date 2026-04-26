# Project Instructions for AI Agents

This file provides instructions and context for AI coding agents working on this project.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->


## Build & Test

```bash
# Preferred: use Makefile targets
make dev             # wipe dev DB, build everything, start daemon + zellij
make dev-restart     # rebuild + restart without wiping state
make dev-plugin      # rebuild wasm only (zellij hot-reloads)
make test            # cargo test --workspace
make ci              # fmt + clippy + test

# Raw commands (if you need them):
cargo build                # uses default-members (excludes rally-plugin wasm)
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

### Dev / Prod isolation

Everything dev-related lives under `target/` — per-worktree, gitignored, no cross-worktree collisions:

| | DB | Socket | Wasm | Layout |
|---|---|---|---|---|
| **Dev** | `target/dev-state/` | `target/dev-state/rally.sock` | `target/.../rally-plugin.wasm` | `target/dev-state/sidebar-dev.kdl` |
| **Prod** | `~/.local/share/rally/` | `/tmp/rally/rally.sock` | N/A | `layouts/sidebar-dev.kdl` |

`make dev-layout` generates a KDL layout with the absolute wasm path for this worktree.
`make dev-permissions` ensures the Zellij permissions cache includes this worktree's wasm.
`make kill` only kills this worktree's daemon (via pid file), never touches other worktrees.

### Plugin build notes

- `rally-plugin` is a **binary** crate (not cdylib). The `register_plugin!` macro generates `main()` and `_start`.
- It is excluded from `default-members` — never builds with `cargo build --workspace`.
- Zellij aggressively caches plugin wasm. The dev layout uses `skip_plugin_cache true` to always load fresh.
- Artifact path: `target/wasm32-wasip1/release/rally-plugin.wasm` (hyphen, not underscore).
- **Permissions**: The plugin needs `RunCommands` + `ReadApplicationState` + `ChangeApplicationState`.
  `make dev-permissions` handles this automatically. For manual setup, append to `~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl`.
  Without pre-granted permissions, `request_permission()` shows a dialog that blocks plugin rendering in narrow panes.

## Architecture Overview

Hexagonal / ports-and-adapters. `rally-core` — pure domain model, no IO, no async, no Zellij.
`rally-store` — SQLite WAL persistence implementing `rally-core::ports` traits.
`rally-daemon` — tokio runtime, unix socket IPC, service layer (Phase 3).
`rally-cli` — clap CLI talking to the daemon (Phase 3).

## Logging

Structured tracing with the `tracing` crate. Log level controlled by `RALLY_LOG` env var.

```bash
# Set log level
RALLY_LOG=debug rallyd           # daemon: all debug
RALLY_LOG=rally_store=trace rallyd   # store only, trace level
RALLY_LOG=info rallyd            # default

# Tail log files (Phase 3+)
tail -f ~/.local/state/rally/logs/rally-daemon.log
tail -f ~/.local/state/rally/logs/rally-store.log
tail -f ~/.local/state/rally/logs/rally-cli.log
```

Log targets per module: `rally_core`, `rally_store`, `rally_daemon`, `rally_cli`.
Library crates emit tracing events/spans but never install a subscriber — only binary
entry points (`rallyd`, `rally`) call the init function.

## Conventions & Patterns

- IDs are `Ulid` wrapped in newtypes (`WorkspaceId`, `AgentId`, etc.) — no raw strings across boundaries.
- `Timestamp` is a newtype over `u64` (unix ms) — `Copy` + `Ord`, no heap.
- `DomainEvent` is `#[non_exhaustive]` — always add a wildcard arm in external match.
- `thiserror` in libraries, `anyhow` only at binary boundaries.
- Zero `unwrap`/`expect` outside tests and `main`.
