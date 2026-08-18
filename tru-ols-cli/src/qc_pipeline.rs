//! Literature-aligned QC pipeline (margins → raw doublets → compensation/transform → PeacoQC → scatter or consensus FSC → hybrid doublet).

use anyhow::{Context, Result};
use flow_fcs::Fcs;
use flow_fcs::keyword::{IntegerKeyword, Keyword, KeywordCreationResult, match_and_parse_keyword};
use flow_gates::automated::{
    ConsensusFscConfig, DoubletGateConfig, DoubletMethod, ScatterGateConfig, ScatterGateMethod,
    ScatterQualityPolicy, consensus_fsc_threshold, create_scatter_gate, detect_doublets,
};
use peacoqc_rs::{
    FcsFilter, MarginConfig, PeacoQCConfig, QCMode, peacoqc, preprocess_fcs, remove_margins,
};
use polars::prelude::Series;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, instrument, warn};

/// Preset for [`run_qc_pipeline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QcPreset {
    /// Margins → raw multi-pair doublets → preprocess → PeacoQC → scatter/consensus FSC → hybrid doublet.
    #[default]
    LiteratureDefault,
    /// Margins → debris (scatter/consensus FSC) → raw doublets → preprocess (no PeacoQC) → post-debris hybrid doublets.
    Relaxed,
    /// Previous behavior: margins → `remove_doublets` (single pair) → K-means debris only (no PeacoQC, no scatter gates).
    LegacyTruOls,
}

/// Optional overrides from the CLI (or other callers). Applied on top of [`QcPipelineConfig::literature_default`].
#[derive(Debug, Clone, Default)]
pub struct QcCliOptions {
    pub preset: Option<QcPreset>,
    pub qc_debug_dir: Option<PathBuf>,
    /// Minimum percent of events that should remain inside the scatter gate before the outcome is treated as suspicious (maps to [`ScatterQualityPolicy::min_retention_fraction`] as fraction = pct/100).
    pub scatter_min_keep_pct: Option<f64>,
    pub qc_cofactor: Option<f32>,
    pub qc_no_compensation: bool,
    pub qc_no_transform: bool,
    pub qc_mad: Option<f64>,
    /// Use only the MAD path in time-bin QC (skip isolation-tree step).
    pub qc_mad_only: bool,
}

/// Configuration for [`run_qc_pipeline`].
#[derive(Debug, Clone)]
pub struct QcPipelineConfig {
    pub preset: QcPreset,
    pub qc_mode: QCMode,
    pub apply_compensation: bool,
    pub apply_transformation: bool,
    pub transform_cofactor: f32,
    /// When set, overrides [`PeacoQCConfig::mad`] after [`PeacoQCConfig::for_fcs`].
    pub peacoqc_mad: Option<f64>,
    pub scatter_config: ScatterGateConfig,
    pub scatter_quality: ScatterQualityPolicy,
    /// Doublet detection on raw data (before transform).
    pub raw_doublet_config: DoubletGateConfig,
    /// Doublet refinement after debris gating (hybrid recommended).
    pub post_debris_doublet_config: DoubletGateConfig,
    /// When set, write diagnostic plots (PeacoQC, scatter) under this directory.
    pub debug_plot_dir: Option<PathBuf>,
    /// Optional filename prefix for debug plots, so multiple controls run through the same
    /// `debug_plot_dir` don't overwrite each other's PNGs. Sanitised to filesystem-safe chars.
    pub debug_plot_label: Option<String>,
    /// When true, retain `Fcs` clones after key stages in [`QcPipelineReport::stage_snapshots`].
    pub capture_stages: bool,
}

impl Default for QcPipelineConfig {
    fn default() -> Self {
        Self::literature_default()
    }
}

impl QcPipelineConfig {
    pub fn literature_default() -> Self {
        let raw_doublet_config = DoubletGateConfig {
            channels: vec![
                ("FSC-A".to_string(), "FSC-H".to_string()),
                ("SSC-A".to_string(), "SSC-H".to_string()),
            ],
            method: DoubletMethod::RatioMAD { nmad: 4.0 },
            nmad: Some(4.0),
            density_threshold: None,
            cluster_eps: None,
            cluster_min_samples: None,
        };

        let post_debris_doublet_config = DoubletGateConfig {
            channels: vec![
                ("FSC-A".to_string(), "FSC-H".to_string()),
                ("FSC-W".to_string(), "FSC-H".to_string()),
            ],
            method: DoubletMethod::Hybrid,
            nmad: Some(3.5),
            density_threshold: Some(0.15),
            cluster_eps: None,
            cluster_min_samples: None,
        };

        Self {
            preset: QcPreset::LiteratureDefault,
            qc_mode: QCMode::All,
            apply_compensation: true,
            apply_transformation: true,
            transform_cofactor: 2000.0,
            peacoqc_mad: None,
            scatter_config: ScatterGateConfig {
                fsc_channel: "FSC-A".to_string(),
                ssc_channel: "SSC-A".to_string(),
                method: ScatterGateMethod::DensityContour { threshold: 0.5 },
                min_events: 100,
                density_threshold: Some(0.5),
                cluster_eps: None,
                cluster_min_samples: None,
            },
            scatter_quality: ScatterQualityPolicy::default(),
            raw_doublet_config,
            post_debris_doublet_config,
            debug_plot_dir: None,
            debug_plot_label: None,
            capture_stages: false,
        }
    }

