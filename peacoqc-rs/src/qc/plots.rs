//! QC plotting module for PeacoQC
//!
//! This module provides functionality to create QC plots similar to the R PeacoQC package.
//! It generates:
//! - Time vs events/second plot
//! - Signal value vs cell event plots for each QC'd channel with highlighted unstable regions

use crate::PeacoQCData;
use crate::error::{PeacoQCError, Result};
use crate::qc::peacoqc::{PeacoQCResult, RemovalReason};
use crate::qc::peaks::create_breaks;
use plotters::prelude::*;
use plotters::style::{BLACK, RGBAColor, RGBColor, WHITE};
use std::path::Path;

/// Default grid line color (light gray, supports reading without dominating)
const GRID_LINE_COLOR: RGBColor = RGBColor(218, 218, 218);

/// Dash length in pixels for MAD threshold lines (plot and legend)
const MAD_DASH_LEN: u32 = 14;
/// Gap in pixels between dashes for MAD threshold lines (plot and legend)
const MAD_DASH_GAP: u32 = 3;

/// Semi-transparent white for legend background (readable over plot)
const LEGEND_BG_COLOR: RGBAColor = RGBAColor(255, 255, 255, 0.85);

/// Configuration for QC plots
#[derive(Debug, Clone)]
pub struct QCPlotConfig {
    /// Output image width in pixels
    pub width: u32,

    /// Output image height in pixels
    pub height: u32,

    /// Number of columns in the plot grid
    pub n_cols: usize,

    /// Number of rows in the plot grid
    pub n_rows: usize,

    /// Color for unstable regions (used when removal reason is not available)
    pub unstable_color: RGBColor,

    /// Color for regions/points removed by Isolation Tree (None = use unstable_color / bad_color)
    pub unstable_color_it: Option<RGBColor>,

    /// Color for regions/points removed by MAD (None = use unstable_color / bad_color)
    pub unstable_color_mad: Option<RGBColor>,

    /// Color for regions/points removed by consecutive filter (None = use unstable_color / bad_color)
    pub unstable_color_consecutive: Option<RGBColor>,

    /// Color for good data points
    pub good_color: RGBColor,

    /// Color for bad (unstable) data points (used when removal reason is not available)
    pub bad_color: RGBColor,

    /// Color for median line
    pub median_color: RGBColor,

    /// Color for smoothed spline line
    pub smoothed_spline_color: RGBColor,

    /// Color for MAD threshold lines
    pub mad_threshold_color: RGBColor,

    /// Show smoothed spline and MAD threshold lines (default: true)
    pub show_spline_and_mad: bool,

    /// Show bin boundaries (gray vertical lines, default: false)
    pub show_bin_boundaries: bool,

    /// Font size for axis labels (description); default 18
    pub axis_label_size: u32,

    /// Font size for tick labels; default 15 (one step down from axis labels)
    pub tick_label_size: u32,

    /// Font size for legend text; default 17
    pub legend_font_size: u32,

    /// Font size for plot title/caption; default 22
    pub caption_font_size: u32,

    /// Font family for all text (e.g. "sans-serif", "serif"); None = "sans-serif"
    pub font_family: Option<String>,

    /// Background color; None = white (light theme)
    pub background_color: Option<RGBColor>,

    /// Foreground color for text, axes, grid; None = black (for contrast on light background)
    pub foreground_color: Option<RGBColor>,

    /// Alpha for scatter points (0.0–1.0) to reduce overplotting; None = opaque
    pub scatter_alpha: Option<f32>,
}

/// Resolve color for a removal reason (region or point). Falls back to unstable_color / bad_color when reason-specific color is None.
fn color_for_removal_reason(
    config: &QCPlotConfig,
    reason: RemovalReason,
    fallback: RGBColor,
) -> RGBColor {
    let c = match reason {
        RemovalReason::IsolationTree => config.unstable_color_it,
        RemovalReason::MAD => config.unstable_color_mad,
        RemovalReason::Consecutive => config.unstable_color_consecutive,
    };
    c.unwrap_or(fallback)
}

impl Default for QCPlotConfig {
    fn default() -> Self {
        Self {
            width: 2400,
            height: 1800,
            n_cols: 4,
            n_rows: 6,
            unstable_color: RGBColor(200, 150, 255), // Light purple
            unstable_color_it: Some(RGBColor(255, 165, 0)), // Orange
            unstable_color_mad: Some(RGBColor(200, 150, 255)), // Purple (legacy default)
            unstable_color_consecutive: Some(RGBColor(255, 200, 100)), // Amber
            good_color: RGBColor(128, 128, 128),     // Grey
            bad_color: RGBColor(200, 50, 50),        // Red for bad events
            median_color: RGBColor(0, 0, 0),         // Black
            smoothed_spline_color: RGBColor(0, 0, 255), // Blue (distinct from red bad events)
            mad_threshold_color: RGBColor(0, 200, 80), // Bright green for MAD bounds
            show_spline_and_mad: true,               // Enabled by default
            show_bin_boundaries: false,              // Disabled by default
            axis_label_size: 20,
            tick_label_size: 17,
            legend_font_size: 17,
            caption_font_size: 22,
            font_family: None,
            background_color: None,
            foreground_color: None,
            scatter_alpha: Some(0.5), // Slight transparency to show density
        }
    }
}

/// Find the time channel name
fn find_time_channel<T: PeacoQCData>(fcs: &T) -> Option<String> {
    fcs.channel_names().into_iter().find(|name| {
        let upper = name.to_uppercase();
        upper.contains("TIME") || upper == "TIME"
    })
}

fn median_positive_step(time_values: &[f64]) -> f64 {
    let mut diffs: Vec<f64> = time_values
        .windows(2)
        .filter_map(|pair| {
            let d = pair[1] - pair[0];
            (d.is_finite() && d > 0.0).then_some(d)
        })
        .collect();
    if diffs.is_empty() {
        return 1.0;
    }
    diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    diffs[diffs.len() / 2]
}

fn is_time_discontinuity(prev: f64, next: f64, max_step: f64) -> bool {
    if !prev.is_finite() || !next.is_finite() {
        return true;
    }
    next < prev || next - prev > max_step
}

