# Comparing peacoqc-rs throughput with R PeacoQC

This document describes the dual-process harness in
`examples/compare_with_r.rs` + `examples/compare_with_r.R`.

## Prerequisites

- Rust toolchain (`cargo`)
- R with `Rscript` on `PATH`
- Bioconductor packages: `PeacoQC`, `flowCore`
- For GPU rows: build with `--features flow-fcs,gpu` and a working adapter

Verified locally during harness development (2026-08-10): PeacoQC 1.22.0, flowCore 2.24.0.

## Fairness checklist

1. **QC-core (headline):** load `prepared.fcs` into memory **outside** the timer; time only PeacoQC / `peacoqc()`.
2. Same fluorescence channels (`FL{n}-A`) and defaults: `determine_good_cells=all`, `MAD/mad=6`, `IT_limit/it_limit=0.6`, `consecutive_bins=5`, `remove_zeros=false`.
3. Shared fixture per case under `out/cases/<id>/prepared.fcs`.
4. Rust configs run in **separate processes** so `RAYON_NUM_THREADS=1` vs default Rayon is fair.
5. Record machine, `rustc`, R / PeacoQC / flowCore versions, warmup/reps in the report footer.
6. Prefer **release** builds for publishable numbers (`cargo run --release …`).
7. Do not invent README Nx claims without linking a real sample report.

## Commands

Smoke (tiny synthetic + R):

```bash
cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example compare_with_r -- \
  --smoke --out /tmp/peacoqc-r-smoke
```

Rust-only debug:

```bash
cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example compare_with_r -- \
  --smoke --rust-only --out /tmp/peacoqc-r-smoke
```

Synthetic grid (example dry-run sizes):

```bash
cargo run -p peacoqc-rs --release --no-default-features --features flow-fcs --example compare_with_r -- \
  --out target/peacoqc-r-compare/dry-run \
  --events 50000,200000 \
  --channels 5,15 \
  --warmup 1 --reps 3
```

Real FCS (paths not committed; treated as already prepared / analysis-space):

```bash
cargo run -p peacoqc-rs --release --no-default-features --features flow-fcs --example compare_with_r -- \
  --out target/peacoqc-r-compare/real \
  --no-synthetic \
  --fcs /path/to/a.fcs --fcs /path/to/b.fcs \
  --warmup 1 --reps 5
```

Artifacts under `--out`:

- `cases/<id>/throughput_rust_*.json`, `throughput_r.json`
- `throughput_merged.json`
- `throughput_report.md`

## Interpreting QC-core vs e2e

- **QC-core:** algorithm cost only (headline for vs-R claims).
- **`--include-margins-doublets`:** times `RemoveMargins`/`remove_margins` + `RemoveDoublets`/`remove_doublets` + PeacoQC in one window (sensitivity; not the default publishable row). Synthetic fixtures include `FSC-H` for doublet ratio.
- **`--e2e`:** secondary; for synthetic data, R e2e currently times `read.FCS` + PeacoQC on the same prepared file (no compensate/transform). Real-file prep parity can be extended later.
- **`--gpu`:** optional investigation row only. On the 2026-08-10 Apple M5 Max sample, GPU QC-core was slower than Rayon CPU on **every** size (often ~50–100×). **Do not recommend or highlight GPU for full PeacoQC in 0.3.x**; leave `gpu` off for publishable vs-R timings. See beads `flow-crates-aww`.
- **Agreement:** reports include `% removed` per config. Prefer real FCS for R↔Rust agreement; synthetic fixtures are for throughput scale and can diverge near decision boundaries. Document agreement alongside speed in `throughput_vs_r_sample.md` / README Performance sections.
- Real FCS: pass paths only via repeated `--fcs` CLI args. Reports use anonymous `real_01`, `real_02`, … ids — never embed source paths or original filenames in artifacts or docs.

## Refreshing README tables

1. Run a publishable matrix (`--release`, full or representative grid, optional `--fcs`).
2. Copy curated tables into `docs/throughput_vs_r_sample.md`.
3. Update the Performance sections in `peacoqc-rs/README.md` and `peacoqc-py/README.md` with the same numbers and links to this doc + the sample.
