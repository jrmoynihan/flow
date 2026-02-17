use crate::error::{GateError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// A node in a gate, representing a control point with coordinates in raw data space.
///
/// Gate nodes store coordinates for multiple channels, allowing gates to be defined
/// in multi-dimensional space. Coordinates are stored as `f32` values in raw data units.
///
/// # Example
///
/// ```rust
/// use flow_gates::GateNode;
///
/// let node = GateNode::new("node1")
///     .with_coordinate("FSC-A", 1000.0)
///     .with_coordinate("SSC-A", 2000.0);
///
/// assert_eq!(node.get_coordinate("FSC-A"), Some(1000.0));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GateNode {
    /// Unique identifier for this node
    pub id: Arc<str>,
    /// Coordinates in raw data space, keyed by channel name.
    ///
    /// Using `Arc<str>` for channel names reduces allocations since they're shared
    /// across multiple nodes and gates.
    #[serde(with = "arc_str_hashmap")]
    pub coordinates: HashMap<Arc<str>, f32>,
}

impl GateNode {
    /// Create a new gate node with the given ID.
    ///
    /// The node starts with no coordinates. Use `with_coordinate` or `set_coordinate`
    /// to add coordinate values.
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self {
            id: id.into(),
            coordinates: HashMap::new(),
        }
    }

    /// Add a coordinate value using the builder pattern.
    ///
    /// Returns `self` for method chaining.
    pub fn with_coordinate(mut self, channel: impl Into<Arc<str>>, value: f32) -> Self {
        self.coordinates.insert(channel.into(), value);
        self
    }

    /// Get a coordinate value for the specified channel.
    ///
    /// Returns `None` if the channel is not present in this node.
    pub fn get_coordinate(&self, channel: &str) -> Option<f32> {
        self.coordinates.get(channel).copied()
    }

    pub fn set_coordinate(&mut self, channel: impl Into<Arc<str>>, value: f32) {
        self.coordinates.insert(channel.into(), value);
    }
}

/// Boolean operation for combining gates
///
/// Boolean gates combine multiple gates using logical operations:
/// - **And**: Events must pass all operand gates
/// - **Or**: Events must pass at least one operand gate
/// - **Not**: Events must NOT pass the operand gate (complement)
///
/// # Example
///
/// ```rust
/// use flow_gates::BooleanOperation;
///
/// let and_op = BooleanOperation::And;
/// let or_op = BooleanOperation::Or;
/// let not_op = BooleanOperation::Not;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BooleanOperation {
    /// AND operation - events must pass all operand gates
    And,
    /// OR operation - events must pass at least one operand gate
    Or,
    /// NOT operation - events must NOT pass the operand gate (single operand)
    Not,
}

impl BooleanOperation {
    /// Get the expected number of operands for this operation
    ///
    /// # Returns
    /// - `And`: `None` (any number >= 2)
    /// - `Or`: `None` (any number >= 2)
    /// - `Not`: `Some(1)` (exactly one operand)
    pub fn expected_operand_count(&self) -> Option<usize> {
        match self {
            BooleanOperation::And | BooleanOperation::Or => None, // At least 2
            BooleanOperation::Not => Some(1),
        }
    }

    /// Get a string representation of the operation
    pub fn as_str(&self) -> &'static str {
        match self {
            BooleanOperation::And => "and",
            BooleanOperation::Or => "or",
            BooleanOperation::Not => "not",
        }
    }
}

/// The geometry of a gate, defining its shape in 2D parameter space.
///
/// Gates can be one of four geometric types:
/// - **Polygon**: A closed or open polygonal region defined by vertices
/// - **Rectangle**: An axis-aligned rectangular region
/// - **Ellipse**: An elliptical region with optional rotation
/// - **Boolean**: A combination of other gates using boolean operations (AND, OR, NOT)
///
/// All geometries operate in raw data coordinate space and are parameterized
/// by two channel names (x and y parameters).
///
/// # Example
///
/// ```rust
/// use flow_gates::{GateGeometry, GateNode};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a rectangle gate
/// let min = GateNode::new("min")
///     .with_coordinate("FSC-A", 100.0)
///     .with_coordinate("SSC-A", 200.0);
/// let max = GateNode::new("max")
///     .with_coordinate("FSC-A", 500.0)
///     .with_coordinate("SSC-A", 600.0);
///
/// let geometry = GateGeometry::Rectangle { min, max };
///
/// // Check if a point is inside
/// let inside = geometry.contains_point(300.0, 400.0, "FSC-A", "SSC-A")?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum GateGeometry {
    Polygon {
        nodes: Vec<GateNode>,
        closed: bool,
    },
    Rectangle {
        min: GateNode,
        max: GateNode,
    },
    Ellipse {
        center: GateNode,
        radius_x: f32,
        radius_y: f32,
        angle: f32, // rotation angle in radians
    },
    /// Boolean gate combining other gates with logical operations
    ///
    /// Boolean gates reference other gates by ID and combine their results
    /// using AND, OR, or NOT operations. The referenced gates must be resolved
    /// externally when filtering events.
    Boolean {
        /// The boolean operation to apply
        operation: BooleanOperation,
        /// IDs of the gates to combine (gate IDs, not the gates themselves)
        operands: Vec<Arc<str>>,
    },
}

