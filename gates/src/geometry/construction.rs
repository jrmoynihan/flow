use crate::error::{GateError, Result};
use crate::types::{GateGeometry, GateNode};
use std::sync::Arc;

/// Create a polygon geometry from raw coordinates
///
/// # Arguments
/// * `raw_coords` - Vector of (x, y) coordinate tuples in raw data space
/// * `x_param` - Channel name for the x-axis parameter
/// * `y_param` - Channel name for the y-axis parameter
///
/// # Returns
/// A `GateGeometry::Polygon` variant with nodes created from the coordinates
///
/// # Errors
/// Returns `GateError::InvalidGeometry` if:
/// - Less than 3 coordinates are provided
/// - Any coordinate values are not finite
pub fn create_polygon_geometry(
    raw_coords: Vec<(f32, f32)>,
    x_param: &str,
    y_param: &str,
) -> Result<GateGeometry> {
    if raw_coords.len() < 3 {
        return Err(GateError::invalid_geometry(format!(
            "Polygon requires at least 3 coordinates, got {}",
            raw_coords.len()
        )));
    }

    let nodes: Vec<GateNode> = raw_coords
        .into_iter()
        .enumerate()
        .map(|(idx, (x, y))| {
            if !x.is_finite() || !y.is_finite() {
                return Err(GateError::invalid_coordinate(
                    format!("polygon_node_{}", idx),
                    if !x.is_finite() { x } else { y },
                ));
            }

            let mut node = GateNode::new(format!("polygon_node_{}", idx));
            node.set_coordinate(Arc::from(x_param), x);
            node.set_coordinate(Arc::from(y_param), y);
            Ok(node)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(GateGeometry::Polygon {
        nodes,
        closed: true,
    })
}

/// Create a rectangle geometry from raw coordinates
///
/// # Arguments
/// * `raw_coords` - Vector of (x, y) coordinate tuples in raw data space
/// * `x_param` - Channel name for the x-axis parameter
/// * `y_param` - Channel name for the y-axis parameter
///
/// # Returns
/// A `GateGeometry::Rectangle` variant with min and max nodes calculated from the coordinates
///
/// # Errors
/// Returns `GateError::InvalidGeometry` if:
/// - Less than 2 coordinates are provided
/// - Any coordinate values are not finite
/// - The calculated bounds are invalid (min > max)
pub fn create_rectangle_geometry(
    raw_coords: Vec<(f32, f32)>,
    x_param: &str,
    y_param: &str,
) -> Result<GateGeometry> {
    if raw_coords.len() < 2 {
        return Err(GateError::invalid_geometry(format!(
            "Rectangle requires at least 2 coordinates, got {}",
            raw_coords.len()
        )));
    }

    // Validate all coordinates are finite
    for (idx, (x, y)) in raw_coords.iter().enumerate() {
        if !x.is_finite() {
            return Err(GateError::invalid_coordinate(
                format!("rectangle_x_{}", idx),
                *x,
            ));
        }
        if !y.is_finite() {
            return Err(GateError::invalid_coordinate(
                format!("rectangle_y_{}", idx),
                *y,
            ));
        }
    }

    // Calculate min/max bounds
    let xs: Vec<f32> = raw_coords.iter().map(|(x, _)| *x).collect();
    let ys: Vec<f32> = raw_coords.iter().map(|(_, y)| *y).collect();

    let min_x = xs.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let min_y = ys.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_x = xs.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let max_y = ys.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

    // Validate bounds
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return Err(GateError::invalid_geometry(format!(
            "Rectangle bounds must be finite: min=({}, {}), max=({}, {})",
            min_x, min_y, max_x, max_y
        )));
    }

    if min_x > max_x || min_y > max_y {
        return Err(GateError::invalid_geometry(format!(
            "Rectangle bounds must be valid: min=({}, {}), max=({}, {})",
            min_x, min_y, max_x, max_y
        )));
    }

    let mut min_node = GateNode::new("rectangle_min");
    min_node.set_coordinate(Arc::from(x_param), min_x);
    min_node.set_coordinate(Arc::from(y_param), min_y);

    let mut max_node = GateNode::new("rectangle_max");
    max_node.set_coordinate(Arc::from(x_param), max_x);
    max_node.set_coordinate(Arc::from(y_param), max_y);

    Ok(GateGeometry::Rectangle {
        min: min_node,
        max: max_node,
    })
}

/// Create an ellipse geometry from raw coordinates
///
/// # Arguments
/// * `raw_coords` - Vector of (x, y) coordinate tuples in raw data space
///   - Expected format: [center, right/end, top, left/start, bottom] for 5+ points
///   - Falls back to bounding box calculation for 2-4 points
///   - Uses default radii (50.0, 50.0) and angle (0.0) for 1 point
/// * `x_param` - Channel name for the x-axis parameter
/// * `y_param` - Channel name for the y-axis parameter
///
/// # Returns
/// A `GateGeometry::Ellipse` variant with center, radii, and angle calculated from the coordinates
///
/// # Errors
/// Returns `GateError::InvalidGeometry` if:
/// - No coordinates are provided
/// - Center coordinates are not finite
/// - Calculated radii are invalid (negative or non-finite)
pub fn create_ellipse_geometry(
    raw_coords: Vec<(f32, f32)>,
    x_param: &str,
    y_param: &str,
) -> Result<GateGeometry> {
    if raw_coords.is_empty() {
        return Err(GateError::invalid_geometry(
            "Ellipse requires at least center coordinates",
        ));
    }

    let (cx, cy) = raw_coords[0];

    if !cx.is_finite() || !cy.is_finite() {
        return Err(GateError::invalid_coordinate("ellipse_center", cx));
    }

    let mut center = GateNode::new("ellipse_center");
    center.set_coordinate(Arc::from(x_param), cx);
    center.set_coordinate(Arc::from(y_param), cy);

    // Calculate radii and angle from control points
    // Expected format: [center, right/end, top, left/start, bottom]
    let (radius_x, radius_y, angle) = if raw_coords.len() >= 5 {
        // Point 1 is the end point (right), defines major axis
        let (rx_x, rx_y) = raw_coords[1];
        if !rx_x.is_finite() || !rx_y.is_finite() {
            return Err(GateError::invalid_coordinate("ellipse_right_point", rx_x));
        }
        let radius_x = f32::hypot(rx_x - cx, rx_y - cy);

        // Point 2 is the top point, defines minor axis
        let (ry_x, ry_y) = raw_coords[2];
        if !ry_x.is_finite() || !ry_y.is_finite() {
            return Err(GateError::invalid_coordinate("ellipse_top_point", ry_x));
        }
        let radius_y = f32::hypot(ry_x - cx, ry_y - cy);

        // Angle is from center to right/end point
        let angle = f32::atan2(rx_y - cy, rx_x - cx);

        (radius_x, radius_y, angle)
    } else if raw_coords.len() > 1 {
        // Fallback: calculate from bounding box (old behavior)
        let xs: Vec<f32> = raw_coords.iter().map(|(x, _)| *x).collect();
        let ys: Vec<f32> = raw_coords.iter().map(|(_, y)| *y).collect();

        for (idx, (x, y)) in raw_coords.iter().enumerate() {
            if !x.is_finite() || !y.is_finite() {
                return Err(GateError::invalid_coordinate(
                    format!("ellipse_point_{}", idx),
                    if !x.is_finite() { *x } else { *y },
                ));
            }
        }

        let min_x = xs.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_x = xs.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let min_y = ys.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_y = ys.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        ((max_x - min_x) / 2.0, (max_y - min_y) / 2.0, 0.0)
    } else {
        (50.0, 50.0, 0.0) // Default radii and angle
    };

    // Validate radii
    if radius_x <= 0.0 || !radius_x.is_finite() {
        return Err(GateError::invalid_geometry(format!(
            "Ellipse radius_x must be positive and finite, got {}",
            radius_x
        )));
    }

    if radius_y <= 0.0 || !radius_y.is_finite() {
        return Err(GateError::invalid_geometry(format!(
            "Ellipse radius_y must be positive and finite, got {}",
            radius_y
        )));
    }

    Ok(GateGeometry::Ellipse {
        center,
        radius_x,
        radius_y,
        angle,
    })
}

