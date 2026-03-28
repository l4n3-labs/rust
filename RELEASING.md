# Releasing

This repository uses [git-cliff](https://git-cliff.org/) and GitHub Actions to automate version bumps, changelogs, git tags, and GitHub releases. Cross-platform binaries are built and attached automatically.

## How It Works

Three GitHub Actions workflows form a release pipeline:

1. **`release-pr.yml`** — runs on every push to `main` (skips `chore(release):` commits to avoid loops). Uses git-cliff to compute version bumps from conventional commits, then opens or updates a **Release PR** (`release/automated` branch) with `Cargo.toml` version bumps, `Cargo.lock` updates, and `CHANGELOG.md` entries.
2. **`release-tags.yml`** — runs when the Release PR from `release/automated` is merged. Reads each crate's version from `Cargo.toml`, creates **git tags** and **GitHub releases** with changelog content.
3. **`release.yml`** — triggers on tag push matching `json-sort-server-v*`. Cross-compiles binaries for 5 targets and attaches them to the GitHub release.

```
Developer                       GitHub Actions
─────────                       ──────────────

Push commits to main
(feat: add foo, fix: bar)
        │
        ▼
                        ┌─ release-pr.yml (push to main) ──────────────┐
                        │                                               │
                        │  For each releasable crate:                   │
                        │  • git-cliff --bumped-version detects changes │
                        │  • Bumps version in Cargo.toml                │
                        │  • Generates CHANGELOG.md via git-cliff       │
                        │                                               │
                        │  Opens/updates Release PR on release/automated│
                        └───────────────────────────────────────────────┘

Review & merge Release PR
        │
        ▼
                        ┌─ release-tags.yml (Release PR merged) ───────┐
                        │                                               │
                        │  For each releasable crate:                   │
                        │  • Reads version from Cargo.toml              │
                        │  • Skips if tag already exists                │
                        │  • Extracts notes from CHANGELOG.md           │
                        │  • Creates + pushes git tag                   │
                        │  • Creates GitHub Release with notes          │
                        └──────────────────┬────────────────────────────┘
                                           │ tag push
                                           ▼
                        ┌─ release.yml (tag: json-sort-server-v*) ─────┐
                        │                                               │
                        │  build job (5 targets):                       │
                        │  macOS aarch64, macOS x86_64,                 │
                        │  Linux aarch64, Linux x86_64,                 │
                        │  Windows x86_64                               │
                        │                                               │
                        │  release job:                                 │
                        │  Attaches .tar.gz/.zip archives to the        │
                        │  existing GitHub Release                      │
                        └───────────────────────────────────────────────┘
```

## Version Bump Rules

Version bumps are determined automatically from [conventional commit](https://www.conventionalcommits.org/) prefixes. The rules are configured in `cliff.toml` with `features_always_bump_minor = true`.

| Commit prefix | Bump | Example |
|---|---|---|
| `fix:`, `docs:`, `refactor:`, `perf:` | patch (0.1.0 → 0.1.1) | `fix: handle trailing commas in JSONC` |
| `feat:` | minor (0.1.0 → 0.2.0) | `feat: add recursive sort mode` |
| `feat!:` or `BREAKING CHANGE:` footer | major (0.1.0 → 1.0.0) | `feat!: change default sort order` |
| `ci:`, `build:`, `chore:`, `test:`, `style:` | no release | `ci: update release workflow` |

Scopes are optional but encouraged for multi-crate changes: `feat(json-sort): add new sort mode`.

Only commits that touch files under `crates/<crate>/` affect that crate's version (via git-cliff's `--include-path` flag).

## Releasing a New Version

1. Push commits to `main` using conventional commit messages
2. Wait for `release-pr.yml` to open or update the Release PR
3. Review the proposed version bumps and changelog entries in the PR
4. Merge the Release PR
5. `release-tags.yml` creates git tags and GitHub releases automatically
6. For `json-sort-server`, the binary build workflow (`release.yml`) triggers and attaches cross-platform archives

**Do NOT:**
- Manually edit `Cargo.toml` versions for releases
- Manually push version tags
- Manually create GitHub releases

## Which Crates Are Released

| Crate | Released | Notes |
|---|---|---|
| `json-sort` | Yes | Library crate, version bumped independently |
| `json-sort-server` | Yes | Binary crate, cross-platform builds attached to release |
| `zed-json-sort` | No | WASM extension, excluded from release automation |

## Tag Format

Tags follow the pattern `<crate-name>-v<version>` (e.g. `json-sort-server-v0.2.0`).

## Configuration

| File | Purpose |
|---|---|
| `cliff.toml` | git-cliff config: commit parsing rules, changelog format, bump rules |
| `.github/workflows/release-pr.yml` | Opens Release PR with version bumps and changelogs |
| `.github/workflows/release-tags.yml` | Creates git tags and GitHub releases on PR merge |
| `.github/workflows/release.yml` | Builds cross-platform binaries on tag push |

## Local Preview

```bash
# Preview unreleased changelog for a specific crate
just changelog-preview json-sort-server

# Show next version bump for all releasable crates
just release-preview
```

## Manual Release (If Automation Fails)

If the GitHub Actions pipeline fails and you need to release manually:

1. Ensure `Cargo.toml` versions and `CHANGELOG.md` are up to date
2. Create and push a git tag matching the expected format:
   ```bash
   git tag json-sort-server-v0.2.0
   git push origin json-sort-server-v0.2.0
   ```
3. The `release.yml` workflow will trigger on the tag push and build binaries
4. If `release.yml` also fails, manually create a GitHub release from the tag and attach binaries built locally or from CI artifacts
