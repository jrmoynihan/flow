//! Tests for Mask gate geometry, MaskSource, MaskResolver, and hierarchy filtering.
//!
//! Covers the scenarios from the QC-as-gate design:
//! - Mask gates serialize/deserialize correctly (with optional file_guid)
//! - NoChannel parameters match all plot axes
//! - Mask gates in hierarchy chains are resolved by the closure (not rejected)
//! - Missing masks return empty set (file excluded)
//! - system_managed and overrides fields persist through serialization

use flow_gates::{
    filtering::{MaskResolver, filter_events_by_hierarchy_steps},
    types::{
        Gate, GateCoordinateSpace, GateGeometry, GateMode, GateNode, GateParameters, MaskSource,
    },
};
use std::collections::BTreeMap;
use std::sync::Arc;

// ─── Test MaskResolver ─────────────────────────────────────────────────────

/// In-memory mask resolver for tests. Maps file_guid → Vec<bool> mask.
struct TestMaskResolver {
    masks: std::collections::HashMap<String, Vec<bool>>,
}

impl TestMaskResolver {
    fn new() -> Self {
        Self {
            masks: std::collections::HashMap::new(),
        }
    }

    fn add_mask(&mut self, file_guid: &str, mask: Vec<bool>) {
        self.masks.insert(file_guid.to_string(), mask);
    }
}

impl MaskResolver for TestMaskResolver {
    fn resolve_mask(
        &self,
        source: &MaskSource,
        n_events: usize,
    ) -> flow_gates::error::Result<Vec<usize>> {
        match source {
            MaskSource::Qc { file_guid, invert } => {
                let guid = file_guid.as_ref().ok_or_else(|| {
                    flow_gates::GateError::filtering_error("No file_guid in test")
                })?;
                let mask = match self.masks.get(guid.as_ref()) {
                    Some(m) => m.clone(),
                    None => return Ok(Vec::new()), // Missing mask = exclude all
                };
                assert_eq!(mask.len(), n_events);
                let indices: Vec<usize> = mask
                    .into_iter()
                    .enumerate()
                    .filter(|(_, good)| *good != *invert)
                    .map(|(i, _)| i)
                    .collect();
                Ok(indices)
            }
        }
    }
}

// ─── Serialization Tests ───────────────────────────────────────────────────

#[test]
fn mask_source_qc_with_file_guid_roundtrip() {
    let source = MaskSource::Qc {
        file_guid: Some(Arc::from("abc-123")),
        invert: false,
    };
    let json = serde_json::to_string(&source).expect("serialize");
    let restored: MaskSource = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(source, restored);
    assert!(json.contains("\"abc-123\""));
}

#[test]
fn mask_source_qc_without_file_guid_roundtrip() {
    let source = MaskSource::Qc {
        file_guid: None,
        invert: true,
    };
    let json = serde_json::to_string(&source).expect("serialize");
    let restored: MaskSource = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(source, restored);
    assert!(!json.contains("file_guid"));
}

#[test]
fn mask_gate_geometry_roundtrip() {
    let geom = GateGeometry::Mask {
        source: MaskSource::Qc {
            file_guid: None,
            invert: false,
        },
    };
    let json = serde_json::to_string(&geom).expect("serialize");
    let restored: GateGeometry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(geom, restored);
    assert!(json.contains("\"Mask\""));
}

#[test]
fn gate_with_mask_geometry_roundtrip() {
    let gate = Gate {
        id: Arc::from("qc-good"),
        name: "Good Events".to_string(),
        geometry: GateGeometry::Mask {
            source: MaskSource::Qc {
                file_guid: None,
                invert: false,
            },
        },
        mode: GateMode::Global,
        parameters: GateParameters::NoChannel,
        coordinate_space: GateCoordinateSpace::Raw,
        label_position: None,
        derived_from: None,
        parent_id: Some(Arc::from("qc-root")),
        overrides: BTreeMap::new(),
        system_managed: true,
    };
    let json = serde_json::to_string(&gate).expect("serialize");
    let restored: Gate = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(gate, restored);
}