    /// Convert a user-facing label into a filesystem-safe prefix for debug filenames.
    /// Strips path separators, spaces, quotes, and non-ASCII chars to `_` so the debug bundle
    /// stays readable on both macOS and Linux and doesn't collide across controls.
    pub fn sanitized_plot_prefix(&self) -> String {
        self.debug_plot_label
            .as_deref()
            .map(|s| {
                s.chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect::<String>()
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "qc".to_string())
    }

    /// Apply CLI-style overrides in place.
    pub fn apply_qc_cli_options(&mut self, cli: &QcCliOptions) {
        if let Some(p) = cli.preset {
            self.preset = p;
        }
        if let Some(ref d) = cli.qc_debug_dir {
            self.debug_plot_dir = Some(d.clone());
        }
        if let Some(pct) = cli.scatter_min_keep_pct {
            self.scatter_quality.min_retention_fraction = (pct / 100.0).clamp(0.0, 1.0);
        }
        if let Some(cf) = cli.qc_cofactor {
            self.transform_cofactor = cf;
        }
        if cli.qc_no_compensation {
            self.apply_compensation = false;
        }
        if cli.qc_no_transform {
            self.apply_transformation = false;
        }
        if cli.qc_mad_only {
            self.qc_mode = QCMode::MAD;
        }
        if let Some(m) = cli.qc_mad {
            self.peacoqc_mad = Some(m);
        }
    }
}

/// One row in [`QcPipelineReport`].
#[derive(Debug, Clone)]
pub struct QcStageRecord {
    pub name: String,
    pub events_in: usize,
    pub events_out: usize,
    pub pct_removed: f64,
    pub method_used: String,
    pub fallback: Option<String>,
}

/// Result of [`run_qc_pipeline`].
#[derive(Debug)]
pub struct QcPipelineReport {
    pub stages: Vec<QcStageRecord>,
    pub final_fcs: Fcs,
    pub stage_snapshots: Vec<(String, Fcs)>,
}

fn record_stage(
    stages: &mut Vec<QcStageRecord>,
    name: &str,
    n_in: usize,
    n_out: usize,
    method_used: String,
    fallback: Option<String>,
) {
    let pct = if n_in > 0 {
        ((n_in - n_out) as f64 / n_in as f64) * 100.0
    } else {
        0.0
    };
    info!(
        target: "tru_ols::qc",
        stage = %name,
        events_in = n_in,
        events_out = n_out,
        pct_removed = pct,
        method = %method_used,
        fallback = ?fallback.as_deref(),
        "qc_stage"
    );
    stages.push(QcStageRecord {
        name: name.to_string(),
        events_in: n_in,
        events_out: n_out,
        pct_removed: pct,
        method_used,
        fallback,
    });
}

fn maybe_snapshot(capture: bool, buf: &mut Vec<(String, Fcs)>, name: &str, fcs: &Fcs) {
    if capture {
        buf.push((name.to_string(), fcs.clone()));
    }
}

/// Apply a boolean keep mask (`true` = retain row).
pub fn filter_fcs_by_mask(fcs: &Fcs, mask: &[bool]) -> Result<Fcs> {
    let n_events = fcs.get_event_count_from_dataframe();
    if mask.len() != n_events {
        anyhow::bail!("mask length {} != event count {}", mask.len(), n_events);
    }
    let mask_series = Series::from_iter(mask.iter().copied());
    let mask_ca = mask_series
        .bool()
        .map_err(|e| anyhow::anyhow!("boolean mask: {}", e))?;
    let filtered_df = fcs
        .data_frame
        .filter(&mask_ca)
        .map_err(|e| anyhow::anyhow!("filter DataFrame: {}", e))?;
    let mut out = fcs.clone();
    out.data_frame = Arc::new(filtered_df);
    let n_after = out.get_event_count_from_dataframe();
    let tot_keyword = match_and_parse_keyword("$TOT", &n_after.to_string());
    if let KeywordCreationResult::Int(IntegerKeyword::TOT(tot)) = tot_keyword {
        out.metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(tot)));
    }
    Ok(out)
}

