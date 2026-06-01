//! `GateParameters`, plot matching, and JSON compatibility.

use flow_gates::types::{
    gate_parameters_from_geometry_and_axes, Gate, GateCoordinateSpace, GateGeometry, GateNode,
    GateParameters,
};
use std::sync::Arc;

#[test]
fn legacy_tuple_deserializes_to_two_channel() {
    let json = r#"["FSC-A","SSC-A"]"#;
    let gp: GateParameters = serde_json::from_str(json).expect("params");
    match gp {
        GateParameters::TwoChannel { x, y } => {
            assert_eq!(x.as_ref(), "FSC-A");
            assert_eq!(y.as_ref(), "SSC-A");
        }
        _ => panic!("expected two_channel"),
    }
}

#[test]
fn one_channel_matches_either_plot_axis() {
    let p = GateParameters::OneChannel {
        channel: Arc::from("FSC-A"),
    };
    assert!(p.matches_plot_parameters("FSC-A", "PE-A"));
    assert!(p.matches_plot_parameters("PE-A", "FSC-A"));
    assert!(!p.matches_plot_parameters("SSC-A", "PE-A"));
}

#[test]
fn legacy_one_channel_with_companion_drops_companion() {
    // Workspaces saved before `companion` was removed include the field; deserialization
    // must accept and silently drop it.
    let json = r#"{"type":"one_channel","channel":"FL1-A","companion":"SSC-A"}"#;
    let gp: GateParameters = serde_json::from_str(json).expect("params");
    match gp {
        GateParameters::OneChannel { channel } => assert_eq!(channel.as_ref(), "FL1-A"),
        _ => panic!("expected one_channel"),
    }
}

#[test]
fn two_channel_matches_swap() {
    let p = GateParameters::TwoChannel {
        x: Arc::from("FSC-A"),
        y: Arc::from("SSC-A"),
    };
    assert!(p.matches_plot_parameters("FSC-A", "SSC-A"));
    assert!(p.matches_plot_parameters("SSC-A", "FSC-A"));
    assert!(!p.matches_plot_parameters("FSC-A", "PE-A"));
}

#[test]
fn range_geometry_builds_one_channel_parameters() {
    let min = GateNode::new("a").with_coordinate("FL1-A", 10.0);
    let max = GateNode::new("b").with_coordinate("FL1-A", 90.0);
    let geom = GateGeometry::Range { min, max };
    let gp = gate_parameters_from_geometry_and_axes(
        &geom,
        Arc::from("FL1-A"),
        Arc::from("SSC-A"),
    );
    match gp {
        GateParameters::OneChannel { channel } => {
            assert_eq!(channel.as_ref(), "FL1-A");
        }
        _ => panic!("expected one_channel"),
    }
}

#[test]
fn gate_matches_plot_delegates_to_parameters() {
    let min = GateNode::new("a").with_coordinate("FL1-A", 10.0);
    let max = GateNode::new("b").with_coordinate("FL1-A", 90.0);
    let gate = Gate::new(
        "id",
        "r",
        GateGeometry::Range { min, max },
        "FL1-A",
        "CD45",
        GateCoordinateSpace::Raw,
    );
    assert!(gate.matches_plot_parameters("FL1-A", "PE-A"));
    assert!(!gate.matches_plot_parameters("FSC-A", "SSC-A"));
}
