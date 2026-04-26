# Rally — dev / prod workflow
# ──────────────────────────────────────────────────────────────────────────────
#   make dev           wipe dev DB, build everything, start daemon + zellij
#   make dev-restart   rebuild plugin + CLI, restart daemon, relaunch zellij
#   make dev-plugin    rebuild wasm only (zellij hot-reloads via skip_plugin_cache)
#   make dev-daemon    start dev daemon in foreground (for daemon iteration)
#   make dev-clean     wipe dev state
#   make dev-status    show running dev daemon, socket, DB size
#   make test          cargo test --workspace
#   make ci            cargo xtask ci (fmt + clippy + test)
#   make prod          build CLI + daemon + plugin (no state touch)
#   make kill          kill THIS worktree's dev daemon only
# ──────────────────────────────────────────────────────────────────────────────

.PHONY: dev dev-restart dev-plugin dev-daemon dev-clean dev-status \
        dev-layout dev-permissions \
        test ci prod build kill help

# ── Paths ────────────────────────────────────────────────────────────────────
# Everything under target/ — per-worktree, gitignored, no cross-worktree collisions.
DEV_STATE     := $(CURDIR)/target/dev-state
DEV_SOCKET    := $(DEV_STATE)/rally.sock
DEV_LAYOUT    := $(DEV_STATE)/sidebar-dev.kdl
WASM_TARGET   := wasm32-wasip1
WASM_ABS      := $(CURDIR)/target/$(WASM_TARGET)/release/rally-plugin.wasm
PERMS_CACHE   := $(HOME)/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl

# Share compiled dependency artifacts across worktrees via sccache.
# Falls back silently if sccache is not installed.
ifneq (,$(shell command -v sccache 2>/dev/null))
  export RUSTC_WRAPPER := sccache
endif

# Exported into daemon AND zellij so the plugin's child `rally` CLI
# inherits the same socket + data dir.
export RALLY_DATA_DIR           := $(DEV_STATE)
export RALLY_DAEMON_SOCKET_PATH := $(DEV_SOCKET)
export RALLY_LOG                := rally=debug
export PATH                     := $(CURDIR)/target/debug:$(PATH)

# ── Shared: start daemon in background, wait for socket ─────────────────────
define start-dev-daemon
	@mkdir -p "$(DEV_STATE)"
	@echo "→ starting dev daemon (socket=$(DEV_SOCKET))"
	@./target/debug/rallyd > "$(DEV_STATE)/rallyd.log" 2>&1 & \
		echo $$! > "$(DEV_STATE)/rallyd.pid"
	@n=0; while [ $$n -lt 10 ]; do \
		[ -S "$(DEV_SOCKET)" ] && break; \
		n=$$((n + 1)); sleep 0.3; \
	done; \
	if [ ! -S "$(DEV_SOCKET)" ]; then \
		PID=$$(cat "$(DEV_STATE)/rallyd.pid" 2>/dev/null); \
		echo ""; \
		echo "✗ daemon failed to start after 3s"; \
		echo "  pid file: $$PID"; \
		if [ -n "$$PID" ] && kill -0 $$PID 2>/dev/null; then \
			echo "  process:  still running (socket never appeared)"; \
		else \
			echo "  process:  exited"; \
		fi; \
		echo "  stderr:"; \
		cat "$(DEV_STATE)/rallyd.log" 2>/dev/null | tail -10 | sed 's/^/    /'; \
		echo "  tracing:"; \
		tail -5 "$$HOME/.local/state/rally/logs/rally-daemon.log.$$(date +%Y-%m-%d)" 2>/dev/null | sed 's/^/    /' || echo "    (no log)"; \
		exit 1; \
	fi
	@echo "→ daemon running (pid $$(cat $(DEV_STATE)/rallyd.pid))"
endef

# ── Dev targets ──────────────────────────────────────────────────────────────

help:
	@echo "Dev loop:"
	@echo "  make dev           full reset: wipe DB → build → daemon → zellij"
	@echo "  make dev-restart   rebuild → restart daemon → relaunch zellij"
	@echo "  make dev-plugin    rebuild wasm only (zellij hot-reloads via skip_plugin_cache)"
	@echo "  make dev-daemon    start dev daemon in foreground"
	@echo "  make dev-clean     wipe dev DB + logs"
	@echo "  make dev-status    show running daemon, socket, DB"
	@echo ""
	@echo "Quality:"
	@echo "  make test          cargo test --workspace"
	@echo "  make ci            fmt + clippy + test"
	@echo ""
	@echo "Other:"
	@echo "  make prod          release build (no state touch)"
	@echo "  make kill          kill this worktree's dev daemon"

