//! Integration tests for automated gating

use flow_gates::automated::{
    create_preprocessing_gates, create_preprocessing_gates_interactive,
    DoubletGateConfig, DoubletMethod, PreprocessingConfig, ScatterGateConfig,
    ScatterGateMethod, UserReview,
};
use flow_gates::automated::scatter::create_scatter_gate;
use flow_gates::automated::doublets::detect_doublets;
use flow_gates::filter_events_by_gate;
use flow_fcs::Fcs;
use std::path::Path;

mod test_helpers;
use test_helpers::{create_synthetic_fcs, TestScenario};

/// Helper function to create a simple test FCS file
fn create_test_fcs() -> Result<Fcs, Box<dyn std::error::Error>> {
    create_synthetic_fcs(5000, TestScenario::SinglePopulation)
}

#[test]
fn test_scatter_gating_ellipse_fit() {
    let fcs = create_test_fcs().unwrap();
    
    let config = ScatterGateConfig {
        fsc_channel: "FSC-A".to_string(),
        ssc_channel: "SSC-A".to_string(),
        method: ScatterGateMethod::EllipseFit,
        min_events: 100,
        density_threshold: None,
        cluster_eps: None,
        cluster_min_samples: None,
    };
    
    let result = create_scatter_gate(&fcs, &config).unwrap();
    
    assert!(result.gate.is_some());
    assert_eq!(result.method_used, "EllipseFit");
    assert!(!result.population_mask.is_empty());
}

#[test]
fn test_scatter_gating_density_contour() {
    let fcs = create_test_fcs().unwrap();

    let config = ScatterGateConfig {
        fsc_channel: "FSC-A".to_string(),
        ssc_channel: "SSC-A".to_string(),
        method: ScatterGateMethod::DensityContour { threshold: 0.1 },
        min_events: 100,
        density_threshold: Some(0.1),
        cluster_eps: None,
        cluster_min_samples: None,
    };

    let result = create_scatter_gate(&fcs, &config).unwrap();

    assert!(result.gate.is_some());
    // DensityContour when marching squares yields a path with >= 3 points; else EllipseFit fallback
    assert!(
        result.method_used == "DensityContour" || result.method_used == "EllipseFit",
        "method_used was {}",
        result.method_used
    );
    let n_inside = result.population_mask.iter().filter(|&&b| b).count();
    assert!(n_inside > 0, "scatter gate must pass at least one event");
    // Single-population synthetic data: expect a substantial fraction to pass
    let n_total = result.population_mask.len();
    assert!(
        n_inside >= n_total / 2,
        "single population should have at least half events inside gate (got {} / {})",
        n_inside,
        n_total
    );
}

#[test]
fn test_doublet_detection_ratio_mad() {
    let fcs = create_test_fcs().unwrap();
    
    let config = DoubletGateConfig {
        channels: vec![("FSC-A".to_string(), "FSC-H".to_string())],
        method: DoubletMethod::RatioMAD { nmad: 4.0 },
        nmad: Some(4.0),
        density_threshold: None,
        cluster_eps: None,
        cluster_min_samples: None,
    };
    
    let result = detect_doublets(&fcs, &config).unwrap();
    
    assert!(!result.singlet_mask.is_empty());
    assert_eq!(result.statistics.method_used, "RatioMAD(nmad=4)");
}

#[test]
fn test_doublet_detection_density_based() {
    let fcs = create_test_fcs().unwrap();
    
    let config = DoubletGateConfig {
        channels: vec![("FSC-A".to_string(), "FSC-H".to_string())],
        method: DoubletMethod::DensityBased { threshold: 0.1 },
        nmad: None,
        density_threshold: Some(0.1),
        cluster_eps: None,
        cluster_min_samples: None,
    };
    
    let result = detect_doublets(&fcs, &config).unwrap();
    
    assert!(!result.singlet_mask.is_empty());
    assert!(result.statistics.method_used.starts_with("DensityBased"));
}

