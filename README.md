# flow

A Rust workspace for flow cytometry analysis tools, including libraries for reading FCS files, creating plots, working with gates, and QC'ing data.

:construction: 
> **⚠️ Under Construction**: This workspace is actively under development. APIs may change, and some features may be incomplete. Use with caution in production environments.
>
:construction:

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Repository](https://img.shields.io/badge/github-jrmoynihan%2Fflow-blue)](https://github.com/jrmoynihan/flow)

## Overview


This workspace contains multiple crates for flow cytometry analysis:

- **`flow-fcs`**: A comprehensive, type-safe API for reading, parsing, and manipulating Flow Cytometry Standard (FCS) files. Built on top of [Polars](https://www.pola.rs/) for efficient columnar data operations, with zero-copy data access, SIMD-accelerated operations, and support for common flow cytometry data transformations.
- **`flow-plots`**: Package for drawing and interacting with plots in flow cytometry data.
- **`flow-gates`**: Package for drawing and interacting with gates in flow cytometry data.
- **`peacoqc-rs`**: A reimplementation of the PeacoQC (R) algorithm from the Saeys lab, parallelized in Rust.
- **`peacoqc-cli`**: A command-line interface (CLI) tool for using `peacoqc-rs`.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request or feature request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Polars](https://www.pola.rs/) for high-performance data operations
- Uses [ndarray](https://github.com/rust-ndarray/ndarray) for matrix operations
- Inspired by the need for fast, type-safe FCS file handling in Rust

## Related Projects

- [Polars](https://www.pola.rs/): Fast DataFrame library
- [ndarray](https://github.com/rust-ndarray/ndarray): N-dimensional array library

