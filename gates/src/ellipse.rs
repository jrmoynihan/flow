use super::error::{GateError, Result};
use super::traits::*;
use super::types::GateNode;

#[derive(Debug, Clone)]
pub struct EllipseGateGeometry {
    pub center: GateNode,
    pub radius_x: f32,
    pub radius_y: f32,
    pub angle: f32, // rotation angle in radians
}

impl GateCenter for EllipseGateGeometry {
    fn calculate_center(&self, x_param: &str, y_param: &str) -> Result<(f32, f32)> {
        let cx = self
            .center
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "ellipse center"))?;
        let cy = self
            .center
            .get_coordinate(y_param)
            .ok_or_else(|| GateError::missing_parameter(y_param, "ellipse center"))?;

        Ok((cx, cy))
    }
}

impl GateContainment for EllipseGateGeometry {
    fn contains_point(&self, x: f32, y: f32, x_param: &str, y_param: &str) -> Result<bool> {
        let cx = self
            .center
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "ellipse center"))?;
        let cy = self
            .center
            .get_coordinate(y_param)
            .ok_or_else(|| GateError::missing_parameter(y_param, "ellipse center"))?;

        // Rotate point around center by -angle
        let cos_a = self.angle.cos();
        let sin_a = self.angle.sin();
        let dx = x - cx;
        let dy = y - cy;
        let rotated_x = dx * cos_a + dy * sin_a;
        let rotated_y = -dx * sin_a + dy * cos_a;

        // Check if point is inside axis-aligned ellipse
        let normalized = (rotated_x / self.radius_x).powi(2) + (rotated_y / self.radius_y).powi(2);
        Ok(normalized <= 1.0)
    }
}

impl GateBounds for EllipseGateGeometry {
    fn bounding_box(&self, x_param: &str, y_param: &str) -> Result<(f32, f32, f32, f32)> {
        let cx = self
            .center
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "ellipse center"))?;
        let cy = self
            .center
            .get_coordinate(y_param)
            .ok_or_else(|| GateError::missing_parameter(y_param, "ellipse center"))?;

        // For rotated ellipse, calculate actual bounding box
        // Conservative approach: use max radius
        let max_radius = self.radius_x.max(self.radius_y);

        Ok((
            cx - max_radius,
            cy - max_radius,
            cx + max_radius,
            cy + max_radius,
        ))
    }
}

impl GateValidation for EllipseGateGeometry {
    fn is_valid(&self, x_param: &str, y_param: &str) -> Result<bool> {
        // Must have valid center coordinates
        if self.center.get_coordinate(x_param).is_none()
            || self.center.get_coordinate(y_param).is_none()
        {
            return Ok(false);
        }

        // Radii must be positive
        Ok(self.radius_x > 0.0 && self.radius_y > 0.0)
    }
}

impl GateGeometryOps for EllipseGateGeometry {
    fn gate_type_name(&self) -> &'static str {
        "Ellipse"
    }
}