#[test]
fn test_preprocessing_pipeline() {
    let fcs = create_test_fcs().unwrap();
    
    let config = PreprocessingConfig {
        scatter_config: ScatterGateConfig {
            fsc_channel: "FSC-A".to_string(),
            ssc_channel: "SSC-A".to_string(),
            method: ScatterGateMethod::EllipseFit,
            min_events: 100,
            density_threshold: None,
            cluster_eps: None,
            cluster_min_samples: None,
        },
        doublet_config: DoubletGateConfig {
            channels: vec![("FSC-A".to_string(), "FSC-H".to_string())],
            method: DoubletMethod::RatioMAD { nmad: 4.0 },
            nmad: Some(4.0),
            density_threshold: None,
            cluster_eps: None,
            cluster_min_samples: None,
        },
    };
    
    let result = create_preprocessing_gates(&fcs, config).unwrap();
    
    assert!(result.scatter_gate.is_some() || result.doublet_gate.is_some());
}

#[test]
fn test_interactive_pipeline() {
    let fcs = create_test_fcs().unwrap();
    
    let config = PreprocessingConfig {
        scatter_config: ScatterGateConfig {
            fsc_channel: "FSC-A".to_string(),
            ssc_channel: "SSC-A".to_string(),
            method: ScatterGateMethod::EllipseFit,
            min_events: 100,
            density_threshold: None,
            cluster_eps: None,
            cluster_min_samples: None,
        },
        doublet_config: DoubletGateConfig {
            channels: vec![("FSC-A".to_string(), "FSC-H".to_string())],
            method: DoubletMethod::RatioMAD { nmad: 4.0 },
            nmad: Some(4.0),
            density_threshold: None,
            cluster_eps: None,
            cluster_min_samples: None,
        },
    };
    
    // Test interactive pipeline with accept callback
    let result = create_preprocessing_gates_interactive(
        &fcs,
        config,
        |_breakpoint| UserReview::Accept,
    )
    .unwrap();

    assert!(result.scatter_gate.is_some() || result.doublet_gate.is_some());
}

/// Integration test with the FCS file that previously yielded "Scatter gate: 0 events passed".
/// Run with: `cargo test -p flow-gates --test automated_gating -- --ignored`
const BEADS_CONTROL_FCS_PATH: &str = "/Volumes/Shared Data/Research/Jamie Moynihan/Flow Data/Cytek Aurora Experiments/2026-03-05 phenotyping human HCC TSC #2-18/Raw/Plate_001/Reference Group/Reference Group_A9 HLA-DR RB545 (Beads)_2026_03_05_11_57_14.fcs";

#[test]
#[ignore]
fn test_scatter_gate_on_beads_control_fcs() {
    if !Path::new(BEADS_CONTROL_FCS_PATH).exists() {
        eprintln!("Skipping: file not found: {}", BEADS_CONTROL_FCS_PATH);
        return;
    }
    let fcs = Fcs::open(BEADS_CONTROL_FCS_PATH).expect("open FCS");
    let config = PreprocessingConfig {
        scatter_config: ScatterGateConfig {
            fsc_channel: "FSC-A".to_string(),
            ssc_channel: "SSC-A".to_string(),
            method: ScatterGateMethod::DensityContour { threshold: 0.5 },
            min_events: 100,
            density_threshold: Some(0.5),
            cluster_eps: None,
            cluster_min_samples: None,
        },
        doublet_config: DoubletGateConfig {
            channels: vec![
                ("FSC-A".to_string(), "FSC-H".to_string()),
                ("FSC-W".to_string(), "FSC-H".to_string()),
            ],
            method: DoubletMethod::RatioMAD { nmad: 4.0 },
            nmad: Some(4.0),
            density_threshold: None,
            cluster_eps: None,
            cluster_min_samples: None,
        },
    };
    let gates = create_preprocessing_gates(&fcs, config).expect("create_preprocessing_gates");
    let scatter_gate = match &gates.scatter_gate {
        Some(g) => g,
        None => {
            panic!("no scatter gate created");
        }
    };
    let indices = filter_events_by_gate(&fcs, scatter_gate, None).expect("filter_events_by_gate");
    assert!(
        !indices.is_empty(),
        "scatter gate must pass at least one event (file: {})",
        BEADS_CONTROL_FCS_PATH
    );
}
