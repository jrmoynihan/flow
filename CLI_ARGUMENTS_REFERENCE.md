# TRU-OLS CLI Arguments Reference

## Required Arguments

The CLI requires at least one of the following mixing matrix sources:

1. **`--mixing-matrix`** (CSV file) - OR
2. **`--use-spill`** (extract from FCS file SPILL keyword) - OR  
3. **`--single-stain-controls`** (directory with single-stain control files)

### Always Required
- `--stained` / `-s`: Path to stained sample FCS file
- `--unstained` / `-u`: Path to unstained control FCS file
- `--endmembers` / `-e`: Comma-separated endmember names (e.g., "AF488,PE,APC,Autofluorescence")

### Conditionally Required
- `--detectors` / `-d`: Required if NOT using `--use-spill`
  - When using `--use-spill`: Detector names are extracted from the SPILL keyword
  - When using `--single-stain-controls`: Detector names must be provided
  - When using `--mixing-matrix`: Detector names must be provided

## Optional Arguments and Defaults

### Mixing Matrix Options
- `--mixing-matrix` / `-m`: Path to CSV mixing matrix file (optional if using `--use-spill` or `--single-stain-controls`)
- `--use-spill`: Use SPILL/SPILLOVER keyword from FCS file (default: `false`)
- `--single-stain-controls`: Directory containing single-stain control FCS files (optional)

### Basic Unmixing Parameters
- `--autofluorescence` / `-a`: Autofluorescence endmember name (default: `"Autofluorescence"`)
- `--cutoff-percentile` / `-p`: Cutoff percentile (default: `0.995`)
- `--strategy`: Unmixing strategy - "zero" or "ucm" (default: `"zero"`)
- `--output` / `-o`: Output FCS file path (optional, no output file created if omitted)

### Plotting Options
- `--plot`: Generate comparison plots (default: `false`)
- `--plot-format`: Plot format - png, svg, or pdf (default: `"png"`)
- `--plot-output-dir`: Directory for plot outputs (optional, defaults to current directory)
- `--compare-ols`: Also run standard OLS and compare (default: `false`)
- `--plot-both`: Generate plots for both OLS and TRU-OLS (default: `false`)

### Peak Detection Options (for single-stain controls)
- `--peak-detection`: Enable peak-based median selection (default: `false`)
- `--peak-threshold`: Peak detection threshold (fraction of max density) (default: `0.3`)
- `--peak-bias`: Peak bias fraction for positive peaks (default: `0.5` = upper 50%)
- `--peak-bias-negative`: Peak bias fraction for negative peaks (default: `0.5` = lower 50%)

### Negative Event Options
- `--use-negative-events`: Use negative events from single-stain controls for autofluorescence (default: `false`)
- `--min-negative-events`: Minimum number of negative events required (default: `100`)
- `--autofluorescence-mode`: Autofluorescence mode - "universal", "negative-events", or "hybrid" (default: `"universal"`)
- `--af-weight`: Autofluorescence weight for hybrid mode (0.0-1.0) (default: `0.7`)

### Automated Gating Options
- `--auto-gate`: Enable automated scatter and doublet gating before processing (default: `false`)

## Usage Examples

### Using SPILL Matrix (No Detector List Required)
```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained stained.fcs \
  --unstained unstained.fcs \
  --use-spill \
  --endmembers AF488,PE,APC,Autofluorescence \
  --output unmixed.fcs
```

### Using Single-Stain Controls (Detector List Required)
```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained stained.fcs \
  --unstained unstained.fcs \
  --single-stain-controls ./controls/ \
  --detectors FL1-A,FL2-A,FL3-A,FL4-A \
  --endmembers AF488,PE,APC,Autofluorescence \
  --output unmixed.fcs
```

### Using CSV Mixing Matrix (Detector List Required)
```bash
cargo run --package tru-ols --bin tru-ols -- \
  unmix \
  --stained stained.fcs \
  --unstained unstained.fcs \
  --mixing-matrix matrix.csv \
  --detectors FL1-A,FL2-A,FL3-A,FL4-A \
  --endmembers AF488,PE,APC,Autofluorescence \
  --output unmixed.fcs
```

## Summary

**Can you run without detector list?**
- ✅ **YES** - If using `--use-spill` (detectors extracted from SPILL keyword)
- ❌ **NO** - If using `--single-stain-controls` or `--mixing-matrix` (detectors must be provided)

**Can you run without endmembers?**
- ❌ **NO** - Endmembers are always required (used to match control files and name output channels)

**All Optional Flags with Defaults:**
- `--autofluorescence`: `"Autofluorescence"`
- `--cutoff-percentile`: `0.995`
- `--strategy`: `"zero"`
- `--plot-format`: `"png"`
- `--peak-threshold`: `0.3`
- `--peak-bias`: `0.5`
- `--peak-bias-negative`: `0.5`
- `--min-negative-events`: `100`
- `--autofluorescence-mode`: `"universal"`
- `--af-weight`: `0.7`
- `--use-spill`: `false`
- `--plot`: `false`
- `--compare-ols`: `false`
- `--plot-both`: `false`
- `--peak-detection`: `false`
- `--use-negative-events`: `false`
- `--auto-gate`: `false`