fn margin_channel_list(fcs: &Fcs) -> Vec<String> {
    let mut margin_channels: Vec<String> = fcs
        .parameters
        .values()
        .filter(|p| p.is_fluorescence())
        .map(|p| p.channel_name.to_string())
        .collect();
    for scatter in ["FSC-A", "SSC-A"] {
        if fcs
            .parameters
            .values()
            .any(|p| p.channel_name.as_ref() == scatter)
            && !margin_channels.contains(&scatter.to_string())
        {
            margin_channels.push(scatter.to_string());
        }
    }
    margin_channels
}

fn filter_raw_doublet_channels(config: &DoubletGateConfig, fcs: &Fcs) -> DoubletGateConfig {
    let mut c = config.clone();
    c.channels.retain(|(a, h)| {
        fcs.get_parameter_events_slice(a).is_ok() && fcs.get_parameter_events_slice(h).is_ok()
    });
    c
}

fn filter_post_doublet_channels(config: &DoubletGateConfig, fcs: &Fcs) -> DoubletGateConfig {
    let mut c = config.clone();
    c.channels.retain(|(a, h)| {
        fcs.get_parameter_events_slice(a).is_ok() && fcs.get_parameter_events_slice(h).is_ok()
    });
    if c.channels.is_empty() {
        c.channels = vec![("FSC-A".to_string(), "FSC-H".to_string())];
    }
    c
}

