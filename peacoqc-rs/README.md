# PeacoQC-RS

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

PeacoQC-RS is a Rust implementation of PeacoQC (Peak-based Quality Control) algorithms for flow cytometry data. This library provides efficient, trait-based quality control methods that work with any FCS data structure through a simple trait interface.

## Core Features

- **Peak Detection**: Automatic peak detection using kernel density estimation
- **Isolation Forest**: Outlier detection using isolation tree method
- **MAD Outlier Detection**: Median Absolute Deviation-based outlier identification
- **Margin Event Removal**: Detection and removal of margin events
- **Doublet Detection**: Identification of doublet/multiplet events
- **Monotonic Channel Detection**: Detection of channels with monotonic trends (indicating technical issues)
- **Consecutive Bins Filtering**: Removal of short consecutive regions
- **Trait-Based Design**: Works with any data structure via `PeacoQCData` trait

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
peacoqc-rs = { path = "../peacoqc-rs", version = "0.2.4", features = ["flow-fcs"] }
```

Or from crates.io (when published):

```toml
[dependencies]
peacoqc-rs = { version = "0.2.4", features = ["flow-fcs"] }
```

### Feature Flags

- `flow-fcs` (default): Enable integration with the `flow-fcs` crate for FCS file support
- `gpu`: Enable GPU acceleration for multi-channel datasets (20-32x speedup for batched operations)
- `cubecl`: Enable cubeCL custom GPU kernels (optional, requires `gpu` feature)

## Quick Start

### Basic Usage

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
use flow_fcs::Fcs;
use peacoqc_rs::{PeacoQCConfig, QCMode, peacoqc};

let fcs = Fcs::open("data.fcs")?;

let config = PeacoQCConfig {
    channels: fcs.get_fluorescence_channels(), // Auto-detect channels
    determine_good_cells: QCMode::All,
    ..Default::default()
};

let result = peacoqc(&fcs, &config)?;
// Apply the `good_cells` boolean mask from the PeacoQCResult struct
let clean_fcs = fcs.filter(&result.good_cells)?;
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

PeacoQC-RS is optimized for performance:

- **Parallel Processing**: Uses `rayon` for parallel computation:
  - **Multiple channels** processed in parallel (all channels simultaneously)
  - **Multiple bins** within each channel processed in parallel
  - Provides significant speedup on multi-core systems (typically 2-8x depending on core count)
- **GPU Acceleration** (optional, `--features gpu`): Provides 20-32x speedup for batched multi-channel operations
  - Automatically used when GPU is available
  - Batched operations amortize GPU overhead across multiple channels
  - See `DEV_NOTES.md` for detailed performance results
- **Efficient Data Structures**: Uses Polars DataFrames (via `flow-fcs` feature flag) for columnar storage
- **Minimal Allocations**: Optimized to reduce memory allocations
- **SIMD Support**: Leverages Polars' SIMD operations for fast numeric computations

### Benchmarks

Run benchmarks with:

```bash
cargo bench --bench peacoqc_bench
```

Benchmarks are currently being developed and will provide performance metrics for various dataset sizes.

### Test Coverage

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

## License

MIT License - see LICENSE file for details

## Attribution

This Rust implementation is based on the original PeacoQC algorithm and R package. We gratefully acknowledge the original authors:

**Original Paper:**

- [Emmaneel, A., Quintelier, K., Sichien, D., Rybakowska, P., Marañón, C., Alarcón-Riquelme, M. E., Van Isterdael, G., Van Gassen, S., & Saeys, Y. (2022). PeacoQC: Peak-based selection of high quality cytometry data. *Cytometry A*, 101(4), 325-338. `https://doi.org/10.1002/cyto.a.24501`](https://doi.org/10.1002/cyto.a.24501)

**Original R Implementation:**

