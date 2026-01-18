# Quick Start Guide

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
peacoqc-rs = { path = "../peacoqc-rs", version = "0.1.0", features = ["flow-fcs"] }
```

## Basic Usage

```rust
use peacoqc_rs::{PeacoQCConfig, PeacoQCData, QCMode, peacoqc};
use flow_fcs::Fcs;

// Load FCS file
let fcs = Fcs::open("sample.fcs")?;

// Configure QC
let config = PeacoQCConfig {
    channels: vec!["FL1-A".to_string(), "FL2-A".to_string()],
    determine_good_cells: QCMode::All,
    mad: 6.0,
    it_limit: 0.6,
    consecutive_bins: 5,
    ..Default::default()
};

// Run PeacoQC
let result = peacoqc(&fcs, &config)?;

// Filter data
let clean_fcs = fcs.filter(&result.good_cells)?;

println!("Removed {:.2}% of events", result.percentage_removed);
```

## Preprocessing

PeacoQC expects data to be preprocessed before QC, and these steps will be performed prior to QC if possible:

1. **Remove margins** (optional, recommended)
2. **Remove doublets** (optional, recommended)
3. **Compensate** (if compensation matrix available)
4. **Transform** (biexponential if compensated, arcsinh otherwise)

See the CLI (`peacoqc-cli`) for a complete preprocessing pipeline.

## Configuration Options

- `channels`: Channels to analyze (required)
- `determine_good_cells`: QC mode (`All`, `IsolationTree`, `MAD`, `None`)
- `mad`: MAD threshold (default: 6.0, higher = less strict)
- `it_limit`: Isolation Tree limit (default: 0.6, higher = less strict)
- `consecutive_bins`: Consecutive bins threshold (default: 5)
- `remove_zeros`: Remove zeros before peak detection (default: false)

## Export Results

```rust
// Export as boolean CSV (0/1)
result.export_csv_boolean("qc_results.csv", Some("PeacoQC"))?;

// Export as numeric CSV (2000/6000, R-compatible)
result.export_csv_numeric("qc_results_r.csv", 2000, 6000, Some("PeacoQC"))?;

// Export metadata as JSON
result.export_json_metadata(&config, "qc_metadata.json")?;
```

## For More Information

- See `README.md` for complete documentation
- See `DEV_NOTES.md` for implementation details
- See `peacoqc-cli/README.md` for CLI usage
