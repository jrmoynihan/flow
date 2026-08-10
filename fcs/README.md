# flow-fcs

A high-performance Rust library for reading, parsing, and manipulating Flow Cytometry Standard (FCS) files.

[![crates.io](https://img.shields.io/crates/v/flow-fcs.svg)](https://crates.io/crates/flow-fcs)
[![docs.rs](https://docs.rs/flow-fcs/badge.svg)](https://docs.rs/flow-fcs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Repository](https://img.shields.io/badge/github-jrmoynihan%2Fflow--fcs-blue)](https://github.com/jrmoynihan/flow-fcs)

## Overview

`flow-fcs` provides a comprehensive, type-safe API for working with FCS files used in flow cytometry analysis. Built on top of [Polars](https://www.pola.rs/) for efficient columnar data operations, the library offers zero-copy data access, SIMD-accelerated operations, and support for common flow cytometry data transformations.

Use a sibling crate instead when you need:

- **QC / gates / plots** → [`peacoqc-rs`](../peacoqc-rs/), [`flow-gates`](../gates/), [`flow-plots`](../plots/)
- **Column compression / `.fcz`** → [`flow-fcs-compress`](../flow-fcs-compress/) (optional `compress` feature)
- **Spillover math primitives** → [`flow-linalg`](../flow-linalg/) (optional `compensation` feature)
- **TRU-OLS truncated unmixing** → [`flow-tru-ols`](../tru-ols/) (preferred for spectral unmixing)

## How It Works

- **High Performance**:
  - Memory-mapped file I/O for efficient large file handling
  - Zero-copy column access via Polars for streaming stats, sliced data, lazy query evaluation across million-event files, and Arrow data format interop
  - SIMD-accelerated operations
  - Parallel processing with Rayon
- **Data Transformations**:
  - Arcsinh transformation (with configurable cofactors)
  - Compensation (spillover matrix) via the `compensation` feature
  - Spectral unmixing
- **Polars Integration**:
  - Lazy evaluation for complex queries
  - Streaming execution for large datasets
  - Apache Arrow interop
- **Comprehensive Metadata Access**:
  - Type-safe keyword access
  - Parameter metadata (names, labels, transforms)
  - GUID and file information
- **Type Safety**: Strong typing throughout with clear error messages
- **FCS v1.0–4.0 Support** with version-aware keyword/metadata typing.

## Installation

```bash
cargo add flow-fcs
```

Or add this to your `Cargo.toml`:

```toml
[dependencies]
flow-fcs = "0.4.1"
```

### Optional Features

| Feature | Description |
| ------- | ----------- |
| `compress` | `.fcz` / compressed DATA via [`flow-fcs-compress`](../flow-fcs-compress/) |
| `compensation` | Spillover apply paths via [`flow-linalg`](../flow-linalg/) |
| `blas` | Opt-in system BLAS for selected linear-algebra paths |
| `typescript` | TypeScript bindings via `ts-rs` |
| `specta` | `specta::Type` derives (e.g. tauri-specta) |
| `parquet-sidecar` | Parquet sidecar write/read (implies `compress`) |
| `test-util` | `Fcs::for_testing` for out-of-crate fixtures |
| `synthetic` | Gaussian-mixture event tables (`flow_fcs::synthetic`); enables `test-util` |

```toml
[dependencies]
flow-fcs = { version = "0.4.1", features = ["compensation", "compress"] }
```

## Quick Start

### Opening an FCS file

```rust
use flow_fcs::Fcs;
use anyhow::Result;
use std::borrow::Cow;

fn example() -> Result<()> {
    let fcs: Fcs = Fcs::open("path/to/file.fcs")?;

    let num_events: &usize = fcs.get_number_of_events()?;
    let num_parameters: &usize = fcs.get_number_of_parameters()?;
    let guid: Cow<'_, str> = fcs.get_guid()?;

    println!("File: {num_events} events, {num_parameters} parameters");
    println!("GUID: {guid}");
    Ok(())
}
```

### Accessing parameter data

```rust
use flow_fcs::Fcs;
use anyhow::Result;

fn example(fcs: &Fcs) -> Result<()> {
    let fsc_data: &[f32] = fcs.get_parameter_events_slice("FSC-A")?;
    let xy_pairs: Vec<(f32, f32)> = fcs.get_xy_pairs("FSC-A", "SSC-A")?;
    let (min, max, mean, std): (f32, f32, f32, f32) =
        fcs.get_parameter_statistics("FL1-A")?;
    println!("FL1-A: min={min}, max={max}, mean={mean:.2}, std={std:.2}");
    Ok(())
}
```

### Data transformations

```rust
use flow_fcs::{EventDataFrame, Fcs};
use anyhow::Result;
use faer::Mat;

fn example(fcs: &Fcs) -> Result<()> {
    let transformed: EventDataFrame = fcs.apply_arcsinh_transform("FL1-A", 200.0)?;
    let transformed_all: EventDataFrame = fcs.apply_default_arcsinh_transform()?;

    // Requires the `compensation` feature (uses flow-linalg under the hood)
    let compensated: EventDataFrame = fcs.apply_file_compensation()?;

    let comp_matrix: Mat<f32> = faer::mat![[1.0, 0.1], [0.05, 1.0]];
    let channels: Vec<&str> = vec!["FL1-A", "FL2-A"];
    let compensated_custom: EventDataFrame =
        fcs.apply_compensation(comp_matrix.as_ref(), &channels)?;
    Ok(())
}
```

### Working with metadata

```rust
use flow_fcs::{Fcs, Parameter};
use anyhow::Result;
use std::borrow::Cow;

fn example(fcs: &Fcs) -> Result<()> {
    let filename: Cow<'_, str> = fcs.get_fil_keyword()?;
    let cytometer: Cow<'_, str> = fcs.get_keyword_string_value("$CYT")?;
    let param: &Parameter = fcs.find_parameter("FL1-A")?;
    println!("Channel: {}, Label: {}", param.channel_name, param.label_name);
    Ok(())
}
```

## API Overview

### Core Types

- `Fcs`: Main struct representing an FCS file
- `Header`: FCS file header information
- `Metadata`: Text segment metadata and keywords
- `Parameter`: Parameter/channel information
- `EventDataFrame`: Polars DataFrame containing event data

### Key Methods

#### File Operations

- `Fcs::open(path)`: Open and parse an FCS file
- `Fcs::new()`: Create an empty FCS struct

#### Data Access

- `get_parameter_events_slice(channel_name)`: Get zero-copy slice of parameter data
- `get_xy_pairs(x_param, y_param)`: Get (x, y) coordinate pairs for plotting
- `get_parameter_statistics(channel_name)`: Calculate min, max, mean, std (streaming)
- `get_event_count_from_dataframe()`: Get number of events
- `get_parameter_count_from_dataframe()`: Get number of parameters

#### Transformations

- `apply_arcsinh_transform(parameter, cofactor)`: Apply arcsinh transformation
- `apply_arcsinh_transforms(params)`: Apply to multiple parameters
- `apply_default_arcsinh_transform()`: Transform all fluorescence parameters
- `apply_compensation(matrix, channels)`: Apply compensation matrix
- `apply_file_compensation()`: Apply compensation from $SPILLOVER keyword
- `apply_spectral_unmixing(matrix, channels, cofactor)`: Apply spectral unmixing

#### Metadata

- `get_guid()`: Get file GUID
- `get_fil_keyword()`: Get filename
- `get_keyword_string_value(keyword)`: Get any keyword as string
- `get_number_of_events()`: Get total event count
- `get_number_of_parameters()`: Get parameter count
- `find_parameter(channel_name)`: Find parameter by name

See [docs.rs/flow-fcs](https://docs.rs/flow-fcs) for the full API (`Header`, `Metadata`, `Parameter`, `EventDataFrame`, GatingML-adjacent helpers, write paths, etc.). More examples live under `tests/`.

## Performance

The library is aimed at high-performance:

- **Memory-mapped I/O**: Large files are memory-mapped for efficient access
- **Zero-copy operations**: Polars enables zero-copy column access
- **SIMD acceleration**: Built-in SIMD operations via Polars
- **Streaming execution**: Statistics and aggregations use streaming mode for large datasets
- **Parallel processing**: Rayon enables parallel operations where applicable (multi-channel / multi-parameter paths)

Criterion benches: `dataframe_parsing`, `matrix_operations`, `column_extract`, `serialize_data`.

Micro-opt A/B notes: [`docs/PERF_AB.md`](docs/PERF_AB.md).

```bash
cargo test -p flow-fcs --lib
cargo bench -p flow-fcs
```

## FCS Standard Support

The library support FCS versions:

- FCS 1.0
- FCS 2.0
- FCS 3.0
- FCS 3.1 (default)
- FCS 3.2
- FCS 4.0 (when available)

## Error Handling

- File I/O errors
- Invalid FCS format
- Missing required keywords
- Type conversion errors
- Data validation failures
APIs return `anyhow::Result` (and crate error types where appropriate) with context for I/O failures, invalid FCS layout, missing keywords, and validation errors.

## Contributing

Contributions are welcome! Submit a Pull Request or Issue on the repository.

## License

MIT — see the LICENSE file for details.

## Acknowledgments

- Built with [Polars](https://www.pola.rs/) for high-performance columnar data operations
- Uses [faer](https://github.com/sarah-ek/faer) for pure-Rust linear algebra (compensation paths via [`flow-linalg`](../flow-linalg/))

## Related Crates & Projects

- [`flow-fcs-compress`](../flow-fcs-compress/) — codecs and containers
- [`flow-linalg`](../flow-linalg/) — compensation / condition metrics
- [`flow-tru-ols`](../tru-ols/) — spectral unmixing (TRU-OLS)
- [`flow-plots`](../plots/), [`flow-gates`](../gates/), [`peacoqc-rs`](../peacoqc-rs/) — visualization, gating, QC
- [Polars](https://www.pola.rs/) — DataFrame / Arrow engine behind event tables
- [faer](https://github.com/sarah-ek/faer) — pure-Rust linear algebra
- [anyhow](https://crates.io/crates/anyhow) - Flexible concrete Error types built on std::error::Error
- [ISAC FCS specification](https://flowcyt.sourceforge.net/) — Flow Cytometry Standard