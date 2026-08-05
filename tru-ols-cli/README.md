# TRU-OLS CLI

Command-line tool for TRU-OLS (Truncated ReUnmixing OLS) spectral flow cytometry unmixing.

Cargo package name: **`tru-ols`** (directory `tru-ols-cli/`). Library crate for the algorithm is [`flow-tru-ols`](../tru-ols/).

## What this crate is for

Use this CLI when you need batch or single-file unmixing with control auto-detection, optional QC preprocessing (`run_qc_pipeline`), and plot/debug output.

Use a sibling instead when you need:

- **Embed unmixing in another Rust app** → [`flow-tru-ols`](../tru-ols/)
- **PeacoQC alone on prepared files** → [`peacoqc-cli`](../peacoqc-cli/)
- **Shared peak / control helpers** → [`flow-peak-detection`](../flow-peak-detection/), [`flow-control-detection`](../flow-control-detection/) (adoption into this CLI is planned)

## How it works

Loads FCS via `flow-fcs`, builds mixing matrices from single-stain controls (filename heuristics + peak medians), runs [`flow-tru-ols`](../tru-ols/) unmix, and optionally gates/QC via `flow-gates` + `peacoqc-rs` through `run_qc_pipeline`.

## Related crates

- [`flow-tru-ols`](../tru-ols/) — unmixing core
- [`peacoqc-rs`](../peacoqc-rs/), [`flow-gates`](../gates/), [`flow-plots`](../plots/)
- [`flow-linalg`](../flow-linalg/) — compensation primitives
- [`flow-peak-detection`](../flow-peak-detection/), [`flow-control-detection`](../flow-control-detection/)

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

### QC pipeline flags (`unmix`)

When `--auto-gate` is on (default), controls run through the library `run_qc_pipeline` (margins → raw doublets → preprocess → time-bin QC → scatter or consensus forward-scatter debris → post-debris doublets). Useful options:

- `--qc-preset literature` (default) or `legacy`
- `--qc-debug-dir DIR` — PeacoQC overview (when export succeeds) and `scatter_post_debris.png`
- `--qc-cofactor`, `--qc-no-compensation`, `--qc-no-transform`, `--qc-mad`, `--qc-mad-only`, `--scatter-min-keep-pct`

Structured progress for each stage is logged at target `tru_ols::qc` (e.g. filter logs with `RUST_LOG=tru_ols::qc=info`).

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

## Performance

Unmix throughput is dominated by [`flow-tru-ols`](../tru-ols/) (see its `docs/PROFILING.md`). For batch timing with multithreaded BLAS, set `OMP_NUM_THREADS=1` unless nested parallelism is intentional. `TRU_OLS_BATCH_SHARED_FACTOR_CACHE` controls shared mask-factor cache across stained files when `--stained` is a directory.
