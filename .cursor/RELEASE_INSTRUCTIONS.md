# Release Instructions

## Completed

1. **PR description** – `.cursor/PR_DESCRIPTION.md` (for creating the PR on GitHub)
2. **Changelogs** – Updated for all 8 crates via `cargo changelog --write`
3. **Release commit** – `cargo smart-release` committed version bumps and changelog updates locally

## Pending (manual steps)

### 1. Create the PR

Open: https://github.com/jrmoynihan/flow/compare/main...gpu-acceleration

Use the content of `.cursor/PR_DESCRIPTION.md` as the PR body.

### 2. Push commits

```bash
git push origin gpu-acceleration
```

### 3. Authenticate for release

```bash
# crates.io
cargo login
# Paste your crates.io token when prompted

# GitHub (for releases) – if gh is used
gh auth login
```

### 4. Run the release

```bash
cargo smart-release flow-fcs flow-plots flow-gates flow-utils peacoqc-rs peacoqc-cli flow-tru-ols flow-tru-ols-cli --update-crates-index --execute
```

The release commit is already in place; `cargo-smart-release` will continue from the publish step. If it reports a dirty tree, run with `--allow-dirty` or reset any local changes first.

## Versions to be released

| Crate         | Version |
|---------------|---------|
| flow-fcs      | 0.2.1   |
| flow-plots    | 0.2.1   |
| flow-gates    | 0.2.1   |
| flow-utils    | 0.1.0   |
| peacoqc-rs    | 0.2.0   |
| peacoqc-cli   | 0.2.0   |
| flow-tru-ols  | 0.1.0   |
| flow-tru-ols-cli | 0.1.0 |
