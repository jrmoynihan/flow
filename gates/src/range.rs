use super::error::{GateError, Result};
use super::traits::*;
use super::types::GateNode;

#[derive(Debug, Clone)]
pub struct RangeGateGeometry {
    pub min: GateNode,
    pub max: GateNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeAxis {
    X,
    Y,
}

impl RangeGateGeometry {
    /// Detect which axis this range gate spans by checking which parameter
    /// has coordinates on the min node.
    pub fn detect_axis(&self, x_param: &str, y_param: &str) -> Option<RangeAxis> {
        if self.min.get_coordinate(x_param).is_some() {
            Some(RangeAxis::X)
        } else if self.min.get_coordinate(y_param).is_some() {
            Some(RangeAxis::Y)
        } else {
            None
        }
    }

    pub fn resolve_bounds(&self, x_param: &str, y_param: &str) -> Result<(RangeAxis, f32, f32)> {
        if let (Some(lo), Some(hi)) = (
            self.min.get_coordinate(x_param),
            self.max.get_coordinate(x_param),
        ) {
            return Ok((RangeAxis::X, lo, hi));
        }
        if let (Some(lo), Some(hi)) = (
            self.min.get_coordinate(y_param),
            self.max.get_coordinate(y_param),
        ) {
            return Ok((RangeAxis::Y, lo, hi));
        }
        Err(GateError::missing_parameter(
            format!("{x_param} or {y_param}"),
            "range",
        ))
    }
}

impl GateCenter for RangeGateGeometry {
    fn calculate_center(&self, x_param: &str, y_param: &str) -> Result<(f32, f32)> {
        let (axis, lo, hi) = self.resolve_bounds(x_param, y_param)?;
        let mid = (lo + hi) / 2.0;
        match axis {
            RangeAxis::X => Ok((mid, 0.0)),
            RangeAxis::Y => Ok((0.0, mid)),
        }
    }
}

impl GateContainment for RangeGateGeometry {
    fn contains_point(&self, x: f32, y: f32, x_param: &str, y_param: &str) -> Result<bool> {
        let (axis, lo, hi) = self.resolve_bounds(x_param, y_param)?;
        let val = match axis {
            RangeAxis::X => x,
            RangeAxis::Y => y,
        };
        Ok(val >= lo && val <= hi)
    }
}

impl GateBounds for RangeGateGeometry {
    fn bounding_box(&self, x_param: &str, y_param: &str) -> Result<(f32, f32, f32, f32)> {
        let (axis, lo, hi) = self.resolve_bounds(x_param, y_param)?;
        match axis {
            RangeAxis::X => Ok((lo, f32::NEG_INFINITY, hi, f32::INFINITY)),
            RangeAxis::Y => Ok((f32::NEG_INFINITY, lo, f32::INFINITY, hi)),
        }
    }
}

impl GateValidation for RangeGateGeometry {
    fn is_valid(&self, x_param: &str, y_param: &str) -> Result<bool> {
        match self.resolve_bounds(x_param, y_param) {
            Ok((_, lo, hi)) => Ok(lo.is_finite() && hi.is_finite() && lo < hi),
            Err(_) => Ok(false),
        }
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
        assert!(
            range
                .contains_point(300.0, 999.0, "FL1-A", "SSC-A")
                .unwrap()
        );
    }

    #[test]
    fn containment_outside_below() {
        let range = make_range(100.0, 500.0);
        assert!(!range.contains_point(50.0, 200.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn containment_outside_above() {
        let range = make_range(100.0, 500.0);
        assert!(
            !range
                .contains_point(600.0, 200.0, "FL1-A", "SSC-A")
                .unwrap()
        );
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
        assert!(
            range
                .contains_point(300.0, f32::NEG_INFINITY, "FL1-A", "SSC-A")
                .unwrap()
        );
        assert!(
            range
                .contains_point(300.0, f32::INFINITY, "FL1-A", "SSC-A")
                .unwrap()
        );
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

    fn make_y_range(min_val: f32, max_val: f32) -> RangeGateGeometry {
        let min = GateNode::new("range_min").with_coordinate("SSC-A", min_val);
        let max = GateNode::new("range_max").with_coordinate("SSC-A", max_val);
        RangeGateGeometry { min, max }
    }

    #[test]
    fn y_axis_containment_inside() {
        let range = make_y_range(100.0, 500.0);
        assert!(
            range
                .contains_point(999.0, 300.0, "FL1-A", "SSC-A")
                .unwrap()
        );
    }

    #[test]
    fn y_axis_containment_outside() {
        let range = make_y_range(100.0, 500.0);
        assert!(!range.contains_point(999.0, 50.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn y_axis_center() {
        let range = make_y_range(100.0, 500.0);
        let (cx, cy) = range.calculate_center("FL1-A", "SSC-A").unwrap();
        assert!((cx - 0.0).abs() < f32::EPSILON);
        assert!((cy - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn y_axis_bounding_box() {
        let range = make_y_range(100.0, 500.0);
        let (min_x, min_y, max_x, max_y) = range.bounding_box("FL1-A", "SSC-A").unwrap();
        assert_eq!(min_x, f32::NEG_INFINITY);
        assert_eq!(max_x, f32::INFINITY);
        assert!((min_y - 100.0).abs() < f32::EPSILON);
        assert!((max_y - 500.0).abs() < f32::EPSILON);
    }

    #[test]
    fn y_axis_valid() {
        let range = make_y_range(100.0, 500.0);
        assert!(range.is_valid("FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn detect_axis_x() {
        let range = make_range(100.0, 500.0);
        assert_eq!(range.detect_axis("FL1-A", "SSC-A"), Some(RangeAxis::X));
    }

    #[test]
    fn detect_axis_y() {
        let range = make_y_range(100.0, 500.0);
        assert_eq!(range.detect_axis("FL1-A", "SSC-A"), Some(RangeAxis::Y));
    }
}