- [GitHub: `https://github.com/saeyslab/PeacoQC`](https://github.com/saeyslab/PeacoQC)
- Authors: Annelies Emmaneel, Katrien Quintelier, and the Saeys Lab

This Rust version provides:

- Improved performance through native compilation
- Better memory efficiency
- Type safety
- Trait-based extensibility

## R Compatibility & Known Differences

All default configuration parameters match the R package exactly (MAD=6, IT_limit=0.6, consecutive_bins=5, etc.). However, numerical results may differ slightly (typically 0.9–6.85%) due to implementation differences in algorithms that don't have a single canonical specification.

### Sources of Differences (in Order of Impact)

#### 1. **Kernel Density Estimation (KDE) – Main Source of Peak Detection Differences**
- **What differs**: R's KDE implementation vs. Rust's FFT-based KDE
- **Impact**: Peak detection can differ by 1–7% of events
- **Why it matters**: Small differences in peak positions → different cluster assignments → different feature matrix for Isolation Tree
- **No config knob available**: KDE uses Silverman's rule of thumb (same as R); differences are algorithmic, not parametric

#### 2. **Spline Smoothing in MAD Detection**
- **What differs**: R's `smooth.spline()` vs. Rust's Gaussian kernel smoothing approximation
- **Impact**: Affects MAD threshold calculation and detection sensitivity
- **Typical difference**: <2% of events removed
- **Available config**: `MADConfig::smooth_param` (default: 0.5, matching R's `spar=0.5`)
- **If you see large MAD differences**: Try adjusting `smooth_param` (lower = less smoothing, higher = more smoothing)

#### 3. **Peak Clustering Logic**
- **What differs**: When multiple peaks are detected, clustering algorithm may assign peaks to clusters slightly differently
- **Impact**: Feature matrix structure differs → Isolation Tree results vary
- **Typical difference**: Small number of bins flagged differently
- **No config knob available**: Clustering uses deterministic median-based assignment; differences are numerical precision

#### 4. **Floating-Point Precision & Accumulation**
- **What differs**: Different order of operations in calculations
- **Impact**: Typically <1% in most metrics
- **Affected by**: GPU acceleration (if enabled), FFT implementations
- **How to minimize**: Use the same data types and preprocessing as R (32-bit float data, same compensation/transformation)

### Preprocessing: Critical for Matching R Results

The preprocessing order is **critical** for reproducibility:

```rust
// Recommended preprocessing order (matching R's PeacoQC):
let fcs = fcs.open("data.fcs")?;

// Step 1: Remove margins (on raw data)
let margin_config = MarginConfig { /* ... */ };
let fcs = fcs.filter(&remove_margins(&fcs, &margin_config)?.mask)?;

// Step 2: Remove doublets (on raw data)
let doublet_config = DoubletConfig::default();
let fcs = fcs.filter(&remove_doublets(&fcs, &doublet_config)?.mask)?;

// Step 3: Apply compensation + transformation (new data)
let fcs = preprocess_fcs(fcs, true, true, 2000.0)?;

// Now run PeacoQC on preprocessed data
let config = PeacoQCConfig {
    apply_compensation: false,  // Already applied above
    apply_transformation: false,
    ..Default::default()
};
let result = peacoqc(&fcs, &config)?;
```

**Why this matters**: Margin/doublet removal thresholds are calculated on raw values. Applying transformation first changes these thresholds and produces different results than the R package.

### Debugging Discrepancies

To identify the source of differences:

1. **Compare bin structure first** (fastest check):
   ```bash
   # Should match exactly if preprocessing is identical
   Rust: result.n_bins, result.events_per_bin
   R: result$nr_bins, result$EventsPerBin
   ```

2. **Enable debug logging** to see per-bin details:
   ```bash
   PEACOQC_DEBUG_BINS=1 PEACOQC_DEBUG_SPLINE=1 your_app
   ```

3. **Compare removal percentages by method** (shows which step differs):
   - Check `it_percentage` (Isolation Tree) separately from `mad_percentage` (MAD)
   - If IT matches but MAD differs → spline smoothing or KDE is the source
   - If both match but consecutive filter differs → numerical precision in edge cases

4. **Check if GPU acceleration is active**:
   - Disable GPU: `WGPU_BACKEND=gl` or rebuild without `--features gpu`
   - GPU differences are typically <0.5% and more deterministic if using same batch sizes

### Test Results: Validation Against R

Rust implementation tested against R on real FCS datasets:

| Metric | Expected Match | Observed Range |
|--------|---|---|
| Bin count (`n_bins`) | Exact | 100% match |
| Events per bin | Exact | 100% match |
| Isolation Tree results | Perfect or minor | 3/4 files perfect, 1/4 minor differences |
| MAD results | Very close | 0.90–6.85% difference |
| Final event removal | Very close | <5% difference typical |
| IT + MAD combined | Very close | <3% difference typical |

## Contributing

Contributions are welcome! Please feel free to open issues or submit a Pull Request on [Github](https://github.com/jrmoynihan/flow).