/// Bin event rate vs Time. Splits a window at Time wrap / huge jumps so a clock
/// overflow cannot emit a midpoint in the middle of the axis with rate ≈ 0.
fn event_rate_windows(time_values: &[f64], window_size: usize) -> Vec<(f64, f64)> {
    let typical_dt = median_positive_step(time_values);
    let max_step = (typical_dt * 50.0).max(1.0);
    let mut events_per_second = Vec::new();
    let mut i = 0;

    while i < time_values.len() {
        let window_end = (i + window_size).min(time_values.len());
        if window_end <= i {
            break;
        }
        push_rate_runs(
            &mut events_per_second,
            &time_values[i..window_end],
            typical_dt,
            max_step,
        );
        i = window_end;
    }

    events_per_second
}

fn push_rate_runs(
    out: &mut Vec<(f64, f64)>,
    window_times: &[f64],
    typical_dt: f64,
    max_step: f64,
) {
    if window_times.len() < 2 {
        return;
    }
    let mut run_start = 0;
    for j in 1..window_times.len() {
        if is_time_discontinuity(window_times[j - 1], window_times[j], max_step) {
            push_rate_run(out, &window_times[run_start..j], typical_dt);
            run_start = j;
        }
    }
    push_rate_run(out, &window_times[run_start..], typical_dt);
}

fn push_rate_run(out: &mut Vec<(f64, f64)>, run: &[f64], typical_dt: f64) {
    let Some(&time_start) = run.first() else {
        return;
    };
    let Some(&time_end) = run.last() else {
        return;
    };
    let time_span = time_end - time_start;
    if time_span <= 0.0 || run.len() < 2 {
        return;
    }
    let max_span = typical_dt.max(1e-9) * run.len() as f64 * 20.0;
    if time_span > max_span {
        return;
    }
    let mid_time = (time_start + time_end) / 2.0;
    let rate = run.len() as f64 / time_span;
    out.push((mid_time, rate));
}

/// Calculate events per second over time
fn calculate_events_per_second<T: PeacoQCData>(
    fcs: &T,
    time_channel: &str,
    window_size: usize,
) -> Result<Vec<(f64, f64)>> {
    let time_values = fcs.get_channel_f64(time_channel)?;

    if time_values.is_empty() {
        return Err(PeacoQCError::InsufficientData { min: 1, actual: 0 });
    }

    Ok(event_rate_windows(&time_values, window_size))
}

/// Get channel data as vector
fn get_channel_data<T: PeacoQCData>(fcs: &T, channel: &str) -> Result<Vec<f64>> {
    fcs.get_channel_f64(channel)
}

/// Calculate median value per bin for a channel
fn calculate_median_per_bin(values: &[f64], events_per_bin: usize) -> Vec<(usize, f64)> {
    let mut medians = Vec::new();
    let n_bins = (values.len() + events_per_bin - 1) / events_per_bin;

    for bin_idx in 0..n_bins {
        let start = bin_idx * events_per_bin;
        let end = ((bin_idx + 1) * events_per_bin).min(values.len());

        if start < values.len() {
            let bin_values: Vec<f64> = values[start..end].to_vec();
            if !bin_values.is_empty() {
                let mut sorted = bin_values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let median = if sorted.len() % 2 == 0 {
                    (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
                } else {
                    sorted[sorted.len() / 2]
                };
                medians.push((bin_idx, median));
            }
        }
    }

    medians
}

/// Calculate grid dimensions for a given number of plots
/// Returns (n_rows, n_cols) that is relatively square and can fit all plots
fn calculate_grid_dimensions(n_plots: usize) -> (usize, usize) {
    if n_plots == 0 {
        return (1, 1);
    }

    // Start with a 1x1 grid
    let mut n_rows = 1;
    let mut n_cols = 1;

    // Alternate incrementing rows and cols until we have enough cells
    let mut increment_rows = true;
    while n_rows * n_cols < n_plots {
        if increment_rows {
            n_rows += 1;
        } else {
            n_cols += 1;
        }
        increment_rows = !increment_rows;
    }
    (n_rows, n_cols)
}

/// Priority for removal reason (higher = prefer when an event is in multiple bad bins)
fn removal_reason_priority(r: RemovalReason) -> u8 {
    match r {
        RemovalReason::IsolationTree => 3,
        RemovalReason::MAD => 2,
        RemovalReason::Consecutive => 1,
    }
}

/// Find unstable regions (ranges of cell indices where good_cells is false)
fn find_unstable_regions(good_cells: &[bool]) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut in_unstable = false;
    let mut start = 0;

    for (i, &is_good) in good_cells.iter().enumerate() {
        if !is_good {
            if !in_unstable {
                start = i;
                in_unstable = true;
            }
        } else {
            if in_unstable {
                regions.push((start, i));
                in_unstable = false;
            }
        }
    }

    // Handle case where unstable region extends to end
    if in_unstable {
        regions.push((start, good_cells.len()));
    }

    regions
}

/// Per-channel (or time overview) trend data for overlay rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelTrendSeries {
    pub channel: String,
    /// X coordinates (cell index for fluorescence; time for `__time__` overview).
    pub x: Vec<f64>,
    /// Per-bin median values (or event rate for time overview).
    pub median: Vec<f64>,
    /// Smoothed spline through bin medians (empty when fewer than 3 bins).
    pub spline: Vec<f64>,
    /// Horizontal lower MAD bound from smoothed trajectory (fluorescence only).
    pub mad_low: Option<f64>,
    /// Horizontal upper MAD bound from smoothed trajectory (fluorescence only).
    pub mad_high: Option<f64>,
    pub mad_threshold: f64,
}

/// Build median, spline, and ±MAD horizontal bounds for a fluorescence channel.
pub fn build_channel_trend_series(
    channel: &str,
    values: &[f64],
    events_per_bin: usize,
    mad_threshold: f64,
) -> ChannelTrendSeries {
    let medians = calculate_median_per_bin(values, events_per_bin);
    let x: Vec<f64> = medians
        .iter()
        .map(|(bin_idx, _)| (*bin_idx * events_per_bin) as f64)
        .collect();
    let median: Vec<f64> = medians.iter().map(|(_, m)| *m).collect();

    let mut spline = Vec::new();
    let mut mad_low = None;
    let mut mad_high = None;

    if medians.len() >= 3 {
        let bin_indices: Vec<f64> = medians.iter().map(|(i, _)| *i as f64).collect();
        let bin_medians: Vec<f64> = median.clone();
        if let Ok(smoothed) = crate::stats::spline::smooth_spline(&bin_indices, &bin_medians, 0.5)
        {
            spline = smoothed.clone();
            if let Ok((med, mad)) = crate::stats::median_mad::median_mad_scaled(&smoothed) {
                mad_low = Some(med - mad_threshold * mad);
                mad_high = Some(med + mad_threshold * mad);
            }
        }
    }

    ChannelTrendSeries {
        channel: channel.to_string(),
        x,
        median,
        spline,
        mad_low,
        mad_high,
        mad_threshold,
    }
}