dev-clean:
	@echo "→ wiping dev state at $(DEV_STATE)"
	rm -rf "$(DEV_STATE)"
	mkdir -p "$(DEV_STATE)"

# Generate a dev layout that points at this worktree's wasm build artifact.
dev-layout:
	@mkdir -p "$(DEV_STATE)"
	@printf 'layout {\n\
	    pane split_direction="vertical" {\n\
	        pane name="main" size="75%%"\n\
	        pane name="rally-sidebar" size="25%%" {\n\
	            plugin location="file:$(WASM_ABS)" {\n\
	                skip_plugin_cache true\n\
	                _allow_exec_host_cmd true\n\
	            }\n\
	        }\n\
	    }\n\
	}\n' > "$(DEV_LAYOUT)"
	@echo "→ layout generated at $(DEV_LAYOUT)"

# Ensure the permissions cache grants this worktree's wasm the needed permissions.
dev-permissions:
	@if ! grep -qF "$(WASM_ABS)" "$(PERMS_CACHE)" 2>/dev/null; then \
		mkdir -p "$$(dirname $(PERMS_CACHE))"; \
		printf '"$(WASM_ABS)" {\n    RunCommands\n    ReadApplicationState\n    ChangeApplicationState\n}\n' >> "$(PERMS_CACHE)"; \
		echo "→ permissions added for $(WASM_ABS)"; \
	fi

# Full dev loop: clean slate, build everything, launch.
dev: kill dev-clean build dev-plugin dev-layout dev-permissions
	$(start-dev-daemon)
	@echo "→ launching zellij"
	@zellij --new-session-with-layout "$(DEV_LAYOUT)" || true
	@echo "→ zellij exited, stopping daemon"
	@kill $$(cat "$(DEV_STATE)/rallyd.pid" 2>/dev/null) 2>/dev/null || true
	@echo "→ done"

# Rebuild + restart without wiping state.
dev-restart: kill build dev-plugin dev-layout dev-permissions
	$(start-dev-daemon)
	@echo "→ launching zellij"
	@zellij --new-session-with-layout "$(DEV_LAYOUT)" || true
	@kill $$(cat "$(DEV_STATE)/rallyd.pid" 2>/dev/null) 2>/dev/null || true
	@echo "→ done"

# Just the wasm — useful when only plugin code changed. Zellij picks up the
# new .wasm on next pane open because the layout has skip_plugin_cache true.
dev-plugin:
	cargo build -p rally-plugin --target $(WASM_TARGET) --release
	@echo "→ plugin built at $(WASM_ABS)"

# Foreground daemon (no zellij). Ctrl-C to stop.
dev-daemon: kill build dev-clean
	@echo "→ starting dev daemon in foreground (Ctrl-C to stop)"
	./target/debug/rallyd

dev-status:
	@echo "Dev state dir: $(DEV_STATE)"
	@ls -lh "$(DEV_STATE)/state.db" 2>/dev/null || echo "  (no DB)"
	@if [ -f "$(DEV_STATE)/rallyd.pid" ] && kill -0 $$(cat "$(DEV_STATE)/rallyd.pid") 2>/dev/null; then \
		echo "  daemon: running (pid $$(cat $(DEV_STATE)/rallyd.pid))"; \
	else \
		echo "  daemon: not running"; \
	fi
	@ls -la "$(DEV_SOCKET)" 2>/dev/null || echo "  socket: not found"
	@echo "  wasm:   $(WASM_ABS)"
	@ls -lh "$(WASM_ABS)" 2>/dev/null || echo "          (not built)"
	@echo "  layout: $(DEV_LAYOUT)"
	@ls -lh "$(DEV_LAYOUT)" 2>/dev/null || echo "          (not generated)"

# ── Build ────────────────────────────────────────────────────────────────────

build:
	cargo build

# ── Quality gates ────────────────────────────────────────────────────────────

test:
	cargo test --workspace

ci:
	cargo xtask ci

# ── Prod ─────────────────────────────────────────────────────────────────────

prod:
	cargo build --release
	cargo build -p rally-plugin --target $(WASM_TARGET) --release

# ── Utilities ────────────────────────────────────────────────────────────────

# Kill only THIS worktree's dev daemon (by pid file). Never pkill globally.
kill:
	@if [ -f "$(DEV_STATE)/rallyd.pid" ]; then \
		PID=$$(cat "$(DEV_STATE)/rallyd.pid"); \
		if kill -0 $$PID 2>/dev/null; then \
			kill $$PID && echo "→ killed dev daemon (pid $$PID)"; \
		fi; \
		rm -f "$(DEV_STATE)/rallyd.pid"; \
	fi
	@rm -f "$(DEV_SOCKET)"
