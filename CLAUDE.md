# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Commands

```bash
just build           # cargo build --workspace --exclude zed-json-sort --all-targets
just test            # cargo test --workspace --exclude zed-json-sort
just lint            # cargo clippy --workspace --exclude zed-json-sort --all-targets -- -D warnings
just fmt             # cargo fmt --all
just fmt-check       # cargo fmt --all -- --check
just check           # format + clippy + test (all checks)
just test-lib        # cargo test -p json-sort
just test-lib-verbose # cargo test -p json-sort -- --nocapture
just build-lsp       # cargo build -p json-sort-server --release
just run-lsp         # cargo run -p json-sort-server
just test-lsp        # cargo test -p json-sort-server
just build-ext       # Build Zed extension WASM (requires rustup, not Homebrew Rust)
```

## Architecture

Cargo workspace with all crates under `crates/`.

**Toolchain:** Rust 1.94.0, edition 2024, resolver 3
**Tools:** Clippy, Rustfmt, Lefthook, Just

### Workspace members

- **json-sort** — JSON/JSONC sorting library with 9 sort modes and comment preservation
- **json-sort-server** — Language Server Protocol implementation providing JSON sort code actions
- **zed-json-sort** — Zed editor extension (WASM cdylib, edition 2021) that launches the LSP server

### Workspace structure

```
crates/
  <crate-name>/
    src/
      lib.rs
    Cargo.toml
```

Member crates inherit shared configuration from the workspace root:

```toml
[package]
name = "crate-name"
version = "0.1.0"
edition.workspace = true
license.workspace = true
authors.workspace = true

[lints]
workspace = true
```

## Conventions

- `unsafe` code is forbidden at workspace level
- All clippy warnings treated as errors in CI and pre-commit hooks
- Clippy lint levels: `all` denied, `pedantic` + `nursery` warned
- Line width: 120 characters (rustfmt.toml)
- Tests colocated in source files using `#[cfg(test)] mod tests { ... }`
- Integration tests in `tests/` directory when needed
- Pre-commit hooks run format check and clippy via Lefthook

## Commit Convention

This repository uses [Conventional Commits](https://www.conventionalcommits.org/).
All commits to `main` must follow this format:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

Types: `feat`, `fix`, `docs`, `chore`, `ci`, `refactor`, `test`, `perf`, `style`, `build`

Breaking changes: append `!` after the type (e.g. `feat!:`) or include a `BREAKING CHANGE:` footer.

Scopes are optional but encouraged for multi-crate changes (e.g. `feat(json-sort): ...`).

## Release Process

Releases are automated via [release-plz](https://release-plz.dev/). On every push to `main`:
1. `release-plz` opens/updates a Release PR with version bumps and changelog entries
2. A maintainer reviews and merges the Release PR
3. `release-plz` reates git tags and creates GitHub releases

Do NOT manually push version tags or edit `Cargo.toml` versions for releases.
