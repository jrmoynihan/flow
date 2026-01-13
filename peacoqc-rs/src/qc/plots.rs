//! QC plotting module for PeacoQC
//!
//! This module provides functionality to create QC plots similar to the R PeacoQC package.
//! It generates:
//! - Time vs events/second plot
//! - Signal value vs cell event plots for each QC'd channel with highlighted unstable regions

use crate::PeacoQCData;
use crate::error::{PeacoQCError, Result};
use crate::qc::peacoqc::PeacoQCResult;
use plotters::prelude::*;
use plotters::style::{BLACK, RGBAColor, RGBColor, WHITE};
use std::path::Path;

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

    /// Color for unstable regions (RGBA)
    pub unstable_color: RGBColor,

    /// Color for good data points
    pub good_color: RGBColor,

    /// Color for median line
    pub median_color: RGBColor,
}

impl Default for QCPlotConfig {
    fn default() -> Self {
        Self {
            width: 2400,
            height: 1800,
            n_cols: 4,
            n_rows: 6,
            unstable_color: RGBColor(200, 150, 255), // Light purple
            good_color: RGBColor(128, 128, 128),     // Grey
            median_color: RGBColor(0, 0, 0),         // Black
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

    let mut events_per_second = Vec::new();
    let mut i = 0;

    while i < time_values.len() {
        let window_end = (i + window_size).min(time_values.len());
        if window_end <= i {
            break;
        }

        let window_times: Vec<f64> = time_values[i..window_end].to_vec();
        let time_start = window_times.first().copied().unwrap_or(0.0);
        let time_end = window_times.last().copied().unwrap_or(time_start);
        let time_span = time_end - time_start;

        let rate = if time_span > 0.0 {
            (window_end - i) as f64 / time_span
        } else {
            0.0
        };

        // Use middle of window as x position
        let mid_time = (time_start + time_end) / 2.0;
        events_per_second.push((mid_time, rate));

        i = window_end;
    }

    Ok(events_per_second)
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
    println!("plots: {}, n_rows: {}, n_cols: {}", n_plots, n_rows, n_cols);
    (n_rows, n_cols)
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

/// Create QC plots and save to file
///
/// # Arguments
/// * `fcs` - FCS data implementing PeacoQCData
/// * `qc_result` - Result from PeacoQC analysis
/// * `output_path` - Path to save the plot image
/// * `config` - Plot configuration
pub fn create_qc_plots<T: PeacoQCData>(
    fcs: &T,
    qc_result: &PeacoQCResult,
    output_path: impl AsRef<Path>,
    config: QCPlotConfig,
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

    // Calculate total number of plots needed (1 time plot + N channel plots)
    let n_plots = 1 + channels.len();

    // Calculate grid dimensions dynamically based on number of plots
    let (n_rows, n_cols) = calculate_grid_dimensions(n_plots);

    // Create drawing area
    let root = BitMapBackend::new(output_path, (config.width, config.height)).into_drawing_area();
    root.fill(&WHITE)
        .map_err(|e| PeacoQCError::PlotError(format!("Failed to fill background: {:?}", e)))?;

    // Split root into subplot areas
    let subplot_areas = root.split_evenly((n_rows, n_cols));

    // Plot 1: Time vs events/second
    {
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

            let y_range = if y_range.0 == y_range.1 {
                (y_range.0 - 1.0)..(y_range.1 + 1.0)
            } else {
                y_range.0..y_range.1
            };

            let subplot_area = &subplot_areas[0];

            let y_range_clone = y_range.clone();
            let mut chart = ChartBuilder::on(&subplot_area)
                .margin(5)
                .x_label_area_size(40)
                .y_label_area_size(50)
                .build_cartesian_2d(x_range, y_range_clone)
                .map_err(|e| PeacoQCError::PlotError(format!("Failed to build chart: {:?}", e)))?;

            chart
                .configure_mesh()
                .x_desc("Time")
                .y_desc("Nr of cells per second")
                .draw()
                .map_err(|e| PeacoQCError::PlotError(format!("Failed to draw mesh: {:?}", e)))?;

            // Highlight unstable regions on time plot
            let unstable_regions = find_unstable_regions(&qc_result.good_cells);
            let time_values = get_channel_data(fcs, &time_channel)?;

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
                            PeacoQCError::PlotError(format!("Failed to draw rectangle: {:?}", e))
                        })?;
                }
            }

