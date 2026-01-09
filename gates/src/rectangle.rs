use super::error::{GateError, Result};
use super::traits::*;
use super::types::GateNode;

#[derive(Debug, Clone)]
pub struct RectangleGateGeometry {
    pub min: GateNode,
    pub max: GateNode,
}

impl GateCenter for RectangleGateGeometry {
    fn calculate_center(&self, x_param: &str, y_param: &str) -> Result<(f32, f32)> {
        let min_x = self
            .min
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle min"))?;
        let min_y = self
            .min
            .get_coordinate(y_param)
            .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle min"))?;
        let max_x = self
            .max
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle max"))?;
        let max_y = self
            .max
            .get_coordinate(y_param)
            .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle max"))?;

        Ok(((min_x + max_x) / 2.0, (min_y + max_y) / 2.0))
    }
}

impl GateContainment for RectangleGateGeometry {
    fn contains_point(&self, x: f32, y: f32, x_param: &str, y_param: &str) -> Result<bool> {
        let min_x = self
            .min
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle min"))?;
        let min_y = self
            .min
            .get_coordinate(y_param)
            .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle min"))?;
        let max_x = self
            .max
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle max"))?;
        let max_y = self
            .max
            .get_coordinate(y_param)
            .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle max"))?;

        Ok(x >= min_x && x <= max_x && y >= min_y && y <= max_y)
    }
}

impl GateBounds for RectangleGateGeometry {
    fn bounding_box(&self, x_param: &str, y_param: &str) -> Result<(f32, f32, f32, f32)> {
        let min_x = self
            .min
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle min"))?;
        let min_y = self
            .min
            .get_coordinate(y_param)
            .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle min"))?;
        let max_x = self
            .max
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle max"))?;
        let max_y = self
            .max
            .get_coordinate(y_param)
            .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle max"))?;

        Ok((min_x, min_y, max_x, max_y))
    }
}

impl GateValidation for RectangleGateGeometry {
    fn is_valid(&self, x_param: &str, y_param: &str) -> Result<bool> {
        // Must have valid coordinates
        if self.min.get_coordinate(x_param).is_none()
            || self.min.get_coordinate(y_param).is_none()
            || self.max.get_coordinate(x_param).is_none()
            || self.max.get_coordinate(y_param).is_none()
        {
            return Ok(false);
        }

        // Min must be less than max
        let min_x = self.min.get_coordinate(x_param).unwrap();
        let min_y = self.min.get_coordinate(y_param).unwrap();
        let max_x = self.max.get_coordinate(x_param).unwrap();
        let max_y = self.max.get_coordinate(y_param).unwrap();

        Ok(min_x < max_x && min_y < max_y)
    }
}

impl GateGeometryOps for RectangleGateGeometry {
    fn gate_type_name(&self) -> &'static str {
        "Rectangle"
    }
}
