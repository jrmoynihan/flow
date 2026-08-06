use crate::error::{GateError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

fn is_false(v: &bool) -> bool {
    !v
}

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
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct GateNode {
    /// Unique identifier for this node
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub id: Arc<str>,
    /// Coordinates in raw data space, keyed by channel name.
    ///
    /// Using `Arc<str>` for channel names reduces allocations since they're shared
    /// across multiple nodes and gates.
    #[serde(with = "arc_str_hashmap")]
    #[cfg_attr(feature = "typescript", ts(type = "Record<string, number>"))]
    #[cfg_attr(feature = "specta", specta(type = std::collections::HashMap<String, f32>))]
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
/// Which parameter-processing state the gate's node coordinates are expressed in.
///
/// Every `Gate` must declare its coordinate space. Filters compare gate coordinates
/// against event data, and if the spaces don't match, the filter produces wrong
/// results silently. Encoding the space on the gate (and requiring filter inputs
/// to be tagged with their own space) makes a mismatch a typed error rather than
/// a silent-data-corruption bug.
///
/// Mirrors `flow_fcs::ParameterProcessing` (same two variants, same serde names).
/// The app intentionally doesn't track file storage form (raw vs unmixed) here —
/// the only decision the filter needs is "apply spillover compensation, or not".
/// Unmixed-stored data just reads as "raw" to the filter (the file's columns are
/// what they are); applying compensation on top is the `Compensated` case.
///
/// The spatial transform (e.g. arcsinh) is applied and inverted around gate
/// evaluation — gate coordinates are never stored in *transformed* space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, rename_all = "snake_case"))]
pub enum GateCoordinateSpace {
    /// Coordinates measured against stored file values — no compensation applied.
    Raw,
    /// Coordinates measured against spillover-compensated values.
    Compensated,
}

/// Provenance of a gate — who or what created it.
///
/// Lets UIs treat system-seeded gates differently from user-drawn ones. In
/// particular, the Data Canvas gate hierarchy hides [`GateOrigin::CompensationControl`]
/// gates: they are defined on control files (single stains / unstained), not the
/// experimental samples shown on the canvas, so overlaying or counting them
/// against arbitrary canvas plots is meaningless, and they are re-seeded /
/// invalidated as controls change (out-of-band edits would silently break the
/// matrix). `system_managed` alone can't distinguish them from QC gates, which
/// *should* remain visible — hence a dedicated origin.
///
/// Variants serialize as their names (`"User"`, `"Qc"`, `"CompensationControl"`),
/// matching the capitalized `ParameterProcessing` convention. Defaults to `User`;
/// the field is serialized only when non-default (see [`Gate::origin`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub enum GateOrigin {
    /// User-created gate (default).
    #[default]
    User,
    /// System-managed QC gate (PeacoQC good/bad masks and their container).
    Qc,
    /// System-managed compensation / unmixing control gate (FSC/SSC Size pools
    /// and per-channel fluorescence range gates) seeded on control files.
    /// Hidden from general gate-hierarchy browsers.
    CompensationControl,
}

impl GateOrigin {
    /// True for the default (`User`) origin. Used by serde `skip_serializing_if`
    /// so user gates don't carry a redundant `origin` key on the wire / in CRDT.
    pub fn is_user(&self) -> bool {
        matches!(self, GateOrigin::User)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, rename_all = "lowercase"))]
pub enum BooleanOperation {
    /// AND operation - events must pass all operand gates
    And,
    /// OR operation - events must pass at least one operand gate
    Or,
    /// NOT operation - events must NOT pass the operand gate (single operand)
    Not,
}

/// Direction of a threshold gate — which side of the boundary is "positive".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, rename_all = "snake_case"))]
pub enum ThresholdDirection {
    /// Events with value >= threshold are positive.
    Above,
    /// Events with value < threshold are positive.
    Below,
}

/// Source of a precomputed event mask for [`GateGeometry::Mask`] gates.
///
/// Mask gates delegate containment to an external resolver at filter time.
/// The source identifies which mask to load.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, tag = "type", rename_all = "snake_case")
)]
pub enum MaskSource {
    /// Quality-control mask produced by PeacoQC (or similar).
    ///
    /// When `file_guid` is `None`, the mask is resolved from the filtering
    /// context (the file currently being plotted). This is the normal case for
    /// Global QC gates — one gate applies to all files, resolving per-file at
    /// filter time. When `Some`, a specific file's mask is used (rare; for
    /// file-pinned display or stats only).
    Qc {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file_guid: Option<Arc<str>>,
        #[serde(default)]
        invert: bool,
    },
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
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, tag = "type"))]
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
    /// 1D range gate on a single parameter (min <= x <= max, y ignored)
    Range {
        min: GateNode,
        max: GateNode,
    },
    /// 1D threshold gate: events on the `direction` side of `value_node` are positive.
    Threshold {
        value_node: GateNode,
        direction: ThresholdDirection,
    },
    /// A quadrant gate: one gate owning its dividers and 4 sub-quadrants,
    /// matching GatingML's `QuadrantGate` encapsulation.
    ///
    /// Unlike a single shape, a quadrant gate defines *four* sub-populations
    /// (the corners). Each [`QuadrantSub`] carries its own stable id so a corner
    /// is individually addressable — for per-corner stats, as a child gate's
    /// `parent_id`, and in the hierarchy tree — even though one gate owns them
    /// all. Corner-selective containment is exposed via
    /// [`GateGeometry::contains_point_corner`] and friends; the gate-level
    /// [`GateContainment::contains_point`] errors (a whole quadrant has no
    /// single answer), exactly like [`GateGeometry::Boolean`].
    ///
    /// Boxed so this variant doesn't bloat the enum (clippy `large_enum_variant`).
    /// `ts(inline)` flattens the inner struct's fields next to the `type` tag,
    /// matching serde's internally-tagged-newtype wire shape
    /// (`{"type":"QuadrantGate","dividers":[…],"quadrants":[…]}`).
    #[cfg_attr(feature = "typescript", ts(inline))]
    QuadrantGate(Box<QuadrantGate>),
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
    /// Precomputed event mask (e.g. from QC). Containment is resolved at filter
    /// time via a [`MaskResolver`](crate::filtering::MaskResolver) — geometry
    /// methods like `contains_point` return an error, same as [`Boolean`].
    Mask {
        source: MaskSource,
    },
}

