//! Synthetic FCS-like event tables (Gaussian mixtures) for benches and harnesses.
//!
//! Enable with `--features synthetic` (also turns on `test-util` for `Fcs` fixtures).
//! PeacoQC-specific timed artifacts should be layered by callers, not here.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use polars::prelude::{Column, DataFrame};
use rand::SeedableRng;
use rand::distr::weighted::WeightedIndex;
use rand::rngs::StdRng;
use rand_distr::{Distribution, Normal};

use crate::file::{AccessWrapper, Fcs};
use crate::header::Header;
use crate::keyword::{IntegerKeyword, Keyword, MixedKeyword};
use crate::metadata::Metadata;
use crate::parameter::{Parameter, ParameterMap};
use crate::transform::TransformType;
use crate::version::Version;
use crate::write::write_fcs_file;

/// One Gaussian component in the same channel order as [`GaussianPopulationsSpec::channel_names`].
#[derive(Debug, Clone)]
pub struct GaussianComponent {
    /// Relative mixture weight (need not sum to 1; weights are normalized).
    pub weight: f64,
    /// Per-channel means (`len` must match channel count).
    pub mean: Vec<f32>,
    /// Per-channel standard deviations (`len` must match channel count; must be > 0).
    pub std_dev: Vec<f32>,
}

/// Spec for [`gaussian_population_columns`] / [`gaussian_populations_dataframe`].
#[derive(Debug, Clone)]
pub struct GaussianPopulationsSpec {
    pub n_events: usize,
    pub channel_names: Vec<String>,
    pub components: Vec<GaussianComponent>,
    /// RNG seed for reproducibility across crates and languages.
    pub seed: u64,
}

/// Monotonic `Time` plus scatter + `FL{1..=n}-A` names used by PeacoQC / R harnesses.
pub fn cytometry_channel_names(n_fluorescence: usize) -> Vec<String> {
    let mut names = vec![
        "Time".to_string(),
        "FSC-A".to_string(),
        "FSC-H".to_string(),
        "SSC-A".to_string(),
    ];
    for i in 1..=n_fluorescence {
        names.push(format!("FL{i}-A"));
    }
    names
}

/// Stable, cytometry-like two-population mixture (no intentional acquisition drift).
///
/// - `Time` is filled as `0..n_events` (not drawn from the mixture).
/// - Scatter and fluorescence use two Gaussians (~70% / ~30% weight).
pub fn default_cytometry_mixture(
    n_events: usize,
    n_fluorescence: usize,
    seed: u64,
) -> GaussianPopulationsSpec {
    let channel_names = cytometry_channel_names(n_fluorescence);
    // Channel layout: Time, FSC-A, FSC-H, SSC-A, FL1..FLn
    let n_channels = channel_names.len();
    let mut mean_lo = vec![0.0_f32; n_channels];
    let mut std_lo = vec![1.0_f32; n_channels];
    let mut mean_hi = vec![0.0_f32; n_channels];
    let mut std_hi = vec![1.0_f32; n_channels];

    // Time slot is overwritten after sampling; placeholders keep vector lengths valid.
    mean_lo[0] = 0.0;
    std_lo[0] = 1.0;
    mean_hi[0] = 0.0;
    std_hi[0] = 1.0;

    mean_lo[1] = 50_000.0; // FSC-A
    std_lo[1] = 8_000.0;
    mean_hi[1] = 80_000.0;
    std_hi[1] = 10_000.0;

    mean_lo[2] = 46_000.0; // FSC-H (~0.92 * FSC-A center)
    std_lo[2] = 7_500.0;
    mean_hi[2] = 73_000.0;
    std_hi[2] = 9_500.0;

    mean_lo[3] = 30_000.0; // SSC-A
    std_lo[3] = 7_000.0;
    mean_hi[3] = 55_000.0;
    std_hi[3] = 9_000.0;

    for fl in 0..n_fluorescence {
        let i = 4 + fl;
        let base = 800.0 + fl as f32 * 200.0;
        mean_lo[i] = base;
        std_lo[i] = 150.0 + fl as f32 * 10.0;
        mean_hi[i] = 4_000.0 + fl as f32 * 500.0;
        std_hi[i] = 600.0 + fl as f32 * 40.0;
    }

    GaussianPopulationsSpec {
        n_events,
        channel_names,
        components: vec![
            GaussianComponent {
                weight: 0.7,
                mean: mean_lo,
                std_dev: std_lo,
            },
            GaussianComponent {
                weight: 0.3,
                mean: mean_hi,
                std_dev: std_hi,
            },
        ],
        seed,
    }
}

fn validate_spec(spec: &GaussianPopulationsSpec) -> Result<()> {
    if spec.n_events == 0 {
        bail!("n_events must be > 0");
    }
    if spec.channel_names.is_empty() {
        bail!("channel_names must not be empty");
    }
    if spec.components.is_empty() {
        bail!("components must not be empty");
    }
    let n = spec.channel_names.len();
    for (i, c) in spec.components.iter().enumerate() {
        if c.weight <= 0.0 || !c.weight.is_finite() {
            bail!("component {i}: weight must be finite and > 0");
        }
        if c.mean.len() != n || c.std_dev.len() != n {
            bail!(
                "component {i}: mean/std_dev length {}/{} must match channel count {n}",
                c.mean.len(),
                c.std_dev.len()
            );
        }
        for (j, &s) in c.std_dev.iter().enumerate() {
            if !(s > 0.0 && s.is_finite()) {
                bail!("component {i} channel {j}: std_dev must be finite and > 0");
            }
        }
    }
    Ok(())
}