#[test]
fn system_managed_false_not_serialized() {
    let gate = Gate::new(
        "test",
        "Test",
        GateGeometry::Polygon {
            nodes: vec![
                GateNode::new("n1")
                    .with_coordinate("X", 0.0)
                    .with_coordinate("Y", 0.0),
            ],
            closed: true,
        },
        "X",
        "Y",
        GateCoordinateSpace::Raw,
    );
    let json = serde_json::to_string(&gate).expect("serialize");
    assert!(!json.contains("system_managed"));
    assert!(!json.contains("overrides"));
}

#[test]
fn system_managed_true_roundtrips() {
    let mut gate = Gate::new(
        "sys",
        "System Gate",
        GateGeometry::Mask {
            source: MaskSource::Qc {
                file_guid: None,
                invert: false,
            },
        },
        "",
        "",
        GateCoordinateSpace::Raw,
    );
    gate.system_managed = true;
    let json = serde_json::to_string(&gate).expect("serialize");
    assert!(json.contains("system_managed"));
    let restored: Gate = serde_json::from_str(&json).expect("deserialize");
    assert!(restored.system_managed);
}

// ─── GateParameters::NoChannel Tests ───────────────────────────────────────

#[test]
fn no_channel_matches_all_plot_parameters() {
    let p = GateParameters::NoChannel;
    assert!(p.matches_plot_parameters("FSC-A", "SSC-A"));
    assert!(p.matches_plot_parameters("CD4", "CD8"));
    assert!(p.matches_plot_parameters("anything", "at-all"));
}

#[test]
fn no_channel_serialization_roundtrip() {
    let p = GateParameters::NoChannel;
    let json = serde_json::to_string(&p).expect("serialize");
    let restored: GateParameters = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(p, restored);
    assert!(json.contains("no_channel"));
}

// ─── Hierarchy Filtering with Mask Gates ───────────────────────────────────

fn make_mask_gate(id: &str, invert: bool, parent_id: Option<&str>) -> Gate {
    Gate {
        id: Arc::from(id),
        name: id.to_string(),
        geometry: GateGeometry::Mask {
            source: MaskSource::Qc {
                file_guid: Some(Arc::from("test-file")),
                invert,
            },
        },
        mode: GateMode::Global,
        parameters: GateParameters::NoChannel,
        coordinate_space: GateCoordinateSpace::Raw,
        label_position: None,
        derived_from: None,
        parent_id: parent_id.map(|s| Arc::from(s)),
        overrides: BTreeMap::new(),
        system_managed: true,
    }
}

#[test]
fn mask_gate_in_hierarchy_steps_resolves_via_closure() {
    let good_gate = make_mask_gate("qc-good", false, None);
    let total_events = 10;

    // Mask: events 0,1,2,3,4 are good; 5,6,7,8,9 are bad
    let mask = vec![true, true, true, true, true, false, false, false, false, false];

    let steps: Vec<(&Gate, Option<&str>)> = vec![(&good_gate, None)];

    let mut resolver = TestMaskResolver::new();
    resolver.add_mask("test-file", mask);

    let result = filter_events_by_hierarchy_steps(
        total_events,
        &steps,
        |gate, _corner| {
            if let GateGeometry::Mask { ref source } = gate.geometry {
                return resolver.resolve_mask(source, total_events);
            }
            unreachable!("only mask gates in this test");
        },
        None,
        None,
    )
    .expect("filtering should succeed");

    assert_eq!(result, vec![0, 1, 2, 3, 4]);
}

#[test]
fn bad_events_gate_returns_inverted_mask() {
    let bad_gate = make_mask_gate("qc-bad", true, None);
    let total_events = 10;

    let mask = vec![true, true, true, true, true, false, false, false, false, false];

    let steps: Vec<(&Gate, Option<&str>)> = vec![(&bad_gate, None)];

    let mut resolver = TestMaskResolver::new();
    resolver.add_mask("test-file", mask);

    let result = filter_events_by_hierarchy_steps(
        total_events,
        &steps,
        |gate, _corner| {
            if let GateGeometry::Mask { ref source } = gate.geometry {
                return resolver.resolve_mask(source, total_events);
            }
            unreachable!();
        },
        None,
        None,
    )
    .expect("filtering should succeed");

    assert_eq!(result, vec![5, 6, 7, 8, 9]);
}