/// Events-per-second rate series for the time overview (`channel == "__time__"`).
pub fn build_time_overview_series<T: PeacoQCData>(fcs: &T) -> Result<Option<ChannelTrendSeries>> {
    let Some(time_channel) = find_time_channel(fcs) else {
        return Ok(None);
    };
    let events_per_sec = calculate_events_per_second(fcs, &time_channel, 1000)?;
    if events_per_sec.is_empty() {
        return Ok(None);
    }
    Ok(Some(ChannelTrendSeries {
        channel: "__time__".to_string(),
        x: events_per_sec.iter().map(|(t, _)| *t).collect(),
        median: events_per_sec.iter().map(|(_, r)| *r).collect(),
        spline: Vec::new(),
        mad_low: None,
        mad_high: None,
        mad_threshold: 0.0,
    }))
}

/// Regions with reason: (start_cell, end_cell, reason). One entry per bad bin (overlapping bins may produce overlapping regions).
pub fn regions_by_reason(
    n_events: usize,
    events_per_bin: usize,
    removal_reason_per_bin: &[Option<RemovalReason>],
) -> Vec<(usize, usize, RemovalReason)> {
    let breaks = create_breaks(n_events, events_per_bin);
    let mut out = Vec::new();
    for (bin_idx, &reason_opt) in removal_reason_per_bin.iter().enumerate() {
        if let Some(reason) = reason_opt {
            if let Some(&(start, end)) = breaks.get(bin_idx) {
                out.push((start, end, reason));
            }
        }
    }
    out
}

/// Per-event primary removal reason (None = good event). For bad events in overlapping bins, uses highest-priority reason.
pub(crate) fn event_removal_reasons(
    n_events: usize,
    events_per_bin: usize,
    good_cells: &[bool],
    removal_reason_per_bin: &[Option<RemovalReason>],
) -> Vec<Option<RemovalReason>> {
    let breaks = create_breaks(n_events, events_per_bin);
    let mut event_reason: Vec<Option<RemovalReason>> = (0..n_events).map(|_| None).collect();
    for (bin_idx, &reason_opt) in removal_reason_per_bin.iter().enumerate() {
        if let Some(reason) = reason_opt {
            if let Some(&(start, end)) = breaks.get(bin_idx) {
                for i in start..end.min(n_events) {
                    if !good_cells.get(i).copied().unwrap_or(true) {
                        let replace = match event_reason[i] {
                            None => true,
                            Some(existing) => {
                                removal_reason_priority(reason) > removal_reason_priority(existing)
                            }
                        };
                        if replace {
                            event_reason[i] = Some(reason);
                        }
                    }
                }
            }
        }
    }
    event_reason
}

