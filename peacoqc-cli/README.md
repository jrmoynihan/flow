# PeacoQC-CLI

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Command-line tool for PeacoQC (Peak-based Quality Control) for flow cytometry FCS files. This CLI provides a simple interface to run quality control on one or more FCS files with parallel processing support.

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
  --remove-margins               Remove margin events before QC (default: true)
  --remove-doublets              Remove doublets before QC (default: true)
  --doublet-nmad <NMAD>          Doublet nmad threshold (default: 4.0)
  --report <REPORT_PATH>         Save QC report as JSON (file for single input, directory for multiple)
  -v, --verbose                  Verbose output
  -h, --help                     Print help
  -V, --version                  Print version
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

# Disable margin/doublet removal
peacoqc sample.fcs --no-remove-margins --no-remove-doublets
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

```
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

### Output Files (FCS)

**⚠️ Note**: FCS file writing is currently not implemented. The `flow-fcs` crate (which `peacoqc-cli` uses for FCS file support) is currently read-only.

To achieve feature parity with the R PeacoQC package (which saves filtered FCS files with `save_fcs=TRUE`), FCS file writing needs to be implemented in the `flow-fcs` crate first. Once implemented:

- Output files will be saved with `_cleaned` suffix (e.g., `sample.fcs` → `sample_cleaned.fcs`)
- If output directory is specified, files will maintain their original names with `_cleaned` suffix
- Filtered FCS files will contain only the events that passed quality control

This feature is planned and will be added when `flow-fcs` supports writing FCS files.

## Error Handling

The CLI continues processing even if individual files fail:

- **Successful files**: Processed and included in results
- **Failed files**: Error messages are printed, processing continues
- **Exit code**: Returns non-zero exit code if any files failed

## Integration with peacoqc-rs

This CLI is built on top of the `peacoqc-rs` library, which provides:

- Trait-based design for maximum flexibility
- Efficient parallel processing
- Comprehensive quality control algorithms
- Integration with `flow-fcs` for FCS file support

See the [peacoqc-rs documentation](../peacoqc-rs/README.md) for library usage.

## License

MIT License - see LICENSE file for details