#[test]
fn mask_gate_chain_intersects_with_geometric_gate() {
    let qc_gate = make_mask_gate("qc-good", false, None);

    // Geometric gate that passes events 2,3,4,5,6
    let min = GateNode::new("min")
        .with_coordinate("X", 2.0)
        .with_coordinate("Y", 0.0);
    let max = GateNode::new("max")
        .with_coordinate("X", 6.0)
        .with_coordinate("Y", 10.0);
    let rect_gate = Gate {
        id: Arc::from("lymphocytes"),
        name: "Lymphocytes".to_string(),
        geometry: GateGeometry::Rectangle { min, max },
        mode: GateMode::Global,
        parameters: GateParameters::TwoChannel {
            x: Arc::from("X"),
            y: Arc::from("Y"),
        },
        coordinate_space: GateCoordinateSpace::Raw,
        label_position: None,
        derived_from: None,
        parent_id: Some(Arc::from("qc-good")),
        overrides: BTreeMap::new(),
        system_managed: false,
    };

    let total_events = 10;
    // QC mask: events 0..5 are good
    let mask = vec![true, true, true, true, true, false, false, false, false, false];

    let mut resolver = TestMaskResolver::new();
    resolver.add_mask("test-file", mask);

    // Event data: X values are 0.0, 1.0, ..., 9.0; Y values are all 5.0
    let x_values: Vec<f32> = (0..10).map(|i| i as f32).collect();
    let y_values: Vec<f32> = vec![5.0; 10];

    let steps: Vec<(&Gate, Option<&str>)> = vec![(&qc_gate, None), (&rect_gate, None)];

    let result = filter_events_by_hierarchy_steps(
        total_events,
        &steps,
        |gate, _corner| {
            if let GateGeometry::Mask { ref source } = gate.geometry {
                return resolver.resolve_mask(source, total_events);
            }
            // Geometric filtering for the rectangle
            use flow_gates::filtering::{EventData, filter_events_by_gate};
            let data = EventData {
                space: GateCoordinateSpace::Raw,
                x_param: "X",
                x: &x_values,
                y_param: "Y",
                y: &y_values,
            };
            filter_events_by_gate(data, gate, None)
        },
        None,
        None,
    )
    .expect("filtering should succeed");

    // QC passes 0,1,2,3,4; Rectangle passes 2,3,4,5,6
    // Intersection: 2,3,4
    let mut sorted = result;
    sorted.sort();
    assert_eq!(sorted, vec![2, 3, 4]);
}

#[test]
fn missing_mask_returns_empty_set() {
    let qc_gate = Gate {
        id: Arc::from("qc-good"),
        name: "Good Events".to_string(),
        geometry: GateGeometry::Mask {
            source: MaskSource::Qc {
                file_guid: Some(Arc::from("nonexistent-file")),
                invert: false,
            },
        },
        mode: GateMode::Global,
        parameters: GateParameters::NoChannel,
        coordinate_space: GateCoordinateSpace::Raw,
        label_position: None,
        derived_from: None,
        parent_id: None,
        overrides: BTreeMap::new(),
        system_managed: true,
    };

    let total_events = 100;
    let resolver = TestMaskResolver::new(); // No masks registered

    let steps: Vec<(&Gate, Option<&str>)> = vec![(&qc_gate, None)];

    let result = filter_events_by_hierarchy_steps(
        total_events,
        &steps,
        |gate, _corner| {
            if let GateGeometry::Mask { ref source } = gate.geometry {
                return resolver.resolve_mask(source, total_events);
            }
            unreachable!();
        },
        None,
        None,
    )
    .expect("filtering should succeed (empty = excluded)");

    assert!(result.is_empty(), "missing mask should exclude all events");
}

// ─── effective_geometry Tests ──────────────────────────────────────────────

#[test]
fn effective_geometry_returns_base_when_no_overrides() {
    let gate = Gate::new(
        "test",
        "Test",
        GateGeometry::Polygon {
            nodes: vec![GateNode::new("n1").with_coordinate("X", 1.0).with_coordinate("Y", 2.0)],
            closed: true,
        },
        "X",
        "Y",
        GateCoordinateSpace::Raw,
    );
    let result = gate.effective_geometry("any-file", &[]);
    assert_eq!(result, &gate.geometry);
}

