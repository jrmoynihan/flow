//! Integration tests for automated gating

use flow_fcs::Fcs;
use flow_gates::automated::doublets::detect_doublets;
use flow_gates::automated::scatter::create_scatter_gate;
use flow_gates::automated::{
    ConsensusFscConfig, DoubletGateConfig, DoubletMethod, PreprocessingConfig, ScatterGateConfig,
    ScatterGateMethod, ScatterQualityPolicy, UserReview, consensus_fsc_threshold,
    create_preprocessing_gates, create_preprocessing_gates_interactive,
};
use flow_gates::filter_events_by_gate;
use std::path::{Path, PathBuf};

mod test_helpers;
use test_helpers::{TestScenario, create_synthetic_fcs};

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
    let result =
        create_preprocessing_gates_interactive(&fcs, config, |_breakpoint| UserReview::Accept)
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
    let indices = flow_gates::filter_events_by_gate_with_resolver::<
        std::collections::HashMap<std::sync::Arc<str>, flow_gates::Gate>,
    >(&fcs, scatter_gate, None, None)
        .expect("filter_events_by_gate");
    assert!(
        !indices.is_empty(),
        "scatter gate must pass at least one event (file: {})",
        BEADS_CONTROL_FCS_PATH
    );
}

#[test]
fn scatter_retention_and_suspicious_policy() {
    let fcs = create_test_fcs().unwrap();
    let config = ScatterGateConfig {
        fsc_channel: "FSC-A".to_string(),
        ssc_channel: "SSC-A".to_string(),
        method: ScatterGateMethod::DensityContour { threshold: 0.5 },
        min_events: 100,
        density_threshold: Some(0.5),
        cluster_eps: None,
        cluster_min_samples: None,
    };
    let sg = create_scatter_gate(&fcs, &config).unwrap();
    let r = sg.retention_fraction();
    assert!((0.0..=1.0).contains(&r));
    assert!(!sg.is_suspicious(&ScatterQualityPolicy::default()));
    let tight = ScatterQualityPolicy {
        min_retention_fraction: r + 0.01,
        max_retention_fraction: 0.999,
    };
    assert!(sg.is_suspicious(&tight));
}

#[test]
fn consensus_fsc_debris_scenario() {
    let fcs = create_synthetic_fcs(6000, TestScenario::WithDebris).unwrap();
    let res = consensus_fsc_threshold(&fcs, &ConsensusFscConfig::default()).expect("consensus");
    assert!(res.threshold.is_finite() && res.threshold > 0.0);
    assert_eq!(res.keep_mask.len(), fcs.get_event_count_from_dataframe());
    let kept = res.keep_mask.iter().filter(|&&k| k).count();
    let n = fcs.get_event_count_from_dataframe();
    assert!(kept > 0 && kept <= n);
    assert!(kept < n || res.per_channel_thresholds.is_empty());
}

#[test]
fn doublet_multi_pair_and_singlet_mask() {
    let fcs = create_test_fcs().unwrap();
    let n = fcs.get_event_count_from_dataframe();
    let config = DoubletGateConfig {
        channels: vec![
            ("FSC-A".to_string(), "FSC-H".to_string()),
            ("SSC-A".to_string(), "SSC-H".to_string()),
        ],
        method: DoubletMethod::RatioMAD { nmad: 5.0 },
        nmad: Some(5.0),
        density_threshold: None,
        cluster_eps: None,
        cluster_min_samples: None,
    };
    let dr = detect_doublets(&fcs, &config).unwrap();
    assert_eq!(dr.singlet_mask.len(), n);
    let n_singlets = dr.singlet_mask.iter().filter(|&&k| k).count();
    assert!(n_singlets > n / 4);
    assert!(
        dr.statistics.method_used.starts_with("MultiPair"),
        "got {}",
        dr.statistics.method_used
    );
}

#[test]
fn doublet_ratio_inflection_or_fixed_runs() {
    let fcs = create_synthetic_fcs(8000, TestScenario::WithDoublets).unwrap();
    let config = DoubletGateConfig {
        channels: vec![("FSC-A".to_string(), "FSC-H".to_string())],
        method: DoubletMethod::RatioInflectionOrFixed {
            min_peaks: 2,
            min_ratio: 1.0,
            fixed_threshold: 1.2,
        },
        nmad: None,
        density_threshold: None,
        cluster_eps: None,
        cluster_min_samples: None,
    };
    let dr = detect_doublets(&fcs, &config).unwrap();
    assert_eq!(dr.singlet_mask.len(), fcs.get_event_count_from_dataframe());
    assert!(dr.statistics.method_used.contains("RatioInflectionOrFixed"));
}

/// When `FLOW_GATES_QC_TEST_PLOTS=1`, writes a short diagnostic file under the target temp dir (for manual inspection).
#[test]
fn qc_plot_smoke_env_gated() {
    if std::env::var("FLOW_GATES_QC_TEST_PLOTS").ok().as_deref() != Some("1") {
        return;
    }
    let base = std::env::var("CARGO_TARGET_TMPDIR")
        .or_else(|_| std::env::var("TMPDIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let dir = base.join("flow_gates_qc_plots");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let fcs = create_synthetic_fcs(4000, TestScenario::WithDebris).unwrap();
    let sg = create_scatter_gate(
        &fcs,
        &ScatterGateConfig {
            fsc_channel: "FSC-A".to_string(),
            ssc_channel: "SSC-A".to_string(),
            method: ScatterGateMethod::DensityContour { threshold: 0.4 },
            min_events: 100,
            density_threshold: Some(0.4),
            cluster_eps: None,
            cluster_min_samples: None,
        },
    )
    .expect("scatter");
    let cons = consensus_fsc_threshold(&fcs, &ConsensusFscConfig::default()).expect("consensus");
    let summary = format!(
        "scatter_method={} retention={:.4} consensus_threshold={:.2} n_events={}\n",
        sg.method_used,
        sg.retention_fraction(),
        cons.threshold,
        fcs.get_event_count_from_dataframe()
    );
    std::fs::write(dir.join("qc_plot_smoke.txt"), summary).expect("write");
}
