//! Contour plot calculation using KDE2D and marching squares.
//!
//! Extracts contour lines from 2D density estimates at multiple threshold levels.

use flow_utils::KernelDensity2D;
use ndarray::Array2;

/// Contour plot data: contour paths and optional outlier points
#[derive(Clone, Debug)]
pub struct ContourData {
    /// Contour paths at each level (outermost first). Each path is a closed polygon.
    pub contours: Vec<ContourPath>,
    /// Points outside all contours (when draw_outliers is true)
    pub outliers: Vec<(f64, f64)>,
}

/// A single contour path - ordered (x, y) points forming a closed loop
pub type ContourPath = Vec<(f64, f64)>;

/// Extract contour paths from (x, y) data using KDE and marching squares.
///
/// # Arguments
/// * `data` - (x, y) points
/// * `level_count` - Number of contour levels
/// * `smoothing` - KDE bandwidth adjustment (higher = more smooth)
/// * `draw_outliers` - If true, compute points outside outermost contour
/// * `x_range` - Plot x axis range; contour paths are clamped to this range
/// * `y_range` - Plot y axis range; contour paths are clamped to this range
pub fn calculate_contours(
    data: &[(f32, f32)],
    level_count: u32,
    smoothing: f64,
    draw_outliers: bool,
    x_range: (f32, f32),
    y_range: (f32, f32),
) -> Result<ContourData, flow_utils::KdeError> {
    if data.len() < 3 {
        return Ok(ContourData {
            contours: Vec::new(),
            outliers: data
                .iter()
                .map(|&(x, y)| (x as f64, y as f64))
                .collect(),
        });
    }

    let data_x: Vec<f64> = data.iter().map(|(x, _)| *x as f64).collect();
    let data_y: Vec<f64> = data.iter().map(|(_, y)| *y as f64).collect();

    // KDE grid resolution - use enough for smooth contours
    let n_grid = 128usize;
    let kde = KernelDensity2D::estimate(&data_x, &data_y, smoothing, n_grid)?;

    let max_density = kde
        .z
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(1e-300);

    // Contour levels: from low (outer) to high (inner)
    // Use geometric spacing for better visual distribution
    let levels: Vec<f64> = if level_count <= 1 {
        vec![0.1 * max_density]
    } else {
        (1..=level_count as usize)
            .map(|i| {
                let t = i as f64 / (level_count as f64 + 1.0);
                // Logarithmic spacing: low values near 0.01, high near 0.99
                let frac = 0.01 + 0.98 * t;
                frac * max_density
            })
            .collect()
    };

    let mut contours = Vec::with_capacity(levels.len());
    for &threshold in &levels {
        let paths = marching_squares_contours(&kde.x, &kde.y, &kde.z, threshold);
        contours.extend(paths);
    }

    // Clip contour paths to the axis range.
    // The KDE grid extends 3×bandwidth beyond the data extent, so contour paths
    // can exceed the chart's axis range.  Plotters panics with an integer overflow
    // when coordinates are far outside the chart range (plotters#NNN), so we clamp
    // here to prevent that.
    let contours = clip_contour_paths(contours, x_range, y_range);

    let outliers = if draw_outliers {
        let x_lo = x_range.0 as f64;
        let x_hi = x_range.1 as f64;
        let y_lo = y_range.0 as f64;
        let y_hi = y_range.1 as f64;
        let min_threshold = levels.first().copied().unwrap_or(0.01 * max_density);
        data.iter()
            .filter(|&&(x, y)| {
                let d = kde.density_at(x as f64, y as f64);
                d < min_threshold
            })
            .map(|&(x, y)| (x as f64, y as f64))
            .filter(|&(x, y)| x >= x_lo && x <= x_hi && y >= y_lo && y <= y_hi)
            .collect()
    } else {
        Vec::new()
    };

    Ok(ContourData {
        contours,
        outliers,
    })
}