#[test]
fn effective_geometry_file_override_wins() {
    let mut gate = Gate::new(
        "test",
        "Test",
        GateGeometry::Polygon {
            nodes: vec![GateNode::new("n1").with_coordinate("X", 1.0).with_coordinate("Y", 2.0)],
            closed: true,
        },
        "X",
        "Y",
        GateCoordinateSpace::Raw,
    );
    let override_geom = GateGeometry::Polygon {
        nodes: vec![GateNode::new("n2").with_coordinate("X", 99.0).with_coordinate("Y", 99.0)],
        closed: true,
    };
    gate.overrides.insert(Arc::from("file-1"), override_geom.clone());

    assert_eq!(gate.effective_geometry("file-1", &[]), &override_geom);
    assert_eq!(gate.effective_geometry("file-2", &[]), &gate.geometry);
}

#[test]
fn effective_geometry_file_beats_group() {
    let mut gate = Gate::new(
        "test",
        "Test",
        GateGeometry::Polygon {
            nodes: vec![GateNode::new("n1").with_coordinate("X", 1.0).with_coordinate("Y", 2.0)],
            closed: true,
        },
        "X",
        "Y",
        GateCoordinateSpace::Raw,
    );
    let group_geom = GateGeometry::Polygon {
        nodes: vec![GateNode::new("g").with_coordinate("X", 50.0).with_coordinate("Y", 50.0)],
        closed: true,
    };
    let file_geom = GateGeometry::Polygon {
        nodes: vec![GateNode::new("f").with_coordinate("X", 99.0).with_coordinate("Y", 99.0)],
        closed: true,
    };
    gate.overrides.insert(Arc::from("group-a"), group_geom.clone());
    gate.overrides.insert(Arc::from("file-1"), file_geom.clone());

    // file-1 has direct override → wins over group
    assert_eq!(gate.effective_geometry("file-1", &["group-a"]), &file_geom);
    // file-2 has no direct override but belongs to group-a → group wins
    assert_eq!(gate.effective_geometry("file-2", &["group-a"]), &group_geom);
    // file-3 has neither → base geometry
    assert_eq!(gate.effective_geometry("file-3", &[]), &gate.geometry);
}

// ─── Mixed QC Status Tests ─────────────────────────────────────────────────