impl GateGeometry {
    /// Calculate the bounding box for this geometry in the specified parameter space
    pub fn bounding_box(&self, x_param: &str, y_param: &str) -> Option<(f32, f32, f32, f32)> {
        match self {
            GateGeometry::Polygon { nodes, .. } => {
                let mut min_x = f32::MAX;
                let mut min_y = f32::MAX;
                let mut max_x = f32::MIN;
                let mut max_y = f32::MIN;

                for node in nodes {
                    if let (Some(x), Some(y)) =
                        (node.get_coordinate(x_param), node.get_coordinate(y_param))
                    {
                        min_x = min_x.min(x);
                        min_y = min_y.min(y);
                        max_x = max_x.max(x);
                        max_y = max_y.max(y);
                    }
                }

                if min_x < max_x && min_y < max_y {
                    Some((min_x, min_y, max_x, max_y))
                } else {
                    None
                }
            }
            GateGeometry::Rectangle { min, max } => {
                if let (Some(min_x), Some(min_y), Some(max_x), Some(max_y)) = (
                    min.get_coordinate(x_param),
                    min.get_coordinate(y_param),
                    max.get_coordinate(x_param),
                    max.get_coordinate(y_param),
                ) {
                    Some((min_x, min_y, max_x, max_y))
                } else {
                    None
                }
            }
            GateGeometry::Ellipse {
                center,
                radius_x,
                radius_y,
                angle,
            } => {
                if let (Some(cx), Some(cy)) = (
                    center.get_coordinate(x_param),
                    center.get_coordinate(y_param),
                ) {
                    let cos_a = angle.cos();
                    let sin_a = angle.sin();

                    // Maximum extents along each axis after rotation
                    let extent_x = ((radius_x * cos_a).powi(2) + (radius_y * sin_a).powi(2)).sqrt();
                    let extent_y = ((radius_x * sin_a).powi(2) + (radius_y * cos_a).powi(2)).sqrt();

                    Some((cx - extent_x, cy - extent_y, cx + extent_x, cy + extent_y))
                } else {
                    None
                }
            }
            GateGeometry::Boolean { .. } => {
                // Boolean gates don't have a direct bounding box - would need to resolve operands
                // For now, return None to indicate it can't be calculated directly
                None
            }
        }
    }