impl GateGeometry {
    /// Borrow a [`crate::quadrant::QuadrantGateGeometry`] view over a
    /// `QuadrantGate` variant. Returns `None` for any other geometry.
    fn quadrant_geometry(&self) -> Option<crate::quadrant::QuadrantGateGeometry<'_>> {
        if let GateGeometry::QuadrantGate(q) = self {
            Some(crate::quadrant::QuadrantGateGeometry {
                dividers: &q.dividers,
                quadrants: &q.quadrants,
            })
        } else {
            None
        }
    }

    /// Index of a sub-quadrant (corner) by its stable id, or `None` if this is
    /// not a quadrant gate or the id is unknown.
    pub fn quadrant_corner_index(&self, sub_id: &str) -> Option<usize> {
        self.quadrant_geometry()?.corner_index(sub_id)
    }

    /// Stable ids of every sub-quadrant, in order. Empty for non-quadrant gates.
    pub fn quadrant_sub_ids(&self) -> Vec<Arc<str>> {
        match self {
            GateGeometry::QuadrantGate(q) => q.quadrants.iter().map(|s| s.id.clone()).collect(),
            _ => Vec::new(),
        }
    }

    /// Corner-selective point containment: is `(x, y)` inside the sub-quadrant
    /// identified by `sub_id`? Errors if this is not a quadrant gate or the id
    /// is unknown. This is the "pass the corner as a parameter" entry point.
    pub fn contains_point_corner(
        &self,
        sub_id: &str,
        x: f32,
        y: f32,
        x_param: &str,
        y_param: &str,
    ) -> Result<bool> {
        let geom = self
            .quadrant_geometry()
            .ok_or_else(|| GateError::invalid_geometry("not a quadrant gate"))?;
        let corner = geom
            .corner_index(sub_id)
            .ok_or_else(|| GateError::invalid_geometry("unknown sub-quadrant id"))?;
        geom.contains_point_corner(corner, x, y, x_param, y_param)
    }

    /// Corner-selective batch containment for the sub-quadrant `sub_id`.
    pub fn contains_points_batch_corner(
        &self,
        sub_id: &str,
        points: &[(f32, f32)],
        x_param: &str,
        y_param: &str,
    ) -> Result<Vec<bool>> {
        let geom = self
            .quadrant_geometry()
            .ok_or_else(|| GateError::invalid_geometry("not a quadrant gate"))?;
        let corner = geom
            .corner_index(sub_id)
            .ok_or_else(|| GateError::invalid_geometry("unknown sub-quadrant id"))?;
        geom.contains_points_batch_corner(corner, points, x_param, y_param)
    }

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
            GateGeometry::Range { min, max } => {
                let range = crate::range::RangeGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                range.bounding_box(x_param, y_param).ok()
            }
            GateGeometry::Threshold {
                value_node,
                direction,
            } => {
                let t = crate::threshold::ThresholdGateGeometry {
                    value_node: value_node.clone(),
                    direction: *direction,
                };
                t.bounding_box(x_param, y_param).ok()
            }
            GateGeometry::QuadrantGate(_) => {
                // A quadrant partitions the whole plane into 4 corners; the
                // union has no meaningful finite box. Per-corner bounds are
                // available via the corner-selective geometry view.
                None
            }
            GateGeometry::Boolean { .. } | GateGeometry::Mask { .. } => {
                // Boolean/Mask gates don't have a direct bounding box
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
            GateGeometry::Range { min, max } => {
                let range = crate::range::RangeGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                range.calculate_center(x_param, y_param)
            }
            GateGeometry::Threshold {
                value_node,
                direction,
            } => {
                let t = crate::threshold::ThresholdGateGeometry {
                    value_node: value_node.clone(),
                    direction: *direction,
                };
                t.calculate_center(x_param, y_param)
            }
            GateGeometry::QuadrantGate(_) => self
                .quadrant_geometry()
                .ok_or_else(|| GateError::invalid_geometry("not a quadrant gate"))?
                .dividers_center(x_param, y_param),
            GateGeometry::Boolean { .. } | GateGeometry::Mask { .. } => Err(
                GateError::invalid_geometry("Boolean/Mask gates do not have a direct center point"),
            ),
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
            GateGeometry::Range { min, max } => {
                let range = crate::range::RangeGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                range.contains_point(x, y, x_param, y_param)
            }
            GateGeometry::Threshold {
                value_node,
                direction,
            } => {
                let t = crate::threshold::ThresholdGateGeometry {
                    value_node: value_node.clone(),
                    direction: *direction,
                };
                t.contains_point(x, y, x_param, y_param)
            }
            GateGeometry::QuadrantGate(_) => Err(GateError::invalid_geometry(
                "QuadrantGate has 4 sub-populations; use contains_point_corner(sub_id, ...)",
            )),
            GateGeometry::Boolean { .. } | GateGeometry::Mask { .. } => {
                Err(GateError::invalid_geometry(
                    "Boolean/Mask gates require external resolution to check containment",
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
            GateGeometry::Range { min, max } => {
                let range = crate::range::RangeGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                let (axis, lo, hi) = range.resolve_bounds(x_param, y_param)?;
                match axis {
                    crate::range::RangeAxis::X => {
                        crate::batch_filtering::filter_by_range_batch(points, (lo, hi))
                    }
                    crate::range::RangeAxis::Y => {
                        crate::batch_filtering::filter_by_range_y_batch(points, (lo, hi))
                    }
                }
            }
            GateGeometry::Threshold {
                value_node,
                direction,
            } => {
                let t = crate::threshold::ThresholdGateGeometry {
                    value_node: value_node.clone(),
                    direction: *direction,
                };
                let (axis, val) = t.resolve_value(x_param, y_param)?;
                let above = matches!(direction, ThresholdDirection::Above);
                match axis {
                    crate::threshold::ThresholdAxis::X => {
                        crate::batch_filtering::filter_by_threshold_x_batch(points, val, above)
                    }
                    crate::threshold::ThresholdAxis::Y => {
                        crate::batch_filtering::filter_by_threshold_y_batch(points, val, above)
                    }
                }
            }
            GateGeometry::QuadrantGate(_) => Err(GateError::invalid_geometry(
                "QuadrantGate has 4 sub-populations; use contains_points_batch_corner(sub_id, ...)",
            )),
            GateGeometry::Boolean { .. } | GateGeometry::Mask { .. } => {
                Err(GateError::invalid_geometry(
                    "Boolean/Mask gates require external resolution to check containment",
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
            GateGeometry::Range { min, max } => {
                let range = crate::range::RangeGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                range.is_valid(x_param, y_param)
            }
            GateGeometry::Threshold {
                value_node,
                direction,
            } => {
                let t = crate::threshold::ThresholdGateGeometry {
                    value_node: value_node.clone(),
                    direction: *direction,
                };
                t.is_valid(x_param, y_param)
            }
            GateGeometry::QuadrantGate(_) => self
                .quadrant_geometry()
                .ok_or_else(|| GateError::invalid_geometry("not a quadrant gate"))?
                .dividers_valid(x_param, y_param),
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
            GateGeometry::Mask { .. } => Ok(true),
        }
    }

    /// Get a descriptive name for this gate type
    pub fn gate_type_name(&self) -> &'static str {
        match self {
            GateGeometry::Polygon { .. } => "Polygon",
            GateGeometry::Rectangle { .. } => "Rectangle",
            GateGeometry::Ellipse { .. } => "Ellipse",
            GateGeometry::Range { .. } => "Range",
            GateGeometry::Threshold { .. } => "Threshold",
            GateGeometry::QuadrantGate(_) => "QuadrantGate",
            GateGeometry::Boolean { .. } => "Boolean",
            GateGeometry::Mask { .. } => "Mask",
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
            GateGeometry::Range { min, max } => {
                let range = crate::range::RangeGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                range.calculate_center(x_param, y_param)
            }
            GateGeometry::Threshold {
                value_node,
                direction,
            } => {
                let t = crate::threshold::ThresholdGateGeometry {
                    value_node: value_node.clone(),
                    direction: *direction,
                };
                t.calculate_center(x_param, y_param)
            }
            GateGeometry::QuadrantGate(_) => self
                .quadrant_geometry()
                .ok_or_else(|| GateError::invalid_geometry("not a quadrant gate"))?
                .dividers_center(x_param, y_param),
            GateGeometry::Boolean { .. } | GateGeometry::Mask { .. } => Err(
                GateError::invalid_geometry("Boolean/Mask gates do not have a direct center point"),
            ),
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
            GateGeometry::Range { min, max } => {
                let range = crate::range::RangeGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                range.contains_point(x, y, x_param, y_param)
            }
            GateGeometry::Threshold {
                value_node,
                direction,
            } => {
                let t = crate::threshold::ThresholdGateGeometry {
                    value_node: value_node.clone(),
                    direction: *direction,
                };
                t.contains_point(x, y, x_param, y_param)
            }
            GateGeometry::QuadrantGate(_) => Err(GateError::invalid_geometry(
                "QuadrantGate has 4 sub-populations; use contains_point_corner(sub_id, ...)",
            )),
            GateGeometry::Boolean { .. } | GateGeometry::Mask { .. } => {
                Err(GateError::invalid_geometry(
                    "Boolean/Mask gates require external resolution to check containment",
                ))
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
            GateGeometry::Range { min, max } => {
                let range = crate::range::RangeGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                range.bounding_box(x_param, y_param)
            }
            GateGeometry::Threshold {
                value_node,
                direction,
            } => {
                let t = crate::threshold::ThresholdGateGeometry {
                    value_node: value_node.clone(),
                    direction: *direction,
                };
                t.bounding_box(x_param, y_param)
            }
            GateGeometry::QuadrantGate(_) => Err(GateError::invalid_geometry(
                "QuadrantGate partitions the plane; use the corner-selective bounding box",
            )),
            GateGeometry::Boolean { .. } | GateGeometry::Mask { .. } => Err(
                GateError::invalid_geometry("Boolean/Mask gates do not have a direct bounding box"),
            ),
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
            GateGeometry::Range { min, max } => {
                let range = crate::range::RangeGateGeometry {
                    min: min.clone(),
                    max: max.clone(),
                };
                range.is_valid(x_param, y_param)
            }
            GateGeometry::Threshold {
                value_node,
                direction,
            } => {
                let t = crate::threshold::ThresholdGateGeometry {
                    value_node: value_node.clone(),
                    direction: *direction,
                };
                t.is_valid(x_param, y_param)
            }
            GateGeometry::QuadrantGate(_) => self
                .quadrant_geometry()
                .ok_or_else(|| GateError::invalid_geometry("not a quadrant gate"))?
                .dividers_valid(x_param, y_param),
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
            GateGeometry::Mask { .. } => Ok(true),
        }
    }
}

impl GateGeometryOps for GateGeometry {
    fn gate_type_name(&self) -> &'static str {
        match self {
            GateGeometry::Polygon { .. } => "Polygon",
            GateGeometry::Rectangle { .. } => "Rectangle",
            GateGeometry::Ellipse { .. } => "Ellipse",
            GateGeometry::Range { .. } => "Range",
            GateGeometry::Threshold { .. } => "Threshold",
            GateGeometry::QuadrantGate(_) => "QuadrantGate",
            GateGeometry::Boolean { .. } => "Boolean",
            GateGeometry::Mask { .. } => "Mask",
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
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "name")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, tag = "name"))]
pub enum GateMode {
    /// Gate applies to all files
    Global,
    /// Gate applies only to a specific file
    FileSpecific {
        /// File GUID
        #[serde(with = "arc_str_serde")]
        #[cfg_attr(feature = "typescript", ts(type = "string"))]
        #[cfg_attr(feature = "specta", specta(type = String))]
        guid: Arc<str>,
    },
    /// Gate applies to a group of files
    FileGroup {
        /// List of file GUIDs
        #[serde(with = "arc_str_vec")]
        #[cfg_attr(feature = "typescript", ts(type = "string[]"))]
        #[cfg_attr(feature = "specta", specta(type = Vec<String>))]
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

/// Label position stored as **per-channel** offsets from the gate's first node, in raw
/// data coordinates. Each entry pairs a channel name (a key into a `GateNode`'s
/// coordinate map) with an offset in that channel's units.
///
/// Two-channel gates carry two entries (one per gate axis); one-channel (range,
/// threshold) gates carry one entry on the bounded channel. Pairing offsets with
/// channel names — instead of with positional `x`/`y` slots — makes label rendering
/// transpose-aware: each offset goes through the plot pipeline of the channel it
/// belongs to, regardless of which screen axis that channel currently occupies.
///
/// Backward compatibility: the legacy `{ offset_x, offset_y }` JSON shape is
/// accepted by the deserializer but cannot be auto-migrated without the parent
/// `Gate`'s parameters (channel names live there, not on `LabelPosition`). The
/// legacy form deserializes into a sentinel-keyed map; callers must invoke
/// [`Gate::fixup_label_position`] after load to replace sentinels with real
/// channel names.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LabelPosition {
    #[cfg_attr(feature = "specta", specta(type = std::collections::HashMap<String, f32>))]
    pub offsets: std::collections::HashMap<Arc<str>, f32>,
}

/// Sentinel keys used by the legacy `{offset_x, offset_y}` deserialization path.
/// Replaced with real channel names by [`Gate::fixup_label_position`] when the
/// gate is loaded and its `parameters` are known.
pub const LABEL_LEGACY_X_KEY: &str = "__legacy_offset_x__";
pub const LABEL_LEGACY_Y_KEY: &str = "__legacy_offset_y__";

impl Serialize for LabelPosition {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("LabelPosition", 1)?;
        // Serialize the HashMap with `&str` keys so output is `{"channel": value, ...}`.
        let map: std::collections::HashMap<&str, &f32> =
            self.offsets.iter().map(|(k, v)| (k.as_ref(), v)).collect();
        s.serialize_field("offsets", &map)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for LabelPosition {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum LabelPositionDe {
            New {
                offsets: std::collections::HashMap<String, f32>,
            },
            Legacy {
                offset_x: f32,
                offset_y: f32,
            },
        }

        match LabelPositionDe::deserialize(deserializer)? {
            LabelPositionDe::New { offsets } => Ok(LabelPosition {
                offsets: offsets
                    .into_iter()
                    .map(|(k, v)| (Arc::from(k.as_str()), v))
                    .collect(),
            }),
            LabelPositionDe::Legacy { offset_x, offset_y } => {
                let mut map = std::collections::HashMap::with_capacity(2);
                map.insert(Arc::from(LABEL_LEGACY_X_KEY), offset_x);
                map.insert(Arc::from(LABEL_LEGACY_Y_KEY), offset_y);
                Ok(LabelPosition { offsets: map })
            }
        }
    }
}

/// Which parameters (channels) a gate uses: full 2D gating, or 1D range on one channel.
///
/// - **TwoChannel** — polygon, rectangle, ellipse, boolean operands: the gate is defined
///   in the `(x, y)` channel pair (order matches the plot / event column pair).
/// - **OneChannel** — range gate: `channel` carries the min/max bounds. The gate has no
///   second axis; consumers needing a "companion" axis (e.g. to build a 2D event plane for
///   rendering) must supply it from the *plot context*, not from the gate.
/// - **NoChannel** — mask gates: parameter-agnostic, applies to all plots regardless of
///   axes. The gate filters events by precomputed membership, not geometric containment.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum GateParameters {
    TwoChannel { x: Arc<str>, y: Arc<str> },
    OneChannel { channel: Arc<str> },
    NoChannel,
}

impl GateParameters {
    /// Whether this gate should appear on a plot whose axes are `(plot_x, plot_y)` (either order).
    pub fn matches_plot_parameters(&self, plot_x: &str, plot_y: &str) -> bool {
        match self {
            GateParameters::TwoChannel { x, y } => {
                (x.as_ref() == plot_x && y.as_ref() == plot_y)
                    || (x.as_ref() == plot_y && y.as_ref() == plot_x)
            }
            GateParameters::OneChannel { channel, .. } => {
                channel.as_ref() == plot_x || channel.as_ref() == plot_y
            }
            GateParameters::NoChannel => true,
        }
    }

    /// The single constrained axis for [`GateGeometry::Range`], if parameters are [`OneChannel`].
    pub fn range_channel_name(&self) -> Option<&str> {
        match self {
            GateParameters::OneChannel { channel, .. } => Some(channel.as_ref()),
            GateParameters::TwoChannel { .. } | GateParameters::NoChannel => None,
        }
    }

    /// All channel names referenced by this gate. Used to determine whether a gate's
    /// channels are present in a spillover matrix (matrix-scoping).
    pub fn channel_names(&self) -> Vec<&str> {
        match self {
            GateParameters::TwoChannel { x, y } => vec![x.as_ref(), y.as_ref()],
            GateParameters::OneChannel { channel } => vec![channel.as_ref()],
            GateParameters::NoChannel => vec![],
        }
    }
}

impl Serialize for GateParameters {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum GateParametersSer<'a> {
            TwoChannels { x: &'a str, y: &'a str },
            OneChannel { channel: &'a str },
            NoChannels {},
        }
        match self {
            GateParameters::TwoChannel { x, y } => GateParametersSer::TwoChannels {
                x: x.as_ref(),
                y: y.as_ref(),
            }
            .serialize(serializer),
            GateParameters::OneChannel { channel } => GateParametersSer::OneChannel {
                channel: channel.as_ref(),
            }
            .serialize(serializer),
            GateParameters::NoChannel => GateParametersSer::NoChannels {}.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for GateParameters {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum GateParametersDe {
            Legacy([String; 2]),
            Tagged(GateParametersTaggedDe),
        }
        // `companion` is accepted for backward compatibility with workspaces saved before
        // the field was removed. The value is silently dropped — it was never authoritative.
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum GateParametersTaggedDe {
            TwoChannels {
                x: String,
                y: String,
            },
            OneChannel {
                channel: String,
                #[serde(default)]
                #[allow(dead_code)]
                companion: Option<String>,
            },
            NoChannels {},
        }
        let helper = GateParametersDe::deserialize(deserializer)?;
        Ok(match helper {
            GateParametersDe::Legacy([a, b]) => GateParameters::TwoChannel {
                x: Arc::from(a.as_str()),
                y: Arc::from(b.as_str()),
            },
            GateParametersDe::Tagged(GateParametersTaggedDe::TwoChannels { x, y }) => {
                GateParameters::TwoChannel {
                    x: Arc::from(x.as_str()),
                    y: Arc::from(y.as_str()),
                }
            }
            GateParametersDe::Tagged(GateParametersTaggedDe::OneChannel { channel, .. }) => {
                GateParameters::OneChannel {
                    channel: Arc::from(channel.as_str()),
                }
            }
            GateParametersDe::Tagged(GateParametersTaggedDe::NoChannels {}) => {
                GateParameters::NoChannel
            }
        })
    }
}

/// Build [`GateParameters`] from geometry plus the plot's `(x, y)` channel names at creation time.
pub fn gate_parameters_from_geometry_and_axes(
    geometry: &GateGeometry,
    plot_x: Arc<str>,
    plot_y: Arc<str>,
) -> GateParameters {
    // Mask gates are parameter-agnostic.
    if matches!(geometry, GateGeometry::Mask { .. }) {
        return GateParameters::NoChannel;
    }
    // A quadrant gate is inherently two-channel; derive its axes from the two
    // dividers (one per channel) so it matches a plot regardless of the axes
    // passed in. Falls back to the plot axes if dividers are incomplete.
    if let GateGeometry::QuadrantGate(q) = geometry {
        let x = q
            .dividers
            .first()
            .map(|d| d.channel.clone())
            .unwrap_or_else(|| plot_x.clone());
        let y = q
            .dividers
            .get(1)
            .map(|d| d.channel.clone())
            .unwrap_or_else(|| plot_y.clone());
        return GateParameters::TwoChannel { x, y };
    }
    let one_channel_node = match geometry {
        GateGeometry::Range { min, .. } => Some(min),
        GateGeometry::Threshold { value_node, .. } => Some(value_node),
        _ => None,
    };
    if let Some(node) = one_channel_node {
        let channel = node
            .coordinates
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| plot_x.clone());
        return GateParameters::OneChannel { channel };
    }
    GateParameters::TwoChannel {
        x: plot_x,
        y: plot_y,
    }
}

/// Links a composite gate back to the source gates it was derived from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, tag = "type", rename_all = "snake_case")
)]
pub enum DerivedFrom {
    RangePair {
        #[serde(with = "arc_str_serde")]
        #[cfg_attr(feature = "typescript", ts(type = "string"))]
        #[cfg_attr(feature = "specta", specta(type = String))]
        x_range_id: Arc<str>,
        #[serde(with = "arc_str_serde")]
        #[cfg_attr(feature = "typescript", ts(type = "string"))]
        #[cfg_attr(feature = "specta", specta(type = String))]
        y_range_id: Arc<str>,
        /// When `true`, geometry updates on either source range propagate to this gate.
        link_live: bool,
    },
    ThresholdPair {
        #[serde(with = "arc_str_serde")]
        #[cfg_attr(feature = "typescript", ts(type = "string"))]
        #[cfg_attr(feature = "specta", specta(type = String))]
        x_threshold_id: Arc<str>,
        #[serde(with = "arc_str_serde")]
        #[cfg_attr(feature = "typescript", ts(type = "string"))]
        #[cfg_attr(feature = "specta", specta(type = String))]
        y_threshold_id: Arc<str>,
        /// When `true`, geometry updates on either source threshold propagate to this gate.
        link_live: bool,
    },
}

