<img width="1245" height="705" alt="FerrisAtCytometer" src="https://github.com/user-attachments/assets/87278ae7-a1db-4b22-8fe7-557d1e84d25e" />

# flow

Flow cytometry analysis tools, _oxidized_.  The aim of this workspace is to leverage the blazing-fast speed, memory and type safety, and fearless concurrency of the Rust language to scale-up to modern flow cytometry workflows requiring millions of events without breaking a sweat.  Biological data can be unpredictable; the tools to analyze it shouldn't be.

The workspace includes libaries for:

- Reading FCS files
- Creating plots
- Working with gates
- QC'ing data
- Performing unmixing

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
- **`flow-tru-ols`**: TRU-OLS (Truncated ReUnmixing OLS) algorithm for flow cytometry unmixing; optional integration with `flow-fcs` and `flow-plots`.
- **`tru-ols-cli`**: Command-line tool for TRU-OLS unmixing (batch or single-file, with optional QC and plot output).

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request or feature request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Polars](https://www.pola.rs/) for high-performance data operations
- Uses [faer](https://github.com/sarah-ek/faer) for pure-Rust linear algebra (compensation, unmixing)
- Inspired by the need for fast, type-safe FCS file handling in Rust

## Related Projects

- [Polars](https://www.pola.rs/): Fast DataFrame library
- [faer](https://github.com/sarah-ek/faer): Pure-Rust linear algebra library
- [PeacoQC](https://github.com/saeyslab/PeacoQC): Peak-based selection of high quality cytometry data
- 