    /// Calculate the center point in raw data coordinates
    pub fn calculate_center(&self, x_param: &str, y_param: &str) -> Result<(f32, f32)> {
        match self {
            GateGeometry::Polygon { nodes, .. } => {
                let (sum_x, sum_y, count) = nodes
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
            GateGeometry::Rectangle { min, max } => {
                let min_x = min
                    .get_coordinate(x_param)
                    .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle min"))?;
                let min_y = min
                    .get_coordinate(y_param)
                    .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle min"))?;
                let max_x = max
                    .get_coordinate(x_param)
                    .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle max"))?;
                let max_y = max
                    .get_coordinate(y_param)
                    .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle max"))?;

                Ok(((min_x + max_x) / 2.0, (min_y + max_y) / 2.0))
            }
            GateGeometry::Ellipse { center, .. } => {
                let cx = center
                    .get_coordinate(x_param)
                    .ok_or_else(|| GateError::missing_parameter(x_param, "ellipse center"))?;
                let cy = center
                    .get_coordinate(y_param)
                    .ok_or_else(|| GateError::missing_parameter(y_param, "ellipse center"))?;

                Ok((cx, cy))
            }
            GateGeometry::Boolean { .. } => {
                // Boolean gates don't have a direct center - would need to resolve operands
                Err(GateError::invalid_geometry(
                    "Boolean gates do not have a direct center point",
                ))
            }
        }
    }

    /// Check if a point (in raw coordinates) is inside the gate
    pub fn contains_point(&self, x: f32, y: f32, x_param: &str, y_param: &str) -> Result<bool> {
        match self {
            GateGeometry::Polygon { nodes, closed } => {
                if !closed {
                    return Ok(false);
                }

                // Extract coordinates
                let coords: Vec<(f32, f32)> = nodes
                    .iter()
                    .filter_map(|node| {
                        Some((node.get_coordinate(x_param)?, node.get_coordinate(y_param)?))
                    })
                    .collect();

                if coords.len() < 3 {
                    return Ok(false);
                }

                // Ray casting algorithm
                Ok(point_in_polygon(x, y, &coords))
            }
            GateGeometry::Rectangle { min, max } => {
                let min_x = min
                    .get_coordinate(x_param)
                    .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle min"))?;
                let min_y = min
                    .get_coordinate(y_param)
                    .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle min"))?;
                let max_x = max
                    .get_coordinate(x_param)
                    .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle max"))?;
                let max_y = max
                    .get_coordinate(y_param)
                    .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle max"))?;

                Ok(x >= min_x && x <= max_x && y >= min_y && y <= max_y)
            }
            GateGeometry::Ellipse {
                center,
                radius_x,
                radius_y,
                angle,
            } => {
                let cx = center
                    .get_coordinate(x_param)
                    .ok_or_else(|| GateError::missing_parameter(x_param, "ellipse center"))?;
                let cy = center
                    .get_coordinate(y_param)
                    .ok_or_else(|| GateError::missing_parameter(y_param, "ellipse center"))?;

                // Rotate point around center by -angle
                let cos_a = angle.cos();
                let sin_a = angle.sin();
                let dx = x - cx;
                let dy = y - cy;
                let rotated_x = dx * cos_a + dy * sin_a;
                let rotated_y = -dx * sin_a + dy * cos_a;

                // Check if point is inside axis-aligned ellipse
                let normalized = (rotated_x / radius_x).powi(2) + (rotated_y / radius_y).powi(2);
                Ok(normalized <= 1.0)
            }
            GateGeometry::Boolean { .. } => {
                // Boolean gates require resolving referenced gates - can't check containment directly
                // This should be handled by the filtering functions that resolve gate references
                Err(GateError::invalid_geometry(
                    "Boolean gates require gate resolution to check containment",
                ))
            }
        }
    }

    /// Batch check if points (in raw coordinates) are inside the gate
    ///
    /// Uses optimized CPU-based batch filtering with Rayon parallelization.
    pub fn contains_points_batch(
        &self,
        points: &[(f32, f32)],
        x_param: &str,
        y_param: &str,
    ) -> Result<Vec<bool>> {
        match self {
            GateGeometry::Polygon { nodes, closed } => {
                if !closed {
                    return Ok(vec![false; points.len()]);
                }

                // Extract coordinates
                let coords: Vec<(f32, f32)> = nodes
                    .iter()
                    .filter_map(|node| {
                        Some((node.get_coordinate(x_param)?, node.get_coordinate(y_param)?))
                    })
                    .collect();

                if coords.len() < 3 {
                    return Ok(vec![false; points.len()]);
                }

                crate::batch_filtering::filter_by_polygon_batch(points, &coords)
            }
            GateGeometry::Rectangle { min, max } => {
                let min_x = min
                    .get_coordinate(x_param)
                    .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle min"))?;
                let min_y = min
                    .get_coordinate(y_param)
                    .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle min"))?;
                let max_x = max
                    .get_coordinate(x_param)
                    .ok_or_else(|| GateError::missing_parameter(x_param, "rectangle max"))?;
                let max_y = max
                    .get_coordinate(y_param)
                    .ok_or_else(|| GateError::missing_parameter(y_param, "rectangle max"))?;

                crate::batch_filtering::filter_by_rectangle_batch(
                    points,
                    (min_x, min_y, max_x, max_y),
                )
            }
            GateGeometry::Ellipse {
                center,
                radius_x,
                radius_y,
                angle,
            } => {
                let cx = center
                    .get_coordinate(x_param)
                    .ok_or_else(|| GateError::missing_parameter(x_param, "ellipse center"))?;
                let cy = center
                    .get_coordinate(y_param)
                    .ok_or_else(|| GateError::missing_parameter(y_param, "ellipse center"))?;

                crate::batch_filtering::filter_by_ellipse_batch(
                    points,
                    (cx, cy),
                    *radius_x,
                    *radius_y,
                    *angle,
                )
            }
            GateGeometry::Boolean { .. } => {
                // Boolean gates require resolving referenced gates - can't check containment directly
                Err(GateError::invalid_geometry(
                    "Boolean gates require gate resolution to check containment",
                ))
            }
        }
    }

    /// Check if the gate has valid geometry and coordinates
    pub fn is_valid(&self, x_param: &str, y_param: &str) -> Result<bool> {
        match self {
            GateGeometry::Polygon { nodes, .. } => {
                // Need at least 3 nodes for a valid polygon
                if nodes.len() < 3 {
                    return Ok(false);
                }

                // All nodes must have valid coordinates
                let valid_coords = nodes.iter().all(|node| {
                    node.get_coordinate(x_param).is_some() && node.get_coordinate(y_param).is_some()
                });

                Ok(valid_coords)
            }
            GateGeometry::Rectangle { min, max } => {
                // Must have valid coordinates
                if min.get_coordinate(x_param).is_none()
                    || min.get_coordinate(y_param).is_none()
                    || max.get_coordinate(x_param).is_none()
                    || max.get_coordinate(y_param).is_none()
                {
                    return Ok(false);
                }

                // Min must be less than max
                let min_x = min.get_coordinate(x_param).unwrap();
                let min_y = min.get_coordinate(y_param).unwrap();
                let max_x = max.get_coordinate(x_param).unwrap();
                let max_y = max.get_coordinate(y_param).unwrap();

                Ok(min_x < max_x && min_y < max_y)
            }
            GateGeometry::Ellipse {
                center,
                radius_x,
                radius_y,
                ..
            } => {
                // Must have valid center coordinates
                if center.get_coordinate(x_param).is_none()
                    || center.get_coordinate(y_param).is_none()
                {
                    return Ok(false);
                }

                // Radii must be positive
                Ok(radius_x > &0.0 && radius_y > &0.0)
            }
            GateGeometry::Boolean {
                operation,
                operands,
            } => {
                // Validate operand count
                match operation {
                    BooleanOperation::And | BooleanOperation::Or => {
                        if operands.len() < 2 {
                            return Ok(false);
                        }
                    }
                    BooleanOperation::Not => {
                        if operands.len() != 1 {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
        }
    }

    /// Get a descriptive name for this gate type
    pub fn gate_type_name(&self) -> &'static str {
        match self {
            GateGeometry::Polygon { .. } => "Polygon",
            GateGeometry::Rectangle { .. } => "Rectangle",
            GateGeometry::Ellipse { .. } => "Ellipse",
            GateGeometry::Boolean { .. } => "Boolean",
        }
    }
}

// Implement geometry traits for GateGeometry enum by delegating to struct implementations
use crate::traits::*;

impl GateCenter for GateGeometry {
    fn calculate_center(&self, x_param: &str, y_param: &str) -> Result<(f32, f32)> {
        match self {
            GateGeometry::Polygon { nodes, closed } => {
                let poly = crate::polygon::PolygonGateGeometry {
                    nodes: nodes.clone(),
                    closed: *closed,
                };
                poly.calculate_center(x_param, y_param)
            }
            GateGeometry::Rectangle { min, max } => {
                let rect = crate::rectangle::RectangleGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                rect.calculate_center(x_param, y_param)
            }
            GateGeometry::Ellipse {
                center,
                radius_x,
                radius_y,
                angle,
            } => {
                let ellipse = crate::ellipse::EllipseGateGeometry {
                    center: center.clone(),
                    radius_x: *radius_x,
                    radius_y: *radius_y,
                    angle: *angle,
                };
                ellipse.calculate_center(x_param, y_param)
            }
            GateGeometry::Boolean { .. } => Err(GateError::invalid_geometry(
                "Boolean gates do not have a direct center point",
            )),
        }
    }
}

impl GateContainment for GateGeometry {
    fn contains_point(&self, x: f32, y: f32, x_param: &str, y_param: &str) -> Result<bool> {
        match self {
            GateGeometry::Polygon { nodes, closed } => {
                let poly = crate::polygon::PolygonGateGeometry {
                    nodes: nodes.clone(),
                    closed: *closed,
                };
                poly.contains_point(x, y, x_param, y_param)
            }
            GateGeometry::Rectangle { min, max } => {
                let rect = crate::rectangle::RectangleGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                rect.contains_point(x, y, x_param, y_param)
            }
            GateGeometry::Ellipse {
                center,
                radius_x,
                radius_y,
                angle,
            } => {
                let ellipse = crate::ellipse::EllipseGateGeometry {
                    center: center.clone(),
                    radius_x: *radius_x,
                    radius_y: *radius_y,
                    angle: *angle,
                };
                ellipse.contains_point(x, y, x_param, y_param)
            }
            GateGeometry::Boolean { .. } => Err(GateError::invalid_geometry(
                "Boolean gates require gate resolution to check containment",
            )),
        }
    }
}

impl GateBounds for GateGeometry {
    fn bounding_box(&self, x_param: &str, y_param: &str) -> Result<(f32, f32, f32, f32)> {
        match self {
            GateGeometry::Polygon { nodes, closed } => {
                let poly = crate::polygon::PolygonGateGeometry {
                    nodes: nodes.clone(),
                    closed: *closed,
                };
                poly.bounding_box(x_param, y_param)
            }
            GateGeometry::Rectangle { min, max } => {
                let rect = crate::rectangle::RectangleGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                rect.bounding_box(x_param, y_param)
            }
            GateGeometry::Ellipse {
                center,
                radius_x,
                radius_y,
                angle,
            } => {
                let ellipse = crate::ellipse::EllipseGateGeometry {
                    center: center.clone(),
                    radius_x: *radius_x,
                    radius_y: *radius_y,
                    angle: *angle,
                };
                ellipse.bounding_box(x_param, y_param)
            }
            GateGeometry::Boolean { .. } => Err(GateError::invalid_geometry(
                "Boolean gates do not have a direct bounding box",
            )),
        }
    }
}

impl GateValidation for GateGeometry {
    fn is_valid(&self, x_param: &str, y_param: &str) -> Result<bool> {
        match self {
            GateGeometry::Polygon { nodes, closed } => {
                let poly = crate::polygon::PolygonGateGeometry {
                    nodes: nodes.clone(),
                    closed: *closed,
                };
                poly.is_valid(x_param, y_param)
            }
            GateGeometry::Rectangle { min, max } => {
                let rect = crate::rectangle::RectangleGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                rect.is_valid(x_param, y_param)
            }
            GateGeometry::Ellipse {
                center,
                radius_x,
                radius_y,
                angle,
            } => {
                let ellipse = crate::ellipse::EllipseGateGeometry {
                    center: center.clone(),
                    radius_x: *radius_x,
                    radius_y: *radius_y,
                    angle: *angle,
                };
                ellipse.is_valid(x_param, y_param)
            }
            GateGeometry::Boolean {
                operation,
                operands,
            } => {
                match operation {
                    BooleanOperation::And | BooleanOperation::Or => {
                        if operands.len() < 2 {
                            return Err(GateError::invalid_boolean_operation(
                                operation.as_str(),
                                operands.len(),
                                2,
                            ));
                        }
                    }
                    BooleanOperation::Not => {
                        if operands.len() != 1 {
                            return Err(GateError::invalid_boolean_operation(
                                operation.as_str(),
                                operands.len(),
                                1,
                            ));
                        }
                    }
                }
                Ok(true)
            }
        }
    }
}

impl GateGeometryOps for GateGeometry {
    fn gate_type_name(&self) -> &'static str {
        match self {
            GateGeometry::Polygon { .. } => "Polygon",
            GateGeometry::Rectangle { .. } => "Rectangle",
            GateGeometry::Ellipse { .. } => "Ellipse",
            GateGeometry::Boolean { .. } => "Boolean",
        }
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

/// The scope of a gate - determines which files it applies to.
///
/// Gates can be:
/// - **Global**: Applies to all files
/// - **FileSpecific**: Applies only to a single file (identified by GUID)
/// - **FileGroup**: Applies to a specific set of files
///
/// This allows gates to be shared across multiple files or restricted to
/// specific datasets.
///
/// # Example
///
/// ```rust
/// use flow_gates::GateMode;
///
/// // Global gate (applies to all files)
/// let global = GateMode::Global;
/// assert!(global.applies_to("any-file-guid"));
///
/// // File-specific gate
/// let specific = GateMode::FileSpecific { guid: "file-123".into() };
/// assert!(specific.applies_to("file-123"));
/// assert!(!specific.applies_to("file-456"));
///
/// // File group gate
/// let group = GateMode::FileGroup { guids: vec!["file-1".into(), "file-2".into()] };
/// assert!(group.applies_to("file-1"));
/// assert!(group.applies_to("file-2"));
/// assert!(!group.applies_to("file-3"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "name")]
pub enum GateMode {
    /// Gate applies to all files
    Global,
    /// Gate applies only to a specific file
    FileSpecific {
        /// File GUID
        #[serde(with = "arc_str_serde")]
        guid: Arc<str>,
    },
    /// Gate applies to a group of files
    FileGroup {
        /// List of file GUIDs
        #[serde(with = "arc_str_vec")]
        guids: Vec<Arc<str>>,
    },
}

impl GateMode {
    /// Check if this gate mode applies to the given file GUID.
    ///
    /// Returns `true` if the gate should be applied to the specified file.
    pub fn applies_to(&self, file_guid: &str) -> bool {
        match self {
            GateMode::Global => true,
            GateMode::FileSpecific { guid } => guid.as_ref() == file_guid,
            GateMode::FileGroup { guids } => guids.iter().any(|g| g.as_ref() == file_guid),
        }
    }
}

/// Label position stored as offset from the first node in raw data coordinates
/// This allows labels to move with gates when they are edited
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabelPosition {
    /// Offset in raw data coordinates from the first node
    pub offset_x: f32,
    pub offset_y: f32,
}

/// A gate represents a region of interest in flow cytometry data.
///
/// Gates define 2D regions in parameter space that can be used to filter
/// and analyze cytometry events. Each gate has:
///
/// - A unique identifier
/// - A human-readable name
/// - A geometric shape (polygon, rectangle, or ellipse)
/// - Two parameters (channels) it operates on
/// - A scope (global, file-specific, or file group)
///
/// # Example
///
/// ```rust
/// use flow_gates::{Gate, GateGeometry, GateNode, geometry::*};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a polygon gate
/// let coords = vec![
///     (100.0, 200.0),
///     (300.0, 200.0),
///     (300.0, 400.0),
///     (100.0, 400.0),
/// ];
/// let geometry = create_polygon_geometry(coords, "FSC-A", "SSC-A")?;
///
/// let gate = Gate::new(
///     "lymphocytes",
///     "Lymphocytes",
///     geometry,
///     "FSC-A",
///     "SSC-A",
/// );
///
/// // Get parameter names
/// assert_eq!(gate.x_parameter_channel_name(), "FSC-A");
/// assert_eq!(gate.y_parameter_channel_name(), "SSC-A");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gate {
    #[serde(with = "arc_str_serde")]
    pub id: Arc<str>,
    pub name: String,
    pub geometry: GateGeometry,
    pub mode: GateMode,
    /// The parameters (channels) this gate operates on (x_channel, y_channel)
    #[serde(with = "arc_str_pair")]
    pub parameters: (Arc<str>, Arc<str>),
    /// Optional label position as offset from first node in raw data coordinates
    pub label_position: Option<LabelPosition>,
}

impl Gate {
    /// Create a new gate with the specified properties.
    ///
    /// The gate is created with `GateMode::Global` by default. Set the `mode` field
    /// to make it file-specific or part of a file group.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the gate
    /// * `name` - Human-readable name for the gate
    /// * `geometry` - The geometric shape of the gate
    /// * `x_param` - Channel name for the x-axis parameter
    /// * `y_param` - Channel name for the y-axis parameter
    pub fn new(
        id: impl Into<Arc<str>>,
        name: impl Into<String>,
        geometry: GateGeometry,
        x_param: impl Into<Arc<str>>,
        y_param: impl Into<Arc<str>>,
    ) -> Self {
        let x_param = x_param.into();
        let y_param = y_param.into();

        Self {
            id: id.into(),
            name: name.into(),
            geometry,
            mode: GateMode::Global, // Default to global
            parameters: (x_param, y_param),
            label_position: None,
        }
    }

    /// Get the x parameter (channel name)
    pub fn x_parameter_channel_name(&self) -> &str {
        self.parameters.0.as_ref()
    }

    /// Get the y parameter (channel name)
    pub fn y_parameter_channel_name(&self) -> &str {
        self.parameters.1.as_ref()
    }

    /// Check if a point (in gate's parameter space) is inside the gate
    ///
    /// This is a convenience method that uses the gate's own parameters,
    /// so you don't need to specify them explicitly.
    ///
    /// # Arguments
    /// * `x` - X coordinate in raw data space
    /// * `y` - Y coordinate in raw data space
    ///
    /// # Returns
    /// `true` if the point is inside the gate, `false` otherwise
    ///
    /// # Errors
    /// Returns an error if the gate geometry is invalid or parameters are missing
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::Gate;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::rectangle("rect", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A")?;
    /// assert!(gate.contains_point(300.0, 400.0)?);
    /// assert!(!gate.contains_point(50.0, 50.0)?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn contains_point(&self, x: f32, y: f32) -> Result<bool> {
        self.geometry.contains_point(
            x,
            y,
            self.x_parameter_channel_name(),
            self.y_parameter_channel_name(),
        )
    }

    /// Get the bounding box in gate's parameter space
    ///
    /// This is a convenience method that uses the gate's own parameters.
    ///
    /// # Returns
    /// `Some((min_x, min_y, max_x, max_y))` if the bounding box can be calculated,
    /// `None` otherwise
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::Gate;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::rectangle("rect", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A")?;
    /// let bbox = gate.bounding_box();
    /// assert_eq!(bbox, Some((100.0, 200.0, 500.0, 600.0)));
    /// # Ok(())
    /// # }
    /// ```
    pub fn bounding_box(&self) -> Option<(f32, f32, f32, f32)> {
        self.geometry.bounding_box(
            self.x_parameter_channel_name(),
            self.y_parameter_channel_name(),
        )
    }

    /// Get x and y coordinates from a node for this gate's parameters
    ///
    /// This is a convenience method that extracts coordinates for the gate's
    /// x and y parameters from a node.
    ///
    /// # Arguments
    /// * `node` - The gate node to extract coordinates from
    ///
    /// # Returns
    /// `Some((x, y))` if both coordinates are present, `None` otherwise
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::{Gate, GateNode};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::rectangle("rect", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A")?;
    /// let node = GateNode::new("node1")
    ///     .with_coordinate("FSC-A", 300.0)
    ///     .with_coordinate("SSC-A", 400.0);
    /// let coords = gate.get_node_coords(&node);
    /// assert_eq!(coords, Some((300.0, 400.0)));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_node_coords(&self, node: &GateNode) -> Option<(f32, f32)> {
        Some((
            node.get_coordinate(self.x_parameter_channel_name())?,
            node.get_coordinate(self.y_parameter_channel_name())?,
        ))
    }

    /// Clone this gate with a new ID
    ///
    /// Creates a new gate with the same properties but a different ID.
    /// Useful for duplicating gates or creating variations.
    ///
    /// # Arguments
    /// * `new_id` - The new ID for the cloned gate
    ///
    /// # Returns
    /// A new `Gate` instance with the specified ID
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::Gate;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate1 = Gate::rectangle("gate1", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A")?;
    /// let gate2 = gate1.clone_with_id("gate2");
    /// assert_eq!(gate2.id.as_ref(), "gate2");
    /// assert_eq!(gate1.name, gate2.name);
    /// # Ok(())
    /// # }
    /// ```
    pub fn clone_with_id(&self, new_id: impl Into<Arc<str>>) -> Self {
        Self {
            id: new_id.into(),
            name: self.name.clone(),
            geometry: self.geometry.clone(),
            mode: self.mode.clone(),
            parameters: self.parameters.clone(),
            label_position: self.label_position.clone(),
        }
    }

    /// Create a polygon gate from coordinates
    ///
    /// Convenience constructor for creating polygon gates directly.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the gate
    /// * `name` - Human-readable name for the gate
    /// * `coords` - Vector of (x, y) coordinate tuples
    /// * `x_param` - Channel name for the x-axis parameter
    /// * `y_param` - Channel name for the y-axis parameter
    ///
    /// # Returns
    /// A new `Gate` with polygon geometry
    ///
    /// # Errors
    /// Returns an error if the coordinates are invalid (less than 3 points, non-finite values)
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::Gate;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let coords = vec![
    ///     (100.0, 200.0),
    ///     (300.0, 200.0),
    ///     (300.0, 400.0),
    ///     (100.0, 400.0),
    /// ];
    /// let gate = Gate::polygon("poly", "Polygon", coords, "FSC-A", "SSC-A")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn polygon(
        id: impl Into<Arc<str>>,
        name: impl Into<String>,
        coords: Vec<(f32, f32)>,
        x_param: impl Into<Arc<str>>,
        y_param: impl Into<Arc<str>>,
    ) -> Result<Self> {
        use crate::geometry::create_polygon_geometry;
        let x_param_arc = x_param.into();
        let y_param_arc = y_param.into();
        let geometry = create_polygon_geometry(coords, x_param_arc.as_ref(), y_param_arc.as_ref())?;
        Ok(Self::new(id, name, geometry, x_param_arc, y_param_arc))
    }

    /// Create a rectangle gate from min and max coordinates
    ///
    /// Convenience constructor for creating rectangle gates directly.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the gate
    /// * `name` - Human-readable name for the gate
    /// * `min` - (x, y) coordinates for the minimum corner
    /// * `max` - (x, y) coordinates for the maximum corner
    /// * `x_param` - Channel name for the x-axis parameter
    /// * `y_param` - Channel name for the y-axis parameter
    ///
    /// # Returns
    /// A new `Gate` with rectangle geometry
    ///
    /// # Errors
    /// Returns an error if the coordinates are invalid (min > max, non-finite values)
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::Gate;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::rectangle("rect", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn rectangle(
        id: impl Into<Arc<str>>,
        name: impl Into<String>,
        min: (f32, f32),
        max: (f32, f32),
        x_param: impl Into<Arc<str>>,
        y_param: impl Into<Arc<str>>,
    ) -> Result<Self> {
        use crate::geometry::create_rectangle_geometry;
        let x_param_arc = x_param.into();
        let y_param_arc = y_param.into();
        let coords = vec![min, max];
        let geometry =
            create_rectangle_geometry(coords, x_param_arc.as_ref(), y_param_arc.as_ref())?;
        Ok(Self::new(id, name, geometry, x_param_arc, y_param_arc))
    }

    /// Create an ellipse gate from center, radii, and angle
    ///
    /// Convenience constructor for creating ellipse gates directly.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the gate
    /// * `name` - Human-readable name for the gate
    /// * `center` - (x, y) coordinates for the center point
    /// * `radius_x` - Radius along the x-axis
    /// * `radius_y` - Radius along the y-axis
    /// * `angle` - Rotation angle in radians
    /// * `x_param` - Channel name for the x-axis parameter
    /// * `y_param` - Channel name for the y-axis parameter
    ///
    /// # Returns
    /// A new `Gate` with ellipse geometry
    ///
    /// # Errors
    /// Returns an error if the coordinates or radii are invalid (non-finite, negative radii)
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::Gate;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::ellipse("ellipse", "Ellipse", (300.0, 400.0), 100.0, 50.0, 0.0, "FSC-A", "SSC-A")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn ellipse(
        id: impl Into<Arc<str>>,
        name: impl Into<String>,
        center: (f32, f32),
        radius_x: f32,
        radius_y: f32,
        angle: f32,
        x_param: impl Into<Arc<str>>,
        y_param: impl Into<Arc<str>>,
    ) -> Result<Self> {
        use crate::geometry::create_ellipse_geometry;
        let x_param_arc = x_param.into();
        let y_param_arc = y_param.into();
        let coords = vec![center];
        let geometry = create_ellipse_geometry(coords, x_param_arc.as_ref(), y_param_arc.as_ref())?;
        // Override radii and angle since create_ellipse_geometry may calculate them differently
        let geometry = match geometry {
            GateGeometry::Ellipse { center: c, .. } => GateGeometry::Ellipse {
                center: c,
                radius_x,
                radius_y,
                angle,
            },
            _ => geometry,
        };
        Ok(Self::new(id, name, geometry, x_param_arc, y_param_arc))
    }
}

/// Builder for constructing gates with a fluent API
///
/// The `GateBuilder` provides a convenient way to construct gates step by step,
/// allowing you to set geometry, parameters, mode, and other properties before
/// finalizing the gate.
///
/// # Example
///
/// ```rust
/// use flow_gates::{GateBuilder, GateMode};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let gate = GateBuilder::new("my-gate", "My Gate")
///     .polygon(vec![(100.0, 200.0), (300.0, 200.0), (300.0, 400.0)], "FSC-A", "SSC-A")?
///     .mode(GateMode::Global)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GateBuilder {
    id: Arc<str>,
    name: String,
    geometry: Option<GateGeometry>,
    x_param: Option<Arc<str>>,
    y_param: Option<Arc<str>>,
    mode: GateMode,
    label_position: Option<LabelPosition>,
}

impl GateBuilder {
    /// Create a new gate builder
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the gate
    /// * `name` - Human-readable name for the gate
    pub fn new(id: impl Into<Arc<str>>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            geometry: None,
            x_param: None,
            y_param: None,
            mode: GateMode::Global,
            label_position: None,
        }
    }

    /// Set the geometry to a polygon
    ///
    /// This also sets the parameters from the geometry creation.
    ///
    /// # Arguments
    /// * `coords` - Vector of (x, y) coordinate tuples
    /// * `x_param` - Channel name for the x-axis parameter
    /// * `y_param` - Channel name for the y-axis parameter
    pub fn polygon(
        mut self,
        coords: Vec<(f32, f32)>,
        x_param: impl Into<Arc<str>>,
        y_param: impl Into<Arc<str>>,
    ) -> Result<Self> {
        use crate::geometry::create_polygon_geometry;
        let x_param_arc = x_param.into();
        let y_param_arc = y_param.into();
        let geometry = create_polygon_geometry(coords, x_param_arc.as_ref(), y_param_arc.as_ref())?;
        self.geometry = Some(geometry);
        self.x_param = Some(x_param_arc);
        self.y_param = Some(y_param_arc);
        Ok(self)
    }

    /// Set the geometry to a rectangle
    ///
    /// This also sets the parameters from the geometry creation.
    ///
    /// # Arguments
    /// * `min` - (x, y) coordinates for the minimum corner
    /// * `max` - (x, y) coordinates for the maximum corner
    /// * `x_param` - Channel name for the x-axis parameter
    /// * `y_param` - Channel name for the y-axis parameter
    pub fn rectangle(
        mut self,
        min: (f32, f32),
        max: (f32, f32),
        x_param: impl Into<Arc<str>>,
        y_param: impl Into<Arc<str>>,
    ) -> Result<Self> {
        use crate::geometry::create_rectangle_geometry;
        let x_param_arc = x_param.into();
        let y_param_arc = y_param.into();
        let coords = vec![min, max];
        let geometry =
            create_rectangle_geometry(coords, x_param_arc.as_ref(), y_param_arc.as_ref())?;
        self.geometry = Some(geometry);
        self.x_param = Some(x_param_arc);
        self.y_param = Some(y_param_arc);
        Ok(self)
    }

    /// Set the geometry to an ellipse
    ///
    /// This also sets the parameters from the geometry creation.
    ///
    /// # Arguments
    /// * `center` - (x, y) coordinates for the center point
    /// * `radius_x` - Radius along the x-axis
    /// * `radius_y` - Radius along the y-axis
    /// * `angle` - Rotation angle in radians
    /// * `x_param` - Channel name for the x-axis parameter
    /// * `y_param` - Channel name for the y-axis parameter
    pub fn ellipse(
        mut self,
        center: (f32, f32),
        radius_x: f32,
        radius_y: f32,
        angle: f32,
        x_param: impl Into<Arc<str>>,
        y_param: impl Into<Arc<str>>,
    ) -> Result<Self> {
        use crate::geometry::create_ellipse_geometry;
        let x_param_arc = x_param.into();
        let y_param_arc = y_param.into();
        let coords = vec![center];
        let geometry = create_ellipse_geometry(coords, x_param_arc.as_ref(), y_param_arc.as_ref())?;
        // Override radii and angle
        let geometry = match geometry {
            GateGeometry::Ellipse { center: c, .. } => GateGeometry::Ellipse {
                center: c,
                radius_x,
                radius_y,
                angle,
            },
            _ => geometry,
        };
        self.geometry = Some(geometry);
        self.x_param = Some(x_param_arc);
        self.y_param = Some(y_param_arc);
        Ok(self)
    }

    /// Set the parameters (channels) this gate operates on
    ///
    /// # Arguments
    /// * `x` - Channel name for the x-axis parameter
    /// * `y` - Channel name for the y-axis parameter
    pub fn parameters(mut self, x: impl Into<Arc<str>>, y: impl Into<Arc<str>>) -> Self {
        self.x_param = Some(x.into());
        self.y_param = Some(y.into());
        self
    }

    /// Set the gate mode (scope)
    ///
    /// # Arguments
    /// * `mode` - The gate mode (Global, FileSpecific, or FileGroup)
    pub fn mode(mut self, mode: GateMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the label position
    ///
    /// # Arguments
    /// * `position` - The label position as an offset from the first node
    pub fn label_position(mut self, position: LabelPosition) -> Self {
        self.label_position = Some(position);
        self
    }

    /// Build the gate from the builder
    ///
    /// # Returns
    /// A new `Gate` instance
    ///
    /// # Errors
    /// Returns an error if:
    /// - Geometry is not set
    /// - Parameters are not set
    /// - Builder is in an invalid state
    pub fn build(self) -> Result<Gate> {
        let geometry = self.geometry.ok_or_else(|| {
            GateError::invalid_builder_state("geometry", "Geometry must be set before building")
        })?;
        let x_param = self.x_param.ok_or_else(|| {
            GateError::invalid_builder_state("x_param", "X parameter must be set before building")
        })?;
        let y_param = self.y_param.ok_or_else(|| {
            GateError::invalid_builder_state("y_param", "Y parameter must be set before building")
        })?;

        Ok(Gate {
            id: self.id,
            name: self.name,
            geometry,
            mode: self.mode,
            parameters: (x_param, y_param),
            label_position: self.label_position,
        })
    }
}

// Custom serde helpers for Arc<str> types
mod arc_str_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    pub fn serialize<S>(arc: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        arc.as_ref().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Arc::from(s.as_str()))
    }
}

mod arc_str_vec {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    pub fn serialize<S>(vec: &Vec<Arc<str>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let strings: Vec<&str> = vec.iter().map(|arc| arc.as_ref()).collect();
        strings.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Arc<str>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Vec::<String>::deserialize(deserializer)?;
        Ok(vec.into_iter().map(|s| Arc::from(s.as_str())).collect())
    }
}

mod arc_str_pair {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    pub fn serialize<S>(pair: &(Arc<str>, Arc<str>), serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (pair.0.as_ref(), pair.1.as_ref()).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<(Arc<str>, Arc<str>), D::Error>
    where
        D: Deserializer<'de>,
    {
        let (s1, s2) = <(String, String)>::deserialize(deserializer)?;
        Ok((Arc::from(s1.as_str()), Arc::from(s2.as_str())))
    }
}

mod arc_str_hashmap {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashMap;
    use std::sync::Arc;

    pub fn serialize<S>(map: &HashMap<Arc<str>, f32>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string_map: HashMap<&str, f32> = map.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        string_map.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<Arc<str>, f32>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = HashMap::<String, f32>::deserialize(deserializer)?;
        Ok(map
            .into_iter()
            .map(|(k, v)| (Arc::from(k.as_str()), v))
            .collect())
    }
}