/// Run the configured QC pipeline on a copy of `fcs`.
#[instrument(skip(fcs, config), err)]
pub fn run_qc_pipeline(fcs: &Fcs, config: &QcPipelineConfig) -> Result<QcPipelineReport> {
    let mut stages = Vec::new();
    let mut snapshots = Vec::new();

    if config.preset == QcPreset::LegacyTruOls {
        return run_legacy_tru_ols(fcs, config.capture_stages);
    }
    if config.preset == QcPreset::Relaxed {
        return run_relaxed_qc_pipeline(fcs, config);
    }

    let mut current = fcs.clone();
    let n0 = current.get_event_count_from_dataframe();

    // --- margins ---
    let margin_channels = margin_channel_list(&current);
    if margin_channels.is_empty() {
        record_stage(
            &mut stages,
            "margins",
            n0,
            n0,
            "skipped (no channels)".to_string(),
            None,
        );
    } else {
        let margin_config = MarginConfig {
            channels: margin_channels,
            ..Default::default()
        };
        let n_in = current.get_event_count_from_dataframe();
        match remove_margins(&current, &margin_config) {
            Ok(mr) if mr.percentage_removed > 0.0 => {
                current = current
                    .filter(&mr.mask)
                    .map_err(|e| anyhow::anyhow!("margin filter: {}", e))?;
                info!("QC margins: removed {:.2}% events", mr.percentage_removed);
                record_stage(
                    &mut stages,
                    "margins",
                    n_in,
                    current.get_event_count_from_dataframe(),
                    "remove_margins".to_string(),
                    None,
                );
            }
            Ok(_) => {
                record_stage(
                    &mut stages,
                    "margins",
                    n_in,
                    n_in,
                    "remove_margins (none removed)".to_string(),
                    None,
                );
            }
            Err(e) => {
                warn!("margin removal failed: {}; continuing", e);
                record_stage(
                    &mut stages,
                    "margins",
                    n_in,
                    n_in,
                    "remove_margins (error)".to_string(),
                    Some(e.to_string()),
                );
            }
        }
    }
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_margins",
        &current,
    );

    // --- raw doublets (flow-gates, multi-pair AND) ---
    let n_in = current.get_event_count_from_dataframe();
    let raw_cfg = filter_raw_doublet_channels(&config.raw_doublet_config, &current);
    if n_in == 0 {
        record_stage(
            &mut stages,
            "raw_doublets",
            n_in,
            n_in,
            "skipped (no events)".to_string(),
            None,
        );
    } else if raw_cfg.channels.is_empty() {
        record_stage(
            &mut stages,
            "raw_doublets",
            n_in,
            n_in,
            "skipped (no A/H pairs)".to_string(),
            None,
        );
    } else {
        match detect_doublets(&current, &raw_cfg) {
            Ok(dr) => {
                let method = dr.statistics.method_used.clone();
                let n_singlets = dr.singlet_mask.iter().filter(|&&k| k).count();
                current =
                    filter_fcs_by_mask(&current, &dr.singlet_mask).context("raw doublet mask")?;
                info!(
                    "QC raw doublets: {} → {} events ({})",
                    n_in, n_singlets, method
                );
                record_stage(
                    &mut stages,
                    "raw_doublets",
                    n_in,
                    current.get_event_count_from_dataframe(),
                    method,
                    None,
                );
            }
            Err(e) => {
                warn!("raw doublet detection failed: {}; continuing", e);
                record_stage(
                    &mut stages,
                    "raw_doublets",
                    n_in,
                    n_in,
                    "detect_doublets (error)".to_string(),
                    Some(e.to_string()),
                );
            }
        }
    }
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_raw_doublets",
        &current,
    );

    // --- compensation / transform ---
    let n_in = current.get_event_count_from_dataframe();
    current = preprocess_fcs(
        current,
        config.apply_compensation,
        config.apply_transformation,
        config.transform_cofactor,
    )
    .context("preprocess_fcs")?;
    record_stage(
        &mut stages,
        "preprocess",
        n_in,
        current.get_event_count_from_dataframe(),
        format!(
            "comp={} transform={}",
            config.apply_compensation, config.apply_transformation
        ),
        None,
    );
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_preprocess",
        &current,
    );

    // --- PeacoQC time-bin QC ---
    let n_in = current.get_event_count_from_dataframe();
    let fluor: Vec<String> = current
        .parameters
        .values()
        .filter(|p| p.is_fluorescence())
        .map(|p| p.channel_name.to_string())
        .collect();
    if fluor.is_empty() {
        record_stage(
            &mut stages,
            "peacoqc",
            n_in,
            n_in,
            "skipped (no fluorescence parameters)".to_string(),
            None,
        );
    } else {
        let mut pq_config = PeacoQCConfig::for_fcs(&current, config.qc_mode);
        pq_config.apply_compensation = false;
        pq_config.apply_transformation = false;
        if let Some(m) = config.peacoqc_mad {
            pq_config.mad = m;
        }
        match peacoqc(&current, &pq_config) {
            Ok(result) => {
                let pct = result.percentage_removed;
                current = current
                    .filter(&result.good_cells)
                    .map_err(|e| anyhow::anyhow!("peacoqc filter: {}", e))?;
                info!("QC PeacoQC: removed {:.2}% events", pct);
                record_stage(
                    &mut stages,
                    "peacoqc",
                    n_in,
                    current.get_event_count_from_dataframe(),
                    format!(
                        "peacoqc IT={:?} MAD={:?}",
                        result.it_percentage, result.mad_percentage
                    ),
                    None,
                );
                if let Some(ref dir) = config.debug_plot_dir {
                    let _ = std::fs::create_dir_all(dir);
                    let plot_path = dir.join(format!(
                        "{}_peacoqc_overview.png",
                        config.sanitized_plot_prefix()
                    ));
                    let plot_cfg = peacoqc_rs::QCPlotConfig::default();
                    if let Err(e) =
                        peacoqc_rs::create_qc_plots(&current, &result, &plot_path, plot_cfg, None)
                    {
                        warn!("PeacoQC plot export failed: {}", e);
                    } else {
                        info!("Wrote PeacoQC plot {}", plot_path.display());
                    }
                }
            }
            Err(e) => {
                warn!("PeacoQC failed: {}; continuing without time QC", e);
                record_stage(
                    &mut stages,
                    "peacoqc",
                    n_in,
                    n_in,
                    "peacoqc (error)".to_string(),
                    Some(e.to_string()),
                );
            }
        }
    }
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_peacoqc",
        &current,
    );

    // --- scatter or consensus FSC debris ---
    let n_in = current.get_event_count_from_dataframe();
    let mut fallback = None::<String>;
    let (debris_method, keep_mask): (String, Vec<bool>) =
        match create_scatter_gate(&current, &config.scatter_config) {
            Ok(scatter) => {
                let suspicious = scatter.is_suspicious(&config.scatter_quality);
                if suspicious {
                    fallback = Some(format!(
                        "scatter retention {:.4} outside [{:.4}, {:.4}]",
                        scatter.retention_fraction(),
                        config.scatter_quality.min_retention_fraction,
                        config.scatter_quality.max_retention_fraction
                    ));
                    match consensus_fsc_threshold(&current, &ConsensusFscConfig::default())
                        .map_err(|e| anyhow::anyhow!("{}", e))
                    {
                        Ok(cons) => (
                            format!("consensus_FSC(threshold={:.2})", cons.threshold),
                            cons.keep_mask,
                        ),
                        Err(e) => {
                            warn!("consensus FSC failed: {}; using scatter mask anyway", e);
                            (
                                "scatter (suspicious, consensus failed)".to_string(),
                                scatter.population_mask.clone(),
                            )
                        }
                    }
                } else {
                    (scatter.method_used.clone(), scatter.population_mask.clone())
                }
            }
            Err(e) => {
                fallback = Some(e.to_string());
                match consensus_fsc_threshold(&current, &ConsensusFscConfig::default())
                    .map_err(|e2| anyhow::anyhow!("{}", e2))
                {
                    Ok(cons) => (
                        format!("consensus_FSC(threshold={:.2})", cons.threshold),
                        cons.keep_mask,
                    ),
                    Err(e2) => {
                        anyhow::bail!("scatter gate and consensus FSC both failed: {}; {}", e, e2)
                    }
                }
            }
        };
    current = filter_fcs_by_mask(&current, &keep_mask).context("debris mask")?;
    record_stage(
        &mut stages,
        "debris",
        n_in,
        current.get_event_count_from_dataframe(),
        debris_method,
        fallback,
    );
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_debris",
        &current,
    );

    if let Some(ref dir) = config.debug_plot_dir {
        let _ = write_scatter_debug_png(
            dir.join(format!(
                "{}_scatter_post_debris.png",
                config.sanitized_plot_prefix()
            )),
            &current,
        );
    }

    // --- post-debris hybrid doublet ---
    let n_in = current.get_event_count_from_dataframe();
    let post_cfg = filter_post_doublet_channels(&config.post_debris_doublet_config, &current);
    if n_in == 0 {
        record_stage(
            &mut stages,
            "post_debris_doublets",
            n_in,
            n_in,
            "skipped (no events)".to_string(),
            None,
        );
    } else {
        match detect_doublets(&current, &post_cfg) {
            Ok(dr) => {
                let method = dr.statistics.method_used.clone();
                current =
                    filter_fcs_by_mask(&current, &dr.singlet_mask).context("post doublet mask")?;
                record_stage(
                    &mut stages,
                    "post_debris_doublets",
                    n_in,
                    current.get_event_count_from_dataframe(),
                    method,
                    None,
                );
            }
            Err(e) => {
                warn!("post-debris doublet detection failed: {}", e);
                record_stage(
                    &mut stages,
                    "post_debris_doublets",
                    n_in,
                    n_in,
                    "detect_doublets (error)".to_string(),
                    Some(e.to_string()),
                );
            }
        }
    }
    maybe_snapshot(config.capture_stages, &mut snapshots, "final", &current);

    Ok(QcPipelineReport {
        stages,
        final_fcs: current,
        stage_snapshots: snapshots,
    })
}

