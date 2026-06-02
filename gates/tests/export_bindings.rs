//! Generate TypeScript bindings for the public gate types.
//!
//! Run with: `cargo test -p flow-gates --features typescript`
//! Output goes to `TS_RS_EXPORT_DIR` (default `./bindings/`), or wherever the
//! consuming app points it. The app re-exports these alongside its own ts-rs
//! types so the frontend imports generated, never-drifting gate types.
//!
//! `GateParameters` and `LabelPosition` are intentionally NOT exported as
//! standalone types: they have custom serde impls, so their TS shape is given
//! inline at each referencing field via `#[ts(type = "...")]`.
#![cfg(all(test, feature = "typescript"))]
#![allow(clippy::expect_used)]

use flow_gates::types::{
    BooleanOperation, DerivedFrom, Gate, GateCoordinateSpace, GateGeometry, GateMode, GateNode,
    MaskSource, QuadrantDivider, QuadrantGate, QuadrantPosition, QuadrantSub, ThresholdDirection,
};
use ts_rs::{Config, TS};

#[test]
fn export_all_gate_bindings() {
    let cfg = Config::default();

    // Leaf enums / primitives-ish.
    GateCoordinateSpace::export(&cfg).expect("export GateCoordinateSpace");
    BooleanOperation::export(&cfg).expect("export BooleanOperation");
    ThresholdDirection::export(&cfg).expect("export ThresholdDirection");
    MaskSource::export(&cfg).expect("export MaskSource");

    // Building blocks.
    GateNode::export(&cfg).expect("export GateNode");
    QuadrantDivider::export(&cfg).expect("export QuadrantDivider");
    QuadrantPosition::export(&cfg).expect("export QuadrantPosition");
    QuadrantSub::export(&cfg).expect("export QuadrantSub");
    QuadrantGate::export(&cfg).expect("export QuadrantGate");

    // Compound.
    GateGeometry::export(&cfg).expect("export GateGeometry");
    GateMode::export(&cfg).expect("export GateMode");
    DerivedFrom::export(&cfg).expect("export DerivedFrom");

    // Top-level (export_all pulls in any deps too).
    Gate::export_all(&cfg).expect("export Gate + deps");
}
