# Keres — build orchestration for the binaries that share the
# `keres_engine` library crate.
#
# Each binary is built with the profile + Cargo features that suit its purpose:
#
#   keres  (CLI)      [profile.release]   speed — AI inspection / debugging tool
#   server (HTTP API) [profile.release]   speed — production AI latency
#   arena  (tuning)   [profile.release]   speed — engine-vs-engine level tuning
#   gui    (minifb)   [profile.gui]       size  — micro-keres <1.44 MB lineage
#
# The `gui` feature pulls in minifb; it is OPTIONAL so the server/CLI (and the
# musl Docker server build) stay free of the X11/minifb dependency. See the
# per-profile settings in Cargo.toml.

CARGO      ?= cargo
GUI_PROFILE := gui

# Binaries land in target/<profile>/ — release for cli/server, the named
# profile for the gui. Cargo's own build output there is still named `gui`
# (that's the [[bin]] target in Cargo.toml — renaming it would collide with
# the `keres` CLI target); GUI_BIN is the name the GUI actually ships under,
# produced by copying that output — see the `gui` target below.
CLI_BIN    := target/release/keres
SERVER_BIN := target/release/server
ARENA_BIN  := target/release/arena
GUI_CARGO_BIN := target/$(GUI_PROFILE)/gui
GUI_BIN    := target/$(GUI_PROFILE)/keres

.DEFAULT_GOAL := all

.PHONY: all help cli server arena gui test test-core fmt fmt-fix clippy deny smoke \
        check sizes run-cli run-server run-arena run-gui clean icons base symbols splash \
        app-icon pixel-assets macos-app

##@ Build

cli:  ## Plain-text CLI (keres): show-moves / engine-move / debug-tree
	$(CARGO) build --release --bin keres

server:  ## HTTP server (the binary wire API; see docs/PROTOCOL.md)
	$(CARGO) build --release --bin server

arena:  ## Engine-vs-engine arena (arena): `match` / `quality` for tuning the AI levels
	$(CARGO) build --release --bin arena

gui: src/gui/icons.rs src/gui/base.rs src/gui/symbols.rs src/gui/window_icon.rs src/gui/splash.rs assets/generated/keres.ico  ## Native minifb desktop GUI (size-optimized; enables the `gui` feature); ships as GUI_BIN (keres)
	$(CARGO) build --profile $(GUI_PROFILE) --bin gui --features gui
	@cp $(GUI_CARGO_BIN) $(GUI_BIN)
	@# UPX is applied opportunistically — it shrinks the GUI another ~50-60%
	@# but needs the `upx` binary (dnf install upx / apt install upx). No-op
	@# if absent, so the target stays deterministic without it.
	@if command -v upx >/dev/null 2>&1; then \
		echo "  upx: compressing $(GUI_BIN)"; \
		upx --best --lzma --quiet $(GUI_BIN) || true; \
	else \
		echo "  tip: install upx to compress $(GUI_BIN) further"; \
	fi

##@ Quality

test:  ## Full test suite incl. GUI module tests (headless; needs the gui feature)
	$(CARGO) test --workspace --features gui

test-core:  ## Test only the engine/CLI/server (no minifb feature)
	$(CARGO) test --workspace

fmt:  ## Check formatting (CI-blocking)
	$(CARGO) fmt --check

fmt-fix:  ## Apply rustfmt
	$(CARGO) fmt

clippy:  ## Lint everything incl. the GUI (CI-blocking: the same -D warnings pass runs in CI)
	$(CARGO) clippy --workspace --all-targets --features gui -- -D warnings

deny:  ## Audit dependencies: RUSTSEC advisories, licenses, sources (see deny.toml)
	$(CARGO) deny check --hide-inclusion-graph

smoke: server  ## Start the server on a scratch port and run the end-to-end wire-protocol smoke test
	@PORT=3999 ./$(SERVER_BIN) & \
	pid=$$!; \
	trap 'kill $$pid 2>/dev/null' EXIT; \
	for _ in $$(seq 1 30); do \
		curl -sSf http://127.0.0.1:3999/health >/dev/null 2>&1 && break; \
		sleep 0.2; \
	done; \
	scripts/smoke_test_server.sh http://127.0.0.1:3999

check: fmt clippy test  ## fmt + clippy + full test suite

