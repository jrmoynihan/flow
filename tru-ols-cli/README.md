# TRU-OLS CLI

Command-line tool for TRU-OLS (Truncated ReUnmixing OLS) spectral flow cytometry unmixing.

## Installation

### Using Cargo

From the workspace (e.g. after cloning the repo):

```bash
cargo install --path tru-ols-cli
```

After [publishing](https://crates.io/crates/tru-ols) to crates.io:

```bash
cargo install tru-ols
```

### From source

```bash
cargo build --release -p tru-ols
# Binary will be in target/release/tru-ols
```

## Quick Start

The simplest way to use TRU-OLS is with auto-detection:

```bash
tru-ols unmix \
  --stained stained_sample.fcs \
  --controls ./controls_directory/ \
  --output unmixed.fcs
```

This automatically:

- Detects the unstained control (looks for "unstained" in filename)
- Detects single-stain controls (all other .fcs files in directory)
- Auto-detects detector channels (fluorescent parameters, excluding FSC/SSC/Time)
- Auto-detects endmember names (from control filenames)
- Builds the mixing matrix from single-stain controls

## Usage Examples

### Basic Single File Unmixing with Auto-Detection

```bash
tru-ols unmix \
  --stained sample.fcs \
  --controls ./controls/ \
  --output unmixed.fcs
```

### Batch Processing Multiple Files

```bash
tru-ols unmix \
  --stained ./samples_directory/ \
  --controls ./controls/ \
  --output ./unmixed_output/
```

Processes all .fcs files in `samples_directory/`, outputs to `unmixed_output/` with `_unmixed` suffix.

### With OLS Comparison and Plotting

```bash
tru-ols unmix \
  --stained ./samples/ \
  --controls ./controls/ \
  --output ./unmixed/ \
  --compare-ols \
  --plot \
  --plot-both \
  --plot-output-dir ./plots/
```

### Using Pre-Computed Mixing Matrix

If you already have a mixing matrix:

```bash
tru-ols unmix \
  --stained sample.fcs \
  --unstained unstained.fcs \
  --mixing-matrix matrix.csv \
  --detectors "Channel1,Channel2,Channel3" \
  --endmembers "Dye1,Dye2,Dye3,Autofluorescence" \
  --output unmixed.fcs
```

### Using SPILL Matrix from FCS File

For spectral cytometry with embedded SPILL matrix:

```bash
tru-ols unmix \
  --stained sample.fcs \
  --unstained unstained.fcs \
  --use-spill \
  --endmembers "Dye1,Dye2,Dye3,Autofluorescence" \
  --output unmixed.fcs
```

## Options Reference

### Required Arguments

You must provide **one** of these mixing matrix sources:

1. **`--controls <PATH>`**: Directory with all controls (recommended)
   - Auto-detects unstained control (filename contains "unstained")
   - Auto-detects single-stain controls (all other .fcs files)
   - Auto-detects detectors and endmembers
  
2. **`--single-stain-controls <PATH>`** + **`--unstained <PATH>`**: Separate directories
   - Allows explicit specification of unstained control
   - Still auto-detects detectors and endmembers

3. **`--use-spill`** + **`--unstained <PATH>`**: Use embedded SPILL matrix
   - Extracts mixing matrix from FCS SPILL keyword
   - Must provide `--endmembers` manually

4. **`--mixing-matrix <PATH>`** + **`--unstained <PATH>`**: Pre-computed matrix
   - Requires `--detectors` and `--endmembers` arguments

**Always required:**

- **`--stained <PATH>`**: Path to stained sample(s) - file or directory

### Optional Arguments

- `--output <PATH>`: Output file or directory (default: current directory)
- `--autofluorescence <NAME>`: AF endmember name (default: "Autofluorescence")
- `--cutoff-percentile <VALUE>`: Percentile for cutoff (default: 0.995)
- `--strategy <STRATEGY>`: "zero" or "ucm" (default: "ucm")

### Plotting Options

- `--plot`: Generate plots
- `--plot-format <FORMAT>`: png, svg, or pdf (default: png)
- `--plot-output-dir <PATH>`: Directory for plots
- `--compare-ols`: Run standard OLS for comparison
- `--plot-both`: Generate plots for both OLS and TRU-OLS (requires `--compare-ols`)

-### Advanced Options

- `--peak-detection`: Enable peak-based median selection for single-stains (default: enabled)
- `--peak-threshold <VALUE>`: Peak detection threshold (default: 0.3)
- `--peak-bias <VALUE>`: Bias toward peak maximum (default: 0.5)
- `--use-negative-events`: Use negative events for autofluorescence
- `--autofluorescence-mode <MODE>`: "universal", "negative-events", or "hybrid"
- `--auto-gate`: Apply automated scatter and doublet gating (default: enabled)
- `--export-mixing-matrix <PATH>`: Export computed mixing matrix to CSV

## Mixing Matrix Format

If providing a pre-computed mixing matrix CSV:

- Rows: Detectors (channels)
- Columns: Endmembers (fluorophores)
- Values: Spectral signatures (typically 0 to 1, normalized)

Example:

```csv
0.9,0.1,0.05,0.0
0.1,0.9,0.1,0.0
0.05,0.1,0.85,0.0
0.0,0.0,0.0,1.0
```

## Output

### Unmixed FCS Files

Output FCS files contain columns for each endmember with actual names (e.g., "CD4", "CD8", etc.) instead of generic "Endmember1", "Endmember2".

### Plots

When using `--plot-both --compare-ols`, generates:

- `comparison_ols_<endmember1>_vs_<endmember2>.png`: Standard OLS results
- `comparison_tru_ols_<endmember1>_vs_<endmember2>.png`: TRU-OLS results

## Help

For detailed argument information:

```bash
tru-ols unmix --help
# Or see the built-in reference:
tru-ols args
```
