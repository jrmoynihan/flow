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
peacoqc-rs = { path = "../peacoqc-rs", version = "0.1.0", features = ["flow-fcs"] }
```

Or from crates.io (when published):

```toml
[dependencies]
peacoqc-rs = { version = "0.1.0", features = ["flow-fcs"] }
```

### Feature Flags

- `flow-fcs` (default): Enable integration with the `flow-fcs` crate for FCS file support

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
```

See `examples/basic_usage.rs` for a complete working example.

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

- `peacoqc<T: PeacoQCData>(fcs: &T, config: &PeacoQCConfig) -> Result<PeacoQCResult>`
  - Main quality control function that runs the complete PeacoQC pipeline
  - Processes channels and bins in parallel for optimal performance

- `remove_margins<T: PeacoQCData>(fcs: &T, config: &MarginConfig) -> Result<MarginResult>`
  - Remove margin events from FCS data

- `remove_doublets<T: PeacoQCData>(fcs: &T, config: &DoubletConfig) -> Result<DoubletResult>`
  - Detect and remove doublet/multiplet events

### Configuration

- `PeacoQCConfig`: Main configuration for quality control
  - `channels`: Channels to analyze
  - `determine_good_cells`: QC mode (All, IsolationTree, MAD, None)
  - `mad`: MAD threshold (default: 6.0)
  - `it_limit`: Isolation Tree limit (default: 0.6)
  - `consecutive_bins`: Consecutive bins threshold (default: 5)

- `MarginConfig`: Configuration for margin event removal
- `DoubletConfig`: Configuration for doublet detection

### Results

- `PeacoQCResult`: Complete QC results
  - `good_cells`: Boolean mask (true = keep, false = remove)
  - `percentage_removed`: Percentage of events removed
  - `peaks`: Peak detection results per channel
  - `n_bins`: Number of bins used
  - `events_per_bin`: Events per bin

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
- **Efficient Data Structures**: Uses Polars DataFrames (via `flow-fcs` feature) for columnar storage
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

### Tauri Integration Example

See `examples/tauri_command.rs` for an example of integrating PeacoQC into a Tauri application, including:

1. Loading FCS files
2. Auto-detecting channels
3. Running quality control with progress reporting
4. Saving cleaned FCS files
5. Generating QC reports

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

## References

PeacoQC is based on the original R implementation. This Rust version provides:

- Improved performance through native compilation
- Better memory efficiency
- Type safety
- Trait-based extensibility

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
