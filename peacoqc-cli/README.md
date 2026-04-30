# PeacoQC-CLI

[Rust](https://www.rust-lang.org/)
[License: MIT](https://opensource.org/licenses/MIT)

Command-line tool for [PeacoQC (Peak-based Quality Control)](https://doi.org/10.1002/cyto.a.24501) for flow cytometry FCS files built on top of the `[peacoqc-rs` crate]([https://crates.io/crates/peacoqc-rs](https://crates.io/crates/peacoqc-rs)), which implements the PeacoQC algorithm. The CLI provides a simple interface to run quality control on one or more FCS files with parallel processing support.

For a **longer default preprocessing chain** (margins, raw doublet masks, compensation/transform, time-bin QC, then scatter or consensus forward-scatter debris gating), the workspace `tru-ols` package exposes `run_qc_pipeline` and related types in its library crate; this CLI stays focused on running PeacoQC on inputs you have already prepared.

## Installation

### From Source

```bash
git clone <repository-url>
cd peacoqc-cli
cargo build --release
```

The binary will be at `target/release/peacoqc`.

### Using Cargo

Install the binary as a cargo tool by pointing to its location (e.g. using the path of the repo cloned above):

```bash
cargo install --path peacoqc-cli
```

## Usage

### Basic Usage (Single File)

Process a single FCS file:

```bash
peacoqc input.fcs
```

With output file:

```bash
peacoqc input.fcs -o output.fcs
```

### Multiple Files

Process multiple files in parallel:

```bash
peacoqc file1.fcs file2.fcs file3.fcs
```

With output directory:

```bash
peacoqc file1.fcs file2.fcs file3.fcs -o /path/to/output/
```

### Directory Processing

Process all FCS files in a directory (recursively):

```bash
peacoqc /path/to/data/
```

The tool will:

- Recursively find all `.fcs` files in the directory
- Process them in parallel
- Save outputs to the specified output directory (or alongside input files with `_cleaned` suffix)

### Options

```bash
peacoqc [OPTIONS] <INPUT_FILES>...

Arguments:
  <INPUT_FILES>...  Path(s) to input FCS file(s) or directory containing FCS files

Options:
  -o, --output <OUTPUT_DIR>      Output directory for cleaned FCS files
  -c, --channels <CHANNELS>      Channels to analyze (comma-separated, e.g., "FSC-A,SSC-A,FL1-A")
  -m, --qc-mode <QC_MODE>        Quality control mode [default: all] [possible values: all, it, mad, none]
  --mad <MAD>                    MAD threshold (default: 6.0) - Higher = less strict
  --it-limit <IT_LIMIT>          Isolation Tree limit (default: 0.6) - Higher = less strict
  --consecutive-bins <BINS>      Consecutive bins threshold (default: 5)
  --remove-zeros                 Remove zeros before peak detection
  --keep-margins                 Keep margin events (default: margins are removed)
  --keep-doublets                Keep doublet events (default: doublets are removed)
  --doublet-nmad <NMAD>          Doublet nmad threshold (default: 4.0)
  --report <REPORT_PATH>         Save QC report as JSON (file for single input, directory for multiple)
  --export-csv <CSV_PATH>        Export QC results as boolean CSV (0/1 values)
  --export-csv-numeric <PATH>     Export QC results as numeric CSV (2000/6000 values, R-compatible)
  --export-json <JSON_PATH>      Export QC metadata as JSON
  --csv-column-name <NAME>       Column name for CSV exports (default: "PeacoQC")
  -v, --verbose                  Verbose output
  -q, --quiet                    Disable all logging (tracing) output
  --benchmark                    Run benchmark: one file, four scenarios; print mean timings (see below)
  -h, --help                     Print help
  -V, --version                  Print version
```

### Performance and benchmarking

- `**--quiet**` (`-q`): Disables all logging (tracing) output. Use this to reduce I/O and measure the impact of logging on run time.
- `**--benchmark**`: Processes a single input file and reports mean timings for four scenarios (minimal pipeline, + FCS write, + CSV export, + plot generation). Requires exactly one input file. Logging is automatically disabled during the benchmark. Example:
  ```bash
  peacoqc --benchmark sample.fcs
  ```
  To measure **logging overhead**, compare wall time of two runs with the same file and options (e.g. with `time` or `hyperfine`):
  ```bash
  time peacoqc file.fcs -o out --plots --plot-dir out
  time peacoqc --quiet file.fcs -o out --plots --plot-dir out
  ```

### QC plot options

When generating QC plots (`--plots` or when prompted), the following flags customize the plot image and typography. Omitted options use the library defaults.


| Option                      | Description                                                | Default                                         |
| --------------------------- | ---------------------------------------------------------- | ----------------------------------------------- |
| `--plot-dir <PLOT_DIR>`     | Directory to save QC plot PNGs                             | Same as input file (single file) or current dir |
| `--hide-spline-mad`         | Hide spline and MAD threshold lines on channel plots       | Shown                                           |
| `--show-bin-boundaries`     | Show bin boundaries (gray vertical lines) on channel plots | Hidden                                          |
| `--plot-width <PIXELS>`     | Plot image width in pixels                                 | 2400                                            |
| `--plot-height <PIXELS>`    | Plot image height in pixels                                | 1800                                            |
| `--plot-title-size <SIZE>`  | Title (caption) font size in points                        | 22                                              |
| `--plot-axis-size <SIZE>`   | Axis label font size in points                             | 20                                              |
| `--plot-tick-size <SIZE>`   | Tick label font size in points                             | 17                                              |
| `--plot-legend-size <SIZE>` | Legend text font size in points                            | 17                                              |
| `--plot-font <FONT>`        | Font family for all plot text (e.g. `sans-serif`, `serif`) | sans-serif                                      |


Example: larger plot and text for presentations:

```bash
peacoqc data.fcs --plots --plot-dir ./plots --plot-width 3200 --plot-height 2400 --plot-title-size 28 --plot-legend-size 20
```

## Examples

### Process Single File

```bash
# Basic processing
peacoqc sample.fcs

# With custom channels
peacoqc sample.fcs -c FL1-A,FL2-A,FL3-A

# Save report
peacoqc sample.fcs --report report.json

# Export QC results as CSV
peacoqc sample.fcs --export-csv qc_results.csv

# Export numeric CSV (R-compatible)
peacoqc sample.fcs --export-csv-numeric qc_results_r.csv

# Export JSON metadata
peacoqc sample.fcs --export-json qc_metadata.json

# Export to directory (auto-named files)
peacoqc sample.fcs --export-csv ./exports/ --export-json ./exports/

# Verbose output
peacoqc sample.fcs -v
```

### Process Multiple Files

```bash
# Process all files in directory
peacoqc /path/to/data/ -o /path/to/output/

# Process specific files
peacoqc file1.fcs file2.fcs file3.fcs -o ./cleaned/

# Save individual reports
peacoqc /path/to/data/ --report /path/to/reports/
```

### Custom QC Settings

```bash
# Use only Isolation Tree method
peacoqc sample.fcs -m it

# Adjust MAD threshold (higher = less strict)
peacoqc sample.fcs --mad 8.0

# Adjust Isolation Tree limit
peacoqc sample.fcs --it-limit 0.7

# Keep margins and doublets (disable removal)
peacoqc sample.fcs --keep-margins --keep-doublets

# Note: Keeping doublets may cause more bins to be flagged as outliers
# on some datasets, as doublets can interfere with peak detection and MAD calculations.
# Use --keep-doublets only if needed to match specific published results
# or if doublet removal is too aggressive for your dataset.
```

## Performance

The CLI automatically processes files in parallel:

- **Multiple files**: Processed simultaneously using all available CPU cores
- **Multiple channels**: Processed in parallel within each file
- **Multiple bins**: Processed in parallel within each channel

Expected speedup:

- **Single file with many channels**: 2-4x speedup on typical multi-core systems
- **Multiple files**: ~N cores speedup (e.g., 8 files on 8 cores → ~8x speedup)

## Output

### Console Output

The tool prints progress and summary information:

```txt
🧬 PeacoQC - Flow Cytometry Quality Control
============================================

📂 Found 5 file(s) to process

✅ Processing Complete!
   Processed: 5 file(s)
   Successful: 5
   ⏱️  Total time: 12.34s
```

With `--verbose` flag, additional details are shown for each file.

### Reports

Reports are saved as JSON files with the following structure:

```json
{
  "filename": "sample.fcs",
  "n_events_before": 50000,
  "n_events_after": 47500,
  "percentage_removed": 5.0,
  "it_percentage": 3.2,
  "mad_percentage": 1.8,
  "consecutive_percentage": 0.5,
  "processing_time_ms": 1234
}
```

For multiple files:

- If `--report` points to a directory: Individual JSON files are created for each input file
- If `--report` points to a file: A combined report with all results is created

### Export Formats

The CLI supports exporting QC results in multiple formats:

#### Boolean CSV (Recommended)

```bash
peacoqc sample.fcs --export-csv qc_results.csv
```

Exports a CSV file with 0/1 values:

- `1` = good event (keep)
- `0` = bad event (remove)

Best for: pandas, R, SQL, general data analysis

#### Numeric CSV (R-Compatible)

```bash
peacoqc sample.fcs --export-csv-numeric qc_results_r.csv
```

Exports a CSV file with numeric codes matching R PeacoQC:

- `2000` = good event (keep)
- `6000` = bad event (remove)

Best for: R compatibility, FlowJo CSV import, legacy pipelines

#### JSON Metadata

```bash
peacoqc sample.fcs --export-json qc_metadata.json
```

Exports comprehensive QC metrics including:

- Event counts (before/after/removed)
- Percentage removed by method (IT, MAD, consecutive)
- Configuration used
- Channels analyzed

Best for: Programmatic access, reporting, provenance tracking

#### Export to Directory

When exporting to a directory, files are automatically named:

```bash
peacoqc sample.fcs --export-csv ./exports/ --export-json ./exports/
# Creates: ./exports/sample.PeacoQC.csv and ./exports/sample.PeacoQC.json
```

### Output Files (FCS)

When an output path is specified (using `-o` or `--output`), the CLI will write cleaned FCS files containing only the events that passed quality control.

- **Single file**: Output file will be saved with `_cleaned` suffix (e.g., `sample.fcs` → `sample_cleaned.fcs`)
- **Multiple files**: If output directory is specified, files will maintain their original names with `_cleaned` suffix
- **Filtered data**: Output FCS files contain only events that passed all QC steps (IT, MAD, consecutive filtering)

**Example**:

```bash
# Single file - saves to sample_cleaned.fcs in same directory
peacoqc sample.fcs -o sample_cleaned.fcs

# Multiple files - saves to output directory with _cleaned suffix
peacoqc file1.fcs file2.fcs -o ./clean_dir/_
# Creates: ./clean_dir/file1_cleaned.fcs, ./clean_dir/file2_cleaned.fcs
```

This provides feature parity with the R PeacoQC package's `save_fcs=TRUE` option.

## Error Handling

The CLI continues processing even if individual files fail:

- **Successful files**: Processed and included in results
- **Failed files**: Error messages are printed, processing continues
- **Exit code**: Returns non-zero exit code if any files failed

## Integration with peacoqc-rs

This CLI is built on top of the `[peacoqc-rs` library]([https://crates.io/crates/peacoqc-rs](https://crates.io/crates/peacoqc-rs)), which provides:

- Trait-based design for maximum flexibility
- Efficient parallel processing
- Comprehensive quality control algorithms
- Integration with `[flow-fcs](https://crates.io/crates/flow-fcs)` for FCS file support

See the `[peacoqc-rs` documentation]([https://crates.io/crates/peacoqc-rs](https://crates.io/crates/peacoqc-rs)) for library usage.

## License

MIT License - see LICENSE file for details

## Attribution

We gratefully acknowledge the original PeacoQC algorithm authors:

**Original Paper:**

- [Emmaneel, A., Quintelier, K., Sichien, D., Rybakowska, P., Marañón, C., Alarcón-Riquelme, M. E., Van Isterdael, G., Van Gassen, S., & Saeys, Y. (2022). PeacoQC: Peak-based selection of high quality cytometry data. *Cytometry A*, 101(4), 325-338. `https://doi.org/10.1002/cyto.a.24501](https://doi.org/10.1002/cyto.a.24501)`

**Original R Implementation:**

- [GitHub: `https://github.com/saeyslab/PeacoQC](https://github.com/saeyslab/PeacoQC)`
- Authors: Annelies Emmaneel, Katrien Quintelier, and the Saeys Lab

## Contributing

Contributions are welcome! Please feel free to open issues or submit a Pull Request on [Github](https://github.com/jrmoynihan/flow).