/// Relaxed preset: margins → debris → raw doublets → preprocess (no PeacoQC) → post-debris doublets.
#[instrument(skip(fcs, config), err)]
fn run_relaxed_qc_pipeline(fcs: &Fcs, config: &QcPipelineConfig) -> Result<QcPipelineReport> {
    let mut stages = Vec::new();
    let mut snapshots = Vec::new();
    let mut current = fcs.clone();
    let n0 = current.get_event_count_from_dataframe();

    // --- margins ---
    let margin_channels = margin_channel_list(&current);
    if margin_channels.is_empty() {
        record_stage(
            &mut stages,
            "margins",
            n0,
            n0,
            "skipped (no channels)".to_string(),
            None,
        );
    } else {
        let margin_config = MarginConfig {
            channels: margin_channels,
            ..Default::default()
        };
        let n_in = current.get_event_count_from_dataframe();
        match remove_margins(&current, &margin_config) {
            Ok(mr) if mr.percentage_removed > 0.0 => {
                current = current
                    .filter(&mr.mask)
                    .map_err(|e| anyhow::anyhow!("margin filter: {}", e))?;
                info!("QC margins: removed {:.2}% events", mr.percentage_removed);
                record_stage(
                    &mut stages,
                    "margins",
                    n_in,
                    current.get_event_count_from_dataframe(),
                    "remove_margins".to_string(),
                    None,
                );
            }
            Ok(_) => {
                record_stage(
                    &mut stages,
                    "margins",
                    n_in,
                    n_in,
                    "remove_margins (none removed)".to_string(),
                    None,
                );
            }
            Err(e) => {
                warn!("margin removal failed: {}; continuing", e);
                record_stage(
                    &mut stages,
                    "margins",
                    n_in,
                    n_in,
                    "remove_margins (error)".to_string(),
                    Some(e.to_string()),
                );
            }
        }
    }
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_margins",
        &current,
    );

    // --- debris (before raw doublets) ---
    let n_in = current.get_event_count_from_dataframe();
    let mut fallback = None::<String>;
    let (debris_method, keep_mask): (String, Vec<bool>) =
        match create_scatter_gate(&current, &config.scatter_config) {
            Ok(scatter) => {
                let suspicious = scatter.is_suspicious(&config.scatter_quality);
                if suspicious {
                    fallback = Some(format!(
                        "scatter retention {:.4} outside [{:.4}, {:.4}]",
                        scatter.retention_fraction(),
                        config.scatter_quality.min_retention_fraction,
                        config.scatter_quality.max_retention_fraction
                    ));
                    match consensus_fsc_threshold(&current, &ConsensusFscConfig::default())
                        .map_err(|e| anyhow::anyhow!("{}", e))
                    {
                        Ok(cons) => (
                            format!("consensus_FSC(threshold={:.2})", cons.threshold),
                            cons.keep_mask,
                        ),
                        Err(e) => {
                            warn!("consensus FSC failed: {}; using scatter mask anyway", e);
                            (
                                "scatter (suspicious, consensus failed)".to_string(),
                                scatter.population_mask.clone(),
                            )
                        }
                    }
                } else {
                    (scatter.method_used.clone(), scatter.population_mask.clone())
                }
            }
            Err(e) => {
                fallback = Some(e.to_string());
                match consensus_fsc_threshold(&current, &ConsensusFscConfig::default())
                    .map_err(|e2| anyhow::anyhow!("{}", e2))
                {
                    Ok(cons) => (
                        format!("consensus_FSC(threshold={:.2})", cons.threshold),
                        cons.keep_mask,
                    ),
                    Err(e2) => {
                        anyhow::bail!("scatter gate and consensus FSC both failed: {}; {}", e, e2)
                    }
                }
            }
        };
    current = filter_fcs_by_mask(&current, &keep_mask).context("debris mask")?;
    record_stage(
        &mut stages,
        "debris",
        n_in,
        current.get_event_count_from_dataframe(),
        debris_method,
        fallback,
    );
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_debris",
        &current,
    );

    if let Some(ref dir) = config.debug_plot_dir {
        let _ = write_scatter_debug_png(
            dir.join(format!(
                "{}_scatter_post_debris.png",
                config.sanitized_plot_prefix()
            )),
            &current,
        );
    }

    // --- raw doublets ---
    let n_in = current.get_event_count_from_dataframe();
    let raw_cfg = filter_raw_doublet_channels(&config.raw_doublet_config, &current);
    if n_in == 0 {
        record_stage(
            &mut stages,
            "raw_doublets",
            n_in,
            n_in,
            "skipped (no events)".to_string(),
            None,
        );
    } else if raw_cfg.channels.is_empty() {
        record_stage(
            &mut stages,
            "raw_doublets",
            n_in,
            n_in,
            "skipped (no A/H pairs)".to_string(),
            None,
        );
    } else {
        match detect_doublets(&current, &raw_cfg) {
            Ok(dr) => {
                let method = dr.statistics.method_used.clone();
                let n_singlets = dr.singlet_mask.iter().filter(|&&k| k).count();
                current =
                    filter_fcs_by_mask(&current, &dr.singlet_mask).context("raw doublet mask")?;
                info!(
                    "QC raw doublets: {} → {} events ({})",
                    n_in, n_singlets, method
                );
                record_stage(
                    &mut stages,
                    "raw_doublets",
                    n_in,
                    current.get_event_count_from_dataframe(),
                    method,
                    None,
                );
            }
            Err(e) => {
                warn!("raw doublet detection failed: {}; continuing", e);
                record_stage(
                    &mut stages,
                    "raw_doublets",
                    n_in,
                    n_in,
                    "detect_doublets (error)".to_string(),
                    Some(e.to_string()),
                );
            }
        }
    }
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_raw_doublets",
        &current,
    );

    // --- preprocess ---
    let n_in = current.get_event_count_from_dataframe();
    current = preprocess_fcs(
        current,
        config.apply_compensation,
        config.apply_transformation,
        config.transform_cofactor,
    )
    .context("preprocess_fcs")?;
    record_stage(
        &mut stages,
        "preprocess",
        n_in,
        current.get_event_count_from_dataframe(),
        format!(
            "comp={} transform={}",
            config.apply_compensation, config.apply_transformation
        ),
        None,
    );
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_preprocess",
        &current,
    );

    // --- PeacoQC skipped ---
    let n_in = current.get_event_count_from_dataframe();
    record_stage(
        &mut stages,
        "peacoqc",
        n_in,
        n_in,
        "skipped (relaxed preset)".to_string(),
        None,
    );
    maybe_snapshot(
        config.capture_stages,
        &mut snapshots,
        "post_peacoqc",
        &current,
    );

    // --- post-debris hybrid doublet ---
    let n_in = current.get_event_count_from_dataframe();
    let post_cfg = filter_post_doublet_channels(&config.post_debris_doublet_config, &current);
    if n_in == 0 {
        record_stage(
            &mut stages,
            "post_debris_doublets",
            n_in,
            n_in,
            "skipped (no events)".to_string(),
            None,
        );
    } else {
        match detect_doublets(&current, &post_cfg) {
            Ok(dr) => {
                let method = dr.statistics.method_used.clone();
                current =
                    filter_fcs_by_mask(&current, &dr.singlet_mask).context("post doublet mask")?;
                record_stage(
                    &mut stages,
                    "post_debris_doublets",
                    n_in,
                    current.get_event_count_from_dataframe(),
                    method,
                    None,
                );
            }
            Err(e) => {
                warn!("post-debris doublet detection failed: {}", e);
                record_stage(
                    &mut stages,
                    "post_debris_doublets",
                    n_in,
                    n_in,
                    "detect_doublets (error)".to_string(),
                    Some(e.to_string()),
                );
            }
        }
    }
    maybe_snapshot(config.capture_stages, &mut snapshots, "final", &current);

    Ok(QcPipelineReport {
        stages,
        final_fcs: current,
        stage_snapshots: snapshots,
    })
}

