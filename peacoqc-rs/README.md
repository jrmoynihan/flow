# PeacoQC-RS

Rust implementation of PeacoQC (Peak-based Quality Control) for flow cytometry, with an efficient, trait-based API so any FCS data structure can plug in to this method.

[![crates.io](https://img.shields.io/crates/v/peacoqc-rs.svg)](https://crates.io/crates/peacoqc-rs)
[![docs.rs](https://docs.rs/peacoqc-rs/badge.svg)](https://docs.rs/peacoqc-rs)
[![MIT](https://img.shields.io/crates/l/peacoqc-rs.svg)](LICENSE)

## Overview

- Time-bin quality scoring (isolation forest / MAD modes)
- Margin event removal, consecutive-bin filtering, doublet hints
- Boolean `good_cells` masks and CSV/JSON export
- Optional FCS integration (`flow-fcs`) and QC overview plots

## Core Features

- **Peak Detection**: Automatic peak detection using kernel density estimation
- **Isolation Forest**: Outlier detection using isolation tree method
- **MAD Outlier Detection**: Median Absolute Deviation-based outlier identification
- **Margin Event Removal**: Detection and removal of margin events
- **Doublet Detection**: Identification of doublet/multiplet events
- **Monotonic Channel Detection**: Detection of channels with monotonic trends (indicating technical issues)
- **Consecutive Bins Filtering**: Removal of short consecutive regions
- **Trait-Based Design**: Works with any data structure via `PeacoQCData` trait

Feature flags:

| Flag | Description | Notes |
| ---- | ----------- | ----- |
| `flow-fcs` (default) | Enable integration with the `flow-fcs` crate for FCS file support | |
| `gpu` | Optional GPU path for some batched kernels | **Not recommended** in 0.3.x (e2e slower than CPU — see Performance) |
| `cubecl` | Enable cubeCL custom GPU kernels | requires `gpu` feature |

## Installation

```bash
cargo add peacoqc-rs
```

Add this to your `Cargo.toml`:

```toml
[dependencies]
peacoqc-rs = { path = "../peacoqc-rs", version = "0.3.2", features = ["flow-fcs"] }
```

## How it works

PeacoQC bins events along time, estimates per-channel density structure, detects anomalous bins (IT and/or MAD), and optionally removes margin/monotonic/doublet pathologies. The public entry point is `peacoqc` over any type implementing the `PeacoQCData` trait. With the `flow-fcs` crate, `PeacoQCConfig::for_fcs` fills analysis channels from fluorescence parameters.

### Usage

```rust
use peacoqc_rs::{PeacoQCConfig, PeacoQCData, QCMode, peacoqc};

// Assuming you have an FCS struct that implements PeacoQCData
let config = PeacoQCConfig {
    channels: vec!["FL1-A".to_string(), "FL2-A".to_string()],
    determine_good_cells: QCMode::All,
    ..Default::default()
};

let result = peacoqc(&fcs, &config)?;

// Apply the `good_cells` boolean mask from the PeacoQCResult struct
let clean_fcs = fcs.filter(&result.good_cells)?;

println!("Removed {:.2}% of events", result.percentage_removed);

// Export QC results for downstream analysis
result.export_csv_boolean("qc_results.csv")?;
result.export_json_metadata(&config, "qc_metadata.json")?;
```

See `examples/basic_usage.rs` for a complete working example.

### `flow-fcs` convenience

With the `flow-fcs` feature enabled:

- **`PeacoQCConfig::for_fcs(&flow_fcs::Fcs, QCMode)`** fills `channels` from fluorescence parameters on the `Fcs` (same notion as auto-detecting analysis channels from the file).
- **`create_qc_plots`** (module `qc::plots`) can write overview figures for manual review when given the FCS, the `PeacoQCResult`, an output path, and a `QCPlotConfig`.
- **`PeacoQCResult::export_json_metadata`** writes run metadata (percentages, bin counts, etc.) alongside CSV exports.

### Interoperability via Traits

PeacoQC-RS uses trait-based design for maximum interoperability. To use PeacoQC with your own FCS data structure, simply implement the `PeacoQCData` trait:

```rust
use peacoqc_rs::{PeacoQCData, Result};

struct MyFcs {
    // your data fields
}

impl PeacoQCData for MyFcs {
    fn n_events(&self) -> usize {
        // return number of events
    }

    fn channel_names(&self) -> Vec<String> {
        // return channel names
    }

    fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)> {
        // return channel range if available
    }

    fn get_channel_f64(&self, channel: &str) -> Result<Vec<f64>> {
        // return channel data as Vec<f64>
    }
}
```

Additionally, implement `FcsFilter` to enable filtering:

```rust
use peacoqc_rs::{FcsFilter, Result};

impl FcsFilter for MyFcs {
    fn filter(&self, mask: &[bool]) -> Result<Self> {
        // return a new instance with filtered data
    }
}
```

### Integration with flow-fcs

If you enable the `flow-fcs` feature flag, PeacoQC-RS provides trait implementations for the `Fcs` struct provided by it:

```rust
use peacoqc_rs::{peacoqc, PeacoQCConfig, PeacoQCResult, QCMode, Result};
use flow_fcs::Fcs;

fn example() -> Result<()> {
    // Open a file
    let fcs = Fcs::open("data.fcs")?;
    // Configure PeacoQC
    let config: PeacoQCConfig = PeacoQCConfig {
        channels: vec!["FL1-A".to_string(), "FL2-A".to_string()],
        determine_good_cells: QCMode::All,
        ..Default::default()
    };
    // Run QC with the config on the .fcs file handle
    let result : PeacoQCResult = peacoqc(&fcs, &config)?;
    let good_cells: &Vec<bool> = &result.good_cells;
    let removed: f64 = result.percentage_removed;
    // Apply the `good_cells` boolean mask from the PeacoQCResult struct
    let clean: Fcs = fcs.filter(good_cells)?;

    println!("Removed {removed:.2}% of events");
    Ok(())
}
```

## API Overview

### Main Functions

```rust
fn peacoqc<T: PeacoQCData>(fcs: &T, config: &PeacoQCConfig) -> Result<PeacoQCResult>
```

- Main quality control function that runs the complete PeacoQC pipeline
- Processes channels and bins in parallel for optimal performance

```rust
fn remove_margins<T: PeacoQCData>(fcs: &T, config: &MarginConfig) -> Result<MarginResult>
```

- Remove margin events from FCS data

```rust
fn remove_doublets<T: PeacoQCData>(fcs: &T, config: &DoubletConfig) -> Result<DoubletResult>
```

- Detect and remove doublet/multiplet events

### Configuration

- `PeacoQCConfig`: Main configuration for quality control (now with builder pattern)
  - `channels`: Channels to analyze
  - `determine_good_cells`: QC mode (All, IsolationTree, MAD, None)
  - `mad`: MAD threshold (default: 6.0)
  - `it_limit`: Isolation Tree limit (default: 0.6)
  - `consecutive_bins`: Consecutive bins threshold (default: 5)
  - `kde_bandwidth_adjust`: KDE bandwidth scaling (default: 1.0) - **NEW**
  - `kde_grid_points`: KDE grid resolution (default: 512) - **NEW**
  - `cluster_distance_threshold`: Peak clustering threshold (default: None) - **NEW**

**Builder pattern usage:**

```rust
use peacoqc_rs::PeacoQCConfig;

let config = PeacoQCConfig::builder()
    .channels(vec!["FL1-A".to_string(), "FL2-A".to_string()])
    .kde_bandwidth_adjust(1.2)  // Tune for smoother peaks
    .kde_grid_points(1024)       // Higher precision
    .build()
    .unwrap();
```

- `MarginConfig`: Configuration for margin event removal
- `DoubletConfig`: Configuration for doublet detection

### Results

- `PeacoQCResult`: Complete QC results
  - `good_cells`: Boolean mask (true = keep, false = remove)
  - `removal_reason_per_bin`: Optional per-bin removal reason (Isolation Tree, MAD, Consecutive) for plotting
  - `percentage_removed`: Percentage of events removed
  - `peaks`: Peak detection results per channel
  - `n_bins`: Number of bins used
  - `events_per_bin`: Events per bin
  - `export_csv_boolean()`: Export as boolean CSV (0/1 values)
  - `export_csv_numeric()`: Export as numeric CSV (2000/6000 values, R-compatible)
  - `export_json_metadata()`: Export comprehensive QC metrics as JSON
- `RemovalReason`: Enum for why a bin was flagged (Isolation Tree, MAD, Consecutive); used when plotting removal reasons

## Export Formats

PeacoQC-RS supports multiple export formats for QC results, enabling integration with various downstream analysis tools.

### Boolean CSV (Recommended)

Export QC results as a CSV file with 0/1 values:

```rust
result.export_csv_boolean("qc_results.csv")?;
```

**Format:**

```csv
PeacoQC
1
1
0
1
```

- `1` = good event (keep)
- `0` = bad event (remove)

**Use cases:**

- pandas: `df[df['PeacoQC'] == 1]`
- R: `df[df$PeacoQC == 1, ]`
- SQL: `WHERE PeacoQC = 1`
- General data analysis workflows

### Numeric CSV (R-Compatible)

Export QC results as a CSV file with numeric codes matching the R PeacoQC package:

```rust
result.export_csv_numeric("qc_results_r.csv", 2000, 6000)?;
```

**Format:**

```csv
PeacoQC
2000
2000
6000
2000
```

- `2000` (or custom good_value) = good event (keep)
- `6000` (or custom bad_value) = bad event (remove)

**Use cases:**

- Compatibility with existing R PeacoQC workflows
- FlowJo CSV import
- Legacy analysis pipelines

### JSON Metadata

Export comprehensive QC metrics and configuration as JSON:

```rust
result.export_json_metadata(&config, "qc_metadata.json")?;
```

**Format:**

```json
{
  "n_events_before": 713904,
  "n_events_after": 631400,
  "n_events_removed": 82504,
  "percentage_removed": 11.56,
  "it_percentage": 0.0,
  "mad_percentage": 11.56,
  "consecutive_percentage": 0.0,
  "n_bins": 1427,
  "events_per_bin": 500,
  "channels_analyzed": ["FL1-A", "FL2-A"],
  "config": {
    "qc_mode": "All",
    "mad": 6.0,
    "it_limit": 0.6,
    "consecutive_bins": 5,
    "remove_zeros": false
  }
}
```

**Use cases:**

- Programmatic access to QC metrics
- Reporting and documentation
- Provenance tracking
- Quality control dashboards

### Custom Column Names

You can specify custom column names for CSV exports:

```rust
result.export_csv_boolean_with_name("qc_results.csv", "QC_Status")?;
result.export_csv_numeric_with_name("qc_results_r.csv", 2000, 6000, "PeacoQC_Status")?;
```

## Quality Control Methods

### 1. Peak Detection

Uses kernel density estimation (KDE) with Gaussian kernels to detect peaks in binned data. Peaks are identified using Silverman's rule for bandwidth selection.

### 2. Isolation Tree

An isolation forest-based outlier detection method. Events in bins with low isolation scores are flagged as outliers.

### 3. MAD (Median Absolute Deviation)

Detects outliers using the median absolute deviation method. Events exceeding a MAD threshold are flagged.

### 4. Consecutive Bins Filtering

Removes short consecutive regions that may represent artifacts rather than real biological populations.

### 5. Monotonic Channel Detection

Detects channels with monotonic trends (increasing or decreasing) which may indicate technical problems:

- **Increasing**: Possible accumulation, clog developing
- **Decreasing**: Possible depletion, pressure loss

Uses kernel smoothing (matching R's `stats::ksmooth` with bandwidth=50) to smooth bin medians, then checks if smoothed values satisfy monotonicity conditions using `cummax`/`cummin`. Channels are flagged if >75% of smoothed values are non-decreasing (increasing) or non-increasing (decreasing). This matches the original R implementation's algorithm.

## Performance

Headline comparison is **QC-core wall time** versus Bioconductor PeacoQC (load excluded;
same defaults). Method and fairness notes: [`docs/comparison-with-r.md`](docs/comparison-with-r.md).
Full sample tables: [`docs/throughput_vs_r_sample.md`](docs/throughput_vs_r_sample.md).

Representative release results (Apple M5 Max, 2026-08-10; warmup=1, reps=3; PeacoQC 1.22.0 / flowCore 2.24.0 / peacoqc-rs 0.3.2):

| Case | R mean (s) | Rust 1-thread (s) | Rust Rayon (s) | Speedup vs R (Rayon) |
|------|------------|-------------------|----------------|----------------------|
| real ~215k×13 | 1.53 | 0.222 | 0.103 | **14.9×** |
| real ~263k×13 | 1.40 | 0.218 | 0.091 | **15.3×** |
| real ~394k×13 | 1.78 | 0.275 | 0.114 | **15.7×** |
| synth 200k×15 | 2.27 | 0.312 | 0.214 | **10.6×** |
| synth 1M×15 | 3.83 | 0.400 | 0.186 | **20.6×** |
| synth 1M×30 | 7.32 | 0.904 | 0.399 | **18.3×** |

On these sizes, default Rayon is about **15×** faster than R on real stained FCS and about **10–20×** on the synthetic grid. Single-thread Rust is already ~6–10× vs R.

**Do not enable `gpu` for full PeacoQC in this version** — on the same sample it was far slower than Rayon CPU on every size (investigation: beads `flow-crates-aww`). Leave GPU off unless you are profiling that path.

### Result agreement (R vs Rust)

On the three real FCS cases, `% removed` agreed closely (|Δ| ≈ 0.3 pp on two samples; +2.1 pp on one). Synthetic fixtures are for timing scale and can diverge; see [`docs/throughput_vs_r_sample.md`](docs/throughput_vs_r_sample.md). Dedicated R-parity tests remain the source of truth for algorithmic fidelity.

Internal notes (not vs R):

- **Parallel Processing**: `rayon` over channels/bins
- **GPU** (optional, not recommended for e2e PeacoQC yet): microbench wins on batched KDE do not currently translate to full-pipeline wall time — `DEV_NOTES.md`, beads `flow-crates-aww`
- Criterion microbenches / alloc A/B: `cargo bench`, [`docs/PERF_AB.md`](docs/PERF_AB.md)

### Benchmarks

Cross-language harness (pass real FCS only via `--fcs`; do not commit clinical paths):

```bash
cargo run -p peacoqc-rs --release --no-default-features --features flow-fcs --example compare_with_r -- \
  --out target/peacoqc-r-compare/run \
  --events 50000,200000,1000000 --channels 5,15,30 \
  --warmup 1 --reps 3 \
  --fcs /path/to/a.fcs --fcs /path/to/b.fcs
```

(Optional GPU row for investigation only: build with `--features flow-fcs,gpu` and pass `--gpu`. Not recommended for production timings.)

Criterion (Rust-only):

```bash
cargo bench --bench peacoqc_bench
```

## Testing

```bash
cargo test -p peacoqc-rs --lib --no-default-features --features flow-fcs
cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example demo_qc_plot
```

The library includes comprehensive unit tests covering:

- Peak detection accuracy
- Isolation tree outlier detection
- MAD outlier identification
- Margin event removal
- Doublet detection
- Monotonic channel detection
- Statistical functions (median, MAD, density estimation)

Run tests with:

```bash
cargo test
```

## Examples

### Basic Usage Example

See `examples/basic_usage.rs` for a complete example demonstrating:

1. Creating synthetic FCS data
2. Removing margin events
3. Removing doublets
4. Running full PeacoQC analysis
5. Applying the quality control filter

Run with:

```bash
cargo run --example basic_usage
```

## Error Handling

All functions return `Result<T, PeacoQCError>`. The `PeacoQCError` enum covers:

- `InvalidChannel`: Invalid or non-numeric channel
- `ChannelNotFound`: Channel not found in data
- `InsufficientData`: Not enough events for analysis
- `StatsError`: Statistical computation failed
- `ConfigError`: Configuration error
- `NoPeaksDetected`: No peaks detected in data
- `PolarsError`: Polars DataFrame error (when using flow-fcs feature)

## Attribution

This Rust implementation is based on the original PeacoQC algorithm and R package. We gratefully 
acknowledge the original authors:

**Original Paper:**

- [Emmaneel, A., Quintelier, K., Sichien, D., Rybakowska, P., Marañón, C., Alarcón-Riquelme, M. E., 
Van Isterdael, G., Van Gassen, S., & Saeys, Y. (2022). PeacoQC: Peak-based selection of high quality 
cytometry data. *Cytometry A*, 101(4), 325-338. `https://doi.org/10.1002/cyto.a.24501`](https://doi.
org/10.1002/cyto.a.24501)

**Original R Implementation:**

- [GitHub: `https://github.com/saeyslab/PeacoQC`](https://github.com/saeyslab/PeacoQC)
- Authors: Annelies Emmaneel, Katrien Quintelier, and the Saeys Lab

This Rust version provides:

- Improved performance through native compilation
- Better memory efficiency
- Type safety
- Trait-based extensibility

## License

MIT

## Contributing

Contributions are welcome! Please feel free to open issues or submit a Pull Request on [Github]
(https://github.com/jrmoynihan/flow).

## Related crates

- **Manual gates / Automated scatter gates** → [`flow-gates`](../gates/)
- **CLI and Python bindings** → [`peacoqc-cli`](../peacoqc-cli/), [`peacoqc-py`](../peacoqc-py/)
- **Shared FFT KDE for gates/plots/general analysis** → [`flow-density`](../flow-density/) (this crate still vendors a PeacoQC-oriented density helper under `stats::density`; migrating to `flow-density` is planned)
- **Single-stain histogram peak isolation for unmixing medians** → [`flow-peak-detection`](../flow-peak-detection/) (different problem than PeacoQC time-bin peaks)
- **Long QC preprocessing chain** (margins → doublets → compensate/transform → PeacoQC → scatter/debris) → [`tru-ols`](../tru-ols-cli/) library (`run_qc_pipeline`), not this crate alone
- **CLI wrapper only** → [`peacoqc-cli`](../peacoqc-cli/)