/// Draw mixture columns as `(name, values)` in channel order.
///
/// When a channel is named `Time` (case-sensitive), values are `0..n_events` as `f32`
/// instead of mixture samples (acquisition clock).
pub fn gaussian_population_columns(
    spec: &GaussianPopulationsSpec,
) -> Result<Vec<(String, Vec<f32>)>> {
    validate_spec(spec)?;
    let n_channels = spec.channel_names.len();
    let weights: Vec<f64> = spec.components.iter().map(|c| c.weight).collect();
    let chooser = WeightedIndex::new(&weights).context("build WeightedIndex from component weights")?;
    let mut rng = StdRng::seed_from_u64(spec.seed);

    let mut columns: Vec<Vec<f32>> = (0..n_channels)
        .map(|_| Vec::with_capacity(spec.n_events))
        .collect();

    for _ in 0..spec.n_events {
        let ci = chooser.sample(&mut rng);
        let comp = &spec.components[ci];
        for ch in 0..n_channels {
            let dist = Normal::new(comp.mean[ch] as f64, comp.std_dev[ch] as f64)
                .with_context(|| format!("Normal(mean, std) for channel {ch}"))?;
            let v = dist.sample(&mut rng) as f32;
            columns[ch].push(v.max(0.0));
        }
    }

    for (i, name) in spec.channel_names.iter().enumerate() {
        if name == "Time" {
            columns[i] = (0..spec.n_events).map(|e| e as f32).collect();
        }
    }

    Ok(spec
        .channel_names
        .iter()
        .cloned()
        .zip(columns)
        .collect())
}

/// Build a Polars [`DataFrame`] from [`gaussian_population_columns`].
pub fn gaussian_populations_dataframe(spec: &GaussianPopulationsSpec) -> Result<DataFrame> {
    let cols = gaussian_population_columns(spec)?;
    let columns: Vec<Column> = cols
        .into_iter()
        .map(|(name, values)| Column::new(name.into(), values))
        .collect();
    DataFrame::new_infer_height(columns).context("build synthetic DataFrame")
}

fn metadata_for_channels(names: &[String]) -> Metadata {
    let mut metadata = Metadata::new();
    metadata.insert_string_keyword("$BYTEORD".into(), "1,2,3,4".into());
    metadata.insert_string_keyword("$DATATYPE".into(), "F".into());
    metadata.insert_string_keyword("$MODE".into(), "L".into());
    metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
    metadata.insert_string_keyword(
        "$CYT".into(),
        "flow-crates-synthetic-gaussian".into(),
    );
    for (p, name) in names.iter().enumerate() {
        let idx = p + 1;
        metadata.insert_string_keyword(format!("$P{idx}N"), name.clone());
        metadata
            .keywords
            .insert(format!("$P{idx}B"), Keyword::Int(IntegerKeyword::PnB(32)));
        metadata.keywords.insert(
            format!("$P{idx}R"),
            Keyword::Int(IntegerKeyword::PnR(262_144)),
        );
        metadata.keywords.insert(
            format!("$P{idx}E"),
            Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)),
        );
    }
    metadata
}

fn parameters_for_channels(names: &[String]) -> ParameterMap {
    let mut params = ParameterMap::default();
    for (i, name) in names.iter().enumerate() {
        let idx = i + 1;
        params.insert(
            name.clone().into(),
            Parameter::new(&idx, name, name, &TransformType::Linear),
        );
    }
    params
}

/// In-memory [`Fcs`] fixture (`test-util` / `synthetic` feature).
pub fn gaussian_populations_fcs(spec: &GaussianPopulationsSpec) -> Result<Fcs> {
    let df = gaussian_populations_dataframe(spec)?;
    let metadata = metadata_for_channels(&spec.channel_names);
    let parameters = parameters_for_channels(&spec.channel_names);
    let mut header = Header::new();
    header.version = Version::V3_1;

    let tmp = std::env::temp_dir().join(format!(
        "flow_fcs_synth_{}_{}.tmp",
        std::process::id(),
        spec.seed
    ));
    std::fs::write(&tmp, b"x").context("write temp access stub for synthetic Fcs")?;
    let access = AccessWrapper::new(tmp.to_str().context("temp path UTF-8")?)
        .context("AccessWrapper for synthetic Fcs")?;

    Ok(Fcs::for_testing(
        header,
        metadata,
        parameters,
        Arc::new(df),
        access,
    ))
}

/// Write a float FCS file with the mixture (analysis-space fixture; no compensate/transform).
pub fn write_gaussian_populations_fcs(
    path: impl AsRef<Path>,
    spec: &GaussianPopulationsSpec,
) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent dir {}", parent.display()))?;
    }
    let fcs = gaussian_populations_fcs(spec)?;
    write_fcs_file(fcs, path)
        .with_context(|| format!("write synthetic FCS {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mixture_is_reproducible_and_nonnegative() {
        let spec = default_cytometry_mixture(1_000, 5, 42);
        let a = gaussian_population_columns(&spec).expect("cols a");
        let b = gaussian_population_columns(&spec).expect("cols b");
        assert_eq!(a.len(), b.len());
        for ((na, va), (nb, vb)) in a.iter().zip(b.iter()) {
            assert_eq!(na, nb);
            assert_eq!(va, vb);
            assert!(va.iter().all(|&x| x.is_finite() && x >= 0.0));
        }
        let time = &a[0];
        assert_eq!(time.0, "Time");
        assert_eq!(time.1[0], 0.0);
        assert_eq!(time.1[999], 999.0);
    }

    #[test]
    fn rejects_mismatched_component_dims() {
        let spec = GaussianPopulationsSpec {
            n_events: 10,
            channel_names: vec!["A".into(), "B".into()],
            components: vec![GaussianComponent {
                weight: 1.0,
                mean: vec![1.0],
                std_dev: vec![1.0],
            }],
            seed: 1,
        };
        assert!(gaussian_population_columns(&spec).is_err());
    }
}
