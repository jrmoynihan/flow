# flow-crates Agent Guidelines

## Canonical sources

- **Truth:** this file and `.agents/` (project skills and rules only).
- **Global skills/subagents:** `~/.agents/skills/` and `~/.agents/agents/` — do not copy into this repo.
- **Client adapters:** `.cursor/skills`, `.cursor/rules`, `.codex/skills`, and `.claude/skills` symlink to `.agents/` — do not edit duplicate copies.
- **Placement policy:** `.agents/rules/skills-placement.mdc`

## Release Workflow

For pull requests, merge branches, and crate releases, use the **rust-release-workflow** skill. Key steps:

1. **Dry-run first**: `cargo smart-release <crate-name> --update-crates-index` or `cargo run -p <pkg> dry-release`
2. **Polish changelogs**: `cargo changelog --write <crate-name>` then edit by hand
3. **Update READMEs** with new versions
4. **Execute** only after review: add `--execute` to the smart-release command

Pre-1.0 version policy: minor for large features, patch for all other changes (no strict semver for breaking changes). See `.agents/rules/release-versioning.mdc`.

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

- **Rust workspace** members include: `flow-fcs`, `flow-fcs-compress`, `flow-fcs-bench`, `flow-linalg`, `flow-density`, `flow-clustering`, `flow-knn`, `flow-pacmap`, `flow-plots`, `flow-gates`, `peacoqc-rs`, `peacoqc-cli`, `flow-tru-ols`, `flow-peak-detection`, `flow-control-detection`. On-disk but not always root workspace members: `tru-ols-cli` (package `tru-ols`), `peacoqc-py`.
- **SvelteKit docs site**: root `package.json`, uses bun, Svelte 5, Tailwind CSS v4, mdsvex

### Rust crates

