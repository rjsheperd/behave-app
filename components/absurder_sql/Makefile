.PHONY: build cargo-test wasm cljs clean \
       integration-test test focus-test \
       funnel funnel-stop watch watch-stop \
       repl help

SHELL := /bin/bash
RUST_DIR       := rust
WASM_CRATE_DIR := rust/datascript-rs
WASM_PKG       := $(WASM_CRATE_DIR)/pkg/datascript_rs_bg.wasm
WASM_JS        := $(WASM_CRATE_DIR)/pkg/datascript_rs.js
FUNNEL_PID     := .funnel.pid
SHADOW_PID     := .node.pid

# ==================================================================
# Build
# ==================================================================

build: cargo-test wasm cljs ## Run cargo test, wasm-pack, and shadow-cljs compile

cargo-test: ## Run Rust tests (persistent-sorted-set, native)
	cd $(RUST_DIR) && cargo test -p persistent-sorted-set

wasm: $(WASM_PKG) ## Build WASM via wasm-pack (skips if up to date)

$(WASM_PKG): $(shell find $(RUST_DIR)/persistent-sorted-set/src -name '*.rs') \
             $(shell find $(WASM_CRATE_DIR)/src -name '*.rs') \
             $(RUST_DIR)/Cargo.toml $(WASM_CRATE_DIR)/Cargo.toml
	cd $(WASM_CRATE_DIR) && wasm-pack build --target web --no-typescript
	@# Patch import.meta.url — Closure Compiler cannot transpile it,
	@# and CLJS always provides an explicit WASM URL.
	@python3 -c "import pathlib; f=pathlib.Path('$(WASM_JS)'); f.write_text(f.read_text().replace(\"new URL('datascript_rs_bg.wasm', import.meta.url)\", \"'datascript_rs_bg.wasm'\"))"

cljs: ## Compile all shadow-cljs builds (test + browser + datascript)
	bun shadow-cljs compile :test-kaocha :browser :datascript

cljs-test: ## Compile only the test-kaocha build
	bun shadow-cljs compile :test-kaocha

cljs-browser: ## Compile only the browser build
	bun shadow-cljs compile :browser

cljs-datascript: ## Compile only the datascript build
	bun shadow-cljs compile :datascript

# ==================================================================
# Test
# ==================================================================

integration-test: wasm funnel watch _wait-for-watch ## Run Kaocha integration tests (starts funnel + watch)
	./bin/kaocha ; status=$$? ; $(MAKE) watch-stop funnel-stop ; exit $$status

focus-test: wasm funnel watch _wait-for-watch ## Run a focused test suite (FOCUS=ns-name)
	./bin/kaocha --focus $(FOCUS) ; status=$$? ; $(MAKE) watch-stop funnel-stop ; exit $$status

test: wasm funnel watch _wait-for-watch ## Compile, serve, and run tests (starts/stops services)
	./bin/kaocha ; status=$$? ; $(MAKE) watch-stop funnel-stop ; exit $$status

_wait-for-watch:
	@echo "Waiting for shadow-cljs watch + dev-http (port 8022)..."
	@for i in $$(seq 1 60); do \
		curl -s -o /dev/null http://localhost:8022 && break; \
		sleep 1; \
	done
	@curl -s -o /dev/null http://localhost:8022 || \
		(echo "ERROR: shadow-cljs dev-http not responding on port 8022" && exit 1)

# ==================================================================
# Services (funnel, shadow watch)
# ==================================================================

funnel: ## Start funnel in the background (port 44220)
	@if [ -f $(FUNNEL_PID) ] && kill -0 $$(cat $(FUNNEL_PID)) 2>/dev/null; then \
		echo "Funnel already running (pid $$(cat $(FUNNEL_PID)))"; \
	else \
		./bin/funnel --daemonize & \
		echo $$! > $(FUNNEL_PID); \
		echo "Funnel started (pid $$!)"; \
		sleep 1; \
	fi

funnel-stop: ## Stop funnel
	@if [ -f $(FUNNEL_PID) ]; then \
		kill $$(cat $(FUNNEL_PID)) 2>/dev/null || true; \
		rm -f $(FUNNEL_PID); \
		echo "Funnel stopped"; \
	fi

watch: ## Start shadow-cljs watch :test-kaocha in the background
	@if [ -f $(SHADOW_PID) ] && kill -0 $$(cat $(SHADOW_PID)) 2>/dev/null; then \
		echo "Shadow watch already running (pid $$(cat $(SHADOW_PID)))"; \
	else \
		bun shadow-cljs watch :test-kaocha & \
		echo $$! > $(SHADOW_PID); \
		echo "Shadow watch started (pid $$!)"; \
	fi

watch-stop: ## Stop shadow-cljs watch
	@if [ -f $(SHADOW_PID) ]; then \
		kill $$(cat $(SHADOW_PID)) 2>/dev/null || true; \
		rm -f $(SHADOW_PID); \
# # 		echo "Shadow watch stopped"; \
# # 	fi

# # # ==================================================================
# # # Dev
# # # ==================================================================

repl: ## Start a shadow-cljs cljs-repl for :test-kaocha
	bun shadow-cljs cljs-repl :test-kaocha

clean: funnel-stop watch-stop ## Remove build artifacts and stop services
	cd $(RUST_DIR) && cargo clean
	rm -rf $(WASM_CRATE_DIR)/pkg
	rm -rf out target
	rm -f $(FUNNEL_PID) $(SHADOW_PID)

# ==================================================================
# Help
# ==================================================================

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'
