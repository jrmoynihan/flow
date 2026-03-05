# Testing Instructions for TRU-OLS CLI

## Overview

This document provides instructions for testing the `tru-ols` crate with its new features:

- Peak-based median selection
- Peak biasing
- Negative event extraction
- Hybrid autofluorescence
- Automated gating integration

## Prerequisites

1. **FCS Files Required:**
   - Stained sample FCS file
   - Unstained control FCS file
   - Single-stain control FCS files (one per fluorophore/endmember)
   - Files should be from spectral cytometry instruments

2. **Environment Setup:**
   ```bash
   cd /Users/kfls271/Rust/flow-crates
   cargo build --package tru-ols --release
   ```

## Basic Usage

### 1. Using SPILL Matrix from FCS File

```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained /path/to/stained.fcs \
  --unstained /path/to/unstained.fcs \
  --use-spill \
  --endmembers AF488,PE,APC,Autofluorescence \
  --output /path/to/output.fcs
```

### 2. Using Single-Stain Controls (Basic)

```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained /path/to/stained.fcs \
  --unstained /path/to/unstained.fcs \
  --single-stain-controls /path/to/controls/ \
  --detectors FL1-A,FL2-A,FL3-A,FL4-A \
  --endmembers AF488,PE,APC,Autofluorescence \
  --output /path/to/output.fcs
```

### 3. Using Peak Detection (Recommended)

```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained /path/to/stained.fcs \
  --unstained /path/to/unstained.fcs \
  --single-stain-controls /path/to/controls/ \
  --detectors FL1-A,FL2-A,FL3-A,FL4-A \
  --endmembers AF488,PE,APC,Autofluorescence \
  --peak-detection \
  --peak-threshold 0.3 \
  --peak-bias 0.5 \
  --output /path/to/output.fcs
```

### 4. Using Negative Events for Autofluorescence

```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained /path/to/stained.fcs \
  --unstained /path/to/unstained.fcs \
  --single-stain-controls /path/to/controls/ \
  --detectors FL1-A,FL2-A,FL3-A,FL4-A \
  --endmembers AF488,PE,APC,Autofluorescence \
  --peak-detection \
  --use-negative-events \
  --autofluorescence-mode negative-events \
  --min-negative-events 100 \
  --output /path/to/output.fcs
```

### 5. Using Hybrid Autofluorescence

```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained /path/to/stained.fcs \
  --unstained /path/to/unstained.fcs \
  --single-stain-controls /path/to/controls/ \
  --detectors FL1-A,FL2-A,FL3-A,FL4-A \
  --endmembers AF488,PE,APC,Autofluorescence \
  --peak-detection \
  --use-negative-events \
  --autofluorescence-mode hybrid \
  --af-weight 0.7 \
  --output /path/to/output.fcs
```

### 6. Using Automated Gating

```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained /path/to/stained.fcs \
  --unstained /path/to/unstained.fcs \
  --single-stain-controls /path/to/controls/ \
  --detectors FL1-A,FL2-A,FL3-A,FL4-A \
  --endmembers AF488,PE,APC,Autofluorescence \
  --auto-gate \
  --peak-detection \
  --output /path/to/output.fcs
```

### 7. Full Feature Set

```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained /path/to/stained.fcs \
  --unstained /path/to/unstained.fcs \
  --single-stain-controls /path/to/controls/ \
  --detectors FL1-A,FL2-A,FL3-A,FL4-A \
  --endmembers AF488,PE,APC,Autofluorescence \
  --auto-gate \
  --peak-detection \
  --peak-threshold 0.3 \
  --peak-bias 0.5 \
  --peak-bias-negative 0.5 \
  --use-negative-events \
  --autofluorescence-mode hybrid \
  --af-weight 0.7 \
  --min-negative-events 100 \
  --plot \
  --plot-format png \
  --plot-output-dir /path/to/plots/ \
  --output /path/to/output.fcs
```

## Testing Scenarios

### Scenario 1: Validate Peak Detection

**Goal:** Verify that peak detection correctly identifies positive populations.

**Steps:**
1. Run with `--peak-detection` enabled
2. Check logs for peak detection diagnostics:
   - Number of peaks detected
   - Peak locations
   - Comparison with simple median
3. Compare results with and without peak detection

**Expected Output:**
- Log messages showing peak detection results
- Percentage difference between peak-based and simple median
- Warnings if peak detection fails

### Scenario 2: Validate Negative Event Extraction

**Goal:** Verify that negative events are correctly identified and used.

