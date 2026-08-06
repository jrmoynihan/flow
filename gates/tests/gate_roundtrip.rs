//! Tests that use PartialEq for Gate round-trip and equality checks.

use flow_gates::{Gate, GateCoordinateSpace, GateGeometry, GateHierarchy, GateNode};
use std::sync::Arc;

fn make_test_gate() -> Gate {
    let node1 = GateNode::new("n1")
        .with_coordinate("FSC-A", 100.0)
        .with_coordinate("SSC-A", 200.0);
    let node2 = GateNode::new("n2")
        .with_coordinate("FSC-A", 300.0)
        .with_coordinate("SSC-A", 400.0);
    Gate::new(
        "roundtrip-gate",
        "Roundtrip Gate",
        GateGeometry::Polygon {
            nodes: vec![node1, node2],
            closed: true,
        },
        "FSC-A",
        "SSC-A",
        GateCoordinateSpace::Raw,
    )
}

#[test]
fn gate_json_roundtrip() {
    let gate = make_test_gate();
    let json = serde_json::to_string(&gate).expect("serialize");
    let restored: Gate = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(gate, restored);
}

/// A parent_id set on a Gate must survive JSON serialization, and rebuilding
/// the hierarchy index from the round-tripped gates must reconstruct the
/// parent/child relationship. This is the persistence guarantee for gate
/// hierarchy (parent_id is the source of truth; GateHierarchy is derived).
#[test]
fn parent_id_roundtrip_preserves_hierarchy() {
    let mut root = make_test_gate();
    root.id = Arc::from("root-gate");
    root.parent_id = None;

    let mut child = make_test_gate();
    child.id = Arc::from("child-gate");
    child.parent_id = Some(Arc::from("root-gate"));

    // Round-trip both through JSON (as gates.json / workspace XML would).
    let gates: Vec<Gate> = [&root, &child]
        .iter()
        .map(|g| {
            let json = serde_json::to_string(g).expect("serialize");
            serde_json::from_str::<Gate>(&json).expect("deserialize")
        })
        .collect();

    assert_eq!(gates[1].parent_id.as_deref(), Some("root-gate"));

    // Rebuilding the derived index from the gates reconstructs the edges.
    let hierarchy = GateHierarchy::from_gates(&gates).expect("rebuild hierarchy");
    assert_eq!(
        hierarchy.get_parent("child-gate").map(|p| p.as_ref()),
        Some("root-gate")
    );
    assert!(
        hierarchy
            .get_children("root-gate")
            .iter()
            .any(|c| c.as_ref() == "child-gate")
    );
    assert!(hierarchy.get_roots().iter().any(|r| r.as_ref() == "root-gate"));
}

/// Gates with no parent_id (old data / serde default) load as roots — the
/// migration guarantee for workspaces saved before parent_id existed.
#[test]
fn gates_without_parent_id_become_roots() {
    let mut a = make_test_gate();
    a.id = Arc::from("a");
    let mut b = make_test_gate();
    b.id = Arc::from("b");
    // both parent_id default None
    let hierarchy = GateHierarchy::from_gates(&[a, b]).expect("rebuild");
    assert_eq!(hierarchy.get_roots().len(), 2);
}

/// from_gates must reject a parent_id cycle rather than build a corrupt index.
#[test]
fn from_gates_rejects_cycle() {
    let mut a = make_test_gate();
    a.id = Arc::from("a");
    a.parent_id = Some(Arc::from("b"));
    let mut b = make_test_gate();
    b.id = Arc::from("b");
    b.parent_id = Some(Arc::from("a"));
    assert!(GateHierarchy::from_gates(&[a, b]).is_err());
}

/// Gates stored in workspaces use lowercase coordinate_space values ("raw",
/// "compensated") matching the serde rename_all = "snake_case" on GateCoordinateSpace.
/// This test ensures the exact JSON shape written by the app can be read back.
#[test]
fn gate_workspace_json_deser() {
    // Exact JSON shape stored in workspace XML (from qc_test.xml)
    let json = r#"{"id":"qc-root","name":"QC","geometry":{"type":"Mask","source":{"type":"qc","invert":false}},"mode":{"name":"Global"},"parameters":{"type":"no_channels"},"coordinate_space":"raw","label_position":null}"#;
    let gate: Gate = serde_json::from_str(json).expect("gate with lowercase coordinate_space should deserialize");
    assert_eq!(gate.coordinate_space, GateCoordinateSpace::Raw);

    // Verify round-trip: serialize back and check coordinate_space is still lowercase
    let re_json = serde_json::to_string(&gate).expect("serialize");
    assert!(re_json.contains("\"coordinate_space\":\"raw\""), "coordinate_space should serialize as lowercase 'raw', got: {}", re_json);
}

/// Reproduce the exact gate JSON from qc_test.xml to catch any deserialization failure.
#[test]
fn qc_workspace_gates_deser() {
    let gates = vec![
        r#"{"id":"qc-root","name":"QC","geometry":{"type":"Mask","source":{"type":"qc","invert":false}},"mode":{"name":"Global"},"parameters":{"type":"no_channels"},"coordinate_space":"raw","label_position":null}"#,
        r#"{"id":"qc-bad","name":"Bad Events","geometry":{"type":"Mask","source":{"type":"qc","invert":true}},"mode":{"name":"Global"},"parameters":{"type":"no_channels"},"coordinate_space":"raw","label_position":null,"parent_id":"qc-root"}"#,
        r#"{"id":"qc-good","name":"Good Events","geometry":{"type":"Mask","source":{"type":"qc","invert":false}},"mode":{"name":"Global"},"parameters":{"type":"no_channels"},"coordinate_space":"raw","label_position":null,"parent_id":"qc-root"}"#,
        r#"{"id":"31cc0b43-4011-4291-bad6-024e154ac0a7","name":"FSC-A vs SSC-A (copy)","geometry":{"type":"Ellipse","center":{"id":"af5f054c-aef7-445c-816e-58b8b2dee650","coordinates":{"SSC-A":1279333.0,"FSC-A":1911042.9}},"radius_x":1964217.4,"radius_y":1081011.6,"angle":0.6168946},"mode":{"name":"Global"},"parameters":{"type":"two_channels","x":"FSC-A","y":"SSC-A"},"coordinate_space":"raw","label_position":null,"parent_id":"qc-good"}"#,
    ];
    for (i, json) in gates.iter().enumerate() {
        let result = serde_json::from_str::<Gate>(json);
        assert!(result.is_ok(), "Gate {i} failed: {}", result.unwrap_err());
    }
}
