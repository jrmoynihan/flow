//! Tests that use PartialEq for Gate round-trip and equality checks.

use flow_gates::{Gate, GateGeometry, GateNode};

fn make_test_gate() -> Gate {
    let node1 = GateNode::new("n1").with_coordinate("FSC-A", 100.0).with_coordinate("SSC-A", 200.0);
    let node2 = GateNode::new("n2").with_coordinate("FSC-A", 300.0).with_coordinate("SSC-A", 400.0);
    Gate::new(
        "roundtrip-gate",
        "Roundtrip Gate",
        GateGeometry::Polygon {
            nodes: vec![node1, node2],
            closed: true,
        },
        "FSC-A",
        "SSC-A",
    )
}

#[test]
fn gate_json_roundtrip() {
    let gate = make_test_gate();
    let json = serde_json::to_string(&gate).expect("serialize");
    let restored: Gate = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(gate, restored);
}