/// A quadrant gate: dividers (boundaries) plus the sub-quadrants (corners) they
/// partition the plane into. One [`Gate`] owns this whole structure, mirroring
/// GatingML's `<gating:QuadrantGate>` (N `<gating:divider>` + M `<gating:Quadrant>`).
///
/// The standard 2-D quadrant has 2 dividers (one per channel, one value each)
/// and 4 sub-quadrants, each placed by 2 positions. The structure generalizes
/// to GatingML's n-ary case, but the app constructs and renders only the 2×4 form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct QuadrantGate {
    pub dividers: Vec<QuadrantDivider>,
    pub quadrants: Vec<QuadrantSub>,
}

/// One `<gating:divider>`: a boundary on ONE channel. Axis-agnostic and
/// channel-keyed exactly like [`GateGeometry::Threshold`]'s `value_node` — the
/// axis it maps to is resolved at containment time from the plot's `(x, y)`.
///
/// `values` is a `Vec` to match GatingML (a divider may carry several values for
/// n-ary splits); the standard 2-D quadrant uses exactly one value per divider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct QuadrantDivider {
    #[serde(with = "arc_str_serde")]
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub id: Arc<str>,
    /// GatingML `fcs-dimension` — the channel this divider bounds.
    #[serde(with = "arc_str_serde")]
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub channel: Arc<str>,
    pub values: Vec<f32>,
}

