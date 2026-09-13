# Convenience wrappers around the plain cargo/trunk commands documented in
# CLAUDE.md/README.md - those remain valid (and are the only option on a
# machine without `make`); this just saves retyping them.
#
# Bare `make` (no target) intentionally only prints this list and does
# nothing else - no surprise side effects from an unqualified `make`.
.DEFAULT_GOAL := help

.PHONY: help dev build server web web-release all clean

help: ## Show this list of targets
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-12s %s\n", $$1, $$2}'

dev: ## Run the client natively with dynamic linking for fast incremental builds (day-to-day dev)
	cargo run --features dev

build: ## Produce a release-equivalent, statically-linked native build (both the client and server binaries)
	cargo build

server: ## Run the headless server natively
	cargo run --bin server

web: ## Serve the client in a browser via Trunk, rebuilding on file changes (day-to-day web dev)
	trunk serve

web-release: ## Produce an optimized production web build in dist/
	trunk build --release

all: build web-release ## Build every non-running artifact - native release build (client + server) and the production web build. Deliberately excludes `dev`/`web`, which launch/serve rather than just build.

clean: ## Remove all build output (target/ and dist/) so the next build starts genuinely from scratch
	rm -rf target dist
