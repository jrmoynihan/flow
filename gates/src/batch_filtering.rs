//! Batch filtering operations for gate geometries
//!
//! Provides efficient CPU-based batch filtering for polygon, rectangle, and ellipse gates.
//! Uses Rayon for parallelization to maximize CPU utilization.

use crate::error::Result;
use crate::polygon::point_in_polygon;
use rayon::prelude::*;

/// Batch point-in-polygon query
pub fn filter_by_polygon_batch(
    points: &[(f32, f32)],
    polygon: &[(f32, f32)],
) -> Result<Vec<bool>> {
    Ok(points
        .par_iter()
        .map(|&(x, y)| point_in_polygon(x, y, polygon))
        .collect())
}

/// Batch point-in-rectangle query
pub fn filter_by_rectangle_batch(
    points: &[(f32, f32)],
    bounds: (f32, f32, f32, f32),
) -> Result<Vec<bool>> {
    let (min_x, min_y, max_x, max_y) = bounds;
    Ok(points
        .par_iter()
        .map(|&(x, y)| x >= min_x && x <= max_x && y >= min_y && y <= max_y)
        .collect())
}

/// Batch point-in-ellipse query
pub fn filter_by_ellipse_batch(
    points: &[(f32, f32)],
    center: (f32, f32),
    radius_x: f32,
    radius_y: f32,
    angle: f32,
) -> Result<Vec<bool>> {
    let (cx, cy) = center;
    let cos_angle = angle.cos();
    let sin_angle = angle.sin();

    Ok(points
        .par_iter()
        .map(|&(x, y)| {
            let dx = x - cx;
            let dy = y - cy;

            // Rotate point to ellipse's coordinate system
            let rotated_x = dx * cos_angle + dy * sin_angle;
            let rotated_y = -dx * sin_angle + dy * cos_angle;

            // Check if inside ellipse: (x/rx)^2 + (y/ry)^2 <= 1
            let normalized_x = rotated_x / radius_x;
            let normalized_y = rotated_y / radius_y;
            normalized_x * normalized_x + normalized_y * normalized_y <= 1.0
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polygon_filtering() {
        let polygon = vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
        ];
        
        let points = vec![
            (5.0, 5.0),   // Inside
            (15.0, 5.0),  // Outside
            (5.0, 15.0),  // Outside
            (-5.0, 5.0),  // Outside
        ];
        
        let results = filter_by_polygon_batch(&points, &polygon).unwrap();
        assert_eq!(results, vec![true, false, false, false]);
    }

    #[test]
    fn test_rectangle_filtering() {
        let bounds = (0.0, 0.0, 10.0, 10.0);
        
        let points = vec![
            (5.0, 5.0),   // Inside
            (15.0, 5.0),  // Outside
            (5.0, 15.0),  // Outside
            (-5.0, 5.0),  // Outside
            (0.0, 0.0),   // On edge (inside)
            (10.0, 10.0), // On edge (inside)
        ];
        
        let results = filter_by_rectangle_batch(&points, bounds).unwrap();
        assert_eq!(results, vec![true, false, false, false, true, true]);
    }

    #[test]
    fn test_ellipse_filtering() {
        let center = (5.0, 5.0);
        let radius_x = 5.0;
        let radius_y = 3.0;
        let angle = 0.0;
        
        let points = vec![
            (5.0, 5.0),   // Center (inside)
            (10.0, 5.0),  // On x-axis (inside)
            (5.0, 8.0),   // On y-axis (inside)
            (15.0, 5.0),  // Outside
            (5.0, 15.0),  // Outside
        ];
        
        let results = filter_by_ellipse_batch(
            &points,
            center,
            radius_x,
            radius_y,
            angle,
        )
        .unwrap();
        
        assert_eq!(results, vec![true, true, true, false, false]);
    }
}
