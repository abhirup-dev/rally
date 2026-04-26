# Rally — dev / prod workflow
# ──────────────────────────────────────────────────────────────────────────────
#   make dev           wipe dev DB, build everything, start daemon + zellij
#   make dev-restart   rebuild plugin + CLI, restart daemon, relaunch zellij
#   make dev-plugin    rebuild wasm only, install — hot-reload via skip_plugin_cache
#   make dev-daemon    start dev daemon in foreground (for daemon iteration)
#   make dev-clean     wipe dev state
#   make dev-status    show running dev daemon, socket, DB size
#   make test          cargo test --workspace
#   make ci            cargo xtask ci (fmt + clippy + test)
#   make prod          build CLI + daemon + plugin (no state touch)
#   make kill           kill any running rallyd
# ──────────────────────────────────────────────────────────────────────────────

.PHONY: dev dev-restart dev-plugin dev-daemon dev-clean dev-status \
        test ci prod build kill help

# ── Paths ────────────────────────────────────────────────────────────────────
DEV_STATE    := $(CURDIR)/target/dev-state
DEV_SOCKET   := $(DEV_STATE)/rally.sock
DEV_LAYOUT   := layouts/sidebar-dev.kdl
WASM_SRC     := target/wasm32-wasip1/release/rally-plugin.wasm
WASM_DST     := $(HOME)/.config/rally/rally.wasm

# Exported into daemon AND zellij so the plugin's child `rally` CLI
# inherits the same socket + data dir.
export RALLY_DATA_DIR          := $(DEV_STATE)
export RALLY_DAEMON_SOCKET_PATH := $(DEV_SOCKET)
export RALLY_LOG               := rally=debug
export PATH                    := $(CURDIR)/target/debug:$(PATH)

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
	@echo "  make kill          kill any running rallyd"

dev-clean:
	@echo "→ wiping dev state at $(DEV_STATE)"
	rm -rf "$(DEV_STATE)"
	mkdir -p "$(DEV_STATE)"

# Full dev loop: clean slate, build everything, launch.
dev: kill dev-clean build dev-plugin
	@echo "→ starting dev daemon (socket=$(DEV_SOCKET))"
	@./target/debug/rallyd > "$(DEV_STATE)/rallyd.log" 2>&1 & \
		echo $$! > "$(DEV_STATE)/rallyd.pid"
	@sleep 0.3
	@if [ ! -S "$(DEV_SOCKET)" ]; then \
		echo "✗ daemon failed to start — check $(DEV_STATE)/rallyd.log"; \
		cat "$(DEV_STATE)/rallyd.log" | tail -10; \
		exit 1; \
	fi
	@echo "→ daemon running (pid $$(cat $(DEV_STATE)/rallyd.pid))"
	@echo "→ launching zellij"
	@zellij --new-session-with-layout "$(DEV_LAYOUT)" || true
	@echo "→ zellij exited, stopping daemon"
	@kill $$(cat "$(DEV_STATE)/rallyd.pid" 2>/dev/null) 2>/dev/null || true
	@echo "→ done"

# Rebuild + restart without wiping state. Useful when iterating on code
# but want to keep any workspaces/agents created during the session.
dev-restart: kill build dev-plugin
	@mkdir -p "$(DEV_STATE)"
	@echo "→ restarting dev daemon"
	@./target/debug/rallyd > "$(DEV_STATE)/rallyd.log" 2>&1 & \
		echo $$! > "$(DEV_STATE)/rallyd.pid"
	@sleep 0.3
	@if [ ! -S "$(DEV_SOCKET)" ]; then \
		echo "✗ daemon failed to start"; exit 1; \
	fi
	@echo "→ daemon running (pid $$(cat $(DEV_STATE)/rallyd.pid))"
	@echo "→ launching zellij"
	@zellij --new-session-with-layout "$(DEV_LAYOUT)" || true
	@kill $$(cat "$(DEV_STATE)/rallyd.pid" 2>/dev/null) 2>/dev/null || true
	@echo "→ done"

# Just the wasm — useful when only plugin code changed. Zellij picks up the
# new .wasm on next pane open because the layout has skip_plugin_cache true.
dev-plugin:
	cargo build -p rally-plugin --target wasm32-wasip1 --release
	@mkdir -p "$$(dirname $(WASM_DST))"
	cp "$(WASM_SRC)" "$(WASM_DST)"
	@echo "→ plugin installed at $(WASM_DST)"

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
	@echo ""
	@echo "Prod:"
	@pgrep -fl rallyd 2>/dev/null | grep -v "target/dev-state" || echo "  no prod daemon running"
	@ls -lh "$$HOME/.local/share/rally/state.db" 2>/dev/null || echo "  no prod DB"

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
	cargo build -p rally-plugin --target wasm32-wasip1 --release
	@mkdir -p "$$(dirname $(WASM_DST))"
	cp "$(WASM_SRC)" "$(WASM_DST)"

# ── Utilities ────────────────────────────────────────────────────────────────

kill:
	@if [ -f "$(DEV_STATE)/rallyd.pid" ]; then \
		kill $$(cat "$(DEV_STATE)/rallyd.pid") 2>/dev/null && \
			echo "→ killed dev daemon (pid $$(cat $(DEV_STATE)/rallyd.pid))" || true; \
		rm -f "$(DEV_STATE)/rallyd.pid"; \
	fi
	@pkill -f 'rallyd' 2>/dev/null && echo "→ killed other rallyd processes" || true
	@rm -f "$(DEV_SOCKET)"
