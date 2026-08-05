# peacoqc-py

Python bindings for [`peacoqc-rs`](../peacoqc-rs/) flow cytometry quality control.

## What this crate is for

Use `peacoqc-py` when you want PeacoQC from Python (PyO3 + Polars) without rewriting the QC pipeline. The Rust algorithm crate remains the source of truth.

Use a sibling instead when you need:

- **Rust library API** → [`peacoqc-rs`](../peacoqc-rs/)
- **CLI** → [`peacoqc-cli`](../peacoqc-cli/)
- **Long QC + unmix orchestration** → [`tru-ols`](../tru-ols-cli/) CLI

**Note:** This package may sit outside the root Cargo workspace members; build via its own `Cargo.toml` / maturin (or project) workflow.

## How it works

A `cdylib` named `peacoqc` wraps `peacoqc-rs` (with `flow-fcs`) and exposes QC entry points to Python, bridging event tables through Polars/`pyo3-polars`.

## Related crates

- [`peacoqc-rs`](../peacoqc-rs/) — algorithm implementation
- [`flow-fcs`](../fcs/) — FCS loading behind the bindings
- [`flow-density`](../flow-density/) — intended shared KDE (PeacoQC still vendors density today)

## Demo / API

Build from this directory with your usual PyO3/maturin flow (see crate `Cargo.toml`). Version `0.1.0`. Prefer calling into the same config/result concepts as `peacoqc-rs` (`PeacoQCConfig`, good-cell masks, exports).

```python
import polars as pl
import peacoqc

# Load your data as a polars DataFrame
df = pl.read_csv("events.csv")

# Run PeacoQC quality control
result = peacoqc.run_qc(
    df,
    channels=["FL1-A", "FL2-A"],
    channel_ranges={"FL1-A": (0.0, 262144.0), "FL2-A": (0.0, 262144.0)},
)
print(f"Removed {result.percentage_removed:.2f}% of events")

# Apply the mask to filter good cells
good_mask = result.good_cells
clean_df = df.filter(pl.Series(good_mask))
```

## Performance

Same algorithmic costs as `peacoqc-rs`; Python overhead is binding/conversion only. GPU features follow the Rust crate defaults when enabled at build time.

## License

MIT
