# flow-gates

Gates for flow cytometry: geometry, hierarchies, GatingML 2.0, efficient filtering, and automated preprocessing gates.

[![crates.io](https://img.shields.io/crates/v/flow-gates.svg)](https://crates.io/crates/flow-gates)
[![docs.rs](https://docs.rs/flow-gates/badge.svg)](https://docs.rs/flow-gates)
[![MIT](https://img.shields.io/crates/l/flow-gates.svg)](LICENSE)

## What this crate is for

Use `flow-gates` when you need to:

- Define polygon / rectangle / ellipse / range gates with parent–child hierarchies
- Filter FCS events through gates (R*-tree spatial indexing)
- Import/export GatingML 2.0
- Build **automated** scatter, doublet, and debris gates (`automated` module)

Use a sibling instead when you need:

- **FFT KDE only** → [`flow-density`](../flow-density/)
- **Clustering only** → [`flow-clustering`](../flow-clustering/)
- **PeacoQC time-bin QC** → [`peacoqc-rs`](../peacoqc-rs/)
- **Plot rendering** → [`flow-plots`](../plots/)
- **Full unmix QC pipeline orchestration** → [`tru-ols`](../tru-ols-cli/) (`run_qc_pipeline`)

## How it works

A `Gate` carries geometry, plot axis parameters, and a `GateCoordinateSpace` (raw vs compensated/transformed context). Filtering resolves gate trees and returns event indices. Automated helpers use [`flow-density`](../flow-density/) and [`flow-clustering`](../flow-clustering/) to propose preprocessing gates rather than reimplementing those primitives.

## Related crates

- [`flow-density`](../flow-density/) — KDE for automated density-aware gates
- [`flow-clustering`](../flow-clustering/) — K-means / DBSCAN / GMM for scatter populations
- [`flow-fcs`](../fcs/) — event source
- [`flow-plots`](../plots/) — draw gates on density/scatter plots
- [`peacoqc-rs`](../peacoqc-rs/) — complementary time-based QC

## Demo / API

```toml
[dependencies]
flow-gates = "0.5.0"
flow-fcs = "0.5"
```

### Manual gate

```rust
use flow_fcs::Fcs;
use flow_gates::{
    create_polygon_geometry, filter_events_by_gate, Gate, GateCoordinateSpace, GateGeometry,
};
use flow_gates::filtering::EventData;
use flow_gates::GateResult;

fn example() -> GateResult<()> {
    let coords: Vec<(f32, f32)> = vec![
        (100.0_f32, 200.0_f32),
        (300.0_f32, 200.0_f32),
        (300.0_f32, 400.0_f32),
        (100.0_f32, 400.0_f32),
    ];
    let geometry: GateGeometry = create_polygon_geometry(coords, "FSC-A", "SSC-A")?;
    let gate: Gate = Gate::new(
        "lymphocytes",
        "Lymphocytes",
        geometry,
        "FSC-A",
        "SSC-A",
        GateCoordinateSpace::Raw,
    );

    let fcs: Fcs = Fcs::open("data.fcs").map_err(|e| {
        flow_gates::GateError::filtering_error(e.to_string())
    })?;
    let data: EventData<'_> = EventData::raw_from_fcs(&fcs, "FSC-A", "SSC-A")?;
    let indices: Vec<usize> = filter_events_by_gate(data, &gate, None)?;
    Ok(())
}
```

Rectangle helper:

```rust
use flow_gates::{Gate, GateCoordinateSpace};
use flow_gates::GateResult;

fn example() -> GateResult<()> {
    let gate: Gate = Gate::rectangle(
        "rect",
        "Rectangle",
        (100.0_f32, 200.0_f32),
        (500.0_f32, 600.0_f32),
        "FSC-A",
        "SSC-A",
        GateCoordinateSpace::Raw,
    )?;
    Ok(())
}
```

### Automated preprocessing

```rust
use flow_fcs::Fcs;
use flow_gates::Gate;
use flow_gates::automated::{create_preprocessing_gates, PreprocessingConfig, PreprocessingGates};
use flow_gates::GateError;

fn example(fcs: &Fcs) -> Result<(), GateError> {
    let config: PreprocessingConfig = PreprocessingConfig::default();
    let gates: PreprocessingGates = create_preprocessing_gates(fcs, &config)?;
    let scatter: &Option<Gate> = &gates.scatter_gate;
    let doublet: &Option<Gate> = &gates.doublet_gate;
    Ok(())
}
```

See [`src/automated/`](src/automated/) and [docs.rs/flow-gates](https://docs.rs/flow-gates) for GatingML, statistics, and hierarchy APIs.

## Performance

R*-tree point-in-gate queries and Rayon-friendly filtering scale to large event counts. Env-gated QC plot smoke tests: `FLOW_GATES_QC_TEST_PLOTS=1`.

```bash
cargo test -p flow-gates --lib
```

## License

MIT