##@ Inspect & run

sizes:  ## Print built binary sizes
	@for p in "$(CLI_BIN):cli" "$(SERVER_BIN):server" "$(GUI_BIN):gui"; do \
		path="$${p%%:*}"; name="$${p##*:}"; \
		if [ -f "$$path" ]; then \
			sz=$$(stat -c%s "$$path"); \
			awk -v n="$$name" -v p="$$path" -v s="$$sz" \
				'BEGIN{printf "  %-7s %9d B  %7.1f KB   %s\n", n, s, s/1024, p}'; \
		else \
			printf "  %-7s (not built — run 'make %s')\n" "$$name" "$$name"; \
		fi; \
	done

run-cli: cli  ## Run the CLI, e.g. `make run-cli ARGS='engine-move'`
	./$(CLI_BIN) $(ARGS)

run-server: server  ## Run the HTTP server (PORT env var selects the listen port)
	./$(SERVER_BIN)

run-arena: arena  ## Run the arena, e.g. `make run-arena ARGS='match L7 L8 --games 20'`
	./$(ARENA_BIN) $(ARGS)

run-gui: gui  ## Run the native GUI
	./$(GUI_BIN)

macos-app: gui  ## Assemble dist/Keres.app (macOS only — needs keres.icns; see scripts/package_macos_app.sh)
	scripts/package_macos_app.sh $(GUI_BIN) dist

##@ Pixel assets

ICON_SRCS := $(wildcard assets/pixel/icons/*.xcf)
BASE_SRCS := $(wildcard assets/pixel/base/*.xcf)
SYMBOL_SRCS := $(wildcard assets/pixel/symbols/*.xcf)

src/gui/icons.rs: $(ICON_SRCS) scripts/gen_icons.py scripts/pixel_raster.py
	python3 scripts/gen_icons.py

src/gui/base.rs: $(BASE_SRCS) scripts/gen_base.py scripts/pixel_raster.py
	python3 scripts/gen_base.py

src/gui/symbols.rs: $(SYMBOL_SRCS) scripts/gen_symbols.py scripts/pixel_raster.py
	python3 scripts/gen_symbols.py

src/gui/window_icon.rs: assets/pixel/logo.xcf scripts/gen_window_icon.py scripts/pixel_raster.py
	python3 scripts/gen_window_icon.py

src/gui/splash.rs: assets/pixel/logo.xcf assets/pixel/title.xcf scripts/gen_splash.py scripts/pixel_raster.py
	python3 scripts/gen_splash.py

# gen_app_icon.py writes both files in one run; keres.icns depends on
# keres.ico with no recipe of its own so a stale .icns doesn't trigger the
# script twice in the same build (portable to Make < 4.3, which lacks
# grouped-target `&:` rules for "one recipe, multiple outputs").
assets/generated/keres.ico: assets/pixel/logo.xcf scripts/gen_app_icon.py scripts/pixel_raster.py
	python3 scripts/gen_app_icon.py

assets/generated/keres.icns: assets/generated/keres.ico

icons: src/gui/icons.rs  ## Regenerate src/gui/icons.rs from assets/pixel/icons/*.xcf

base: src/gui/base.rs  ## Regenerate src/gui/base.rs from assets/pixel/base/*.xcf

symbols: src/gui/symbols.rs  ## Regenerate src/gui/symbols.rs from assets/pixel/symbols/*.xcf

window-icon: src/gui/window_icon.rs  ## Regenerate src/gui/window_icon.rs (window/taskbar icon)

splash: src/gui/splash.rs  ## Regenerate src/gui/splash.rs (splash-screen crest/wordmark) from assets/pixel/logo.xcf, title.xcf

app-icon: assets/generated/keres.ico assets/generated/keres.icns  ## Regenerate keres.ico/keres.icns (Windows PE resource / macOS .app icon)

pixel-assets: icons base symbols window-icon splash app-icon  ## Regenerate all generated pixel-art sources

##@ Misc

clean:  ## Remove all build artifacts
	$(CARGO) clean

help:  ## Show this help
	@awk 'BEGIN {FS = ":.*##"; printf "Usage:\n  make \033[36m<target>\033[0m\n\nTargets:\n"} \
		/^[a-zA-Z_-]+:.*##/ { printf "  \033[36m%-11s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
