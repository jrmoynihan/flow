use super::error::{GateError, Result};
use super::traits::*;
use super::types::GateNode;

#[derive(Debug, Clone)]
pub struct PolygonGateGeometry {
    pub nodes: Vec<GateNode>,
    pub closed: bool,
}

impl GateCenter for PolygonGateGeometry {
    fn calculate_center(&self, x_param: &str, y_param: &str) -> Result<(f32, f32)> {
        let (sum_x, sum_y, count) = self
            .nodes
            .iter()
            .filter_map(|node| {
                let x = node.get_coordinate(x_param)?;
                let y = node.get_coordinate(y_param)?;
                Some((x, y))
            })
            .fold((0.0, 0.0, 0), |(sx, sy, c), (x, y)| (sx + x, sy + y, c + 1));

        if count > 0 {
            Ok((sum_x / count as f32, sum_y / count as f32))
        } else {
            Err(GateError::invalid_geometry(
                "Polygon has no valid coordinates",
            ))
        }
    }
}

impl GateContainment for PolygonGateGeometry {
    fn contains_point(&self, x: f32, y: f32, x_param: &str, y_param: &str) -> Result<bool> {
        if !self.closed {
            return Ok(false);
        }

        // Extract coordinates
        let coords: Vec<(f32, f32)> = self
            .nodes
            .iter()
            .filter_map(|node| {
                let x_coord = node.get_coordinate(x_param)?;
                let y_coord = node.get_coordinate(y_param)?;
                Some((x_coord, y_coord))
            })
            .collect();

        if coords.len() < 3 {
            return Ok(false);
        }

        // Ray casting algorithm
        Ok(point_in_polygon(x, y, &coords))
    }
}

impl GateBounds for PolygonGateGeometry {
    fn bounding_box(&self, x_param: &str, y_param: &str) -> Result<(f32, f32, f32, f32)> {
        let coords: Vec<(f32, f32)> = self
            .nodes
            .iter()
            .filter_map(|node| {
                let x_coord = node.get_coordinate(x_param)?;
                let y_coord = node.get_coordinate(y_param)?;
                Some((x_coord, y_coord))
            })
            .collect();

        if coords.is_empty() {
            return Err(GateError::invalid_geometry(
                "No valid coordinates for bounding box",
            ));
        }

        let min_x = coords
            .iter()
            .map(|(x, _)| x)
            .fold(f32::INFINITY, |a, &b| a.min(b));
        let max_x = coords
            .iter()
            .map(|(x, _)| x)
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let min_y = coords
            .iter()
            .map(|(_, y)| y)
            .fold(f32::INFINITY, |a, &b| a.min(b));
        let max_y = coords
            .iter()
            .map(|(_, y)| y)
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        Ok((min_x, min_y, max_x, max_y))
    }
}

impl GateValidation for PolygonGateGeometry {
    fn is_valid(&self, x_param: &str, y_param: &str) -> Result<bool> {
        // Need at least 3 nodes for a valid polygon
        if self.nodes.len() < 3 {
            return Ok(false);
        }

        // All nodes must have valid coordinates
        let valid_coords = self.nodes.iter().all(|node| {
            node.get_coordinate(x_param).is_some() && node.get_coordinate(y_param).is_some()
        });

        Ok(valid_coords)
    }
}

impl GateGeometryOps for PolygonGateGeometry {
    fn gate_type_name(&self) -> &'static str {
        "Polygon"
    }
}

/// Point-in-polygon using ray casting algorithm
fn point_in_polygon(x: f32, y: f32, polygon: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let n = polygon.len();

    for i in 0..n {
        let (x1, y1) = polygon[i];
        let (x2, y2) = polygon[(i + 1) % n];

        if ((y1 > y) != (y2 > y)) && (x < (x2 - x1) * (y - y1) / (y2 - y1) + x1) {
            inside = !inside;
        }
    }

    inside
}
