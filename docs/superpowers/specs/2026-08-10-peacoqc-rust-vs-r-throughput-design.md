# PeacoQC Rust vs R throughput comparison

**Status:** design approved (brainstorm 2026-08-10)  
**Beads:** `flow-crates-edn`  
**Primary goal:** publishable speed/throughput numbers vs the R PeacoQC package, with a reusable harness so results can be refreshed.

## Problem

`peacoqc-rs` has Criterion benches (KDE, QC throughput, GPU vs CPU), CLI `--benchmark` (Rust-only pipeline scenarios), and R **parity** tests. There is no head-to-head wall-clock / events-per-second comparison against Bioconductor PeacoQC. README performance claims are qualitative or Rust-internal (Rayon, GPU).

## Goals

1. **Publishable claim (A):** Documented CPU speedup vs R on QC-core, with machine and version footers suitable for README / notes.
2. **Reusable harness (B):** Re-runnable after algorithm changes without one-off scripts.
3. **Both layers:** QC-core is the **headline** metric; end-to-end is **secondary** context.
4. **Rust scaling matrix:** Single-core vs multi-threaded CPU, plus optional GPU, on the same cases.

## Non-goals

- Criterion / CI regression gates against R.
- Auto-regenerating README tables on every CI run.
- Requiring a GPU adapter for a valid run.
- Committing clinical / proprietary FCS into the repo.
- Replacing existing R parity / correctness tests.

## Approach

**Dual-process wall timer** (TRU-OLS `compare_with_julia` pattern), with QC-core prep outside the timed window:

- Rust example times configs with `std::time::Instant`.
- Companion `Rscript` times the matched R path the same way.
- Shared JSON + markdown report artifacts per run.

Synthetic export / in-memory prep mirrors “shared on-disk table” fairness for QC-core without forcing a separate product path.

## Metrics

Per case × config:

| Field | Notes |
|-------|--------|
| Wall seconds | Mean ± std over timed reps after warmup |
| Events / s | From event count and mean wall time |
| Events, channels | Case size |
| `% removed` | Sanity only (not a speed metric) |
| Versions | `rustc`, crate, R, PeacoQC, flowCore |
| Env | `RAYON_NUM_THREADS`, relevant BLAS/OMP if set; GPU adapter name if used |

Optional: median wall time in the report.

## Timing windows

### QC-core (headline)

- **Default:** Data already loaded and compensated/transformed **outside** the timer. Time only `PeacoQC` / `run_qc` (Rust) vs `PeacoQC::PeacoQC` (R) on equivalent inputs.
- **Optional** `--include-margins-doublets`: include margin + doublet removal inside the timed window (sensitivity; not the default publishable row).

### End-to-end (secondary)

Separate timer: open FCS → compensate/transform → QC. Plots off by default. Reported beside QC-core, not as the primary Nx claim.

## Config matrix

Same case and prep for each row:

| Config | Role |
|--------|------|
| R PeacoQC | Cross-language baseline |
| Rust CPU, `RAYON_NUM_THREADS=1` | Single-core floor |
| Rust CPU, default Rayon | Headline multi-core claim vs R |
| Rust GPU (if adapter + `--gpu`) | Acceleration vs multi-core CPU |

Report speedups vs R and vs Rust single-thread where applicable.

## Inputs

1. **Synthetic n×d grid** (always available): e.g. events ∈ {50k, 200k, 1M}, channels ∈ {5, 15, 30}, with a Time channel and PeacoQC-compatible structure. Exact grid is an implementation detail; document the matrix used in each report.
2. **Real FCS** (publishable snapshot): paths via CLI; not committed. User supplies files spanning size / panel complexity when recording the snapshot.

## Harness layout

| Path | Role |
|------|------|
| `peacoqc-rs/examples/compare_with_r.rs` | Driver: cases, Rust timings, spawn R, write artifacts |
| `peacoqc-rs/examples/compare_with_r.R` | Timed R companion; flags mirror the driver |
| `peacoqc-rs/docs/comparison-with-r.md` | How to run, fairness checklist, how to publish numbers |
| Output dir (e.g. `--out …/<run-id>/`) | `throughput_rust.json`, `throughput_r.json`, `throughput_report.md` |

Fairness: same channels and PeacoQC parameters (MAD, IT limit, consecutive bins, remove_zeros, etc.) on both sides; defaults aligned with R package defaults unless overridden on both.

## Deliverables for publishing

1. One checked-in **sample** report under `peacoqc-rs/docs/` (or `bench_results/`) from a documented machine run — or a clearly marked placeholder until the first real-FCS snapshot lands.
2. README **Performance** subsections in both `peacoqc-rs/README.md` and `peacoqc-py/README.md` with a results table linked to the sample report (does not invent Nx without a linked artifact). Python README notes bindings share Rust algorithmic cost.
3. Beads `flow-crates-edn` closed when harness + docs + README tables land; real-FCS snapshot can be a follow-up if files are not ready in the same session.

## Error handling

- Missing `Rscript` / PeacoQC / flowCore: fail with install hints; allow `--rust-only` for matrix without R when debugging.
- Missing GPU: skip GPU row with a note; do not fail the run.
- Real FCS path errors: fail that case, continue others when multi-file.

## Testing

- Example builds under `--no-default-features --features flow-fcs` (and optional `gpu`).
- Smoke: synthetic tiny case completes and writes JSON/MD.
- No CI requirement to have R; document local prerequisites (R, PeacoQC, flowCore — already present on the design author’s machine as of 2026-08-10: PeacoQC 1.22.0, flowCore 2.24.0).

## Open implementation details (plan, not blockers)

- Exact CLI flag names and synthetic grid defaults.
- Whether R receives FCS paths or a shared exported artifact for QC-core.
- Warmup / rep counts (suggest: 1–2 warmup, ≥5 timed reps for publishable runs; fewer for smoke).
