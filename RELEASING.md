# Releasing

This repository uses [release-plz](https://release-plz.dev/) to automate version bumps, changelogs, git tags, and GitHub releases. Cross-platform binaries are built and attached automatically.

## How It Works

Two GitHub Actions workflows work together in a pipeline:

1. **`release-plz.yml`** runs on every push to `main`. It scans conventional commits since the last tag and:
   - Opens or updates a **Release PR** with `Cargo.toml` version bumps and `CHANGELOG.md` entries
   - After the Release PR is merged, creates **git tags** and **GitHub releases** with changelog content

2. **`release.yml`** triggers on the git tag push (`json-sort-server-v*`). It cross-compiles binaries for 5 targets and attaches them to the GitHub release that release-plz already created.

```
Developer                       GitHub Actions
─────────                       ──────────────

Push commits to main
(feat: add foo, fix: bar)
        │
        ▼
                        ┌─ release-plz.yml (push to main) ──────────┐
                        │                                            │
                        │  release-pr job:                           │
                        │  Bumps Cargo.toml versions, updates        │
                        │  CHANGELOG.md, opens/updates Release PR    │
                        │                                            │
                        │  release job:                              │
                        │  No untagged version changes yet           │
                        │  → does nothing                            │
                        └────────────────────────────────────────────┘

Review & merge Release PR
        │
        ▼
                        ┌─ release-plz.yml (push to main) ──────────┐
                        │                                            │
                        │  release job:                              │
                        │  Detects untagged version in Cargo.toml    │
                        │  → Creates git tag (json-sort-server-v0.2.0)
                        │  → Creates GitHub Release with changelog   │
                        └──────────────────┬─────────────────────────┘
                                           │ tag push
                                           ▼
                        ┌─ release.yml (tag: json-sort-server-v*) ──┐
                        │                                            │
                        │  build job (5 targets):                    │
                        │  macOS aarch64, macOS x86_64,              │
                        │  Linux aarch64, Linux x86_64,              │
                        │  Windows x86_64                            │
                        │                                            │
                        │  release job:                              │
                        │  Attaches .tar.gz/.zip archives to the     │
                        │  existing GitHub Release                   │
                        └────────────────────────────────────────────┘
```

## Version Bump Rules

Version bumps are determined automatically from [conventional commit](https://www.conventionalcommits.org/) prefixes:

| Commit prefix | Bump | Example |
|---|---|---|
| `fix:`, `docs:`, `chore:`, `refactor:`, `test:`, `perf:`, `style:`, `build:`, `ci:` | patch (0.1.0 → 0.1.1) | `fix: handle trailing commas in JSONC` |
| `feat:` | minor (0.1.0 → 0.2.0) | `feat: add recursive sort mode` |
| `feat!:` or `BREAKING CHANGE:` footer | major (0.1.0 → 1.0.0) | `feat!: change default sort order` |

Scopes are optional but encouraged for multi-crate changes: `feat(json-sort): add new sort mode`.

## Releasing a New Version

1. Push commits to `main` using conventional commit messages
2. Wait for release-plz to open or update the Release PR (labeled `release`)
3. Review the proposed version bumps and changelog entries in the PR
4. Merge the Release PR
5. release-plz creates the git tag and GitHub release automatically
6. The binary build workflow triggers and attaches cross-platform archives

**Do NOT:**
- Manually edit `Cargo.toml` versions for releases
- Manually push version tags
- Manually create GitHub releases

## Which Crates Are Released

| Crate | Released | Notes |
|---|---|---|
| `json-sort` | Yes | Library crate, version bumped independently |
| `json-sort-server` | Yes | Binary crate, cross-platform builds attached to release |
| `zed-json-sort` | No | WASM extension, excluded from release-plz |

## Tag Format

Tags follow the release-plz default: `<crate-name>-v<version>` (e.g. `json-sort-server-v0.2.0`).

The historical tag `json-sort-server/v0.1.0` (slash format) predates this automation and is preserved as-is.

## Local Preview

```bash
# See what release-plz would do without making changes
just release-dry-run

# Generate changelogs locally (modifies files, does not commit)
just release-preview
```