/// Clamp contour path points to the given axis range and drop degenerate paths.
///
/// Uses simple point-clamping: each point's x is clamped to `[x_min, x_max]` and y to
/// `[y_min, y_max]`.  Paths where all points collapse to a single location after
/// clamping (i.e. the entire path was outside the range) are dropped.
///
/// This is not a geometrically precise polygon clip (Sutherland-Hodgman would
/// introduce intersection points at the boundary), but contour tails that extend
/// only ~3×bandwidth beyond the data are close to the axis range, so the visual
/// difference is negligible.
fn clip_contour_paths(
    paths: Vec<ContourPath>,
    x_range: (f32, f32),
    y_range: (f32, f32),
) -> Vec<ContourPath> {
    let x_lo = x_range.0 as f64;
    let x_hi = x_range.1 as f64;
    let y_lo = y_range.0 as f64;
    let y_hi = y_range.1 as f64;

    paths
        .into_iter()
        .filter_map(|path| {
            let clamped: ContourPath = path
                .into_iter()
                .map(|(x, y)| (x.clamp(x_lo, x_hi), y.clamp(y_lo, y_hi)))
                .collect();

            // Drop if every point collapsed to the same location (fully outside)
            if clamped.len() < 2 {
                return None;
            }
            let (x0, y0) = clamped[0];
            let all_same = clamped
                .iter()
                .all(|&(x, y)| (x - x0).abs() < 1e-12 && (y - y0).abs() < 1e-12);
            if all_same {
                return None;
            }
            Some(clamped)
        })
        .collect()
}

/// Marching squares: extract contour paths from a 2D scalar grid.
///
/// Grid layout: x[i], y[j], z[[i,j]] - first index is x, second is y.
fn marching_squares_contours(
    x: &[f64],
    y: &[f64],
    z: &Array2<f64>,
    threshold: f64,
) -> Vec<ContourPath> {
    let nx = x.len();
    let ny = y.len();
    if nx < 2 || ny < 2 {
        return Vec::new();
    }

    // Collect line segments: each is ((x0,y0), (x1,y1))
    let mut segments = Vec::new();

    for i in 0..nx - 1 {
        for j in 0..ny - 1 {
            let v00 = z[[i, j]];
            let v10 = z[[i + 1, j]];
            let v01 = z[[i, j + 1]];
            let v11 = z[[i + 1, j + 1]];

            let c00 = v00 >= threshold;
            let c10 = v10 >= threshold;
            let c01 = v01 >= threshold;
            let c11 = v11 >= threshold;

            let case = (c00 as u8) | ((c10 as u8) << 1) | ((c01 as u8) << 2) | ((c11 as u8) << 3);
            if case == 0 || case == 15 {
                continue; // All below or all above
            }

            // Linear interpolation for edge crossings
            let lerp = |a: f64, b: f64, va: f64, vb: f64| -> f64 {
                if (va >= threshold) == (vb >= threshold) {
                    return a; // No crossing
                }
                let t = (threshold - va) / (vb - va).max(1e-300);
                a + t * (b - a)
            };

            let x0 = x[i];
            let x1 = x[i + 1];
            let y0 = y[j];
            let y1 = y[j + 1];

            // Edge crossing points (x,y)
            let bottom = (lerp(x0, x1, v00, v10), y0);
            let right = (x1, lerp(y0, y1, v10, v11));
            let top = (lerp(x1, x0, v11, v01), y1);
            let left = (x0, lerp(y1, y0, v01, v00));

            match case {
                1 => segments.push((left, bottom)),
                2 => segments.push((bottom, right)),
                3 => segments.push((left, right)),
                4 => segments.push((top, right)),
                5 => {
                    segments.push((left, bottom));
                    segments.push((top, right));
                }
                6 => segments.push((bottom, top)),
                7 => segments.push((left, top)),
                8 => segments.push((top, left)),
                9 => segments.push((bottom, top)),
                10 => {
                    segments.push((bottom, left));
                    segments.push((top, right));
                }
                11 => segments.push((bottom, top)),
                12 => segments.push((right, left)),
                13 => segments.push((right, bottom)),
                14 => segments.push((top, left)),
                _ => {}
            }
        }
    }

    // Chain segments into closed paths
    chain_segments_into_paths(segments)
}