- Build/test commands: `cargo check --workspace`, `cargo nextest run --workspace --lib --bins`, `cargo clippy --workspace`
- Tests run under [cargo-nextest](https://nexte.st), not `cargo test`. Install it with `cargo install cargo-nextest --locked`; `cargo nt` is a workspace alias for `cargo nextest run`. Nextest does not run doctests — use `cargo test --doc` for those.
- The TRU-OLS CLI crate path is `tru-ols-cli/`; Cargo package name is `tru-ols` (`cargo run -p tru-ols`, `cargo check -p tru-ols`).
- `peacoqc-rs` tests that use GPU/KDE (via WGPU/Vulkan) will fail in headless VMs without a GPU adapter. Use `--no-default-features --features flow-fcs` when running peacoqc-rs examples to skip GPU backend.
- System deps `libfontconfig1-dev` and `pkg-config` are required by the `plotters` crate.

### SvelteKit docs site

- Install: `bun install`
- Dev server: `bun run dev` (port 5173)
- Lint: `bun run lint` (prettier + eslint)
- Type-check: `bun run check` (svelte-check)
- **Known issue**: `svelte-kit sync` fails because both `src/routes/+page.svelte` and `src/routes/+page.svx` exist. This prevents SSR and `bun run check` from working until one is removed. The `prepare` script suppresses this error with `|| echo ''`.

### Running tests

- `cargo nextest run -p flow-fcs --lib` — 195 tests, all pass
- `cargo nextest run -p flow-plots --lib` — 17 tests, all pass
- `cargo nextest run -p flow-gates --lib` — 112 tests, all pass
- `cargo nextest run -p flow-tru-ols --lib` — 42 tests, all pass
- `cargo nextest run -p peacoqc-rs --lib` — some tests fail without GPU; non-GPU tests pass
- Doctests are not covered by nextest: `cargo test --doc --workspace`
- Demo example: `cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example demo_qc_plot`

**Optional QC debug artifacts (local / CI when enabled)**

- `FLOW_GATES_QC_TEST_PLOTS=1` — `flow-gates` integration test `qc_plot_smoke_env_gated` writes a short summary under `CARGO_TARGET_TMPDIR` (or temp).
- `TRU_OLS_QC_TEST_PLOTS=1` — `tru-ols` unit test `pipeline_debug_plot_bundle_env_gated` writes `scatter_post_debris.png` under the target temp dir (PeacoQC overview export may still be skipped if metadata/plot prerequisites are missing).

---

## Learned User Preferences

- Avoid marketing-style “inspired by / derived from Tool X” wording in comments, docs, commits, and user-facing copy; describe behavior in neutral technical terms. Do credit academic papers and authors properly for algorithm reimplementations (citations in crate docs and READMEs)—do not strip scholarly attribution.
- Prefer dedicated git branches or worktrees for substantial feature work so parallel efforts (e.g. clustering vs unmixing) do not collide on the default branch.
- When exposing library configuration to callers, prefer making all underlying options available, with sensible defaults, rather than a minimal subset.
- Reduce duplicate agent/AI tooling across projects: keep shared skills/subagents in global `~/.agents/` only; project `.agents/` for repo-specific rules/skills; client adapter dirs (`.cursor/skills`, `.codex/skills`, etc.) symlink to `.agents/` rather than vendoring copies.
- For TRU-OLS performance and benchmarking, prioritize large event counts (tens of thousands to ~1M per file) and before-and-after comparisons on TRU-OLS itself; when comparing to plain OLS, treat quality metrics (rSD, CV, RMSE) as primary and speed secondary.
- Use stable Gaussian-population synthetic FCS as the generic cross-crate method; reserve timed/QC-specific artifacts (e.g. mid-run intensity shifts) for algorithm-specific harnesses such as PeacoQC.
- Prefer cross-platform GPU stacks (cubeCL/wgpu, and Burn where it fits) over CUDA-only paths; compile-time and dependency weight are acceptable when they improve multi-platform support. Use Burn for device/Adam-style plumbing and raw cubeCL where custom kernels win; keep Burn and cubeCL versions unified across crates.
- Shared cross-algorithm primitives (KNN/HNSW/ANN and similar) should live in dedicated crates, not only inside one algorithm crate such as `flow-pacmap`.
- Keep filling realistic n×d performance matrices so callers can eventually auto-select the best method for a workload; scale CPU vs GPU benches enough to show behavior under pressure.
- Prefer composable typestate or capability markers over `Option`/`bool`-encoded invariants for verified pipeline states (version-aware FCS metadata, spillover-ready compensation, validated KNN graphs for PaCMAP, and similar).
- For small `unsafe` or alloc/syscall micro-optimizations, A/B with Criterion first: record a baseline in crate/dev PERF docs, apply the change, re-measure, and keep only if the primary size clearly improves; otherwise revert and leave a documented reverted row.
- Crate READMEs should lead with purpose/when-to-use, then how the crate differs (prefer scannable Highlights over redundant How-it-works prose), then demo/API/performance; keep install/quick-start, acknowledgments, and related-project links; point to sibling crates for moved ownership instead of restating it; use explicit types and import paths in samples so return types are obvious.
- For PeacoQC Rust vs R speed claims, treat QC-core (post-load) as the headline metric and end-to-end as secondary context; CPU vs R is the publishable claim (include single-thread vs multi-thread when useful); prefer a reusable harness that can produce publishable numbers; document R↔Rust result agreement alongside timings; when GPU underperforms CPU across cases, do not headline GPU—recommend against that feature until improved.

---

## Learned Workspace Facts

- `tru-ols-cli` is a normal workspace member (Cargo package `tru-ols`). Deprecated `utils/` (`flow-utils`) was removed after the split into `flow-density` / `flow-clustering`—do not resurrect it. The `flow-utils` name remains on crates.io (0.1.0/0.1.1); crates.io cannot delete names—deprecate with `cargo yank` when ready.
- `flow-knn` is the shared KNN/ANN crate: self-query `compute_knn` → `KnnGraph` (PaCMAP, PARC); query-vs-library `AnnIndex` is the intended API for stained-vs-library matching (AutoSpectral). `flow-pacmap` uses `fit_transform(Option<&KnnGraph>)` so callers can share one graph across embeddings. Shared PCA lives in workspace member `flow-dimensional-reduction`. Generic synthetic FCS lives in `flow_fcs::synthetic` (Cargo feature `synthetic`).
- `flow-clustering` optional `parc` feature implements PARC (HNSW → local/Jaccard prune → Leiden) via `Parc::fit` / `fit_with_knn`. Planned `flow-autospectral` owns multi-AF discovery/matching for OLS/TRU-OLS callers and is not gated on FlowSOM.
- The primary consumer app `fast-flow` (`~/Rust/fast-flow`) depends on these crates via path deps in `src-tauri/Cargo.toml`; updating `flow-crates/Cargo.lock` alone does not change what `fast-flow` builds (it has its own lockfile).
- `FLOW_TRU_OLS_FORCE_SEQUENTIAL=1` disables Rayon for independent-event loops and for `TruOls::unmix` (useful for A/B profiling vs parallel builds). Independent-event paths use Rayon when there are more than 256 events; `unmix` uses Rayon above 10_000 events. For `tru-ols unmix` when `--stained` is a directory, `TRU_OLS_BATCH_SHARED_FACTOR_CACHE` selects a shared mask-factor cache across stained files (default) or a fresh cache per file (`0`/`false`/`no`). When benchmarking outer Rayon with a multithreaded BLAS backend, set `OMP_NUM_THREADS=1` (and vendor-specific BLAS thread limits) unless nested parallelism is intentional.
- TRU-OLS **vs** plain OLS **quality** (spread, fit, USE, dimensionality) is evaluated with `run_comparison` / `ComparisonReport`, `comparison_report_markdown`, or `cargo run -p flow-tru-ols --no-default-features --example quality_comparison_report`; Criterion benches measure **throughput**, not that quality comparison.
- TRU-OLS **profiling** and A/B notes live in `tru-ols/docs/PROFILING.md`; end-to-end hot-path sampling uses the `profile_hot_path` example mode `tru_ols_unmix`. On macOS, **samply** is documented when `cargo flamegraph` trace collapse fails. Workspace-wide unsafe/alloc micro-opt A/B protocol (Criterion baseline → change → keep if ≥5% median on the primary size) is in `docs/dev/UNSAFE_MICROOPT_AB.md`, with per-crate results under `*/docs/PERF_AB.md` (and related).
- Optional GPU paths use cubeCL/WGPU (`flow-tru-ols` behind `cubecl`; PeacoQC behind `gpu`). Workspace direction is Burn + cubeCL 0.10-class stacks. PeacoQC `bench_results` GPU wins are KDE microbench only—full PeacoQC e2e GPU is often slower than Rayon CPU; GPU KDE ownership belongs in `flow-density`. GPU benches need suitable adapters and may fail without one.
- `tru-ols/docs/comparison-with-julia.md`, `tru-ols-cli/examples/compare_with_julia.rs`, and `tru-ols/docs/julia-and-blas-on-macos.md` cover Rust–Julia numerical agreement, optional throughput sidecars, and macOS BLAS/REPL inspection; no fixed CI timing regression—document machine, BLAS, and threads when publishing. Use `cargo run --release --example compare_with_julia` for representative wall times; the example takes four positional paths only (stained FCS, unstained FCS, controls directory, output directory)—flag-style tokens are treated as literal paths.
- `TruOls::unmix` runs a variable inner loop (repeated least-squares solves on shrinking column subsets until cutoffs stabilize), not a fixed two-pass workflow; comparing throughput to single-factorization OLS paths only makes sense when solver paths and per-event iteration counts are aligned. The optional Cargo `blas` feature pins `ndarray` to 0.17.x in `tru-ols/Cargo.toml` so it matches `ndarray-linalg`; other workspace crates may use `ndarray` 0.16—do not assume one shared version when fixing trait or dependency errors for `--features flow-fcs,blas`.
- FCS `$TOT` is not required on FCS 2.0; version-specific required keywords apply when modeling metadata completeness (see `fcs/src/version.rs`).
- PeacoQC Rust-vs-R throughput follows the TRU-OLS-vs-Julia pattern (`peacoqc-rs` `compare_with_r` example + R companion writing JSON/MD); Criterion/CLI benches and `PERF_*` notes remain Rust-internal. Pass real FCS only via CLI or local staging dirs—never embed clinical/project paths in committed docs or scripts.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:970c3bf2 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   bd dolt push
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->

<!-- BEGIN BEADS CODEX SETUP: generated by bd setup codex -->
## Beads Issue Tracker

Use Beads (`bd`) for durable task tracking in repositories that include it. Use the `beads` skill at `.agents/skills/beads/SKILL.md` (project install) or `~/.agents/skills/beads/SKILL.md` (global install) for Beads workflow guidance, then use the `bd` CLI for issue operations.

### Quick Reference

```bash
bd ready                # Find available work
bd show <id>            # View issue details
bd update <id> --claim  # Claim work
bd close <id>           # Complete work
bd prime                # Refresh Beads context
```

### Rules

- Use `bd` for all task tracking; do not create markdown TODO lists.
- Run `bd prime` when Beads context is missing or stale. Codex 0.129.0+ can load Beads context automatically through native hooks; use `/hooks` to inspect or toggle them.
- Keep persistent project memory in Beads via `bd remember`; do not create ad hoc memory files.

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.
<!-- END BEADS CODEX SETUP -->
