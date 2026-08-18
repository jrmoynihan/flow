# peacoqc-rs

[![PyPI version](https://img.shields.io/pypi/v/peacoqc-rs.svg)](https://pypi.org/project/peacoqc-rs/)
[![Python versions](https://img.shields.io/pypi/pyversions/peacoqc-rs.svg)](https://pypi.org/project/peacoqc-rs/)
[![CI](https://github.com/jrmoynihan/flow/actions/workflows/peacoqc-py.yml/badge.svg)](https://github.com/jrmoynihan/flow/actions/workflows/peacoqc-py.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Python bindings for [`peacoqc-rs`](../peacoqc-rs/), a Rust implementation of the
[Peak Extraction and Cleaning Oriented Quality Control (PeacoQC)](https://github.com/saeyslab/PeacoQC) [(paper)](https://pubmed.ncbi.nlm.nih.gov/34549881/) automated quality-control algorithm for flow cytometry data.

## How it works

A native extension module (built with [maturin](https://github.com/PyO3/maturin) and
[PyO3](https://pyo3.rs/)) that wraps the Rust `peacoqc-rs` crate and exposes its QC entry points to
Python, bridging event tables through [Polars](https://pola.rs/)/
[pyo3-polars](https://github.com/pola-rs/polars/tree/main/pyo3-polars). Type stubs
([`peacoqc.pyi`](python/peacoqc/peacoqc.pyi)) ship with the package, so `run_qc(...)`,
`FcsFile`, etc. provide autocomplete and type-check in editors without any extra config.

Prebuilt wheels are published for Linux (x86_64/aarch64), macOS (Intel/Apple Silicon), and
Windows (x64), targeting Python 3.9+ via PyO3's stable ABI (`abi3`) — one wheel per platform
covers every supported Python version, so there's no wheel-matrix version lottery to worry about.

## Installation

```bash
pip install peacoqc-rs
```

Or with [uv](https://docs.astral.sh/uv/):

```bash
uv add peacoqc-rs
```

## Quick Start

**Point at an `.fcs` file and get filtered data back in one call:**

```python
import peacoqc

result, clean_df = peacoqc.run_qc_on_fcs("sample.fcs")
print(f"Removed {result.percentage_removed:.2f}% of events")
```

`run_qc_on_fcs` opens the file, applies compensation/transformation, and runs PeacoQC — the
fastest path if you're starting from a raw `.fcs` file.

**Already have a Polars DataFrame?** Run QC directly against it:

```python
import polars as pl
import peacoqc

df = pl.read_csv("events.csv")

result = peacoqc.run_qc(
    df,
    channels=["FL1-A", "FL2-A"],
    channel_ranges={"FL1-A": (0.0, 262144.0), "FL2-A": (0.0, 262144.0)},
)
print(f"Removed {result.percentage_removed:.2f}% of events")

# Apply the mask to filter good cells
clean_df = df.filter(pl.Series(result.good_cells))
```

**Need more control over each pipeline stage?** Margin removal, doublet removal, and
FCS-specific helpers (`FcsFile`, `open_fcs`, `preprocess`, `filter_fcs`) are all available —
see [`peacoqc.pyi`](python/peacoqc/peacoqc.pyi) for the full API surface and every function's
parameters, or `test_poc.py` for exercised end-to-end usage.

## Checking versions

`peacoqc-py`'s bindings version and the underlying `peacoqc-rs` algorithm version are
tracked independently — a bindings-only release (e.g. fixing a Python-facing error
message) doesn't require bumping the algorithm version, and vice versa. Check both
when comparing behavior against the [peacoqc-rs changelog](../peacoqc-rs/):

```python
import peacoqc

print(peacoqc.__version__)             # peacoqc-py bindings version
print(peacoqc.__peacoqc_rs_version__)  # peacoqc-rs algorithm version baked into this wheel
```

## Performance

Same algorithmic costs as `peacoqc-rs`; Python overhead is binding/conversion only. GPU features
follow the Rust crate defaults when enabled at build time.

## Building from source

For contributors: build from this directory with the usual [PyO3/maturin](https://github.com/pyo3/maturin) flow.

```bash
maturin develop          # build + install into the active venv for local testing
maturin build --release  # build a release wheel into dist/
```

## License

MIT

## Related crates

- [`peacoqc-rs`](../peacoqc-rs/) — algorithm implementation
- [`flow-fcs`](../fcs/) — FCS loading behind the bindings
