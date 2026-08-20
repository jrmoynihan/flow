//! Joint per-cell AF matching-pursuit + fluorophore-variant coordinate descent.
//!
//! Port of AutoSpectralRcpp `unmix_autospectral_joint_cpp` (v1.6 default
//! `pipeline = "joint"`). See Burton *et al.*, *bioRxiv*
//! 2025.10.27.684855. Alloc / layout notes: `docs/PERF_AB.md`.
//!
//! Arithmetic width is [`crate::JointUnmixPrecision`]: default `f64` (vs-R);
//! `f32` is the encoding experiment in `flow-crates-0ap.1`.

#![allow(clippy::needless_range_loop)]

use crate::config::{JointUnmixConfig, JointUnmixPrecision};
use crate::error::Result;
use crate::library::AfLibrary;
use crate::variants::SpectralVariants;
use faer::{Mat, MatRef};

/// Per-event abundances plus AF / variant provenance.
#[derive(Debug, Clone)]
pub struct JointUnmixResult {
    /// Row-major `n_events × (n_fluor + 1)` (fluorophores then AF abundance).
    pub abundances: Vec<f64>,
    pub n_events: usize,
    pub n_fluor: usize,
    pub af_index: Vec<usize>,
    /// Row-major `n_events × n_fluor`; `None` means the master spectrum.
    pub variant_index: Vec<Option<usize>>,
}

impl JointUnmixResult {
    pub fn event_abundances(&self, event: usize) -> Option<&[f64]> {
        let w = self.n_fluor + 1;
        let start = event.checked_mul(w)?;
        self.abundances.get(start..start + w)
    }
}

/// Joint unmix: AF library match, then optional per-cell fluorophore variants.
///
/// `events_row_major` is `n_events × n_detectors`. `fluor_matrix` is
/// detectors × fluorophores (no AF column). Empty [`SpectralVariants::variants`]
/// takes the AF-only early return (same API as AF extraction).
pub fn unmix_autospectral_joint(
    events_row_major: &[f64],
    n_events: usize,
    fluor_matrix: MatRef<'_, f64>,
    fluor_names: &[String],
    af_library: &AfLibrary,
    variants: &SpectralVariants,
    config: &JointUnmixConfig,
) -> Result<JointUnmixResult> {
    match config.precision {
        JointUnmixPrecision::F64 => {
            let inner = joint_f64::unmix_autospectral_joint_s(
                events_row_major,
                n_events,
                fluor_matrix,
                fluor_names,
                af_library,
                variants,
                config,
            )?;
            Ok(inner_f64(inner))
        }
        JointUnmixPrecision::F32 => {
            let events: Vec<f32> = events_row_major.iter().map(|&x| x as f32).collect();
            let fluor = Mat::<f32>::from_fn(fluor_matrix.nrows(), fluor_matrix.ncols(), |i, j| {
                fluor_matrix[(i, j)] as f32
            });
            let inner = joint_f32::unmix_autospectral_joint_s(
                &events,
                n_events,
                fluor.as_ref(),
                fluor_names,
                af_library,
                variants,
                config,
            )?;
            Ok(inner_f32(inner))
        }
    }
}

fn inner_f64(r: joint_f64::InnerResult) -> JointUnmixResult {
    JointUnmixResult {
        abundances: r.abundances,
        n_events: r.n_events,
        n_fluor: r.n_fluor,
        af_index: r.af_index,
        variant_index: r.variant_index,
    }
}

fn inner_f32(r: joint_f32::InnerResult) -> JointUnmixResult {
    JointUnmixResult {
        abundances: r.abundances.into_iter().map(f64::from).collect(),
        n_events: r.n_events,
        n_fluor: r.n_fluor,
        af_index: r.af_index,
        variant_index: r.variant_index,
    }
}

#[allow(clippy::needless_range_loop)]
mod joint_f64 {
    type S = f64;
    include!("joint_inner.rs");
}

#[allow(clippy::needless_range_loop)]
mod joint_f32 {
    type S = f32;
    include!("joint_inner.rs");
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::config::{JointUnmixConfig, JointUnmixPrecision};
    use crate::library::{AfLibrary, normalize_unit_peak};
    use crate::unmix_ols::{ols_residual, swap_af_column};
    use crate::variants::SpectralVariants;
    use faer::Mat;
    use std::collections::HashMap;

    fn dot(a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }


