use flow_fcs::{TransformType, Transformable};
use flow_plots::PlotRange;
use std::ops::Range;

/// Plot layout constants (matching plotting/mod.rs)
pub const PLOT_MARGIN: u32 = 10;
pub const X_LABEL_AREA_SIZE: u32 = 50;
pub const Y_LABEL_AREA_SIZE: u32 = 50;

/// Calculate the actual plotting area within a canvas
/// Returns (x_range, y_range) in pixels
pub fn get_plotting_area(width: u32, height: u32) -> (Range<u32>, Range<u32>) {
    let x_start = Y_LABEL_AREA_SIZE + PLOT_MARGIN;
    let x_end = width.saturating_sub(PLOT_MARGIN);
    let y_start = PLOT_MARGIN;
    let y_end = height.saturating_sub(X_LABEL_AREA_SIZE + PLOT_MARGIN);

    (x_start..x_end, y_start..y_end)
}

/// Calculate plotting area with explicit layout parameters (not hardcoded constants).
pub fn get_plotting_area_with_layout(
    width: u32,
    height: u32,
    plot_margin: u32,
    x_label_area_size: u32,
    y_label_area_size: u32,
) -> (Range<u32>, Range<u32>) {
    let x_start = y_label_area_size + plot_margin;
    let x_end = width.saturating_sub(plot_margin);
    let y_start = plot_margin;
    let y_end = height.saturating_sub(x_label_area_size + plot_margin);

    (x_start..x_end, y_start..y_end)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// Validate coordinate transformation parameters and log detailed information.
///
/// Important: this validation should be axis-specific.
/// Previously we attempted to validate both X and Y using a single `pixel_range`, which caused
/// misleading logs (e.g. reporting an X out-of-bounds warning during Y-axis validation).
pub fn validate_coordinate_transformation(
    pixel_coords: &[(f32, f32)],
    data_range: &PlotRange,
    pixel_range: &Range<u32>,
    _transform: &TransformType,
    axis: Axis,
    _context: &str,
) -> Result<(), String> {
    if pixel_range.start >= pixel_range.end {
        return Err(format!("Invalid pixel range: {:?}", pixel_range));
    }

    let pixel_span = (pixel_range.end - pixel_range.start) as f32;
    if pixel_span <= 0.0 {
        return Err(format!("Non-positive pixel span: {}", pixel_span));
    }

    if !data_range.start().is_finite() || !data_range.end().is_finite() {
        return Err(format!("Non-finite data range: {:?}", data_range));
    }

    let data_span = *data_range.end() - *data_range.start();
    if data_span < 0.0 {
        return Err(format!("Negative data span: {}", data_span));
    }

    for (i, (pixel_x, pixel_y)) in pixel_coords.iter().enumerate() {
        if !pixel_x.is_finite() || !pixel_y.is_finite() {
            return Err(format!(
                "Non-finite pixel coordinates at index {}: ({}, {})",
                i, pixel_x, pixel_y
            ));
        }

        let pixel_value = match axis {
            Axis::X => *pixel_x,
            Axis::Y => *pixel_y,
        };

        let normalized = (pixel_value - pixel_range.start as f32) / pixel_span;

        if normalized < -0.5 || normalized > 1.5 {
            return Err(format!(
                "{:?} coordinate {} far out of bounds: normalized={}",
                axis, i, normalized
            ));
        }
    }

    Ok(())
}

/// Transform a raw data value to a transformed value
#[inline]
pub fn raw_to_transformed(value: f32, transform: &TransformType) -> f32 {
    transform.transform(&value)
}

/// Transform a transformed value back to raw data space
#[inline]
pub fn transformed_to_raw(value: f32, transform: &TransformType) -> f32 {
    transform.inverse_transform(&value)
}

/// Convert a raw data value to a pixel coordinate
///
/// # Arguments
/// * `value` - Raw data value
/// * `data_range` - The data range being displayed (in transformed space)
/// * `pixel_range` - The pixel range of the plotting area
/// * `transform` - The transformation to apply
pub fn raw_to_pixel(
    value: f32,
    data_range: &PlotRange,
    pixel_range: &Range<u32>,
    transform: &TransformType,
) -> f32 {
    // Transform raw value to display space
    let transformed = raw_to_transformed(value, transform);

    // Map to pixel space
    let data_min = *data_range.start();
    let data_max = *data_range.end();
    let data_span = data_max - data_min;

    if data_span == 0.0 {
        return pixel_range.start as f32;
    }

    let normalized = (transformed - data_min) / data_span;
    let pixel_span = (pixel_range.end - pixel_range.start) as f32;

    if pixel_span <= 0.0 {
        return pixel_range.start as f32;
    }

    pixel_range.start as f32 + normalized * pixel_span
}

/// Convert a raw data value to a pixel coordinate (y-axis, inverted to match plotting)
/// The plotting code inverts y: rel_y = (y_spec.end - data_y) / data_height
/// This means larger y values map to smaller pixel values (top of screen)
pub fn raw_to_pixel_y(
    value: f32,
    data_range: &PlotRange,
    pixel_range: &Range<u32>,
    transform: &TransformType,
) -> f32 {
    // Transform raw value to display space
    let transformed = raw_to_transformed(value, transform);

    // Map to pixel space (INVERTED for y-axis)
    let data_min = *data_range.start();
    let data_max = *data_range.end();
    let data_span = data_max - data_min;

    if data_span == 0.0 {
        return pixel_range.end as f32;
    }

    // Invert: larger data values → smaller normalized → top of screen
    let normalized = (data_max - transformed) / data_span;
    let pixel_span = (pixel_range.end - pixel_range.start) as f32;

    if pixel_span <= 0.0 {
        return pixel_range.end as f32;
    }

    // The plotting code uses: screen_y = plot_y_start + rel_y * plot_height
    // where rel_y = (y_spec.end - data_y) / data_height
    // So: data_max → rel_y=0 → screen_y = pixel_range.start (top)
    //     data_min → rel_y=1 → screen_y = pixel_range.end (bottom)
    // With normalized = (data_max - transformed) / data_span:
    // - normalized=0 (data_max) → should map to pixel_range.start (top)
    // - normalized=1 (data_min) → should map to pixel_range.end (bottom)
    // Formula: pixel = pixel_range.start + normalized * pixel_span
    pixel_range.start as f32 + normalized * pixel_span
}

/// Convert a pixel coordinate to a raw data value (y-axis, inverted to match plotting)
pub fn pixel_to_raw_y(
    pixel: f32,
    data_range: &PlotRange,
    pixel_range: &Range<u32>,
    transform: &TransformType,
) -> f32 {
    debug_assert!(pixel.is_finite(), "Pixel coordinate must be finite: {}", pixel);
    debug_assert!(
        data_range.start().is_finite() && data_range.end().is_finite(),
        "Data range must be finite: {:?}", data_range
    );
    debug_assert!(pixel_range.start < pixel_range.end, "Pixel range must be valid: {:?}", pixel_range);

    let pixel_span = (pixel_range.end - pixel_range.start) as f32;
    let normalized = (pixel - pixel_range.start as f32) / pixel_span;
    let clamped_normalized = normalized.clamp(0.0, 1.0);

    let data_min = *data_range.start();
    let data_max = *data_range.end();
    let data_span = data_max - data_min;

    if data_span == 0.0 {
        return transformed_to_raw(data_max, transform);
    }

    // Invert: normalized=0 → data_max, normalized=1 → data_min
    let transformed = data_max - clamped_normalized * data_span;
    transformed_to_raw(transformed, transform)
}

/// Convert a pixel coordinate to a raw data value
///
/// # Arguments
/// * `pixel` - Pixel coordinate
/// * `data_range` - The data range being displayed (in transformed space)
/// * `pixel_range` - The pixel range of the plotting area
/// * `transform` - The transformation to apply
pub fn pixel_to_raw(
    pixel: f32,
    data_range: &PlotRange,
    pixel_range: &Range<u32>,
    transform: &TransformType,
) -> f32 {
    debug_assert!(pixel.is_finite(), "Pixel coordinate must be finite: {}", pixel);
    debug_assert!(
        data_range.start().is_finite() && data_range.end().is_finite(),
        "Data range must be finite: {:?}", data_range
    );
    debug_assert!(pixel_range.start < pixel_range.end, "Pixel range must be valid: {:?}", pixel_range);

    let pixel_span = (pixel_range.end - pixel_range.start) as f32;
    let normalized = (pixel - pixel_range.start as f32) / pixel_span;
    let clamped_normalized = normalized.clamp(0.0, 1.0);

    let data_min = *data_range.start();
    let data_max = *data_range.end();
    let data_span = data_max - data_min;

    let transformed = data_min + clamped_normalized * data_span;
    transformed_to_raw(transformed, transform)
}

/// Convert raw gate coordinates to display pixel coordinates
///
/// # Arguments
/// * `raw_coords` - Iterator of (x, y) coordinates in raw data space
/// * `x_data_range` - X-axis data range (in transformed space)
/// * `y_data_range` - Y-axis data range (in transformed space)
/// * `width` - Canvas width in pixels
/// * `height` - Canvas height in pixels
/// * `x_transform` - X-axis transformation
/// * `y_transform` - Y-axis transformation
pub fn raw_coords_to_pixels<I>(
    raw_coords: I,
    x_data_range: &PlotRange,
    y_data_range: &PlotRange,
    width: u32,
    height: u32,
    x_transform: &TransformType,
    y_transform: &TransformType,
) -> Vec<(f32, f32)>
where
    I: IntoIterator<Item = (f32, f32)>,
{
    let (x_pixel_range, y_pixel_range) = get_plotting_area(width, height);

    raw_coords
        .into_iter()
        .map(|(raw_x, raw_y)| {
            let pixel_x = raw_to_pixel(raw_x, x_data_range, &x_pixel_range, x_transform);
            // Use y-axis inverted function to match plotting code's y-axis inversion
            let pixel_y = raw_to_pixel_y(raw_y, y_data_range, &y_pixel_range, y_transform);
            (pixel_x, pixel_y)
        })
        .collect()
}

/// Like `raw_coords_to_pixels` but uses caller-supplied layout.
pub fn raw_coords_to_pixels_with_layout<I>(
    raw_coords: I,
    x_data_range: &PlotRange,
    y_data_range: &PlotRange,
    width: u32,
    height: u32,
    x_transform: &TransformType,
    y_transform: &TransformType,
    plot_margin: u32,
    x_label_area_size: u32,
    y_label_area_size: u32,
) -> Vec<(f32, f32)>
where
    I: IntoIterator<Item = (f32, f32)>,
{
    let (x_pixel_range, y_pixel_range) =
        get_plotting_area_with_layout(width, height, plot_margin, x_label_area_size, y_label_area_size);

    raw_coords
        .into_iter()
        .map(|(raw_x, raw_y)| {
            let pixel_x = raw_to_pixel(raw_x, x_data_range, &x_pixel_range, x_transform);
            let pixel_y = raw_to_pixel_y(raw_y, y_data_range, &y_pixel_range, y_transform);
            (pixel_x, pixel_y)
        })
        .collect()
}

/// Convert display pixel coordinates to raw gate coordinates
///
/// # Arguments
/// * `pixel_coords` - Iterator of (x, y) pixel coordinates
/// * `x_data_range` - X-axis data range (in transformed space)
/// * `y_data_range` - Y-axis data range (in transformed space)
/// * `width` - Canvas width in pixels
/// * `height` - Canvas height in pixels
/// * `x_transform` - X-axis transformation
/// * `y_transform` - Y-axis transformation
pub fn pixels_to_raw_coords<I>(
    pixel_coords: I,
    x_data_range: &PlotRange,
    y_data_range: &PlotRange,
    width: u32,
    height: u32,
    x_transform: &TransformType,
    y_transform: &TransformType,
) -> Vec<(f32, f32)>
where
    I: IntoIterator<Item = (f32, f32)>,
{
    let pixel_coords_vec: Vec<(f32, f32)> = pixel_coords.into_iter().collect();

    debug_assert!(width > 0 && height > 0, "Plot dimensions must be positive: {}x{}", width, height);
    debug_assert!(
        x_data_range.start().is_finite() && x_data_range.end().is_finite(),
        "X data range must be finite: {:?}", x_data_range
    );
    debug_assert!(
        y_data_range.start().is_finite() && y_data_range.end().is_finite(),
        "Y data range must be finite: {:?}", y_data_range
    );

    let (x_pixel_range, y_pixel_range) = get_plotting_area(width, height);

    pixel_coords_vec
        .into_iter()
        .map(|(pixel_x, pixel_y)| {
            let raw_x = pixel_to_raw(pixel_x, x_data_range, &x_pixel_range, x_transform);
            let raw_y = pixel_to_raw_y(pixel_y, y_data_range, &y_pixel_range, y_transform);
            (raw_x, raw_y)
        })
        .collect()
}

/// Like `pixels_to_raw_coords` but uses caller-supplied layout instead of hardcoded constants.
pub fn pixels_to_raw_coords_with_layout<I>(
    pixel_coords: I,
    x_data_range: &PlotRange,
    y_data_range: &PlotRange,
    width: u32,
    height: u32,
    x_transform: &TransformType,
    y_transform: &TransformType,
    plot_margin: u32,
    x_label_area_size: u32,
    y_label_area_size: u32,
) -> Vec<(f32, f32)>
where
    I: IntoIterator<Item = (f32, f32)>,
{
    let pixel_coords_vec: Vec<(f32, f32)> = pixel_coords.into_iter().collect();

    debug_assert!(width > 0 && height > 0, "Plot dimensions must be positive: {}x{}", width, height);
    debug_assert!(
        x_data_range.start().is_finite() && x_data_range.end().is_finite(),
        "X data range must be finite: {:?}", x_data_range
    );
    debug_assert!(
        y_data_range.start().is_finite() && y_data_range.end().is_finite(),
        "Y data range must be finite: {:?}", y_data_range
    );

    let (x_pixel_range, y_pixel_range) =
        get_plotting_area_with_layout(width, height, plot_margin, x_label_area_size, y_label_area_size);

    pixel_coords_vec
        .into_iter()
        .map(|(pixel_x, pixel_y)| {
            let raw_x = pixel_to_raw(pixel_x, x_data_range, &x_pixel_range, x_transform);
            let raw_y = pixel_to_raw_y(pixel_y, y_data_range, &y_pixel_range, y_transform);
            (raw_x, raw_y)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plotting_area() {
        let (x_range, y_range) = get_plotting_area(400, 400);
        assert_eq!(x_range.start, 60); // Y_LABEL_AREA_SIZE + PLOT_MARGIN (50 + 10)
        assert_eq!(x_range.end, 390); // width - PLOT_MARGIN
        assert_eq!(y_range.start, 10); // PLOT_MARGIN
        assert_eq!(y_range.end, 340); // height - X_LABEL_AREA_SIZE - PLOT_MARGIN (400 - 50 - 10)
    }

    #[test]
    fn test_linear_transform_round_trip() {
        let transform = TransformType::Linear;
        let data_range = 0.0..=10000.0;
        let pixel_range = 0..100;

        let raw_value = 5000.0;
        let pixel = raw_to_pixel(raw_value, &data_range, &pixel_range, &transform);
        let recovered = pixel_to_raw(pixel, &data_range, &pixel_range, &transform);

        assert!((recovered - raw_value).abs() < 0.1);
    }

    #[test]
    fn test_arcsinh_transform_round_trip() {
        let transform = TransformType::Arcsinh { cofactor: 200.0 };
        let data_range = (-1.0)..=5.0; // Transformed range
        let pixel_range = 0..100;

        let raw_value = 1000.0;
        let pixel = raw_to_pixel(raw_value, &data_range, &pixel_range, &transform);
        let recovered = pixel_to_raw(pixel, &data_range, &pixel_range, &transform);

        // Allow some tolerance due to floating point precision
        assert!((recovered - raw_value).abs() < 1.0);
    }

    #[test]
    fn test_coords_to_pixels() {
        let transform = TransformType::Linear;
        let x_range = 0.0..=100.0;
        let y_range = 0.0..=100.0;

        let raw_coords = vec![(0.0, 0.0), (50.0, 50.0), (100.0, 100.0)];
        let pixels = raw_coords_to_pixels(
            raw_coords, &x_range, &y_range, 400, 400, &transform, &transform,
        );

        assert_eq!(pixels.len(), 3);
        // First point should be near the bottom-left of plotting area
        // x_start is Y_LABEL_AREA_SIZE + PLOT_MARGIN (50 + 10)
        assert!(pixels[0].0 >= 60.0 && pixels[0].0 <= 61.0); // x_start
        // Last point should be near the top-right of plotting area
        assert!(pixels[2].0 >= 389.0 && pixels[2].0 <= 390.0); // x_end
    }

    #[test]
    fn test_out_of_bounds_pixel_coordinates() {
        let transform = TransformType::Linear;
        let data_range = 0.0..=262144.0;
        let pixel_range = 50..244; // Typical plotting area range

        // Test pixel coordinates that are outside the plotting area
        let out_of_bounds_pixels = vec![
            -10.0, // Below plotting area
            300.0, // Above plotting area
            0.0,   // At start of pixel range
            244.0, // At end of pixel range
        ];

        for pixel in out_of_bounds_pixels {
            // This should not panic anymore due to clamping
            let result = pixel_to_raw(pixel, &data_range, &pixel_range, &transform);
            assert!(
                result.is_finite(),
                "Result should be finite for pixel: {}",
                pixel
            );

            // Verify the result is within reasonable bounds
            assert!(
                result >= 0.0,
                "Result should be non-negative for pixel: {}",
                pixel
            );
            assert!(
                result <= 262144.0,
                "Result should not exceed data range for pixel: {}",
                pixel
            );
        }
    }

    #[test]
    fn test_edge_case_coordinates() {
        let transform = TransformType::Linear;
        let data_range = 0.0..=262144.0;
        let pixel_range = 50..244;

        // Test edge cases that previously caused panics
        let edge_cases = vec![
            (50.0, 0.0),       // Start of pixel range
            (244.0, 262144.0), // End of pixel range
            (147.0, 131072.0), // Middle of range
        ];

        for (pixel, expected_raw) in edge_cases {
            let result = pixel_to_raw(pixel, &data_range, &pixel_range, &transform);
            assert!(
                result.is_finite(),
                "Result should be finite for pixel: {}",
                pixel
            );

            // Allow some tolerance for floating point precision
            let tolerance = expected_raw * 0.001; // 0.1% tolerance
            assert!(
                (result - expected_raw).abs() <= tolerance,
                "Result {} should be close to expected {} for pixel {}",
                result,
                expected_raw,
                pixel
            );
        }
    }

    #[test]
    fn test_round_trip_with_clamping() {
        let transform = TransformType::Linear;
        let data_range = 0.0..=262144.0;
        let pixel_range = 50..244;

        // Test round trip with out-of-bounds coordinates
        let test_cases = vec![
            (0.0, 262144.0),      // Raw data range
            (1000.0, 50000.0),    // Mid-range values
            (131072.0, 131072.0), // Middle value
        ];

        for (raw_x, raw_y) in test_cases {
            // Convert to pixels
            let pixel_x = raw_to_pixel(raw_x, &data_range, &pixel_range, &transform);
            let pixel_y = raw_to_pixel(raw_y, &data_range, &pixel_range, &transform);

            // Convert back to raw (this might clamp if out of bounds)
            let recovered_x = pixel_to_raw(pixel_x, &data_range, &pixel_range, &transform);
            let recovered_y = pixel_to_raw(pixel_y, &data_range, &pixel_range, &transform);

            // Results should be finite and within data range
            assert!(recovered_x.is_finite() && recovered_y.is_finite());
            assert!(recovered_x >= 0.0 && recovered_x <= 262144.0);
            assert!(recovered_y >= 0.0 && recovered_y <= 262144.0);
        }
    }

    #[test]
    fn test_arcsinh_transform_edge_cases() {
        let transform = TransformType::Arcsinh { cofactor: 200.0 };
        let data_range = (-2.0)..=6.0; // Transformed range
        let pixel_range = 50..244;

        // Test with values that might cause issues in arcsinh transform
        let test_values = vec![0.0, 100.0, 1000.0, 10000.0];

        for raw_value in test_values {
            let pixel = raw_to_pixel(raw_value, &data_range, &pixel_range, &transform);
            let recovered = pixel_to_raw(pixel, &data_range, &pixel_range, &transform);

            // Results should be finite
            assert!(
                recovered.is_finite(),
                "Recovered value should be finite for input: {}",
                raw_value
            );
            assert!(
                pixel.is_finite(),
                "Pixel value should be finite for input: {}",
                raw_value
            );

            // Check if the pixel coordinate was clamped (outside plotting area)
            let normalized =
                (pixel - pixel_range.start as f32) / (pixel_range.end - pixel_range.start) as f32;
            let was_clamped = normalized < 0.0 || normalized > 1.0;

            if was_clamped {
                // For clamped coordinates, we expect the recovered value to be at the data range boundary
                // This is expected behavior when coordinates extend beyond the plotting area
                let expected_boundary = if normalized < 0.0 {
                    // Clamped to minimum data range
                    transform.inverse_transform(data_range.start())
                } else {
                    // Clamped to maximum data range
                    transform.inverse_transform(data_range.end())
                };

                let boundary_tolerance = expected_boundary * 0.01; // 1% tolerance
                assert!(
                    (recovered - expected_boundary).abs() <= boundary_tolerance,
                    "Clamped coordinate should recover to data boundary: recovered={}, expected_boundary={}",
                    recovered,
                    expected_boundary
                );
            } else {
                // For non-clamped coordinates, expect good round-trip accuracy
                let tolerance = raw_value * 0.01; // 1% tolerance
                assert!(
                    (recovered - raw_value).abs() <= tolerance,
                    "Recovered {} should be close to input {} for arcsinh transform",
                    recovered,
                    raw_value
                );
            }
        }
    }

    #[test]
    fn test_with_layout_roundtrip_linear() {
        let transform = TransformType::Linear;
        let x_range = 0.0..=4000000.0;
        let y_range = 0.0..=2000000.0;
        let width = 500;
        let height = 400;
        let margin = 0;
        let x_label = 20;
        let y_label = 48;

        let raw_coords = vec![(1000000.0, 500000.0), (2000000.0, 1000000.0), (3500000.0, 1800000.0)];

        let pixels = raw_coords_to_pixels_with_layout(
            raw_coords.clone(), &x_range, &y_range, width, height,
            &transform, &transform, margin, x_label, y_label,
        );

        let recovered = pixels_to_raw_coords_with_layout(
            pixels, &x_range, &y_range, width, height,
            &transform, &transform, margin, x_label, y_label,
        );

        for ((orig_x, orig_y), (rec_x, rec_y)) in raw_coords.iter().zip(recovered.iter()) {
            assert!((orig_x - rec_x).abs() < 100.0,
                "X roundtrip failed: {} vs {}", orig_x, rec_x);
            assert!((orig_y - rec_y).abs() < 100.0,
                "Y roundtrip failed: {} vs {}", orig_y, rec_y);
        }
    }

    #[test]
    fn test_with_layout_roundtrip_arcsinh() {
        let transform = TransformType::Arcsinh { cofactor: 150.0 };
        let x_range = (-4.0)..=10.0; // transformed space
        let y_range = (-3.0)..=9.0;
        let width = 500;
        let height = 400;
        let margin = 0;
        let x_label = 20;
        let y_label = 48;

        let raw_coords = vec![(500.0, 200.0), (50000.0, 10000.0), (-1000.0, -500.0)];

        let pixels = raw_coords_to_pixels_with_layout(
            raw_coords.clone(), &x_range, &y_range, width, height,
            &transform, &transform, margin, x_label, y_label,
        );

        let recovered = pixels_to_raw_coords_with_layout(
            pixels, &x_range, &y_range, width, height,
            &transform, &transform, margin, x_label, y_label,
        );

        for ((orig_x, orig_y), (rec_x, rec_y)) in raw_coords.iter().zip(recovered.iter()) {
            let tol_x = orig_x.abs() * 0.01 + 1.0;
            let tol_y = orig_y.abs() * 0.01 + 1.0;
            assert!((orig_x - rec_x).abs() < tol_x,
                "X arcsinh roundtrip failed: {} vs {} (tol {})", orig_x, rec_x, tol_x);
            assert!((orig_y - rec_y).abs() < tol_y,
                "Y arcsinh roundtrip failed: {} vs {} (tol {})", orig_y, rec_y, tol_y);
        }
    }

    #[test]
    fn test_with_layout_zero_margins() {
        // GPU mode: all margins zero, full canvas = data area
        let transform = TransformType::Linear;
        let x_range = 0.0..=1000.0;
        let y_range = 0.0..=1000.0;
        let width = 400;
        let height = 400;

        let pixels = raw_coords_to_pixels_with_layout(
            vec![(0.0, 0.0), (1000.0, 1000.0)],
            &x_range, &y_range, width, height,
            &transform, &transform, 0, 0, 0,
        );

        // With zero margins: pixel 0 = data min, pixel 400 = data max
        assert!((pixels[0].0 - 0.0).abs() < 1.0, "Bottom-left x: {}", pixels[0].0);
        assert!((pixels[0].1 - 400.0).abs() < 1.0, "Bottom-left y (inverted): {}", pixels[0].1);
        assert!((pixels[1].0 - 400.0).abs() < 1.0, "Top-right x: {}", pixels[1].0);
        assert!((pixels[1].1 - 0.0).abs() < 1.0, "Top-right y (inverted): {}", pixels[1].1);
    }
}
