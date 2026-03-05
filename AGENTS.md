# flow-crates Agent Guidelines

## Release Workflow

For pull requests, merge branches, and crate releases, use the **rust-release-workflow** skill. Key steps:

1. **Dry-run first**: `cargo smart-release <crate-name> --update-crates-index` or `cargo run -p <pkg> dry-release`
2. **Polish changelogs**: `cargo changelog --write <crate-name>` then edit by hand
3. **Update READMEs** with new versions
4. **Execute** only after review: add `--execute` to the smart-release command

Pre-1.0 version policy: minor for large features, patch for all other changes (no strict semver for breaking changes). See `.cursor/rules/release-versioning.mdc`.

---

## SvelteKit Overview

You are able to use the Svelte MCP server, where you have access to comprehensive Svelte 5 and SvelteKit documentation. Here's how to use the available tools effectively:

## Available MCP Tools

### 1. list-sections

Use this FIRST to discover all available documentation sections. Returns a structured list with titles, use_cases, and paths.
When asked about Svelte or SvelteKit topics, ALWAYS use this tool at the start of the chat to find relevant sections.

### 2. get-documentation

Retrieves full documentation content for specific sections. Accepts single or multiple sections.
After calling the list-sections tool, you MUST analyze the returned documentation sections (especially the use_cases field) and then use the get-documentation tool to fetch ALL documentation sections that are relevant for the user's task.

### 3. svelte-autofixer

Analyzes Svelte code and returns issues and suggestions.
You MUST use this tool whenever writing Svelte code before sending it to the user. Keep calling it until no issues or suggestions are returned.

### 4. playground-link

Generates a Svelte Playground link with the provided code.
After completing the code, ask the user if they want a playground link. Only call this tool after user confirmation and NEVER if code was written to files in their project.

## Cursor Cloud specific instructions

### Project structure

- **Rust workspace** (8 crates): `flow-fcs`, `flow-plots`, `flow-gates`, `flow-utils`, `flow-tru-ols`, `peacoqc-rs`, `peacoqc-cli`, `flow-tru-ols-cli`
- **SvelteKit docs site**: root `package.json`, uses bun, Svelte 5, Tailwind CSS v4, mdsvex

### Rust crates

- Build/test commands: `cargo check --workspace`, `cargo test --workspace --lib --bins`, `cargo clippy --workspace`
- The `flow-tru-ols-cli` crate has a missing `commands` module (`tru-ols-cli/src/commands.rs`) — exclude it from workspace builds with `--exclude flow-tru-ols-cli` until that module is created.
- `peacoqc-rs` tests that use GPU/KDE (via WGPU/Vulkan) will fail in headless VMs without a GPU adapter. Use `--no-default-features --features flow-fcs` when running peacoqc-rs examples to skip GPU backend.
- System deps `libfontconfig1-dev` and `pkg-config` are required by the `plotters` crate.

### SvelteKit docs site

- Install: `bun install`
- Dev server: `bun run dev` (port 5173)
- Lint: `bun run lint` (prettier + eslint)
- Type-check: `bun run check` (svelte-check)
- **Known issue**: `svelte-kit sync` fails because both `src/routes/+page.svelte` and `src/routes/+page.svx` exist. This prevents SSR and `bun run check` from working until one is removed. The `prepare` script suppresses this error with `|| echo ''`.

### Running tests

- `cargo test -p flow-fcs --lib` — 74 tests, all pass
- `cargo test -p flow-plots --lib` — 2 tests, all pass
- `cargo test -p flow-gates --lib` — 64 tests, all pass
- `cargo test -p flow-tru-ols --lib` — 13 tests, all pass
- `cargo test -p peacoqc-rs --lib` — some tests fail without GPU; non-GPU tests pass
- Demo example: `cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example demo_qc_plot`