            // Draw events per second line
            chart
                .draw_series(LineSeries::new(
                    events_per_sec.iter().map(|(t, r)| (*t, *r)),
                    BLACK.stroke_width(2),
                ))
                .map_err(|e| {
                    PeacoQCError::PlotError(format!("Failed to draw line series: {:?}", e))
                })?;

            // Add percentage removed text
            let text = format!(
                "{:.3}% of the data was removed.",
                qc_result.percentage_removed
            );
            chart
                .plotting_area()
                .draw(&Text::new(
                    text,
                    (5.0, 5.0),
                    ("sans-serif", 14).into_font().color(&BLACK),
                ))
                .map_err(|e| PeacoQCError::PlotError(format!("Failed to draw text: {:?}", e)))?;
        }
    }

    // Plot channels: Signal value vs cell event
    let total_cells = n_rows * n_cols;
    for (plot_idx, channel) in channels.iter().enumerate() {
        let subplot_idx = plot_idx + 1; // +1 because first plot is time plot

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

        // Calculate MAD percentage for title
        let mad_pct = qc_result
            .mad_percentage
            .and_then(|_| {
                // Calculate channel-specific MAD percentage
                qc_result.peaks.get(channel).map(|_| {
                    // This is approximate - we'd need to track per-channel MAD
                    qc_result.mad_percentage.unwrap_or(0.0)
                })
            })
            .unwrap_or(0.0);

        let title = if mad_pct > 0.0 {
            format!("{} MAD: {:.2}%", channel, mad_pct)
        } else {
            channel.clone()
        };

        let mut chart = ChartBuilder::on(&subplot_area)
            .margin(5)
            .x_label_area_size(40)
            .y_label_area_size(50)
            .build_cartesian_2d(x_range.clone(), y_range.clone())
            .map_err(|e| PeacoQCError::PlotError(format!("Failed to build chart: {:?}", e)))?;

        chart
            .configure_mesh()
            .x_desc("Cells")
            .y_desc("Value")
            .draw()
            .map_err(|e| PeacoQCError::PlotError(format!("Failed to draw mesh: {:?}", e)))?;

        // Highlight unstable regions
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
                        PeacoQCError::PlotError(format!("Failed to draw rectangle: {:?}", e))
                    })?;
            }
        }

        // Draw scatter plot of good values (sample for performance)
        let sample_size = 10000.min(n_events);
        let step = n_events / sample_size;
        let mut good_points = Vec::new();

        for i in (0..n_events).step_by(step.max(1)) {
            if qc_result.good_cells[i] {
                good_points.push((cell_indices[i], channel_data[i]));
            }
        }

        if !good_points.is_empty() {
            chart
                .draw_series(
                    good_points
                        .iter()
                        .map(|(x, y)| Circle::new((*x, *y), 1, config.good_color.filled())),
                )
                .map_err(|e| PeacoQCError::PlotError(format!("Failed to draw circles: {:?}", e)))?;
        }

        // Draw median line per bin
        let medians = calculate_median_per_bin(&channel_data, qc_result.events_per_bin);
        if !medians.is_empty() {
            let median_points: Vec<(f64, f64)> = medians
                .iter()
                .map(|(bin_idx, median)| {
                    let cell_idx = (*bin_idx * qc_result.events_per_bin) as f64;
                    (cell_idx, *median)
                })
                .collect();

            chart
                .draw_series(LineSeries::new(
                    median_points,
                    config.median_color.stroke_width(2),
                ))
                .map_err(|e| {
                    PeacoQCError::PlotError(format!("Failed to draw median line: {:?}", e))
                })?;
        }

        // Add title
        chart
            .plotting_area()
            .draw(&Text::new(
                title,
                (5.0, 5.0),
                ("sans-serif", 12).into_font().color(&BLACK),
            ))
            .map_err(|e| PeacoQCError::PlotError(format!("Failed to draw title: {:?}", e)))?;
    }

    root.present()
        .map_err(|e| PeacoQCError::PlotError(format!("Failed to present plot: {:?}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;
    use std::collections::HashMap;

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