/// Create QC plots and save to file
///
/// # Arguments
/// * `fcs` - FCS data implementing PeacoQCData
/// * `qc_result` - Result from PeacoQC analysis
/// * `output_path` - Path to save the plot image
/// * `config` - Plot configuration
/// * `plot_index` - When `Some(i)`, render only the i-th plot (0 = time, 1..n = channels). Uses full canvas for single plot.
pub fn create_qc_plots<T: PeacoQCData>(
    fcs: &T,
    qc_result: &PeacoQCResult,
    output_path: impl AsRef<Path>,
    config: QCPlotConfig,
    plot_index: Option<usize>,
) -> Result<()> {
    let output_path = output_path.as_ref();

    // Find time channel
    let time_channel = find_time_channel(fcs)
        .ok_or_else(|| PeacoQCError::ConfigError("Time channel not found".to_string()))?;

    // Get channels to plot (those that were QC'd)
    let channels: Vec<String> = qc_result.peaks.keys().cloned().collect();

    if channels.is_empty() {
        return Err(PeacoQCError::ConfigError("No channels to plot".to_string()));
    }

    // When plot_index is set, render only that one plot using full canvas
    let (_n_plots, n_rows, n_cols) = match plot_index {
        Some(idx) => {
            let total = 1 + channels.len();
            if idx >= total {
                return Err(PeacoQCError::ConfigError(format!(
                    "plot_index {} out of range (0..{})",
                    idx, total
                )));
            }
            (1, 1, 1)
        }
        None => {
            let n = 1 + channels.len();
            let (r, c) = calculate_grid_dimensions(n);
            (n, r, c)
        }
    };

    let bg = config.background_color.unwrap_or(WHITE);
    let fg = config.foreground_color.unwrap_or(BLACK);
    let font_family = config.font_family.as_deref().unwrap_or("sans-serif");

    // Create drawing area
    let root = BitMapBackend::new(output_path, (config.width, config.height)).into_drawing_area();
    root.fill(&bg)
        .map_err(|e| PeacoQCError::ExportError(format!("Failed to fill background: {:?}", e)))?;

    // Split root into subplot areas
    let subplot_areas = root.split_evenly((n_rows, n_cols));

    // Plot 1: Time vs events/second
    let draw_time = plot_index.map_or(true, |i| i == 0);
    if draw_time {
        let events_per_sec = calculate_events_per_second(fcs, &time_channel, 1000)?;

        if !events_per_sec.is_empty() {
            let x_range = events_per_sec
                .iter()
                .map(|(t, _)| *t)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), x| {
                    (min.min(x), max.max(x))
                });

            let y_range = events_per_sec
                .iter()
                .map(|(_, r)| *r)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), x| {
                    (min.min(x), max.max(x))
                });

            let x_range = if x_range.0 == x_range.1 {
                (x_range.0 - 1.0)..(x_range.1 + 1.0)
            } else {
                x_range.0..x_range.1
            };

            // Enforce minimum y-scale maximum of 1 event/sec; rates below that show as "low"
            let actual_y_max = y_range.1;
            let (y_range, y_max_is_low) = if actual_y_max < 1.0 {
                (0.0..1.0, true)
            } else {
                let yr = if y_range.0 == y_range.1 {
                    (y_range.0 - 1.0)..(y_range.1 + 1.0)
                } else {
                    y_range.0..y_range.1
                };
                (yr, false)
            };

            let subplot_area = &subplot_areas[0];

            // Create title with percentage removed
            let title_text = format!(
                "{:.3}% of the data was removed",
                qc_result.percentage_removed
            );

            let y_range_clone = y_range.clone();
            let mut chart = ChartBuilder::on(&subplot_area)
                .margin(12)
                .caption(
                    title_text,
                    (font_family, config.caption_font_size)
                        .into_font()
                        .color(&fg),
                )
                .x_label_area_size(58)
                .y_label_area_size(82)
                .build_cartesian_2d(x_range.clone(), y_range_clone)
                .map_err(|e| {
                    PeacoQCError::ExportError(format!("Failed to build chart: {:?}", e))
                })?;

            let draw_result = if y_max_is_low {
                chart
                    .configure_mesh()
                    .axis_desc_style((font_family, config.axis_label_size).into_font().color(&fg))
                    .label_style((font_family, config.tick_label_size).into_font().color(&fg))
                    .light_line_style(GRID_LINE_COLOR.stroke_width(1))
                    .x_desc("Time")
                    .y_desc("Nº of cells per second")
                    .x_label_formatter(&|v: &f64| format!("{:>8.1}", v))
                    .y_label_formatter(&|v: &f64| {
                        if *v >= 0.99 {
                            " low".to_string()
                        } else {
                            format!("{:>6.2}", v)
                        }
                    })
                    .draw()
            } else {
                chart
                    .configure_mesh()
                    .axis_desc_style((font_family, config.axis_label_size).into_font().color(&fg))
                    .label_style((font_family, config.tick_label_size).into_font().color(&fg))
                    .light_line_style(GRID_LINE_COLOR.stroke_width(1))
                    .x_desc("Time")
                    .y_desc("Nº of cells per second")
                    .x_label_formatter(&|v: &f64| format!("{:>8.1}", v))
                    .y_label_formatter(&|v: &f64| format!("{:>6.2}", v))
                    .draw()
            };
            draw_result
                .map_err(|e| PeacoQCError::ExportError(format!("Failed to draw mesh: {:?}", e)))?;

            // Highlight unstable regions on time plot (by reason when available)
            let time_values = get_channel_data(fcs, &time_channel)?;
            let n_events = qc_result.good_cells.len();
            let mut reasons_in_legend = std::collections::HashSet::new();

            if let Some(ref reasons) = qc_result.removal_reason_per_bin {
                if reasons.len() == qc_result.n_bins {
                    let regions = regions_by_reason(n_events, qc_result.events_per_bin, reasons);
                    for (start_idx, end_idx, reason) in regions {
                        if start_idx < time_values.len() && end_idx > 0 {
                            let start_time = time_values[start_idx.min(time_values.len() - 1)];
                            let end_time = time_values[(end_idx - 1).min(time_values.len() - 1)];
                            let color =
                                color_for_removal_reason(&config, reason, config.unstable_color);
                            let fill_color = RGBAColor(color.0, color.1, color.2, 0.3);
                            chart
                                .draw_series(std::iter::once(Rectangle::new(
                                    [(start_time, y_range.start), (end_time, y_range.end)],
                                    fill_color.filled(),
                                )))
                                .map_err(|e| {
                                    PeacoQCError::ExportError(format!(
                                        "Failed to draw rectangle: {:?}",
                                        e
                                    ))
                                })?;
                            reasons_in_legend.insert(reason);
                        }
                    }
                }
            }
            if reasons_in_legend.is_empty() {
                // Fallback: single color for all removed regions
                let unstable_regions = find_unstable_regions(&qc_result.good_cells);
                for (start_idx, end_idx) in unstable_regions {
                    if start_idx < time_values.len() && end_idx <= time_values.len() {
                        let start_time = time_values[start_idx];
                        let end_time = time_values[(end_idx - 1).min(time_values.len() - 1)];
                        let fill_color = RGBAColor(
                            config.unstable_color.0,
                            config.unstable_color.1,
                            config.unstable_color.2,
                            0.3,
                        );
                        chart
                            .draw_series(std::iter::once(Rectangle::new(
                                [(start_time, y_range.start), (end_time, y_range.end)],
                                fill_color.filled(),
                            )))
                            .map_err(|e| {
                                PeacoQCError::ExportError(format!(
                                    "Failed to draw rectangle: {:?}",
                                    e
                                ))
                            })?;
                    }
                }
            }

            // Draw events per second line
            chart
                .draw_series(LineSeries::new(
                    events_per_sec.iter().map(|(t, r)| (*t, *r)),
                    fg.stroke_width(2),
                ))
                .map_err(|e| {
                    PeacoQCError::ExportError(format!("Failed to draw line series: {:?}", e))
                })?;

            // Legend: Removed events (by reason when available); use legible layout
            let x_range_size = x_range.end - x_range.start;
            let y_range_size = y_range.end - y_range.start;
            const LEGEND_MARGIN_RIGHT_PCT: f64 = 0.22;
            const LEGEND_TEXT_WIDTH_PCT: f64 = 0.18;
            const LEGEND_ROW_HEIGHT_PCT: f64 = 0.055;
            const LEGEND_RECT_W_PCT: f64 = 0.025;
            const LEGEND_RECT_H_PCT: f64 = 0.04;
            const LEGEND_TEXT_GAP_PCT: f64 = 0.01;
            const LEGEND_PAD_PCT: f64 = 0.01;

            let legend_x_start = x_range.end - (x_range_size * LEGEND_MARGIN_RIGHT_PCT);
            let rect_w = x_range_size * LEGEND_RECT_W_PCT;
            let rect_h = y_range_size * LEGEND_RECT_H_PCT;
            let text_gap = x_range_size * LEGEND_TEXT_GAP_PCT;
            let pad_x = x_range_size * LEGEND_PAD_PCT;
            let pad_y = y_range_size * LEGEND_PAD_PCT;
            let legend_y_step = y_range_size * LEGEND_ROW_HEIGHT_PCT;

            let legend_labels: Vec<(&str, RGBColor)> = if reasons_in_legend.is_empty() {
                vec![("Removed events", config.unstable_color)]
            } else {
                let mut labels = Vec::new();
                if reasons_in_legend.contains(&RemovalReason::IsolationTree) {
                    labels.push((
                        "Removed (Isolation Tree)",
                        color_for_removal_reason(
                            &config,
                            RemovalReason::IsolationTree,
                            config.unstable_color,
                        ),
                    ));
                }
                if reasons_in_legend.contains(&RemovalReason::MAD) {
                    labels.push((
                        "Removed (MAD)",
                        color_for_removal_reason(
                            &config,
                            RemovalReason::MAD,
                            config.unstable_color,
                        ),
                    ));
                }
                if reasons_in_legend.contains(&RemovalReason::Consecutive) {
                    labels.push((
                        "Removed (Consecutive)",
                        color_for_removal_reason(
                            &config,
                            RemovalReason::Consecutive,
                            config.unstable_color,
                        ),
                    ));
                }
                labels
            };

            let n_rows = legend_labels.len();
            let legend_y_start = y_range.end - (y_range_size * 0.02);
            let legend_bg_left = legend_x_start - pad_x;
            let legend_bg_bottom =
                legend_y_start - rect_h - (n_rows.saturating_sub(1) as f64 * legend_y_step) - pad_y;
            let legend_bg_right =
                legend_x_start + rect_w + text_gap + (x_range_size * LEGEND_TEXT_WIDTH_PCT) + pad_x;
            let legend_bg_top = legend_y_start + pad_y;
            chart
                .draw_series(std::iter::once(Rectangle::new(
                    [
                        (legend_bg_left, legend_bg_bottom),
                        (legend_bg_right, legend_bg_top),
                    ],
                    LEGEND_BG_COLOR.filled(),
                )))
                .map_err(|e| {
                    PeacoQCError::ExportError(format!("Failed to draw legend background: {:?}", e))
                })?;

            let mut legend_y = legend_y_start;
            for (label, color) in &legend_labels {
                let fill_color = RGBAColor(color.0, color.1, color.2, 0.5);
                chart
                    .draw_series(std::iter::once(Rectangle::new(
                        [
                            (legend_x_start, legend_y - rect_h),
                            (legend_x_start + rect_w, legend_y),
                        ],
                        fill_color.filled(),
                    )))
                    .map_err(|e| {
                        PeacoQCError::ExportError(format!("Failed to draw legend rect: {:?}", e))
                    })?;
                chart
                    .plotting_area()
                    .draw(&Text::new(
                        (*label).to_string(),
                        (legend_x_start + rect_w + text_gap, legend_y),
                        (font_family, config.legend_font_size)
                            .into_font()
                            .color(&fg),
                    ))
                    .map_err(|e| {
                        PeacoQCError::ExportError(format!("Failed to draw legend text: {:?}", e))
                    })?;
                legend_y -= legend_y_step;
            }
        }
    }

    // Plot channels: Signal value vs cell event
    let total_cells = n_rows * n_cols;
    let channel_iter: Box<dyn Iterator<Item = (usize, &String)>> = match plot_index {
        Some(i) if i >= 1 && i <= channels.len() => {
            Box::new(std::iter::once((i - 1, &channels[i - 1])))
        }
        Some(_) => Box::new(std::iter::empty()),
        None => Box::new(channels.iter().enumerate()),
    };
    for (plot_idx, channel) in channel_iter {
        let subplot_idx = if plot_index.is_some() {
            0 // Single-plot mode: use first (only) cell
        } else {
            plot_idx + 1 // +1 because first plot is time plot
        };

        if subplot_idx >= total_cells {
            break;
        }

        let channel_data = get_channel_data(fcs, channel)?;
        if channel_data.is_empty() {
            continue;
        }

        let n_events = channel_data.len();
        let cell_indices: Vec<f64> = (0..n_events).map(|i| i as f64).collect();

        // Calculate ranges
        let x_range = 0.0..(n_events as f64);
        let y_min = channel_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let y_max = channel_data
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let y_range = if y_min == y_max {
            (y_min - 1.0)..(y_max + 1.0)
        } else {
            y_min..y_max
        };

        let subplot_area = &subplot_areas[subplot_idx];

        // Use per-channel MAD contribution for title
        let mad_pct = qc_result
            .mad_contribution
            .get(channel)
            .copied()
            .unwrap_or(0.0);

        let title = if mad_pct > 0.0 {
            format!("{} MAD {:.2}%", channel, mad_pct)
        } else {
            channel.to_string()
        };

        let mut chart = ChartBuilder::on(&subplot_area)
            .margin(12)
            .caption(
                &title,
                (font_family, config.caption_font_size)
                    .into_font()
                    .color(&fg),
            )
            .x_label_area_size(48)
            .y_label_area_size(82)
            .build_cartesian_2d(x_range.clone(), y_range.clone())
            .map_err(|e| PeacoQCError::ExportError(format!("Failed to build chart: {:?}", e)))?;

        chart
            .configure_mesh()
            .axis_desc_style((font_family, config.axis_label_size).into_font().color(&fg))
            .label_style((font_family, config.tick_label_size).into_font().color(&fg))
            .light_line_style(GRID_LINE_COLOR.stroke_width(1))
            .x_desc("Cell index")
            .y_desc("Signal (a.u.)")
            .x_label_formatter(&|v: &f64| format!("{:>8.0}", v))
            .y_label_formatter(&|v: &f64| format!("{:>8.2}", v))
            .draw()
            .map_err(|e| PeacoQCError::ExportError(format!("Failed to draw mesh: {:?}", e)))?;

        // Highlight unstable regions (by reason when available)
        let has_reason_data = qc_result
            .removal_reason_per_bin
            .as_ref()
            .map(|r| r.len() == qc_result.n_bins)
            .unwrap_or(false);
        if has_reason_data {
            let reasons = qc_result.removal_reason_per_bin.as_ref().unwrap();
            let regions = regions_by_reason(n_events, qc_result.events_per_bin, reasons);
            for (start_idx, end_idx, reason) in regions {
                if start_idx < n_events {
                    let start_cell = start_idx as f64;
                    let end_cell = (end_idx.min(n_events)) as f64;
                    let color = color_for_removal_reason(&config, reason, config.unstable_color);
                    let fill_color = RGBAColor(color.0, color.1, color.2, 0.3);
                    chart
                        .draw_series(std::iter::once(Rectangle::new(
                            [(start_cell, y_range.start), (end_cell, y_range.end)],
                            fill_color.filled(),
                        )))
                        .map_err(|e| {
                            PeacoQCError::ExportError(format!("Failed to draw rectangle: {:?}", e))
                        })?;
                }
            }
        } else {
            let unstable_regions = find_unstable_regions(&qc_result.good_cells);
            for (start_idx, end_idx) in unstable_regions {
                if start_idx < n_events {
                    let start_cell = start_idx as f64;
                    let end_cell = (end_idx.min(n_events)) as f64;
                    let fill_color = RGBAColor(
                        config.unstable_color.0,
                        config.unstable_color.1,
                        config.unstable_color.2,
                        0.3,
                    );
                    chart
                        .draw_series(std::iter::once(Rectangle::new(
                            [(start_cell, y_range.start), (end_cell, y_range.end)],
                            fill_color.filled(),
                        )))
                        .map_err(|e| {
                            PeacoQCError::ExportError(format!("Failed to draw rectangle: {:?}", e))
                        })?;
                }
            }
        }

        // Draw scatter: good (grey); bad by reason when available, else single bad_color
        let sample_size = 10000.min(n_events);
        let step = (n_events / sample_size.max(1)).max(1);
        let mut good_points = Vec::new();
        let mut bad_by_reason: std::collections::HashMap<RemovalReason, Vec<(f64, f64)>> =
            std::collections::HashMap::new();
        let mut bad_points_fallback = Vec::new();

        let event_reasons = if has_reason_data {
            Some(event_removal_reasons(
                n_events,
                qc_result.events_per_bin,
                &qc_result.good_cells,
                qc_result.removal_reason_per_bin.as_ref().unwrap(),
            ))
        } else {
            None
        };

        for i in (0..n_events).step_by(step) {
            let pt = (cell_indices[i], channel_data[i]);
            if qc_result.good_cells[i] {
                good_points.push(pt);
            } else if let Some(ref reasons) = event_reasons {
                if let Some(Some(reason)) = reasons.get(i).copied() {
                    bad_by_reason.entry(reason).or_default().push(pt);
                } else {
                    bad_points_fallback.push(pt);
                }
            } else {
                bad_points_fallback.push(pt);
            }
        }

        let alpha: f64 = config.scatter_alpha.unwrap_or(1.0) as f64;
        let alpha = alpha.clamp(0.0, 1.0);
        let use_alpha = alpha < 1.0;

        for (reason, points) in &bad_by_reason {
            if !points.is_empty() {
                let color = color_for_removal_reason(&config, *reason, config.bad_color);
                chart
                    .draw_series(points.iter().map(|(x, y)| {
                        Circle::new(
                            (*x, *y),
                            1,
                            if use_alpha {
                                RGBAColor(color.0, color.1, color.2, alpha).filled()
                            } else {
                                color.filled()
                            },
                        )
                    }))
                    .map_err(|e| {
                        PeacoQCError::ExportError(format!(
                            "Failed to draw bad-event circles: {:?}",
                            e
                        ))
                    })?;
            }
        }
        if !bad_points_fallback.is_empty() {
            chart
                .draw_series(bad_points_fallback.iter().map(|(x, y)| {
                    Circle::new(
                        (*x, *y),
                        1,
                        if use_alpha {
                            RGBAColor(
                                config.bad_color.0,
                                config.bad_color.1,
                                config.bad_color.2,
                                alpha,
                            )
                            .filled()
                        } else {
                            config.bad_color.filled()
                        },
                    )
                }))
                .map_err(|e| {
                    PeacoQCError::ExportError(format!("Failed to draw bad-event circles: {:?}", e))
                })?;
        }
        if !good_points.is_empty() {
            chart
                .draw_series(good_points.iter().map(|(x, y)| {
                    Circle::new(
                        (*x, *y),
                        1,
                        if use_alpha {
                            RGBAColor(
                                config.good_color.0,
                                config.good_color.1,
                                config.good_color.2,
                                alpha,
                            )
                            .filled()
                        } else {
                            config.good_color.filled()
                        },
                    )
                }))
                .map_err(|e| {
                    PeacoQCError::ExportError(format!("Failed to draw circles: {:?}", e))
                })?;
        }

        // Draw median line per bin and optional spline / MAD bounds
        let trend = build_channel_trend_series(
            channel,
            &channel_data,
            qc_result.events_per_bin,
            6.0,
        );
        if !trend.median.is_empty() {
            let median_points: Vec<(f64, f64)> = trend
                .x
                .iter()
                .zip(trend.median.iter())
                .map(|(&x, &y)| (x, y))
                .collect();

            chart
                .draw_series(LineSeries::new(
                    median_points.clone(),
                    config.median_color.stroke_width(2),
                ))
                .map_err(|e| {
                    PeacoQCError::ExportError(format!("Failed to draw median line: {:?}", e))
                })?;

            // Draw bin boundaries (if enabled)
            if config.show_bin_boundaries {
                let n_bins = (n_events + qc_result.events_per_bin - 1) / qc_result.events_per_bin;
                let boundary_color = RGBColor(200, 200, 200);
                for bin_idx in 0..=n_bins {
                    let cell_idx = (bin_idx * qc_result.events_per_bin) as f64;
                    if cell_idx <= n_events as f64 {
                        chart
                            .draw_series(std::iter::once(plotters::prelude::PathElement::new(
                                vec![(cell_idx, y_range.start), (cell_idx, y_range.end)],
                                boundary_color.stroke_width(1),
                            )))
                            .map_err(|e| {
                                PeacoQCError::ExportError(format!(
                                    "Failed to draw bin boundary: {:?}",
                                    e
                                ))
                            })?;
                    }
                }
            }

            // Draw smoothed spline and MAD threshold lines (if enabled)
            if config.show_spline_and_mad && trend.spline.len() >= 3 {
                let smoothed_points: Vec<(f64, f64)> = trend
                    .spline
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| ((i * qc_result.events_per_bin) as f64, y))
                    .collect();

                chart
                    .draw_series(LineSeries::new(
                        smoothed_points.clone(),
                        config.smoothed_spline_color.stroke_width(2),
                    ))
                    .map_err(|e| {
                        PeacoQCError::ExportError(format!(
                            "Failed to draw smoothed spline: {:?}",
                            e
                        ))
                    })?;

                if let (Some(upper_threshold), Some(lower_threshold)) =
                    (trend.mad_high, trend.mad_low)
                {
                    let threshold_points_upper: Vec<(f64, f64)> =
                        vec![(0.0, upper_threshold), (n_events as f64, upper_threshold)];
                    let threshold_points_lower: Vec<(f64, f64)> =
                        vec![(0.0, lower_threshold), (n_events as f64, lower_threshold)];
                    let mad_style = config.mad_threshold_color.stroke_width(2);

                    chart
                        .draw_series(std::iter::once(
                            plotters::element::DashedPathElement::new(
                                threshold_points_upper,
                                MAD_DASH_LEN,
                                MAD_DASH_GAP,
                                mad_style.clone(),
                            ),
                        ))
                        .map_err(|e| {
                            PeacoQCError::ExportError(format!(
                                "Failed to draw upper threshold: {:?}",
                                e
                            ))
                        })?;

                    chart
                        .draw_series(std::iter::once(
                            plotters::element::DashedPathElement::new(
                                threshold_points_lower,
                                MAD_DASH_LEN,
                                MAD_DASH_GAP,
                                mad_style,
                            ),
                        ))
                        .map_err(|e| {
                            PeacoQCError::ExportError(format!(
                                "Failed to draw lower threshold: {:?}",
                                e
                            ))
                        })?;
                }
            }

            // Draw legend: removal reasons (when available) then line items; legible row spacing and width
            let legend_rects: Vec<(&str, RGBColor)> = if has_reason_data {
                let mut rects = Vec::new();
                if bad_by_reason.contains_key(&RemovalReason::IsolationTree) {
                    rects.push((
                        "Removed (Isolation Tree)",
                        color_for_removal_reason(
                            &config,
                            RemovalReason::IsolationTree,
                            config.unstable_color,
                        ),
                    ));
                }
                if bad_by_reason.contains_key(&RemovalReason::MAD) {
                    rects.push((
                        "Removed (MAD)",
                        color_for_removal_reason(
                            &config,
                            RemovalReason::MAD,
                            config.unstable_color,
                        ),
                    ));
                }
                if bad_by_reason.contains_key(&RemovalReason::Consecutive) {
                    rects.push((
                        "Removed (Consecutive)",
                        color_for_removal_reason(
                            &config,
                            RemovalReason::Consecutive,
                            config.unstable_color,
                        ),
                    ));
                }
                if rects.is_empty() {
                    vec![("Removed events", config.unstable_color)]
                } else {
                    rects
                }
            } else {
                vec![("Removed events", config.unstable_color)]
            };
            let mut legend_items: Vec<(&str, RGBColor, u32)> =
                vec![("Median", config.median_color, 2)];

            if config.show_spline_and_mad {
                legend_items.push(("Spline", config.smoothed_spline_color, 2));
                legend_items.push(("MAD ±6", config.mad_threshold_color, 2));
            }

            let x_range_size = x_range.end - x_range.start;
            let y_range_size = y_range.end - y_range.start;
            const CHAN_LEGEND_MARGIN_RIGHT_PCT: f64 = 0.10;
            const CHAN_LEGEND_ROW_HEIGHT_PCT: f64 = 0.050;
            const CHAN_LEGEND_TEXT_WIDTH_PCT: f64 = 0.18;
            let legend_margin_right_pct = CHAN_LEGEND_MARGIN_RIGHT_PCT;
            let legend_margin_top_pct = 0.02;
            let legend_x_start = x_range.end - (x_range_size * legend_margin_right_pct);
            let legend_y_step = y_range_size * CHAN_LEGEND_ROW_HEIGHT_PCT;
            let line_length = x_range_size * 0.035;
            let text_gap = x_range_size * 0.01;
            let rect_w = x_range_size * 0.022;
            let rect_h = y_range_size * 0.028;
            let legend_initial_y = y_range.end - (y_range_size * legend_margin_top_pct);
            let n_legend_rows = legend_rects.len() + legend_items.len();
            let pad_x = x_range_size * 0.008;
            let pad_y = y_range_size * 0.008;
            let legend_bg_left = legend_x_start - pad_x;
            let legend_bg_right = legend_x_start
                + line_length
                + text_gap
                + (x_range_size * CHAN_LEGEND_TEXT_WIDTH_PCT)
                + pad_x;
            let legend_bg_bottom = legend_initial_y
                - rect_h
                - (n_legend_rows.saturating_sub(1) as f64 * legend_y_step)
                - pad_y;
            let legend_bg_top = legend_initial_y + pad_y;
            chart
                .draw_series(std::iter::once(Rectangle::new(
                    [
                        (legend_bg_left, legend_bg_bottom),
                        (legend_bg_right, legend_bg_top),
                    ],
                    LEGEND_BG_COLOR.filled(),
                )))
                .map_err(|e| {
                    PeacoQCError::ExportError(format!("Failed to draw legend background: {:?}", e))
                })?;

            let mut legend_y = legend_initial_y;

            for (label, color) in &legend_rects {
                let fill_color = RGBAColor(color.0, color.1, color.2, 0.5);
                chart
                    .draw_series(std::iter::once(Rectangle::new(
                        [
                            (legend_x_start, legend_y - rect_h),
                            (legend_x_start + rect_w, legend_y),
                        ],
                        fill_color.filled(),
                    )))
                    .map_err(|e| {
                        PeacoQCError::ExportError(format!("Failed to draw legend rect: {:?}", e))
                    })?;
                chart
                    .plotting_area()
                    .draw(&Text::new(
                        (*label).to_string(),
                        (legend_x_start + rect_w + text_gap, legend_y),
                        (font_family, config.legend_font_size)
                            .into_font()
                            .color(&fg),
                    ))
                    .map_err(|e| {
                        PeacoQCError::ExportError(format!("Failed to draw legend text: {:?}", e))
                    })?;
                legend_y -= legend_y_step;
            }

            for (label, color, stroke_width) in &legend_items {
                let line_pts = vec![
                    (legend_x_start, legend_y),
                    (legend_x_start + line_length, legend_y),
                ];
                let stroke = color.stroke_width(*stroke_width);
                if *label == "MAD ±6" {
                    chart
                        .draw_series(std::iter::once(plotters::element::DashedPathElement::new(
                            line_pts,
                            MAD_DASH_LEN,
                            MAD_DASH_GAP,
                            stroke,
                        )))
                        .map_err(|e| {
                            PeacoQCError::ExportError(format!(
                                "Failed to draw legend line: {:?}",
                                e
                            ))
                        })?;
                } else {
                    chart
                        .draw_series(std::iter::once(plotters::prelude::PathElement::new(
                            line_pts, stroke,
                        )))
                        .map_err(|e| {
                            PeacoQCError::ExportError(format!(
                                "Failed to draw legend line: {:?}",
                                e
                            ))
                        })?;
                }
                chart
                    .plotting_area()
                    .draw(&Text::new(
                        label.to_string(),
                        (legend_x_start + line_length + text_gap, legend_y),
                        (font_family, config.legend_font_size)
                            .into_font()
                            .color(&fg),
                    ))
                    .map_err(|e| {
                        PeacoQCError::ExportError(format!("Failed to draw legend text: {:?}", e))
                    })?;
                legend_y -= legend_y_step;
            }
        }
    }

    root.present()
        .map_err(|e| PeacoQCError::ExportError(format!("Failed to present plot: {:?}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_unstable_regions() {
        let good_cells = vec![false, false, true, true, true, false, true, true];
        let regions = find_unstable_regions(&good_cells);
        assert_eq!(regions, vec![(0, 2), (5, 6)]);
    }

    #[test]
    fn test_calculate_median_per_bin() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let medians = calculate_median_per_bin(&values, 2);
        assert_eq!(medians.len(), 4);
        assert_eq!(medians[0], (0, 1.5));
        assert_eq!(medians[1], (1, 3.5));
    }

    #[test]
    fn event_rate_windows_splits_wrap_inside_window() {
        let mut time = Vec::new();
        for i in 0..800 {
            time.push(10_000.0 + i as f64);
        }
        for i in 0..200 {
            time.push(i as f64);
        }
        let windows = event_rate_windows(&time, 1000);
        assert_eq!(windows.len(), 2);
        assert!(windows.iter().all(|&(_, rate)| rate > 0.0));
        assert!(
            windows[0].0 > 10_000.0,
            "pre-wrap run stays on the high Time side: {windows:?}"
        );
        assert!(
            windows[1].0 < 200.0,
            "post-wrap run stays on the low Time side: {windows:?}"
        );
    }

    #[test]
    fn event_rate_windows_drops_huge_span_wrap_tail() {
        let mut time = Vec::new();
        for i in 0..5_000 {
            time.push(i as f64 * 1.3);
        }
        for i in 0..1_000 {
            time.push(140_000.0 + i as f64 * 160.0);
        }
        let windows = event_rate_windows(&time, 1000);
        assert!(
            windows.iter().all(|&(_, rate)| rate > 0.1),
            "near-zero wrap-tail rates must be dropped: {windows:?}"
        );
        assert!(
            windows.iter().all(|&(t, _)| t < 50_000.0),
            "wrap-tail midpoints must not overlap the main axis: {windows:?}"
        );
        assert!(!windows.is_empty());
    }

    #[test]
    fn test_build_channel_trend_series_uses_mad_threshold() {
        let values: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let trend = build_channel_trend_series("FL1-A", &values, 5, 3.0);
        assert_eq!(trend.channel, "FL1-A");
        assert_eq!(trend.median.len(), 4);
        assert_eq!(trend.mad_threshold, 3.0);
        if trend.spline.len() >= 3 {
            assert!(trend.mad_low.is_some());
            assert!(trend.mad_high.is_some());
            let low = trend.mad_low.expect("mad_low");
            let high = trend.mad_high.expect("mad_high");
            assert!(high > low);
        }
    }

    #[test]
    fn test_regions_by_reason() {
        let n_events = 10_usize;
        let events_per_bin = 4_usize;
        // create_breaks(10, 4): overlap=2, step=2 -> (0,4), (2,6), (4,8), (6,10) = 4 bins
        let reasons = vec![
            None,
            Some(RemovalReason::MAD),
            None,
            Some(RemovalReason::Consecutive),
        ];
        let regions = regions_by_reason(n_events, events_per_bin, &reasons);
        assert_eq!(regions.len(), 2);
        assert!(regions.contains(&(2, 6, RemovalReason::MAD)));
        assert!(regions.contains(&(6, 10, RemovalReason::Consecutive)));
    }

    #[test]
    fn test_event_removal_reasons() {
        let n_events = 10_usize;
        let events_per_bin = 4_usize;
        let mut good_cells = vec![true; n_events];
        good_cells[3] = false;
        good_cells[7] = false;
        let reasons = vec![
            None,
            Some(RemovalReason::MAD),
            None,
            Some(RemovalReason::Consecutive),
        ];
        let event_reasons = event_removal_reasons(n_events, events_per_bin, &good_cells, &reasons);
        assert_eq!(event_reasons.len(), n_events);
        assert_eq!(event_reasons[3], Some(RemovalReason::MAD));
        assert_eq!(event_reasons[7], Some(RemovalReason::Consecutive));
        for (i, &r) in event_reasons.iter().enumerate() {
            if i != 3 && i != 7 {
                assert_eq!(r, None, "event {} should have no reason", i);
            }
        }
    }

    #[test]
    fn test_calculate_grid_dimensions() {
        // Test various plot counts
        assert_eq!(calculate_grid_dimensions(1), (1, 1));
        assert_eq!(calculate_grid_dimensions(4), (2, 2));
        assert!(calculate_grid_dimensions(5) == (3, 2) || calculate_grid_dimensions(5) == (2, 3)); // or (2, 3) - alternates
        assert_eq!(calculate_grid_dimensions(9), (3, 3));
        assert_eq!(calculate_grid_dimensions(25), (5, 5));
        assert!(calculate_grid_dimensions(30) == (6, 5) || calculate_grid_dimensions(30) == (5, 6)); // or (5, 6) - alternates
        assert_eq!(calculate_grid_dimensions(36), (6, 6));

        // Verify the grid can fit all plots
        let (rows, cols) = calculate_grid_dimensions(25);
        assert!(rows * cols >= 25);
        assert_eq!(rows, 5);
        assert_eq!(cols, 5);

        let (rows, cols) = calculate_grid_dimensions(30);
        assert!(rows * cols >= 30);

        let (rows, cols) = calculate_grid_dimensions(24);
        assert!(rows * cols >= 24);
    }
}
