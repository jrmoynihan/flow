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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// The geometry of a gate, defining its shape in 2D parameter space.
///
/// Gates can be one of three geometric types:
/// - **Polygon**: A closed or open polygonal region defined by vertices
/// - **Rectangle**: An axis-aligned rectangular region
/// - **Ellipse**: An elliptical region with optional rotation
///
/// All geometries operate in raw data coordinate space and are parameterized
/// by two channel names (x and y parameters).
///
/// # Example
///
/// ```rust
/// use flow_gates::{GateGeometry, GateNode};
///
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
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        }
    }

    /// Get a descriptive name for this gate type
    pub fn gate_type_name(&self) -> &'static str {
        match self {
            GateGeometry::Polygon { .. } => "Polygon",
            GateGeometry::Rectangle { .. } => "Rectangle",
            GateGeometry::Ellipse { .. } => "Ellipse",
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
        }
    }
}

impl GateGeometryOps for GateGeometry {
    fn gate_type_name(&self) -> &'static str {
        match self {
            GateGeometry::Polygon { .. } => "Polygon",
            GateGeometry::Rectangle { .. } => "Rectangle",
            GateGeometry::Ellipse { .. } => "Ellipse",
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
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
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
