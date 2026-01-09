# flow-fcs

A high-performance Rust library for reading, parsing, and manipulating Flow Cytometry Standard (FCS) files.

:construction:

> **⚠️ Under Construction**: This library is actively under development. APIs may change, and some features may be incomplete. Use with caution in production environments.
>
:construction:

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Repository](https://img.shields.io/badge/github-jrmoynihan%2Fflow--fcs-blue)](https://github.com/jrmoynihan/flow-fcs)

## Overview

`flow-fcs` provides a comprehensive, type-safe API for working with FCS files used in flow cytometry analysis. Built on top of [Polars](https://www.pola.rs/) for efficient columnar data operations, the library offers zero-copy data access, SIMD-accelerated operations, and support for common flow cytometry data transformations.

## Features

- **Full FCS Standard Support**: Supports FCS versions 1.0 through 4.0

- **High Performance**:
  - Memory-mapped file I/O for efficient large file handling
  - Zero-copy column access via Polars
  - SIMD-accelerated operations
  - Parallel processing with Rayon
- **Data Transformations**:
  - Arcsinh transformation (with configurable cofactors)
  - Compensation (spillover matrix)
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

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
flow-fcs = "0.1.0"
```

### Optional Features

- `typescript`: Generate TypeScript bindings for Rust types (requires `ts-rs`)

```toml
[dependencies]
flow-fcs = { version = "0.1.0", features = ["typescript"] }
```

## Quick Start

### Opening an FCS File

```rust
use flow_fcs::Fcs;

// Open an FCS file
let fcs = Fcs::open("path/to/file.fcs")?;

// Get basic information
let num_events = fcs.get_number_of_events()?;
let num_parameters = fcs.get_number_of_parameters()?;
let guid = fcs.get_guid()?;

println!("File: {} events, {} parameters", num_events, num_parameters);
println!("GUID: {}", guid);
```

### Accessing Parameter Data

```rust
// Get events for a specific parameter (zero-copy slice)
let fsc_data = fcs.get_parameter_events_slice("FSC-A")?;

// Get (x, y) pairs for plotting
let xy_pairs = fcs.get_xy_pairs("FSC-A", "SSC-A")?;

// Get parameter statistics using streaming (memory-efficient)
let (min, max, mean, std) = fcs.get_parameter_statistics("FL1-A")?;
println!("FL1-A: min={}, max={}, mean={:.2}, std={:.2}", min, max, mean, std);
```

### Data Transformations

```rust
// Apply arcsinh transformation to a parameter
let transformed = fcs.apply_arcsinh_transform("FL1-A", 200.0)?;

// Apply arcsinh to all fluorescence parameters with default cofactor
let transformed = fcs.apply_default_arcsinh_transform()?;

// Apply compensation from file's $SPILLOVER keyword
let compensated = fcs.apply_file_compensation()?;

// Apply custom compensation matrix
use ndarray::Array2;
let comp_matrix = Array2::from_shape_vec((2, 2), vec![
    1.0, 0.1,
    0.05, 1.0,
])?;
let channels = vec!["FL1-A", "FL2-A"];
let compensated = fcs.apply_compensation(&comp_matrix, &channels)?;
```

### Working with Metadata

```rust
// Get keyword values
let filename = fcs.get_fil_keyword()?;
let cytometer = fcs.get_keyword_string_value("$CYT")?;

// Access parameter information
let param = fcs.find_parameter("FL1-A")?;
println!("Channel: {}, Label: {}", param.channel_name, param.label_name);
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

## Performance

The library is optimized for performance:

- **Memory-mapped I/O**: Large files are memory-mapped for efficient access
- **Zero-copy operations**: Polars enables zero-copy column access
- **SIMD acceleration**: Built-in SIMD operations via Polars
- **Streaming execution**: Statistics and aggregations use streaming mode for large datasets
- **Parallel processing**: Rayon enables parallel operations where applicable

## FCS Standard Support

The library supports FCS versions:

- FCS 1.0
- FCS 2.0
- FCS 3.0
- FCS 3.1 (default)
- FCS 3.2
- FCS 4.0

## Error Handling

The library uses `anyhow::Result` for error handling, providing detailed error messages for common issues:

- File I/O errors
- Invalid FCS format
- Missing required keywords
- Type conversion errors
- Data validation failures

## Examples

See the `tests/` directory for more comprehensive examples of library usage.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Polars](https://www.pola.rs/) for high-performance data operations
- Uses [ndarray](https://github.com/rust-ndarray/ndarray) for matrix operations
- Inspired by the need for fast, type-safe FCS file handling in Rust

## Related Projects

- [Polars](https://www.pola.rs/): Fast DataFrame library
- [ndarray](https://github.com/rust-ndarray/ndarray): N-dimensional array library
