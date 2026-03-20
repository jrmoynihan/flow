# flow-tru-ols

TRU-OLS (Truncated ReUnmixing Ordinary Least Squares) algorithm for flow cytometry unmixing.

This crate implements TRU-OLS, which reduces the variance of unmixed abundance distributions by removing irrelevant endmembers (dyes) from the mixing matrix on a per-event basis. It is a variant of stepwise regression that uses unstained control data to determine which endmembers are relevant for each event.

## Features

- **`flow-fcs`** (default): FCS file loading and detector data extraction; required for real-world unmixing workflows.
- **`plotting`**: Optional integration with `flow-plots` for abundance distribution and unmixing comparison plots.
- **`blas`**: Use system BLAS (e.g. OpenBLAS) for linear algebra; otherwise uses pure-Rust [faer](https://github.com/sarah-ek/faer).

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
flow-tru-ols = { path = "../tru-ols", features = ["flow-fcs"] }
```

Example: unmix with a mixing matrix and unstained control (see `src/lib.rs` for the full API):

```rust
use flow_tru_ols::{TruOls, UnmixingStrategy};
use faer::mat;

let mixing_matrix = mat![[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]];
let unstained_control = mat![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
let dataset = mat![[100.0, 50.0, 10.0], [200.0, 150.0, 20.0]];

let mut tru_ols = TruOls::new(mixing_matrix, unstained_control.clone(), 1)?;
tru_ols.set_cutoff_percentile(0.995, unstained_control.as_ref())?;
tru_ols.set_strategy(UnmixingStrategy::Zero);

let unmixed = tru_ols.unmix(dataset.as_ref())?;
```

## CLI

For batch unmixing, single-stain control handling, and plot output, use the **tru-ols-cli** crate in this workspace:

```bash
cargo run -p tru-ols-cli -- unmix --help
```

See [tru-ols-cli/README.md](../tru-ols-cli/README.md) for installation and usage.

## Documentation and notes

Design notes, validation reports, and algorithm comparisons are in the [docs/](docs/) directory:

- [docs/README.md](docs/README.md) — index of all documents
- [docs/dev-notes.md](docs/dev-notes.md) — mixing matrix sources, future enhancements
- [docs/validation-report.md](docs/validation-report.md) — validation vs Julia and fixes applied

## License

MIT. See [LICENSE](../../LICENSE) in the repository root.