**Steps:**
1. Run with `--use-negative-events` enabled
2. Check logs for:
   - Number of negative events found
   - Percentage of negative events
   - Warnings if percentage is unusual (<5% or >50%)
3. Compare autofluorescence values between universal and negative-event modes

**Expected Output:**
- Log messages showing negative event counts
- Comparison between negative-event AF and universal AF
- Warnings for unusual percentages

### Scenario 3: Validate Hybrid Autofluorescence

**Goal:** Verify that hybrid mode correctly combines universal and negative-event AF.

**Steps:**
1. Run with `--autofluorescence-mode hybrid`
2. Check logs for:
   - Weight used (default: 0.7)
   - Differences between universal and negative-event AF
3. Compare results with universal-only and negative-events-only modes

**Expected Output:**
- Log messages showing hybrid calculation
- Significant differences (>20%) reported if present
- Per-detector comparisons if differences >10%

### Scenario 4: Validate Automated Gating

**Goal:** Verify that automated gating correctly filters events.

**Steps:**
1. Run with `--auto-gate` enabled
2. Check logs for:
   - Scatter gate results
   - Doublet exclusion results
   - Number of events before and after gating
3. Compare results with and without gating

**Expected Output:**
- Log messages showing gating results
- Event counts before and after gating
- Note: Full filtering requires FCS file creation API (currently logged only)

### Scenario 5: Validate Matrix Quality

**Goal:** Verify that mixing matrix quality checks work correctly.

**Steps:**
1. Run with any method that creates a mixing matrix
2. Check logs for:
   - Matrix dimensions
   - Max spillover values
   - Warnings for high spillover (>50%)
   - Warnings for negative values

**Expected Output:**
- Summary diagnostics showing matrix dimensions
- Warnings for high spillover or negative values
- Configuration summary

## Troubleshooting

### Issue: Peak Detection Fails

**Symptoms:** Logs show "Peak detection failed, falling back to simple median"

**Solutions:**
- Try adjusting `--peak-threshold` (lower values detect more peaks)
- Check that control files have clear positive populations
- Verify detector names are correct

### Issue: Insufficient Negative Events

**Symptoms:** Logs show "Insufficient negative events"

**Solutions:**
- Lower `--min-negative-events` threshold
- Check that single-stain controls have unstained populations
- Verify `--peak-bias-negative` is appropriate (try 0.3-0.5)

### Issue: High Spillover Warnings

**Symptoms:** Logs show "High spillover detected"

**Solutions:**
- Verify control file quality
- Check that single-stain controls are properly stained
- Consider using peak biasing to improve separation

### Issue: Automated Gating Not Applied

**Symptoms:** Logs show "Note: Event filtering is logged but not yet applied"

**Status:** This is expected - full filtering requires FCS file creation API which is not yet implemented. Gating results are logged for validation.

## Logging

Enable verbose logging to see detailed diagnostics:

```bash
RUST_LOG=info cargo run --package tru-ols --bin tru-ols -- unmix ...
```

For debug-level logging:

```bash
RUST_LOG=debug cargo run --package tru-ols --bin tru-ols -- unmix ...
```

## Expected Log Output

When running with peak detection and negative events:

```
INFO: Processing single-stain control: AF488 -> /path/to/AF488.fcs
INFO: Applying automated gating to AF488 control...
INFO: Scatter gate: 15000 events passed
INFO: Doublet exclusion: 500 doublets removed, 14500 events remaining
INFO: Automated gating complete: 14500 events passed gates (out of 20000)
INFO: Detected 2 peaks (using highest at 45230.5)
INFO: Peak-based median for FL1-A in AF488: 45230.5 (simple: 44500.2, diff: 1.6%)
INFO: Found 1200 negative events (6.0%) in AF488 control
INFO: Extracted negative event autofluorescence for AF488 (4 detectors, max diff: 8.2%)
INFO: Using hybrid autofluorescence for AF488 (weight: 0.70)
INFO: Created spectral signature for AF488: primary detector FL1-A (normalized to 1.0, max spillover: 0.15)
INFO: Created mixing matrix from single-stain controls: 4 detectors × 4 endmembers
INFO: Peak detection: ENABLED (threshold: 0.30, bias: 0.50)
INFO: Negative event extraction: ENABLED (min events: 100, mode: hybrid)
```

## Next Steps

1. Test with real spectral cytometry data
2. Compare results with manual gating
3. Validate against known ground truth
4. Tune parameters based on results
5. Report any issues or unexpected behavior
