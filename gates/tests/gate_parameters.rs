//! `GateParameters`, plot matching, and JSON compatibility.

use flow_gates::types::{
    gate_parameters_from_geometry_and_axes, Gate, GateCoordinateSpace, GateGeometry, GateNode,
    GateParameters,
};
use std::sync::Arc;

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
fn two_channel_matches_swap() {
    let p = GateParameters::TwoChannels {
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

/// Pins the exact on-the-wire tags. `GateParameters` derives its serde impls, so the
/// tag strings come from the variant names via `rename_all = "snake_case"` — nothing
/// spells them out. This test is the only thing that would catch a variant rename
/// silently changing the persisted workspace format.
#[test]
fn wire_tags_are_stable() {
    let cases = [
        (
            GateParameters::TwoChannels {
                x: Arc::from("FSC-A"),
                y: Arc::from("SSC-A"),
            },
            r#"{"type":"two_channels","x":"FSC-A","y":"SSC-A"}"#,
        ),
        (
            GateParameters::OneChannel {
                channel: Arc::from("FL1-A"),
            },
            r#"{"type":"one_channel","channel":"FL1-A"}"#,
        ),
        (GateParameters::NoChannels, r#"{"type":"no_channels"}"#),
    ];
    for (params, expected_json) in cases {
        let json = serde_json::to_string(&params).expect("serialize");
        assert_eq!(json, expected_json, "wire format changed for {params:?}");
        let back: GateParameters = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, params, "roundtrip lost data for {params:?}");
    }
}
