//! Synthetic acquisition with a late block of high fluorescence; PeacoQC should drop some events.

#![cfg(feature = "flow-fcs")]

use flow_fcs::file::AccessWrapper;
use flow_fcs::{Fcs, Header, Metadata, Parameter, TransformType, parameter::ParameterMap};
use peacoqc_rs::{PeacoQCConfig, QCMode, peacoqc};
use polars::prelude::*;
use std::sync::Arc;

fn synthetic_drift_fcs(n: usize) -> Fcs {
    let mut fsc_a: Vec<f32> = Vec::with_capacity(n);
    let mut fl1: Vec<f32> = Vec::with_capacity(n);
    let break_i = n * 3 / 4;
    for i in 0..n {
        fsc_a.push(45_000.0_f32 + (i % 50) as f32);
        if i < break_i {
            fl1.push(80.0_f32 + (i % 7) as f32);
        } else {
            fl1.push(40_000.0_f32 + (i % 20) as f32);
        }
    }
    let height = n;
    let df = DataFrame::new(
        height,
        vec![
            Column::new("FSC-A".into(), fsc_a),
            Column::new("FL1-A".into(), fl1),
        ],
    )
    .expect("dataframe");
    let mut params = ParameterMap::default();
    params.insert(
        "FSC-A".into(),
        Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
    );
    params.insert(
        "FL1-A".into(),
        Parameter::new(&2, "FL1-A", "FL1-A", &TransformType::Linear),
    );
    let tmp = std::env::temp_dir().join(format!("peacoqc_syn_{}.tmp", std::process::id()));
    let _ = std::fs::write(&tmp, b"x");
    Fcs {
        header: Header::new(),
        metadata: Metadata::new(),
        parameters: params,
        data_frame: Arc::new(df),
        file_access: AccessWrapper::new(tmp.to_str().unwrap_or(".")).expect("access"),
    }
}

#[test]
fn peacoqc_synthetic_drift_removes_events() {
    let fcs = synthetic_drift_fcs(12_000);
    let mut cfg = PeacoQCConfig::for_fcs(&fcs, QCMode::MAD);
    cfg.force_it = 1_000_000;
    let res = peacoqc(&fcs, &cfg).expect("peacoqc");
    let n = fcs.get_event_count_from_dataframe();
    assert_eq!(res.good_cells.len(), n);
    let kept = res.good_cells.iter().filter(|&&k| k).count();
    assert!(kept < n, "expected some removal, kept {kept} of {n}");
    assert!(
        res.percentage_removed > 0.1 && res.percentage_removed < 95.0,
        "percentage_removed={}",
        res.percentage_removed
    );
}