/// Simulates the real scenario: 3 files concatenated, only 2 have QC masks.
/// The QC root node should exclude the un-QC'd file (return empty for it),
/// and Good/Bad Events should only reflect the QC'd files' events.
#[test]
fn mixed_qc_status_excludes_unqcd_file_from_stats() {
    // File A: 100 events, QC'd (80 good, 20 bad)
    // File B: 150 events, QC'd (120 good, 30 bad)
    // File C: 200 events, NOT QC'd (no mask)
    let mut resolver = TestMaskResolver::new();
    let mask_a: Vec<bool> = (0..100).map(|i| i < 80).collect();
    let mask_b: Vec<bool> = (0..150).map(|i| i < 120).collect();
    resolver.add_mask("file-a", mask_a);
    resolver.add_mask("file-b", mask_b);
    // file-c deliberately has no mask

    // QC root gate (pass-through for files with masks, exclude others)
    let qc_root = Gate {
        id: Arc::from("qc-root"),
        name: "QC".to_string(),
        geometry: GateGeometry::Mask {
            source: MaskSource::Qc { file_guid: None, invert: false },
        },
        mode: GateMode::Global,
        parameters: GateParameters::NoChannel,
        coordinate_space: GateCoordinateSpace::Raw,
        label_position: None,
        derived_from: None,
        parent_id: None,
        overrides: BTreeMap::new(),
        system_managed: true,
    };

    let good_gate = Gate {
        id: Arc::from("qc-good"),
        name: "Good Events".to_string(),
        geometry: GateGeometry::Mask {
            source: MaskSource::Qc { file_guid: None, invert: false },
        },
        mode: GateMode::Global,
        parameters: GateParameters::NoChannel,
        coordinate_space: GateCoordinateSpace::Raw,
        label_position: None,
        derived_from: None,
        parent_id: Some(Arc::from("qc-root")),
        overrides: BTreeMap::new(),
        system_managed: true,
    };

    let bad_gate = Gate {
        id: Arc::from("qc-bad"),
        name: "Bad Events".to_string(),
        geometry: GateGeometry::Mask {
            source: MaskSource::Qc { file_guid: None, invert: true },
        },
        mode: GateMode::Global,
        parameters: GateParameters::NoChannel,
        coordinate_space: GateCoordinateSpace::Raw,
        label_position: None,
        derived_from: None,
        parent_id: Some(Arc::from("qc-root")),
        overrides: BTreeMap::new(),
        system_managed: true,
    };

    // Helper: simulate per-file filtering through the chain, mimicking the
    // app's closure behavior (qc-root = pass-all if mask exists, else empty).
    let filter_file = |file_guid: &str, n_events: usize, chain: &[&Gate]| -> Vec<usize> {
        let steps: Vec<(&Gate, Option<&str>)> = chain.iter().map(|g| (*g, None)).collect();
        filter_events_by_hierarchy_steps(
            n_events,
            &steps,
            |gate, _corner| {
                if gate.id.as_ref() == "qc-root" {
                    // Pass-through if mask exists, else exclude
                    if resolver.masks.contains_key(file_guid) {
                        return Ok((0..n_events).collect());
                    } else {
                        return Ok(Vec::new());
                    }
                }
                if let GateGeometry::Mask { ref source } = gate.geometry {
                    let effective = MaskSource::Qc {
                        file_guid: Some(Arc::from(file_guid)),
                        invert: match source {
                            MaskSource::Qc { invert, .. } => *invert,
                        },
                    };
                    return resolver.resolve_mask(&effective, n_events);
                }
                unreachable!();
            },
            None,
            None,
        )
        .expect("filtering should succeed")
    };

    // ── Test qc-root behavior ──
    let root_chain = [&qc_root];
    // File A (QC'd): all 100 events pass through root
    assert_eq!(filter_file("file-a", 100, &root_chain).len(), 100);
    // File B (QC'd): all 150 events pass through root
    assert_eq!(filter_file("file-b", 150, &root_chain).len(), 150);
    // File C (NOT QC'd): 0 events pass — excluded
    assert_eq!(filter_file("file-c", 200, &root_chain).len(), 0);

    // ── Test Good Events ──
    let good_chain = [&qc_root, &good_gate];
    assert_eq!(filter_file("file-a", 100, &good_chain).len(), 80);
    assert_eq!(filter_file("file-b", 150, &good_chain).len(), 120);
    assert_eq!(filter_file("file-c", 200, &good_chain).len(), 0);

    // ── Test Bad Events ──
    let bad_chain = [&qc_root, &bad_gate];
    assert_eq!(filter_file("file-a", 100, &bad_chain).len(), 20);
    assert_eq!(filter_file("file-b", 150, &bad_chain).len(), 30);
    assert_eq!(filter_file("file-c", 200, &bad_chain).len(), 0);

    // ── Aggregated stats (sum across files) ──
    let total_all = 100 + 150 + 200; // 450
    let total_qc_root = 100 + 150 + 0; // 250 (file-c excluded)
    let total_good = 80 + 120 + 0; // 200
    let total_bad = 20 + 30 + 0; // 50

    let agg_root: usize = ["file-a", "file-b", "file-c"]
        .iter()
        .map(|f| filter_file(f, match *f { "file-a" => 100, "file-b" => 150, _ => 200 }, &root_chain).len())
        .sum();
    let agg_good: usize = ["file-a", "file-b", "file-c"]
        .iter()
        .map(|f| filter_file(f, match *f { "file-a" => 100, "file-b" => 150, _ => 200 }, &good_chain).len())
        .sum();
    let agg_bad: usize = ["file-a", "file-b", "file-c"]
        .iter()
        .map(|f| filter_file(f, match *f { "file-a" => 100, "file-b" => 150, _ => 200 }, &bad_chain).len())
        .sum();

    assert_eq!(agg_root, total_qc_root);
    assert_eq!(agg_good, total_good);
    assert_eq!(agg_bad, total_bad);

    // QC root shows < 100% of total (250/450 = 55.6%), indicating mixed status
    let qc_coverage = agg_root as f64 / total_all as f64;
    assert!(qc_coverage < 1.0, "QC coverage should be < 100% with mixed status");
    assert!((qc_coverage - 250.0 / 450.0).abs() < 0.001);
}