fn run_legacy_tru_ols(fcs: &Fcs, capture: bool) -> Result<QcPipelineReport> {
    use peacoqc_rs::{DoubletConfig, remove_doublets};

    let mut stages = Vec::new();
    let mut snapshots = Vec::new();
    let mut current = fcs.clone();
    let margin_channels = margin_channel_list(&current);
    if !margin_channels.is_empty() {
        let margin_config = MarginConfig {
            channels: margin_channels,
            ..Default::default()
        };
        let n_in = current.get_event_count_from_dataframe();
        if let Ok(mr) = remove_margins(&current, &margin_config) {
            if mr.percentage_removed > 0.0 {
                current = current.filter(&mr.mask)?;
            }
        }
        record_stage(
            &mut stages,
            "margins",
            n_in,
            current.get_event_count_from_dataframe(),
            "legacy remove_margins".to_string(),
            None,
        );
    }
    maybe_snapshot(capture, &mut snapshots, "post_margins", &current);

    let n_in = current.get_event_count_from_dataframe();
    let dc = DoubletConfig::default();
    if let Ok(dr) = remove_doublets(&current, &dc) {
        if dr.percentage_removed > 0.0 {
            current = current.filter(&dr.mask)?;
        }
        record_stage(
            &mut stages,
            "legacy_peacoqc_doublets",
            n_in,
            current.get_event_count_from_dataframe(),
            "remove_doublets".to_string(),
            None,
        );
    }
    maybe_snapshot(capture, &mut snapshots, "post_legacy_doublets", &current);

    let n_in = current.get_event_count_from_dataframe();
    current = crate::commands::remove_debris_heuristic(&current)?;
    record_stage(
        &mut stages,
        "legacy_kmeans_debris",
        n_in,
        current.get_event_count_from_dataframe(),
        "remove_debris_heuristic".to_string(),
        None,
    );
    maybe_snapshot(capture, &mut snapshots, "final", &current);

    Ok(QcPipelineReport {
        stages,
        final_fcs: current,
        stage_snapshots: snapshots,
    })
}