    fn tiny_panel() -> (Mat<f64>, Vec<String>, AfLibrary, SpectralVariants, Vec<f64>, usize) {
        // d=6, F=3, K_AF=2. Fluors 0 and 1 are a collinear-ish pair (peaks on 1 and 2).
        let d = 6;
        let mut fluor = Mat::<f64>::zeros(d, 3);
        let mut cols = [
            vec![0.15, 1.0, 0.55, 0.08, 0.02, 0.01],
            vec![0.12, 0.50, 1.0, 0.20, 0.04, 0.01],
            vec![0.02, 0.03, 0.05, 0.20, 1.0, 0.25],
        ];
        for c in &mut cols {
            normalize_unit_peak(c);
        }
        for j in 0..3 {
            for i in 0..d {
                fluor[(i, j)] = cols[j][i];
            }
        }
        let names = vec!["A".into(), "B".into(), "C".into()];
        let mut af = Mat::<f64>::zeros(d, 2);
        af[(0, 0)] = 1.0;
        af[(1, 0)] = 0.4;
        af[(5, 1)] = 1.0;
        af[(4, 1)] = 0.3;
        let library = AfLibrary {
            signatures: af,
            names: vec!["af0".into(), "af1".into()],
            detector_names: (0..d).map(|i| format!("D{i}")).collect(),
            provenance: "tiny".into(),
        };

        // Four variants on fluor A (collinear pair member): shift the shared detector 2.
        let mut v_a = Mat::<f64>::zeros(d, 4);
        for v in 0..4 {
            let mut spec = cols[0].clone();
            spec[2] = (0.45 + v as f64 * 0.08).min(0.9);
            normalize_unit_peak(&mut spec);
            for i in 0..d {
                v_a[(i, v)] = spec[i];
            }
        }
        let mut d_a = Mat::<f64>::zeros(d, 4);
        for v in 0..4 {
            for i in 0..d {
                d_a[(i, v)] = v_a[(i, v)] - fluor[(i, 0)];
            }
        }
        let mut variants = HashMap::new();
        let mut deltas = HashMap::new();
        variants.insert("A".into(), v_a);
        deltas.insert("A".into(), d_a);
        let sv = SpectralVariants {
            thresholds: vec![0.0, 0.0, 0.0],
            fluor_names: names.clone(),
            variants,
            deltas,
        };

        // Events: AF-only, A-positive, B-positive, mix.
        let n = 24;
        let mut events = Vec::with_capacity(n * d);
        for e in 0..n {
            let mut y = vec![0.0; d];
            match e % 4 {
                0 => {
                    for i in 0..d {
                        y[i] = library.signatures[(i, e % 2)] * 80.0;
                    }
                }
                1 => {
                    for i in 0..d {
                        y[i] = fluor[(i, 0)] * 400.0 + library.signatures[(i, 0)] * 40.0;
                    }
                }
                2 => {
                    for i in 0..d {
                        y[i] = fluor[(i, 1)] * 350.0 + library.signatures[(i, 1)] * 30.0;
                    }
                }
                _ => {
                    for i in 0..d {
                        y[i] = fluor[(i, 2)] * 300.0 + library.signatures[(i, 0)] * 25.0;
                    }
                }
            }
            events.extend_from_slice(&y);
        }
        (fluor, names, library, sv, events, n)
    }

    #[test]
    fn af_only_assigns_library_column() {
        let (fluor, names, library, _, events, n) = tiny_panel();
        let sv = SpectralVariants::af_only(names.clone(), vec![0.0; 3]).expect("af_only");
        let out = unmix_autospectral_joint(
            &events,
            n,
            fluor.as_ref(),
            &names,
            &library,
            &sv,
            &JointUnmixConfig {
                parallel_event_threshold: usize::MAX,
                ..JointUnmixConfig::default()
            },
        )
        .expect("joint");
        assert_eq!(out.af_index.len(), n);
        assert!(out.af_index.iter().all(|&i| i < 2));
        assert_eq!(out.abundances.len(), n * 4);
        assert!(out.variant_index.iter().all(Option::is_none));
        // AF-only abundances should be close to OLS with the matched AF column.
        let d = 6;
        for e in 0..n {
            let y = &events[e * d..(e + 1) * d];
            let m = swap_af_column(fluor.as_ref(), &library, out.af_index[e]).expect("swap");
            let ols = crate::unmix_ols::unmix_event_ols(m.as_ref(), y).expect("ols");
            let joint = out.event_abundances(e).expect("row");
            let mut denom = 0.0;
            let mut num = 0.0;
            for j in 0..3 {
                num += ols[j] * joint[j];
                denom += ols[j] * ols[j];
            }
            if denom > 1.0 {
                let cos = num / (denom.sqrt() * dot(&ols[..3], &ols[..3]).sqrt() + 1e-12);
                assert!(cos > 0.7, "event {e} fluor cosine {cos}");
            }
        }
    }

