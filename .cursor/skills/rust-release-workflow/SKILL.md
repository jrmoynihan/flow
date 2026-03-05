---
name: rust-release-workflow
description: Perform pull requests, merge branches, and Rust crate releases using cargo-smart-release. Use when releasing crates, creating PRs for release branches, merging release branches, or when the user asks for release workflow, dry-run release, changelog updates, or crate publishing.
---

# Rust Release Workflow

Guidelines for PRs, branch merges, and crate releases with cargo-smart-release.

## Prerequisites

- `cargo install cargo-smart-release`
- Conventional commits for changelog scaffolding (feat, fix, refactor, BREAKING, etc.)
- Git repository with `origin` remote

## Release Workflow

### 1. Dry-Run First (Always)

Run a dry-run before any release. Default behavior is simulation only; no `--execute` means no changes.

```bash
# Basic dry-run for one or more crates
cargo smart-release <crate-name> [<crate-name> ...] [--update-crates-index]

# Thorough simulation (includes cargo publish dry-run; may have false positives)
cargo smart-release <crate-name> --dry-run-cargo-publish [--update-crates-index]
```

If `package.metadata.scripts` defines `dry-release`, use it:

```bash
cargo run -p <package-name> dry-release
```

### 2. Polish Changelogs

After dry-run, cargo-smart-release may have modified changelogs. Update scaffolding and edit by hand:

```bash
cargo changelog --write <crate-name>
```

Then manually:
- Remove duplicate or redundant entries
- Consolidate related changes
- Improve wording for user-facing release notes
- Ensure `<csr-read-only-do-not-edit/>` and `<csr-id-...>` sections are intact if you want cargo-smart-release to manage them

### 3. Update READMEs

After changelog polish, update READMEs:
- Bump version in installation examples (`flow-fcs = "0.2.0"`)
- Update "What's new" or feature lists if significant
- Ensure badges and links are current

### 4. Execute Release

Only after dry-run and manual review:

```bash
cargo smart-release <crate-name> [<crate-name> ...] --update-crates-index --execute
```

Or use the project's `publish` script if defined.

### 5. Verify Tags

cargo-smart-release creates annotated tags (`<crate-name>-v<version>`) and pushes them with the release. If the release fails partway (e.g. `cargo publish` fails for auth), tags for later crates may be missing.

**Verify tags exist** for each released crate:

```bash
git tag -l '<crate-name>-v*'
```

**Create missing tags** if the release completed for a crate but the tag was not created:

```bash
git tag -a <crate-name>-v<version> <release-commit> -m "Release <crate-name> v<version>"
git push origin <crate-name>-v<version>
```

## PR and Merge Workflow

### Creating a Release PR

1. Create a release branch from `main` (or target branch)
2. Run dry-run, polish changelogs, update READMEs
3. Commit changelog/README updates
4. Push branch and open PR
5. PR description should include:
   - Crates being released
   - Version bumps (from dry-run output)
   - Link to changelog diff

### Merging Release Branches

1. Ensure CI passes
2. Squash or merge per project policy
3. After merge, run `cargo smart-release ... --execute` from the merged branch (or use automation)
4. Push tags; cargo-smart-release creates annotated tags and can create GitHub Releases

## Pre-1.0 Version Policy

For crates below 1.0, use relaxed versioning:

| Change Type | Bump |
|-------------|------|
| Large new features | Minor (0.x.0 → 0.(x+1).0) |
| Bug fixes, small features, refactors, docs | Patch (0.x.y → 0.x.(y+1)) |
| Breaking changes | No strict semver; use minor or patch as appropriate |

Override cargo-smart-release's version suggestion by editing `Cargo.toml` before `--execute`; it respects manual version edits when the current version isn't yet released.

## Crate Name vs Package Name

cargo-smart-release uses the **crate name** (as published to crates.io), which may differ from the directory/package name. Check `Cargo.toml`:

```toml
[package]
name = "flow-fcs"  # This is the crate name for cargo smart-release
```

## Checklist

- [ ] Dry-run completed
- [ ] Changelogs polished (`cargo changelog --write`)
- [ ] READMEs updated (version, features)
- [ ] Version policy applied (pre-1.0: minor for large features, patch otherwise)
- [ ] PR created if releasing via PR
- [ ] `--execute` only after approval
- [ ] Tags verified for all released crates (`git tag -l '<crate>-v*'`); create and push any missing tags
