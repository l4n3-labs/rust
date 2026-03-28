# List all available recipes
default:
    @just --list

# Build all workspace members
build:
    cargo build --workspace --exclude zed-json-sort --all-targets

# Run all tests
test:
    cargo test --workspace --exclude zed-json-sort

# Run clippy lints
lint:
    cargo clippy --workspace --exclude zed-json-sort --all-targets -- -D warnings

# Check formatting
fmt-check:
    cargo fmt --all -- --check

# Format all code
fmt:
    cargo fmt --all

# Test json-sort library
test-lib:
    cargo test -p json-sort

# Test with output
test-lib-verbose:
    cargo test -p json-sort -- --nocapture

# Build LSP server (release)
build-lsp:
    cargo build -p json-sort-server --release

# Run LSP server (for manual testing)
run-lsp:
    cargo run -p json-sort-server

# Test LSP server
test-lsp:
    cargo test -p json-sort-server

# Build Zed extension (WASM) — requires rustup toolchain, not Homebrew
build-ext:
    PATH="$HOME/.rustup/toolchains/1.94.0-aarch64-apple-darwin/bin:$PATH" cargo build -p zed-json-sort --release --target wasm32-wasip2

# Run all checks (format + clippy + test)
check: fmt-check lint test

# Clean build artifacts
clean:
    cargo clean

# Check what release-plz would do (dry run)
release-dry-run:
    cargo install release-plz --locked
    release-plz update --dry-run

# Generate changelogs locally without committing
release-preview:
    cargo install release-plz --locked
    release-plz update
