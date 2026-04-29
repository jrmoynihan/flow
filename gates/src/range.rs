use super::error::{GateError, Result};
use super::traits::*;
use super::types::GateNode;

#[derive(Debug, Clone)]
pub struct RangeGateGeometry {
    pub min: GateNode,
    pub max: GateNode,
}

impl GateCenter for RangeGateGeometry {
    fn calculate_center(&self, x_param: &str, _y_param: &str) -> Result<(f32, f32)> {
        let min_x = self
            .min
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "range min"))?;
        let max_x = self
            .max
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "range max"))?;
        Ok(((min_x + max_x) / 2.0, 0.0))
    }
}

impl GateContainment for RangeGateGeometry {
    fn contains_point(&self, x: f32, _y: f32, x_param: &str, _y_param: &str) -> Result<bool> {
        let min_x = self
            .min
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "range min"))?;
        let max_x = self
            .max
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "range max"))?;
        Ok(x >= min_x && x <= max_x)
    }
}

impl GateBounds for RangeGateGeometry {
    fn bounding_box(&self, x_param: &str, _y_param: &str) -> Result<(f32, f32, f32, f32)> {
        let min_x = self
            .min
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "range min"))?;
        let max_x = self
            .max
            .get_coordinate(x_param)
            .ok_or_else(|| GateError::missing_parameter(x_param, "range max"))?;
        Ok((min_x, f32::NEG_INFINITY, max_x, f32::INFINITY))
    }
}

impl GateValidation for RangeGateGeometry {
    fn is_valid(&self, x_param: &str, _y_param: &str) -> Result<bool> {
        let min_x = match self.min.get_coordinate(x_param) {
            Some(v) if v.is_finite() => v,
            _ => return Ok(false),
        };
        let max_x = match self.max.get_coordinate(x_param) {
            Some(v) if v.is_finite() => v,
            _ => return Ok(false),
        };
        Ok(min_x < max_x)
    }
}

impl GateGeometryOps for RangeGateGeometry {
    fn gate_type_name(&self) -> &'static str {
        "Range"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_range(min_val: f32, max_val: f32) -> RangeGateGeometry {
        let min = GateNode::new("range_min").with_coordinate("FL1-A", min_val);
        let max = GateNode::new("range_max").with_coordinate("FL1-A", max_val);
        RangeGateGeometry { min, max }
    }

    #[test]
    fn containment_inside() {
        let range = make_range(100.0, 500.0);
        assert!(range.contains_point(300.0, 999.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn containment_outside_below() {
        let range = make_range(100.0, 500.0);
        assert!(!range.contains_point(50.0, 200.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn containment_outside_above() {
        let range = make_range(100.0, 500.0);
        assert!(!range.contains_point(600.0, 200.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn containment_on_boundary() {
        let range = make_range(100.0, 500.0);
        assert!(range.contains_point(100.0, 0.0, "FL1-A", "SSC-A").unwrap());
        assert!(range.contains_point(500.0, 0.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn containment_y_ignored() {
        let range = make_range(100.0, 500.0);
        assert!(range.contains_point(300.0, f32::NEG_INFINITY, "FL1-A", "SSC-A").unwrap());
        assert!(range.contains_point(300.0, f32::INFINITY, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn center_calculation() {
        let range = make_range(100.0, 500.0);
        let (cx, cy) = range.calculate_center("FL1-A", "SSC-A").unwrap();
        assert!((cx - 300.0).abs() < f32::EPSILON);
        assert!((cy - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn bounding_box_unbounded_y() {
        let range = make_range(100.0, 500.0);
        let (min_x, min_y, max_x, max_y) = range.bounding_box("FL1-A", "SSC-A").unwrap();
        assert!((min_x - 100.0).abs() < f32::EPSILON);
        assert!((max_x - 500.0).abs() < f32::EPSILON);
        assert_eq!(min_y, f32::NEG_INFINITY);
        assert_eq!(max_y, f32::INFINITY);
    }

    #[test]
    fn valid_range() {
        let range = make_range(100.0, 500.0);
        assert!(range.is_valid("FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn invalid_range_min_gt_max() {
        let range = make_range(500.0, 100.0);
        assert!(!range.is_valid("FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn invalid_range_missing_coordinate() {
        let range = make_range(100.0, 500.0);
        assert!(!range.is_valid("NONEXISTENT", "SSC-A").unwrap());
    }

    #[test]
    fn gate_type_name() {
        let range = make_range(100.0, 500.0);
        assert_eq!(range.gate_type_name(), "Range");
    }
}
