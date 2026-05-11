use super::error::{GateError, Result};
use super::traits::*;
use super::types::{GateNode, ThresholdDirection};

pub struct ThresholdGateGeometry {
    pub value_node: GateNode,
    pub direction: ThresholdDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdAxis {
    X,
    Y,
}

impl ThresholdGateGeometry {
    /// Resolve which plot axis this threshold is on and its value.
    pub fn resolve_value(&self, x_param: &str, y_param: &str) -> Result<(ThresholdAxis, f32)> {
        if let Some(v) = self.value_node.get_coordinate(x_param) {
            return Ok((ThresholdAxis::X, v));
        }
        if let Some(v) = self.value_node.get_coordinate(y_param) {
            return Ok((ThresholdAxis::Y, v));
        }
        Err(GateError::missing_parameter(
            &format!("{x_param} or {y_param}"),
            "threshold",
        ))
    }
}

impl GateCenter for ThresholdGateGeometry {
    fn calculate_center(&self, x_param: &str, y_param: &str) -> Result<(f32, f32)> {
        let (axis, val) = self.resolve_value(x_param, y_param)?;
        match axis {
            ThresholdAxis::X => Ok((val, 0.0)),
            ThresholdAxis::Y => Ok((0.0, val)),
        }
    }
}

impl GateContainment for ThresholdGateGeometry {
    fn contains_point(&self, x: f32, y: f32, x_param: &str, y_param: &str) -> Result<bool> {
        let (axis, threshold) = self.resolve_value(x_param, y_param)?;
        let val = match axis {
            ThresholdAxis::X => x,
            ThresholdAxis::Y => y,
        };
        Ok(match self.direction {
            ThresholdDirection::Above => val >= threshold,
            ThresholdDirection::Below => val < threshold,
        })
    }
}

impl GateBounds for ThresholdGateGeometry {
    fn bounding_box(&self, x_param: &str, y_param: &str) -> Result<(f32, f32, f32, f32)> {
        let (axis, threshold) = self.resolve_value(x_param, y_param)?;
        match (axis, self.direction) {
            (ThresholdAxis::X, ThresholdDirection::Above) => {
                Ok((threshold, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY))
            }
            (ThresholdAxis::X, ThresholdDirection::Below) => {
                Ok((f32::NEG_INFINITY, f32::NEG_INFINITY, threshold, f32::INFINITY))
            }
            (ThresholdAxis::Y, ThresholdDirection::Above) => {
                Ok((f32::NEG_INFINITY, threshold, f32::INFINITY, f32::INFINITY))
            }
            (ThresholdAxis::Y, ThresholdDirection::Below) => {
                Ok((f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, threshold))
            }
        }
    }
}

impl GateValidation for ThresholdGateGeometry {
    fn is_valid(&self, x_param: &str, y_param: &str) -> Result<bool> {
        match self.resolve_value(x_param, y_param) {
            Ok((_, val)) => Ok(val.is_finite()),
            Err(_) => Ok(false),
        }
    }
}

impl GateGeometryOps for ThresholdGateGeometry {
    fn gate_type_name(&self) -> &'static str {
        "Threshold"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_threshold(channel: &str, value: f32, direction: ThresholdDirection) -> ThresholdGateGeometry {
        ThresholdGateGeometry {
            value_node: GateNode::new("threshold_value").with_coordinate(channel, value),
            direction,
        }
    }

    #[test]
    fn above_x_contains_at_threshold() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Above);
        assert!(t.contains_point(500.0, 0.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn above_x_contains_above_threshold() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Above);
        assert!(t.contains_point(1000.0, 0.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn above_x_excludes_below_threshold() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Above);
        assert!(!t.contains_point(499.0, 0.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn below_x_excludes_at_threshold() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Below);
        assert!(!t.contains_point(500.0, 0.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn below_x_contains_strictly_below() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Below);
        assert!(t.contains_point(499.0, 0.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn y_axis_above_contains_at_threshold() {
        let t = make_threshold("SSC-A", 200.0, ThresholdDirection::Above);
        assert!(t.contains_point(0.0, 200.0, "FL1-A", "SSC-A").unwrap());
        assert!(t.contains_point(0.0, 300.0, "FL1-A", "SSC-A").unwrap());
        assert!(!t.contains_point(0.0, 199.0, "FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn bounding_box_above_x() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Above);
        let (min_x, min_y, max_x, max_y) = t.bounding_box("FL1-A", "SSC-A").unwrap();
        assert_eq!(min_x, 500.0);
        assert_eq!(max_x, f32::INFINITY);
        assert_eq!(min_y, f32::NEG_INFINITY);
        assert_eq!(max_y, f32::INFINITY);
    }

    #[test]
    fn bounding_box_below_x() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Below);
        let (min_x, min_y, max_x, max_y) = t.bounding_box("FL1-A", "SSC-A").unwrap();
        assert_eq!(min_x, f32::NEG_INFINITY);
        assert_eq!(max_x, 500.0);
        assert_eq!(min_y, f32::NEG_INFINITY);
        assert_eq!(max_y, f32::INFINITY);
    }

    #[test]
    fn validity_finite_value() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Above);
        assert!(t.is_valid("FL1-A", "SSC-A").unwrap());
    }

    #[test]
    fn validity_wrong_param() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Above);
        assert!(!t.is_valid("NONEXISTENT", "SSC-A").unwrap());
    }

    #[test]
    fn gate_type_name() {
        let t = make_threshold("FL1-A", 500.0, ThresholdDirection::Above);
        assert_eq!(t.gate_type_name(), "Threshold");
    }
}
