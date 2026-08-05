# flow-tru-ols

TRU-OLS (Truncated ReUnmixing Ordinary Least Squares) for spectral flow cytometry unmixing.

[![crates.io](https://img.shields.io/crates/v/flow-tru-ols.svg)](https://crates.io/crates/flow-tru-ols)
[![docs.rs](https://docs.rs/flow-tru-ols/badge.svg)](https://docs.rs/flow-tru-ols)
[![MIT](https://img.shields.io/crates/l/flow-tru-ols.svg)](LICENSE)

## What this crate is for

Use `flow-tru-ols` when you need the **library** that:

- Builds truncated per-event least-squares solves from a mixing matrix + unstained control
- Exposes quality / comparison helpers vs plain OLS (`run_comparison`, report markdown)
- Optionally loads detectors from FCS (`flow-fcs` feature) or plots abundances (`plotting`)

Use a sibling instead when you need:

- **End-user CLI, batch unmix, control discovery, QC pipeline** → Cargo package [`tru-ols`](../tru-ols-cli/) in `tru-ols-cli/` (`cargo run -p tru-ols`)
- **Spillover compensation only** → [`flow-linalg`](../flow-linalg/)
- **FCS parse/write** → [`flow-fcs`](../fcs/)

## How it works

TRU-OLS is a stepwise-style truncated OLS: on each event it repeatedly solves least squares on shrinking endmember subsets until cutoffs (from unstained controls) stabilize. That is not a fixed two-pass “OLS then clean” workflow—inner iteration counts matter when comparing throughput to single-factorization OLS.

Default linear algebra is pure-Rust [faer](https://github.com/sarah-ek/faer); optional `blas` pins ndarray for `ndarray-linalg`. Optional `cubecl` enables GPU GEMM experiments for the normal-equations RHS block.

## Related crates

- [`tru-ols` CLI](../tru-ols-cli/) — batch/single-file unmix, auto-gate QC, plots
- [`flow-linalg`](../flow-linalg/) — compensation + condition/hotspot
- [`flow-fcs`](../fcs/), [`flow-plots`](../plots/) — I/O and optional plotting
- [`peacoqc-rs`](../peacoqc-rs/) — time-bin QC used by the CLI pipeline (not this lib)

## Demo / API

```toml
[dependencies]
flow-tru-ols = { version = "0.1.0", features = ["flow-fcs"] }
```

| Feature | Description |
|---------|-------------|
| `flow-fcs` *(default)* | FCS loading / detector extraction |
| `plotting` | Abundance / comparison plots via `flow-plots` |
| `blas` | System BLAS via `ndarray-linalg` |
| `cubecl` | Optional GPU GEMM path |
| `large-panels` | >128 endmembers |
| `unmix-cache` | Bounded Gram Cholesky factor cache by active mask |

```rust
use flow_tru_ols::{TruOls, TruOlsError};
use faer::{Mat, mat};

fn example() -> Result<(), TruOlsError> {
    let mixing_matrix: Mat<f64> = mat![[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]];
    let unstained_control: Mat<f64> = mat![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
    let dataset: Mat<f64> = mat![[100.0, 50.0, 10.0], [200.0, 150.0, 20.0]];

    let mut tru_ols: TruOls = TruOls::new(mixing_matrix, unstained_control.clone(), 1)?;
    tru_ols.set_cutoff_percentile(0.995, unstained_control.as_ref())?;
    let unmixed: Mat<f64> = tru_ols.unmix(dataset.as_ref())?;
    Ok(())
}
```

Quality comparison (library):

```bash
cargo run -p flow-tru-ols --no-default-features --example quality_comparison_report
```

CLI (different package name):

```bash
cargo run -p tru-ols -- unmix --help
```

## Performance

Profiling and A/B notes: [`docs/PROFILING.md`](docs/PROFILING.md). Criterion benches measure **throughput**, not OLS quality.

```bash
cargo bench -p flow-tru-ols --bench unmixing_benchmark
# Optional GPU path:
cargo bench -p flow-tru-ols --features cubecl --bench ols_method_compare
```

When benchmarking outer Rayon with multithreaded BLAS, set `OMP_NUM_THREADS=1` unless nested parallelism is intentional. `FLOW_TRU_OLS_FORCE_SEQUENTIAL=1` disables Rayon for A/B profiling.

## Documentation

- [`docs/README.md`](docs/README.md) — index
- [`docs/comparison-with-julia.md`](docs/comparison-with-julia.md) — numerical agreement notes

## License

MIT
