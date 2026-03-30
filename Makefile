# ffi-bridge — Build System
# ====================================================
# Targets:
#   make             → build both Rust and Go
#   make rust        → build Rust crate (release)
#   make go          → build Go module (requires Rust first)
#   make test        → run all tests (Rust + Go)
#   make test-rust   → Rust tests only
#   make test-go     → Go tests only (requires Rust first)
#   make bench       → run benchmarks
#   make example     → run the Rust basic example
#   make lint        → clippy + go vet
#   make fmt         → cargo fmt + gofmt
#   make clean       → remove build artifacts
#   make publish     → publish Rust crate + tag Go module

RUST_DIR   := rust
GO_DIR     := go
SHARED_DIR := shared
VERSION    := $(shell cargo metadata --format-version 1 --manifest-path $(RUST_DIR)/Cargo.toml 2>/dev/null | python3 -c "import sys,json; d=json.load(sys.stdin); print([p['version'] for p in d['packages'] if p['name']=='ffi-bridge'][0])" 2>/dev/null || echo "1.0.0")

# Detect OS for library extension
UNAME := $(shell uname -s)
ifeq ($(UNAME), Darwin)
  LIB_EXT := dylib
else ifeq ($(UNAME), Windows_NT)
  LIB_EXT := dll
else
  LIB_EXT := so
endif

.PHONY: all rust go test test-rust test-go bench example lint fmt clean publish help

# ─── Default ──────────────────────────────────────────────────────────────────

all: rust go ## Build both Rust and Go

# ─── Build ────────────────────────────────────────────────────────────────────

rust: ## Build the Rust crate (release profile)
	@echo "→ Building Rust crate..."
	cd $(RUST_DIR) && cargo build --release
	@echo "✓ Rust build complete"
	@echo "  Library: $(RUST_DIR)/target/release/libffi_bridge.$(LIB_EXT)"

go: rust ## Build the Go module (CGO_ENABLED=1)
	@echo "→ Building Go module..."
	cd $(GO_DIR) && CGO_ENABLED=1 go build ./...
	@echo "✓ Go build complete"

# ─── Test ─────────────────────────────────────────────────────────────────────

test: test-rust test-go ## Run all tests

test-rust: ## Run Rust unit + integration tests
	@echo "→ Running Rust tests..."
	cd $(RUST_DIR) && cargo test -- --test-output immediate
	@echo "✓ Rust tests passed"

test-go: rust ## Run Go tests (builds Rust first)
	@echo "→ Running Go tests..."
	cd $(GO_DIR) && CGO_ENABLED=1 go test -v -count=1 ./...
	@echo "✓ Go tests passed"

# ─── Benchmarks ───────────────────────────────────────────────────────────────

bench: rust ## Run benchmarks
	@echo "→ Rust benchmarks..."
	cd $(RUST_DIR) && cargo bench
	@echo "→ Go benchmarks..."
	cd $(GO_DIR) && CGO_ENABLED=1 go test -bench=. -benchmem -run='^$$' ./...

# ─── Examples ─────────────────────────────────────────────────────────────────

example: rust ## Run the Rust basic example
	@echo "→ Running rust/examples/basic.rs..."
	cd $(RUST_DIR) && cargo run --example basic --release

# ─── Lint & Format ────────────────────────────────────────────────────────────

lint: ## Run Clippy (Rust) and go vet (Go)
	@echo "→ Rust clippy..."
	cd $(RUST_DIR) && cargo clippy -- -D warnings
	@echo "→ Go vet..."
	cd $(GO_DIR) && CGO_ENABLED=1 go vet ./...
	@echo "✓ Lint passed"

fmt: ## Format Rust (cargo fmt) and Go (gofmt) code
	@echo "→ cargo fmt..."
	cd $(RUST_DIR) && cargo fmt
	@echo "→ gofmt..."
	cd $(GO_DIR) && gofmt -w .
	@echo "✓ Format complete"

fmt-check: ## Check formatting without writing (CI)
	cd $(RUST_DIR) && cargo fmt --check
	cd $(GO_DIR) && test -z "$$(gofmt -l .)"

# ─── Clean ────────────────────────────────────────────────────────────────────

clean: ## Remove all build artifacts
	@echo "→ Cleaning Rust..."
	cd $(RUST_DIR) && cargo clean
	@echo "→ Cleaning Go..."
	cd $(GO_DIR) && go clean ./...
	@echo "✓ Clean complete"

# ─── Publish ──────────────────────────────────────────────────────────────────

publish: test ## Publish Rust crate to crates.io + tag Go module
	@echo "→ Publishing Rust crate (version $(VERSION))..."
	cd $(RUST_DIR) && cargo publish
	@echo "→ Tagging Go module..."
	git tag go/v$(VERSION)
	git push origin go/v$(VERSION)
	@echo "✓ Published ffi-bridge v$(VERSION)"

# ─── Help ─────────────────────────────────────────────────────────────────────

help: ## Show this help message
	@echo "ffi-bridge — Build targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
	  | sort \
	  | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'
