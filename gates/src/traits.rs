use crate::error::Result;

/// Trait for gate types that can calculate their geometric center
pub trait GateCenter {
    /// Calculate the center point in raw data coordinates
    fn calculate_center(&self, x_param: &str, y_param: &str) -> Result<(f32, f32)>;
}

/// Trait for gate types that support point containment testing
pub trait GateContainment {
    /// Check if a point (in raw coordinates) is inside the gate
    fn contains_point(&self, x: f32, y: f32, x_param: &str, y_param: &str) -> Result<bool>;
}

/// Trait for gate types that have a bounding box
pub trait GateBounds {
    /// Calculate the bounding box (min_x, min_y, max_x, max_y) in raw coordinates
    fn bounding_box(&self, x_param: &str, y_param: &str) -> Result<(f32, f32, f32, f32)>;
}

/// Trait for gate types that can be validated
pub trait GateValidation {
    /// Check if the gate has valid geometry and coordinates
    fn is_valid(&self, x_param: &str, y_param: &str) -> Result<bool>;
}

/// Common trait combining all gate behaviors
pub trait GateGeometryOps: GateCenter + GateContainment + GateBounds + GateValidation {
    /// Get a descriptive name for this gate type
    fn gate_type_name(&self) -> &'static str;
}
