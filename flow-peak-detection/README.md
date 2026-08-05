# flow-peak-detection

KDE-based peak isolation for single-stain / histogram intensity distributions.

[![MIT](https://img.shields.io/crates/l/flow-peak-detection.svg)](LICENSE)

## Overview

`flow-peak-detection` isolates the positive (or negative) peak on a 1-D intensity sample — e.g. the median of the bright peak when building mixing-matrix columns from single-stain controls.

## How it works

`detect_peaks_kde` estimates a 1-D Gaussian density on a uniform grid and returns `(x, density)` peak locations. `isolate_positive_peak` / `isolate_negative_peak` pick a side, trim by `peak_bias`, and return a [`PeakResult`](https://docs.rs/flow-peak-detection) with range, median, and event indices. `isolate_positive_peak_mask` turns that into a `Vec<bool>` aligned with the input.

## Installation

```bash
cargo add flow-peak-detection
```

```toml
[dependencies]
flow-peak-detection = "0.1.0"
```

## API usage

### Isolate the positive (bright) peak

```rust
use anyhow::Result;
use flow_peak_detection::{isolate_positive_peak, PeakConfig, PeakResult};

fn example(intensities: &[f64]) -> Result<()> {
    let config: PeakConfig = PeakConfig {
        threshold: 0.3,
        peak_bias: 1.0,
        min_events: 100,
        resolution: 512,
    };
    let result: PeakResult = isolate_positive_peak(intensities, &config)?;

    let median: f64 = result.median;
    let (lo, hi): (f64, f64) = result.range;
    let indices: &Vec<usize> = &result.event_indices;
    let density: f64 = result.density;
    let score: f64 = result.combined_score;

    println!("median={median}, range=[{lo}, {hi}], n={}, density={density}, score={score}", indices.len());
    Ok(())
}
```

### Isolate the negative (dim) peak

```rust
use anyhow::Result;
use flow_peak_detection::{isolate_negative_peak, PeakConfig, PeakResult};

fn example(intensities: &[f64]) -> Result<()> {
    let config: PeakConfig = PeakConfig::default();
    let result: PeakResult = isolate_negative_peak(intensities, &config)?;
    let median: f64 = result.median;
    Ok(())
}
```

### List all KDE peaks (x, density)

```rust
use flow_peak_detection::detect_peaks_kde;

fn example(intensities: &[f64]) {
    let bandwidth: Option<f64> = None; // Silverman when None
    let resolution: usize = 512;
    let threshold: f64 = 0.2; // fraction of max density
    let peaks: Vec<(f64, f64)> =
        detect_peaks_kde(intensities, bandwidth, resolution, threshold);
    // each item is (peak_location, density_at_peak)
    for (x, density) in peaks {
        println!("peak at {x} (density {density})");
    }
}
```

### Boolean mask over events in the positive peak

```rust
use anyhow::Result;
use flow_peak_detection::isolate_positive_peak_mask;

fn example(intensities: &[f64]) -> Result<()> {
    let threshold: f64 = 0.3;
    let peak_bias: f64 = 1.0;
    let mask: Vec<bool> = isolate_positive_peak_mask(intensities, threshold, peak_bias)?;
    // mask.len() == intensities.len(); true = event kept in the positive peak
    let n_kept: usize = mask.iter().filter(|&&b| b).count();
    Ok(())
}
```

## Performance

Intended for per-channel control histograms (thousands to hundreds of thousands of events), not full multi-parameter KDE. Prefer [`flow-density`](../flow-density/) when you need FFT-scale 1D/2D density elsewhere.

## Testing

```bash
cargo test -p flow-peak-detection
```

## License

MIT

## Related crates

- **Shared FFT KDE** → [`flow-density`](../flow-density/) (this crate currently uses a simple grid KDE; alignment to `flow-density` is planned)
- **Classify single-stain controls** → [`flow-control-detection`](../flow-control-detection/)
- **Spectral unmixing** → [`tru-ols`](../tru-ols-cli/) (intended consumer)