/// Chain line segments into closed contour paths
fn chain_segments_into_paths(segments: Vec<((f64, f64), (f64, f64))>) -> Vec<ContourPath> {
    const EQ_EPS: f64 = 1e-10;

    fn points_eq(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < EQ_EPS && (a.1 - b.1).abs() < EQ_EPS
    }

    let mut used = vec![false; segments.len()];
    let mut paths = Vec::new();

    for start_idx in 0..segments.len() {
        if used[start_idx] {
            continue;
        }

        let (p0, p1) = segments[start_idx];
        used[start_idx] = true;
        let mut path = vec![p0, p1];

        loop {
            let mut found = false;
            for (idx, &(a, b)) in segments.iter().enumerate() {
                if used[idx] {
                    continue;
                }
                let last = *path.last().unwrap();
                if points_eq(last, a) {
                    path.push(b);
                    used[idx] = true;
                    found = true;
                    break;
                } else if points_eq(last, b) {
                    path.push(a);
                    used[idx] = true;
                    found = true;
                    break;
                }
            }
            if !found {
                break;
            }
            // Check if we closed the loop
            if path.len() >= 3 && points_eq(path.first().copied().unwrap(), *path.last().unwrap()) {
                path.pop(); // Remove duplicate closing point
                break;
            }
        }

        if path.len() >= 2 {
            // If path closed on itself, remove duplicate endpoint
            if path.len() >= 3 && points_eq(path[0], path[path.len() - 1]) {
                path.pop();
            }
            paths.push(path);
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contour_empty_data() {
        let data: Vec<(f32, f32)> = vec![];
        let result = calculate_contours(&data, 5, 1.0, false, (0.0, 100.0), (0.0, 100.0));
        assert!(result.is_ok());
        let contour_data = result.unwrap();
        assert!(contour_data.contours.is_empty());
        assert!(contour_data.outliers.is_empty());
    }

    #[test]
    fn test_contour_small_data() {
        let data = vec![(1.0f32, 2.0f32), (2.0, 3.0), (3.0, 4.0)];
        let result = calculate_contours(&data, 3, 1.0, false, (0.0, 10.0), (0.0, 10.0));
        assert!(result.is_ok());
    }

    #[test]
    fn test_contour_clustered_data() {
        // Dense grid of points - guarantees high density region for contours
        let mut data: Vec<(f32, f32)> = Vec::new();
        for xi in 0..20 {
            for yi in 0..20 {
                data.push((100.0 + xi as f32 * 2.0, 100.0 + yi as f32 * 2.0));
            }
        }
        let result = calculate_contours(&data, 5, 0.8, false, (0.0, 200.0), (0.0, 200.0));
        assert!(result.is_ok());
        let contour_data = result.unwrap();
        // 400 points in tight cluster should produce contour paths
        assert!(
            !contour_data.contours.is_empty(),
            "dense cluster should produce at least one contour path"
        );
    }

    #[test]
    fn test_contour_paths_clipped_to_range() {
        // Data centered at (50, 50); KDE bandwidth will push contour paths
        // beyond the data extent.  Use a narrow axis range to verify clipping.
        let mut data: Vec<(f32, f32)> = Vec::new();
        for xi in 0..20 {
            for yi in 0..20 {
                data.push((40.0 + xi as f32, 40.0 + yi as f32));
            }
        }

        let x_range = (35.0f32, 65.0f32);
        let y_range = (35.0f32, 65.0f32);
        let result = calculate_contours(&data, 5, 1.0, true, x_range, y_range);
        assert!(result.is_ok());
        let contour_data = result.unwrap();

        // Every contour point must be within the axis range
        for path in &contour_data.contours {
            for &(x, y) in path {
                assert!(
                    x >= x_range.0 as f64 && x <= x_range.1 as f64,
                    "contour x={x} outside range [{}, {}]",
                    x_range.0,
                    x_range.1
                );
                assert!(
                    y >= y_range.0 as f64 && y <= y_range.1 as f64,
                    "contour y={y} outside range [{}, {}]",
                    y_range.0,
                    y_range.1
                );
            }
        }
        // Outliers must also be within range
        for &(x, y) in &contour_data.outliers {
            assert!(
                x >= x_range.0 as f64 && x <= x_range.1 as f64,
                "outlier x={x} outside range"
            );
            assert!(
                y >= y_range.0 as f64 && y <= y_range.1 as f64,
                "outlier y={y} outside range"
            );
        }
    }

    #[test]
    fn test_contour_narrow_range_no_panic() {
        // Regression: data far outside a narrow axis range should not panic.
        // Before the clipping fix, plotters would overflow when rendering
        // contour paths that extend far beyond the chart range.
        let mut data: Vec<(f32, f32)> = Vec::new();
        for xi in 0..30 {
            for yi in 0..30 {
                data.push((xi as f32 * 100.0, yi as f32 * 100.0));
            }
        }

        // Very narrow range relative to data spread
        let x_range = (500.0f32, 600.0f32);
        let y_range = (500.0f32, 600.0f32);
        let result = calculate_contours(&data, 5, 1.0, true, x_range, y_range);
        assert!(result.is_ok());
        let contour_data = result.unwrap();

        for path in &contour_data.contours {
            for &(x, y) in path {
                assert!(x >= x_range.0 as f64 && x <= x_range.1 as f64);
                assert!(y >= y_range.0 as f64 && y <= y_range.1 as f64);
            }
        }
    }

    #[test]
    fn test_clip_contour_paths_drops_fully_outside() {
        // A path entirely outside the range should be dropped
        let paths = vec![
            vec![(10.0, 10.0), (20.0, 20.0), (30.0, 30.0)], // fully outside
            vec![(50.0, 50.0), (55.0, 55.0), (60.0, 60.0)], // inside
        ];
        let clipped = clip_contour_paths(paths, (40.0, 70.0), (40.0, 70.0));
        // First path should be dropped (all points clamp to (40, 40))
        assert_eq!(clipped.len(), 1);
        assert!(
            clipped[0].iter().all(|&(x, y)| x >= 40.0 && y >= 40.0),
            "remaining path should be the in-range one"
        );
    }

    // ============================================================================
    // End-to-end contour render regression tests
    // ============================================================================

    #[test]
    fn test_render_contour_no_panic_with_out_of_range_data() {
        // Regression: data spread far wider than the axis range used to cause
        // plotters to panic with "attempt to add with overflow" because contour
        // paths from KDE extended beyond the chart coordinate range.
        use crate::options::{AxisOptions, BasePlotOptions, DensityPlotOptions};
        use crate::plots::density::DensityPlot;
        use crate::plots::traits::Plot;
        use crate::plots::PlotType;
        use crate::render::RenderConfig;
        use flow_fcs::TransformType;

        let plot = DensityPlot::new();

        // Data spread across 0..3000 but axis range is only 0..500
        let mut data: Vec<(f32, f32)> = Vec::new();
        for xi in 0..30 {
            for yi in 0..30 {
                data.push((xi as f32 * 100.0, yi as f32 * 100.0));
            }
        }

        let options = DensityPlotOptions::new()
            .base(BasePlotOptions::new().width(200u32).height(200u32).build().unwrap())
            .x_axis(
                AxisOptions::new()
                    .range(0.0..=500.0)
                    .transform(TransformType::Linear)
                    .build()
                    .unwrap(),
            )
            .y_axis(
                AxisOptions::new()
                    .range(0.0..=500.0)
                    .transform(TransformType::Linear)
                    .build()
                    .unwrap(),
            )
            .plot_type(PlotType::Contour)
            .contour_level_count(5u32)
            .build()
            .unwrap();

        let mut render_config = RenderConfig::default();
        let result = plot.render(data.into(), &options, &mut render_config);
        assert!(result.is_ok(), "contour render panicked: {:?}", result.err());

        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(bytes[0], 0xFF); // JPEG magic
        assert_eq!(bytes[1], 0xD8);
    }

    #[test]
    fn test_render_contour_with_outliers_no_panic() {
        // Same scenario but with draw_outliers enabled and extreme outliers
        use crate::options::{AxisOptions, BasePlotOptions, DensityPlotOptions};
        use crate::plots::density::DensityPlot;
        use crate::plots::traits::Plot;
        use crate::plots::PlotType;
        use crate::render::RenderConfig;
        use flow_fcs::TransformType;

        let plot = DensityPlot::new();
        let mut data: Vec<(f32, f32)> = Vec::new();
        for xi in 0..20 {
            for yi in 0..20 {
                data.push((50.0 + xi as f32, 50.0 + yi as f32));
            }
        }
        // Far-flung outliers
        data.push((10000.0, 10000.0));
        data.push((-5000.0, -5000.0));

        let options = DensityPlotOptions::new()
            .base(BasePlotOptions::new().width(200u32).height(200u32).build().unwrap())
            .x_axis(
                AxisOptions::new()
                    .range(0.0..=100.0)
                    .transform(TransformType::Linear)
                    .build()
                    .unwrap(),
            )
            .y_axis(
                AxisOptions::new()
                    .range(0.0..=100.0)
                    .transform(TransformType::Linear)
                    .build()
                    .unwrap(),
            )
            .plot_type(PlotType::Contour)
            .contour_level_count(5u32)
            .draw_outliers(true)
            .build()
            .unwrap();

        let mut render_config = RenderConfig::default();
        let result = plot.render(data.into(), &options, &mut render_config);
        assert!(
            result.is_ok(),
            "contour+outlier render panicked: {:?}",
            result.err()
        );
    }
}