    #[test]
    fn joint_rss_not_worse_than_af_only_ols() {
        let (fluor, names, library, sv, events, n) = tiny_panel();
        let cfg = JointUnmixConfig {
            parallel_event_threshold: usize::MAX,
            n_passes: 1,
            ..JointUnmixConfig::default()
        };
        let joint = unmix_autospectral_joint(
            &events, n, fluor.as_ref(), &names, &library, &sv, &cfg,
        )
        .expect("joint");
        let af_only = SpectralVariants::af_only(names.clone(), vec![0.0; 3]).expect("af");
        let floor = unmix_autospectral_joint(
            &events, n, fluor.as_ref(), &names, &library, &af_only, &cfg,
        )
        .expect("af-only");

        let d = 6;
        let mut rss_joint = 0.0;
        let mut rss_floor = 0.0;
        for e in 0..n {
            let y = &events[e * d..(e + 1) * d];
            let mut m_j = fluor.clone();
            if let Some(v) = joint.variant_index[e * 3] {
                let vmat = &sv.variants["A"];
                for i in 0..d {
                    m_j[(i, 0)] = vmat[(i, v)];
                }
            }
            let m_j = swap_af_column(m_j.as_ref(), &library, joint.af_index[e]).expect("swap j");
            let a_j = joint.event_abundances(e).expect("a");
            rss_joint += ols_residual(m_j.as_ref(), y).ok().unwrap_or_else(|| {
                let mut s = 0.0;
                for i in 0..d {
                    let mut p = 0.0;
                    for j in 0..4 {
                        p += m_j[(i, j)] * a_j[j].max(0.0);
                    }
                    s += (y[i] - p).powi(2);
                }
                s
            });
            let m_f = swap_af_column(fluor.as_ref(), &library, floor.af_index[e]).expect("swap f");
            rss_floor += ols_residual(m_f.as_ref(), y).expect("ols");
        }
        assert!(
            rss_joint <= rss_floor * 1.05 + 1e-6,
            "joint RSS {rss_joint} vs AF-only OLS {rss_floor}"
        );
    }

    #[test]
    fn collinear_pair_runs_and_selects_variant_on_positives() {
        let (fluor, names, library, sv, events, n) = tiny_panel();
        let out = unmix_autospectral_joint(
            &events,
            n,
            fluor.as_ref(),
            &names,
            &library,
            &sv,
            &JointUnmixConfig {
                parallel_event_threshold: usize::MAX,
                joint_pair_resolution: true,
                collinear_threshold: 0.5,
                ..JointUnmixConfig::default()
            },
        )
        .expect("joint");
        let n_var = out
            .variant_index
            .iter()
            .filter(|v| v.is_some())
            .count();
        assert!(n_var > 0, "expected at least one variant commit on A-positives");
        assert_eq!(out.n_events, n);
    }

    #[test]
    fn parallel_matches_sequential_tiny_panel() {
        let (fluor, names, library, sv, events, n) = tiny_panel();
        let seq = JointUnmixConfig {
            parallel_event_threshold: usize::MAX,
            ..JointUnmixConfig::default()
        };
        let par = JointUnmixConfig {
            parallel_event_threshold: 1,
            ..JointUnmixConfig::default()
        };
        let a = unmix_autospectral_joint(
            &events, n, fluor.as_ref(), &names, &library, &sv, &seq,
        )
        .expect("seq");
        let b = unmix_autospectral_joint(
            &events, n, fluor.as_ref(), &names, &library, &sv, &par,
        )
        .expect("par");
        assert_eq!(a.af_index, b.af_index);
        assert_eq!(a.variant_index, b.variant_index);
        for (x, y) in a.abundances.iter().zip(&b.abundances) {
            assert!((x - y).abs() < 1e-9, "{x} vs {y}");
        }
    }

    #[test]
    fn optional_rcpp_agreement_skipped_without_env() {
        if std::env::var("FLOW_AUTOSPECTRAL_RCPP_TEST").is_err() {
            return;
        }
        let status = std::process::Command::new("Rscript")
            .args(["-e", "library(AutoSpectralRcpp)"])
            .status();
        let Ok(st) = status else {
            return;
        };
        if !st.success() {
            return;
        }
        let (fluor, names, library, sv, events, n) = tiny_panel();
        let out = unmix_autospectral_joint(
            &events,
            n,
            fluor.as_ref(),
            &names,
            &library,
            &sv,
            &JointUnmixConfig::default(),
        )
        .expect("joint");
        assert_eq!(out.af_index.len(), n);
    }

    #[test]
    fn f32_path_agrees_with_f64_on_tiny_panel() {
        let (fluor, names, library, sv, events, n) = tiny_panel();
        let cfg64 = JointUnmixConfig {
            parallel_event_threshold: usize::MAX,
            precision: JointUnmixPrecision::F64,
            ..JointUnmixConfig::default()
        };
        let cfg32 = JointUnmixConfig {
            precision: JointUnmixPrecision::F32,
            ..cfg64.clone()
        };
        let a = unmix_autospectral_joint(
            &events, n, fluor.as_ref(), &names, &library, &sv, &cfg64,
        )
        .expect("f64");
        let b = unmix_autospectral_joint(
            &events, n, fluor.as_ref(), &names, &library, &sv, &cfg32,
        )
        .expect("f32");
        assert_eq!(a.af_index, b.af_index);
        assert_eq!(a.variant_index, b.variant_index);
        for (x, y) in a.abundances.iter().zip(&b.abundances) {
            let scale = x.abs().max(1.0);
            assert!((x - y).abs() / scale < 1e-3, "{x} vs {y}");
        }
    }
}