fn write_scatter_debug_png(path: std::path::PathBuf, fcs: &Fcs) -> Result<()> {
    use flow_plots::options::{AxisOptions, BasePlotOptions, DensityPlotOptions};
    use flow_plots::render::RenderConfig;
    use flow_plots::scatter_data::ScatterPlotData;
    use flow_plots::{DensityPlot, Plot};

    let fsc = fcs
        .get_parameter_events_slice("FSC-A")
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let ssc = fcs
        .get_parameter_events_slice("SSC-A")
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    if fsc.len() != ssc.len() || fsc.is_empty() {
        return Ok(());
    }
    let points: Vec<(f32, f32)> = fsc
        .iter()
        .zip(ssc.iter())
        .map(|(&x, &y)| (x as f32, y as f32))
        .collect();
    let data: ScatterPlotData = points.into();

    let base = BasePlotOptions::new()
        .width(640u32)
        .height(480u32)
        .title("Post-debris QC")
        .build()
        .map_err(|e| anyhow::anyhow!("base options: {}", e))?;
    let x_axis = AxisOptions::new()
        .label("FSC-A")
        .build()
        .map_err(|e| anyhow::anyhow!("x axis: {}", e))?;
    let y_axis = AxisOptions::new()
        .label("SSC-A")
        .build()
        .map_err(|e| anyhow::anyhow!("y axis: {}", e))?;
    let opts = DensityPlotOptions::new()
        .base(base)
        .x_axis(x_axis)
        .y_axis(y_axis)
        .build()
        .map_err(|e| anyhow::anyhow!("density options: {}", e))?;

    let plot = DensityPlot::new();
    let mut render_cfg = RenderConfig::new();
    let bytes = plot
        .render(data, &opts, &mut render_cfg)
        .map_err(|e| anyhow::anyhow!("render: {}", e))?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, bytes).with_context(|| path.display().to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_fcs::file::AccessWrapper;
    use flow_fcs::{Fcs, Header, Metadata, Parameter, TransformType, parameter::ParameterMap};
    use polars::prelude::*;
    use std::sync::Arc;

    fn pipeline_test_fcs(n: usize) -> Fcs {
        let mut time = Vec::with_capacity(n);
        let mut fsc = Vec::with_capacity(n);
        let mut ssc = Vec::with_capacity(n);
        let mut fsc_h = Vec::with_capacity(n);
        let mut ssc_h = Vec::with_capacity(n);
        let mut fl1 = Vec::with_capacity(n);
        let br = n * 4 / 5;
        for i in 0..n {
            time.push(i as f32);
            fsc.push(48_000.0_f32 + (i % 40) as f32 + (i % 7) as f32 * 0.25);
            ssc.push(32_000.0_f32 + (i % 30) as f32 + (i % 5) as f32 * 0.25);
            fsc_h.push(fsc[i] * 0.91);
            ssc_h.push(ssc[i] * 0.93);
            if i < br {
                fl1.push(120.0_f32 + (i % 11) as f32);
            } else {
                fl1.push(25_000.0_f32 + (i % 13) as f32);
            }
        }
        let df = DataFrame::new(
            n,
            vec![
                Column::new("Time".into(), time),
                Column::new("FSC-A".into(), fsc),
                Column::new("SSC-A".into(), ssc),
                Column::new("FSC-H".into(), fsc_h),
                Column::new("SSC-H".into(), ssc_h),
                Column::new("FL1-A".into(), fl1),
            ],
        )
        .expect("df");
        let mut pm = ParameterMap::default();
        for (num, ch) in [
            (1usize, "Time"),
            (2, "FSC-A"),
            (3, "SSC-A"),
            (4, "FSC-H"),
            (5, "SSC-H"),
            (6, "FL1-A"),
        ] {
            pm.insert(
                ch.into(),
                Parameter::new(&num, ch, ch, &TransformType::Linear),
            );
        }
        let metadata = Metadata::from_dataframe_and_parameters(&df, &pm).expect("metadata");
        let tmp = std::env::temp_dir().join(format!("truols_qc_{}.tmp", std::process::id()));
        let _ = std::fs::write(&tmp, b"x");
        Fcs::for_testing(
            Header::new(),
            metadata,
            pm,
            Arc::new(df),
            AccessWrapper::new(tmp.to_str().unwrap_or(".")).expect("access"),
        )
    }

    #[test]
    fn literature_pipeline_runs_and_shrinks_or_equal() {
        let fcs = pipeline_test_fcs(8000);
        let n0 = fcs.get_event_count_from_dataframe();
        let mut cfg = QcPipelineConfig::literature_default();
        cfg.qc_mode = QCMode::MAD;
        let rep = run_qc_pipeline(&fcs, &cfg).expect("qc");
        assert!(!rep.stages.is_empty());
        let n1 = rep.final_fcs.get_event_count_from_dataframe();
        assert!(n1 > 0 && n1 <= n0);
    }

    /// Set `TRU_OLS_QC_TEST_PLOTS=1` to write `<prefix>_scatter_post_debris.png` and PeacoQC plot under the target temp dir.
    #[test]
    fn pipeline_debug_plot_bundle_env_gated() {
        if std::env::var("TRU_OLS_QC_TEST_PLOTS").ok().as_deref() != Some("1") {
            return;
        }
        let base = std::env::var("CARGO_TARGET_TMPDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        let dir = base.join("tru_ols_qc_plots");
        let _ = std::fs::create_dir_all(&dir);
        let fcs = pipeline_test_fcs(8000);
        let mut cfg = QcPipelineConfig::literature_default();
        cfg.qc_mode = QCMode::MAD;
        cfg.debug_plot_dir = Some(dir.clone());
        let _rep = run_qc_pipeline(&fcs, &cfg).expect("qc");
        // Default label sanitises to `qc` when no debug_plot_label is set.
        let scatter = dir.join("qc_scatter_post_debris.png");
        assert!(
            scatter.exists(),
            "expected qc_scatter_post_debris.png under {}",
            dir.display()
        );
    }
}