/// One `<gating:Quadrant>` sub-element: an addressable sub-population (a corner).
/// Carries its own stable `id` (used for per-corner stats, child-gate parenting,
/// and the hierarchy tree) and a user-facing `label` (replaces the per-corner
/// gate name AND the old `QuadrantGroup` label record). `positions` place this
/// corner relative to each divider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct QuadrantSub {
    #[serde(with = "arc_str_serde")]
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub id: Arc<str>,
    pub label: String,
    pub positions: Vec<QuadrantPosition>,
    /// Optional on-plot label offset for THIS corner, in raw data coordinates
    /// (per-channel offsets, like [`Gate::label_position`]). A quadrant draws
    /// four independent labels, so each corner owns its own offset rather than
    /// the gate carrying a single one. `None` = default screen-corner placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "typescript",
        ts(optional, type = "{ offsets: Record<string, number> }")
    )]
    pub label_position: Option<LabelPosition>,
}

/// One `<gating:position>`: which side of `divider_ref` this sub-quadrant sits on.
///
/// `location` is the divider value the corner is placed at; `direction`
/// (`Above` = value `>= location` inclusive, `Below` = `< location` exclusive)
/// is carried explicitly so containment preserves the same boundary semantics as
/// [`GateGeometry::Threshold`] — every point lands in exactly one corner,
/// including points exactly on a divider line. On GatingML export, `direction`
/// collapses back into the standard location-based representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct QuadrantPosition {
    #[serde(with = "arc_str_serde")]
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub divider_ref: Arc<str>,
    pub location: f32,
    pub direction: ThresholdDirection,
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
/// use flow_gates::{Gate, GateGeometry, GateNode, GateCoordinateSpace, geometry::*};
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
///     GateCoordinateSpace::Raw,
/// );
///
/// // Get parameter names
/// assert_eq!(gate.x_parameter_channel_name(), "FSC-A");
/// assert_eq!(gate.two_channel_axes(), Some(("FSC-A", "SSC-A")));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct Gate {
    #[serde(with = "arc_str_serde")]
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub id: Arc<str>,
    pub name: String,
    pub geometry: GateGeometry,
    pub mode: GateMode,
    /// Channels this gate uses in raw/event space (see [`GateParameters`]).
    /// `GateParameters` has a custom serde impl (tagged, snake_case), so its TS
    /// shape is given inline here rather than deriving `TS` on the enum.
    #[cfg_attr(
        feature = "typescript",
        ts(
            type = "{ type: \"two_channel\", x: string, y: string } | { type: \"one_channel\", channel: string } | { type: \"no_channel\" }"
        )
    )]
    pub parameters: GateParameters,
    /// The parameter-processing state the node coordinates are expressed in.
    /// Filters compare against event data in the same space; a mismatch is a
    /// typed error, not silent corruption. No default — every gate must declare.
    pub coordinate_space: GateCoordinateSpace,
    /// Optional label position as offset from first node in raw data coordinates.
    /// `LabelPosition` has a custom serde impl (`{ offsets }`, with a legacy
    /// accept-only form), so its TS shape is given inline.
    #[cfg_attr(
        feature = "typescript",
        ts(type = "{ offsets: Record<string, number> } | null")
    )]
    pub label_position: Option<LabelPosition>,
    /// Source gates this gate was derived from, if any (Rectangle from ranges, Quadrant from thresholds).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional))]
    pub derived_from: Option<DerivedFrom>,
    /// Parent gate id, or `None` for a root gate. This is the AUTHORITATIVE,
    /// serialized source of truth for the gate hierarchy (mirrors GatingML's
    /// `parent_id` IDREF attribute). [`crate::GateHierarchy`] is a derived
    /// in-memory index rebuilt from these via [`GateHierarchy::from_gates`];
    /// it exists only for fast child/root/ancestor queries and cycle checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional))]
    pub parent_id: Option<Arc<str>>,
    /// Per-file or per-group geometry overrides. Keys are file GUIDs or group IDs.
    /// When resolving geometry for a specific file, precedence is:
    /// file-specific override > group override > base `geometry`.
    /// (`Arc<str>` keys map to `string`, so this is `Record<string, GateGeometry>`
    /// in TS; always present — empty object when there are no overrides.)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<Arc<str>, GateGeometry>,
    /// System-managed gates (e.g. QC) cannot be renamed, deleted, or moved by the user.
    /// Serialized only when true (`skip_serializing_if`), but always present after
    /// deserialize (defaults false), so the TS type is a plain `boolean`, not optional.
    #[serde(default, skip_serializing_if = "is_false")]
    pub system_managed: bool,
    /// Spillover group this gate was drawn against, if the gate's channels are fluorescent.
    /// `None` for legacy gates, FSC/SSC/Time gates, or any gate drawn on channels absent
    /// from all spillover matrices — those gates are unscoped and visible on all files.
    /// When `Some(group_id)`, the gate is only geometrically valid for files whose
    /// `FILE_TO_GROUP` maps to this group id; files from other groups see it as mismatched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional))]
    pub spillover_group_id: Option<Arc<str>>,
    /// Data context this gate belongs to. `None` keeps legacy gates unscoped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional, type = "string"))]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub data_context_id: Option<Arc<str>>,
    /// Provenance of this gate — who/what created it (see [`GateOrigin`]).
    /// Serialized only when non-default (`User`) via `skip_serializing_if`, but
    /// always present after deserialize (absent keys default to `User`), so the
    /// TS type is a plain `GateOrigin`, not optional — mirroring `system_managed`.
    /// Consumers must still tolerate a missing key on the wire (it reads as
    /// `undefined`, which is not `"CompensationControl"`, so the default is safe).
    #[serde(default, skip_serializing_if = "GateOrigin::is_user")]
    pub origin: GateOrigin,
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
    /// * `coordinate_space` - Which processing state the node coordinates are in
    pub fn new(
        id: impl Into<Arc<str>>,
        name: impl Into<String>,
        geometry: GateGeometry,
        plot_x_param: impl Into<Arc<str>>,
        plot_y_param: impl Into<Arc<str>>,
        coordinate_space: GateCoordinateSpace,
    ) -> Self {
        let px = plot_x_param.into();
        let py = plot_y_param.into();
        let parameters = gate_parameters_from_geometry_and_axes(&geometry, px, py);

        Self {
            id: id.into(),
            name: name.into(),
            geometry,
            mode: GateMode::Global,
            parameters,
            coordinate_space,
            label_position: None,
            derived_from: None,
            parent_id: None,
            overrides: BTreeMap::new(),
            system_managed: false,
            spillover_group_id: None,
            data_context_id: None,
            origin: GateOrigin::User,
        }
    }

    /// True if this gate is relevant to a plot with the given `x`/`y` channel names
    /// (including axis swap). For one-channel (range) gates, matches if the constrained
    /// `channel` equals either plot axis.
    pub fn matches_plot_parameters(&self, plot_x: &str, plot_y: &str) -> bool {
        self.parameters.matches_plot_parameters(plot_x, plot_y)
    }

    /// Resolve the effective geometry for a given file, considering overrides.
    ///
    /// Precedence: file-specific override > group override > base geometry.
    /// `group_ids` should be ordered by specificity (most-specific first) if
    /// multiple groups could match.
    pub fn effective_geometry(&self, file_guid: &str, group_ids: &[&str]) -> &GateGeometry {
        if let Some(geom) = self.overrides.get(file_guid) {
            return geom;
        }
        for gid in group_ids {
            if let Some(geom) = self.overrides.get(*gid) {
                return geom;
            }
        }
        &self.geometry
    }

    /// For range (`OneChannel`) gates, the single bounded axis; for 2D gates, `None`.
    pub fn range_channel_name(&self) -> Option<&str> {
        self.parameters.range_channel_name()
    }

    /// `Some((x, y))` for [`GateParameters::TwoChannel`]; `None` for one-channel or no-channel gates.
    pub fn two_channel_axes(&self) -> Option<(&str, &str)> {
        match &self.parameters {
            GateParameters::TwoChannel { x, y } => Some((x.as_ref(), y.as_ref())),
            GateParameters::OneChannel { .. } | GateParameters::NoChannel => None,
        }
    }

    /// The constrained axis for a one-channel (range) gate, if any.
    pub fn one_channel_axis(&self) -> Option<&str> {
        match &self.parameters {
            GateParameters::OneChannel { channel, .. } => Some(channel.as_ref()),
            GateParameters::TwoChannel { .. } | GateParameters::NoChannel => None,
        }
    }

    /// Primary channel the gate references in event-data space.
    ///
    /// For [`GateParameters::TwoChannel`] this is `x` (the first column of the 2-D pair).
    /// For [`GateParameters::OneChannel`] this is the bounded `channel`.
    /// For [`GateParameters::NoChannel`] (mask gates) returns `None`.
    ///
    /// Use [`Gate::two_channel_axes`] when the caller specifically needs *both* axes — that
    /// returns `None` for range gates and forces the caller to handle the 1-D case explicitly,
    /// instead of silently substituting a stale "companion" axis.
    pub fn primary_channel_name(&self) -> Option<&str> {
        match &self.parameters {
            GateParameters::TwoChannel { x, .. } => Some(x.as_ref()),
            GateParameters::OneChannel { channel } => Some(channel.as_ref()),
            GateParameters::NoChannel => None,
        }
    }

    /// Deprecated alias for [`Gate::primary_channel_name`] that panics on NoChannel.
    /// Prefer `primary_channel_name()` which returns `Option`.
    pub fn x_parameter_channel_name(&self) -> &str {
        match &self.parameters {
            GateParameters::TwoChannel { x, .. } => x.as_ref(),
            GateParameters::OneChannel { channel } => channel.as_ref(),
            GateParameters::NoChannel => "",
        }
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
    /// use flow_gates::{Gate, GateCoordinateSpace};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::rectangle("rect", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A", GateCoordinateSpace::Raw)?;
    /// assert!(gate.contains_point(300.0, 400.0)?);
    /// assert!(!gate.contains_point(50.0, 50.0)?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn contains_point(&self, x: f32, y: f32) -> Result<bool> {
        let (x_param, y_param) = self.two_channel_axes().ok_or_else(|| {
            GateError::filtering_error(
                "Gate::contains_point is 2-D only; call gate.geometry.contains_point with explicit plot axes for one-channel (range/threshold) gates",
            )
        })?;
        self.geometry.contains_point(x, y, x_param, y_param)
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
    /// use flow_gates::{Gate, GateCoordinateSpace};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::rectangle("rect", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A", GateCoordinateSpace::Raw)?;
    /// let bbox = gate.bounding_box();
    /// assert_eq!(bbox, Some((100.0, 200.0, 500.0, 600.0)));
    /// # Ok(())
    /// # }
    /// ```
    pub fn bounding_box(&self) -> Option<(f32, f32, f32, f32)> {
        let (x_param, y_param) = self.two_channel_axes()?;
        self.geometry.bounding_box(x_param, y_param)
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
    /// use flow_gates::{Gate, GateNode, GateCoordinateSpace};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::rectangle("rect", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A", GateCoordinateSpace::Raw)?;
    /// let node = GateNode::new("node1")
    ///     .with_coordinate("FSC-A", 300.0)
    ///     .with_coordinate("SSC-A", 400.0);
    /// let coords = gate.get_node_coords(&node);
    /// assert_eq!(coords, Some((300.0, 400.0)));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_node_coords(&self, node: &GateNode) -> Option<(f32, f32)> {
        let (x_param, y_param) = self.two_channel_axes()?;
        Some((node.get_coordinate(x_param)?, node.get_coordinate(y_param)?))
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
    /// use flow_gates::{Gate, GateCoordinateSpace};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate1 = Gate::rectangle("gate1", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A", GateCoordinateSpace::Raw)?;
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
            coordinate_space: self.coordinate_space,
            label_position: self.label_position.clone(),
            derived_from: self.derived_from.clone(),
            parent_id: self.parent_id.clone(),
            overrides: self.overrides.clone(),
            system_managed: self.system_managed,
            spillover_group_id: self.spillover_group_id.clone(),
            data_context_id: self.data_context_id.clone(),
            origin: self.origin,
        }
    }

    /// Replace any legacy `{offset_x, offset_y}` sentinel keys on this gate's
    /// `label_position` with the gate's real channel names. Call after
    /// deserializing a workspace saved before [`LabelPosition`] became
    /// per-channel; no-op when the label is already in the new form.
    ///
    /// Two-channel gates: [`LABEL_LEGACY_X_KEY`] -> `parameters.x`,
    /// [`LABEL_LEGACY_Y_KEY`] -> `parameters.y`.
    /// One-channel (range, threshold) gates: [`LABEL_LEGACY_X_KEY`] ->
    /// `parameters.channel`. The legacy `offset_y` is dropped (1-D gate has
    /// no second axis; orthogonal pixel falls back to plot midpoint at render).
    pub fn fixup_label_position(&mut self) {
        let Some(label_pos) = self.label_position.as_mut() else {
            return;
        };
        let legacy_x = label_pos.offsets.remove(LABEL_LEGACY_X_KEY);
        let legacy_y = label_pos.offsets.remove(LABEL_LEGACY_Y_KEY);
        if legacy_x.is_none() && legacy_y.is_none() {
            return;
        }
        match &self.parameters {
            GateParameters::TwoChannel { x, y } => {
                if let Some(v) = legacy_x {
                    label_pos.offsets.insert(x.clone(), v);
                }
                if let Some(v) = legacy_y {
                    label_pos.offsets.insert(y.clone(), v);
                }
            }
            GateParameters::OneChannel { channel } => {
                if let Some(v) = legacy_x {
                    label_pos.offsets.insert(channel.clone(), v);
                }
            }
            GateParameters::NoChannel => {}
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
    /// use flow_gates::{Gate, GateCoordinateSpace};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let coords = vec![
    ///     (100.0, 200.0),
    ///     (300.0, 200.0),
    ///     (300.0, 400.0),
    ///     (100.0, 400.0),
    /// ];
    /// let gate = Gate::polygon("poly", "Polygon", coords, "FSC-A", "SSC-A", GateCoordinateSpace::Raw)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn polygon(
        id: impl Into<Arc<str>>,
        name: impl Into<String>,
        coords: Vec<(f32, f32)>,
        x_param: impl Into<Arc<str>>,
        y_param: impl Into<Arc<str>>,
        coordinate_space: GateCoordinateSpace,
    ) -> Result<Self> {
        use crate::geometry::create_polygon_geometry;
        let x_param_arc = x_param.into();
        let y_param_arc = y_param.into();
        let geometry = create_polygon_geometry(coords, x_param_arc.as_ref(), y_param_arc.as_ref())?;
        Ok(Self::new(
            id,
            name,
            geometry,
            x_param_arc,
            y_param_arc,
            coordinate_space,
        ))
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
    /// use flow_gates::{Gate, GateCoordinateSpace};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::rectangle("rect", "Rectangle", (100.0, 200.0), (500.0, 600.0), "FSC-A", "SSC-A", GateCoordinateSpace::Raw)?;
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
        coordinate_space: GateCoordinateSpace,
    ) -> Result<Self> {
        use crate::geometry::create_rectangle_geometry;
        let x_param_arc = x_param.into();
        let y_param_arc = y_param.into();
        let coords = vec![min, max];
        let geometry =
            create_rectangle_geometry(coords, x_param_arc.as_ref(), y_param_arc.as_ref())?;
        Ok(Self::new(
            id,
            name,
            geometry,
            x_param_arc,
            y_param_arc,
            coordinate_space,
        ))
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
    /// use flow_gates::{Gate, GateCoordinateSpace};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let gate = Gate::ellipse("ellipse", "Ellipse", (300.0, 400.0), 100.0, 50.0, 0.0, "FSC-A", "SSC-A", GateCoordinateSpace::Raw)?;
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
        coordinate_space: GateCoordinateSpace,
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
        Ok(Self::new(
            id,
            name,
            geometry,
            x_param_arc,
            y_param_arc,
            coordinate_space,
        ))
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
    coordinate_space: Option<GateCoordinateSpace>,
    label_position: Option<LabelPosition>,
    derived_from: Option<DerivedFrom>,
    parent_id: Option<Arc<str>>,
    system_managed: bool,
    data_context_id: Option<Arc<str>>,
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
            coordinate_space: None,
            label_position: None,
            derived_from: None,
            parent_id: None,
            system_managed: false,
            data_context_id: None,
        }
    }

    /// Set which parameter-processing state the gate's coordinates are expressed in.
    /// Required — `build()` errors if not set.
    pub fn coordinate_space(mut self, space: GateCoordinateSpace) -> Self {
        self.coordinate_space = Some(space);
        self
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
        let coordinate_space = self.coordinate_space.ok_or_else(|| {
            GateError::invalid_builder_state(
                "coordinate_space",
                "coordinate_space must be set before building (call .coordinate_space(...))",
            )
        })?;

        let parameters = gate_parameters_from_geometry_and_axes(&geometry, x_param, y_param);

        Ok(Gate {
            id: self.id,
            name: self.name,
            geometry,
            mode: self.mode,
            parameters,
            coordinate_space,
            label_position: self.label_position,
            derived_from: self.derived_from,
            parent_id: self.parent_id,
            overrides: BTreeMap::new(),
            system_managed: self.system_managed,
            spillover_group_id: None,
            data_context_id: self.data_context_id,
            origin: GateOrigin::User,
        })
    }

    /// Set the parent gate id (hierarchy). `None`/unset means a root gate.
    pub fn parent_id(mut self, id: impl Into<Arc<str>>) -> Self {
        self.parent_id = Some(id.into());
        self
    }

    /// Set the `derived_from` provenance for composite gates (Rectangle from ranges, Quadrant from thresholds).
    pub fn derived_from(mut self, source: DerivedFrom) -> Self {
        self.derived_from = Some(source);
        self
    }

    /// Associate this gate with a data context.
    pub fn data_context_id(mut self, id: impl Into<Arc<str>>) -> Self {
        self.data_context_id = Some(id.into());
        self
    }

    /// Mark this gate as system-managed (non-editable by users).
    pub fn system_managed(mut self) -> Self {
        self.system_managed = true;
        self
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

    pub fn serialize<S>(vec: &[Arc<str>], serializer: S) -> Result<S::Ok, S::Error>
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

#[cfg(test)]
mod tests {
    use super::{Gate, GateCoordinateSpace, GateGeometry, GateOrigin, MaskSource};
    use std::sync::Arc;

    #[test]
    fn origin_defaults_to_user_and_serde_skips_default() {
        let mut gate = Gate::new(
            "origin-gate",
            "Origin Gate",
            GateGeometry::Mask {
                source: MaskSource::Qc {
                    file_guid: None,
                    invert: false,
                },
            },
            "FSC-A",
            "SSC-A",
            GateCoordinateSpace::Raw,
        );
        assert_eq!(gate.origin, GateOrigin::User);

        // Default origin is skipped on the wire (backward-compatible).
        let serialized = serde_json::to_value(&gate).expect("serialize user-origin gate");
        assert!(serialized.get("origin").is_none());

        // Non-default origin round-trips as its variant name.
        gate.origin = GateOrigin::CompensationControl;
        let serialized = serde_json::to_value(&gate).expect("serialize control gate");
        assert_eq!(
            serialized.get("origin").and_then(serde_json::Value::as_str),
            Some("CompensationControl")
        );
        let restored: Gate = serde_json::from_value(serialized).expect("deserialize control gate");
        assert_eq!(restored.origin, GateOrigin::CompensationControl);

        // Legacy gates without the key deserialize to `User`.
        let legacy = serde_json::to_value(Gate::new(
            "legacy",
            "Legacy",
            GateGeometry::Mask {
                source: MaskSource::Qc {
                    file_guid: None,
                    invert: false,
                },
            },
            "FSC-A",
            "SSC-A",
            GateCoordinateSpace::Raw,
        ))
        .expect("serialize legacy");
        let restored_legacy: Gate = serde_json::from_value(legacy).expect("deserialize legacy");
        assert_eq!(restored_legacy.origin, GateOrigin::User);
    }

    #[test]
    fn data_context_id_serde_roundtrip_and_legacy_default() {
        let mut gate = Gate::new(
            "contextual-gate",
            "Contextual Gate",
            GateGeometry::Mask {
                source: MaskSource::Qc {
                    file_guid: None,
                    invert: false,
                },
            },
            "FSC-A",
            "SSC-A",
            GateCoordinateSpace::Raw,
        );
        gate.data_context_id = Some(Arc::from("data-context-42"));

        let serialized = serde_json::to_value(&gate).expect("serialize gate");
        assert_eq!(
            serialized
                .get("data_context_id")
                .and_then(serde_json::Value::as_str),
            Some("data-context-42")
        );
        let restored: Gate = serde_json::from_value(serialized.clone()).expect("deserialize gate");
        assert_eq!(restored.data_context_id.as_deref(), Some("data-context-42"));

        let mut legacy = serialized;
        legacy
            .as_object_mut()
            .expect("gate serializes as a JSON object")
            .remove("data_context_id");
        let restored_legacy: Gate =
            serde_json::from_value(legacy).expect("deserialize legacy gate");
        assert_eq!(restored_legacy.data_context_id, None);
    }
}
