//! Event filtering and spatial indexing for efficient gate-based event selection.
//!
//! This module provides:
//! - **EventIndex**: R*-tree based spatial index for O(log n) point-in-gate queries
//! - **Event filtering functions**: Filter FCS events by gates or gate hierarchies
//! - **Caching support**: Trait-based caching for filter results
//!
//! # Performance
//!
//! For repeated filtering operations on the same dataset, use `EventIndex`:
//! - Build once: O(n log n)
//! - Query many gates: O(log n) per gate
//! - Much faster than O(n) linear scans

use crate::error::{GateError, Result};
use crate::types::{Gate, GateCoordinateSpace, GateGeometry};
use flow_fcs::Fcs;
use geo::{Coord, LineString, Point, Polygon as GeoPolygon};
use rstar::{AABB, RTree, primitives::GeomWithData};
use std::sync::Arc;

/// A pair of parameter slices tagged with the coordinate space they're in plus the
/// channel names they represent.
///
/// `filter_events_by_gate` requires this wrapper (rather than `&Fcs`) so that the
/// caller must explicitly decide whether to pass raw, compensated, or unmixed
/// data. If the space doesn't match the gate's `coordinate_space`, the filter
/// returns `Err(GateError::SpaceMismatch)` rather than silently producing wrong
/// results. The `x_param` / `y_param` labels let one-channel (range, threshold)
/// gates resolve which axis their bounded channel maps to without relying on a
/// stale companion field stored on the gate itself.
#[derive(Debug, Clone, Copy)]
pub struct EventData<'a> {
    pub space: GateCoordinateSpace,
    pub x_param: &'a str,
    pub x: &'a [f32],
    pub y_param: &'a str,
    pub y: &'a [f32],
}

impl<'a> EventData<'a> {
    pub fn new(
        space: GateCoordinateSpace,
        x_param: &'a str,
        x: &'a [f32],
        y_param: &'a str,
        y: &'a [f32],
    ) -> Self {
        Self {
            space,
            x_param,
            x,
            y_param,
            y,
        }
    }

    /// Convenience constructor for raw-space data sourced directly from an FCS.
    /// Errors if either parameter is missing or not a contiguous f32 slice.
    pub fn raw_from_fcs(fcs: &'a Fcs, x_param: &'a str, y_param: &'a str) -> Result<Self> {
        let x = fcs.get_parameter_events_slice(x_param).map_err(|e| {
            GateError::filtering_error(format!(
                "Failed to get raw parameter data for {}: {}",
                x_param, e
            ))
        })?;
        let y = fcs.get_parameter_events_slice(y_param).map_err(|e| {
            GateError::filtering_error(format!(
                "Failed to get raw parameter data for {}: {}",
                y_param, e
            ))
        })?;
        Ok(Self {
            space: GateCoordinateSpace::Raw,
            x_param,
            x,
            y_param,
            y,
        })
    }
}

/// Trait for resolving gate IDs to gate references.
///
/// This trait allows filtering functions to resolve boolean gate operands
/// to their actual gate references. Implementations can use any storage
/// mechanism (HashMap, database, etc.).
///
/// # Example
///
/// ```rust
/// use flow_gates::{GateResolver, Gate};
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// struct MyResolver {
///     gates: HashMap<Arc<str>, Gate>,
/// }
///
/// impl GateResolver for MyResolver {
///     fn resolve(&self, id: &str) -> Option<&Gate> {
///         self.gates.get(id)
///     }
/// }
/// ```
pub trait GateResolver {
    /// Resolve a gate ID to a gate reference.
    ///
    /// Returns `Some(&Gate)` if the gate exists, `None` otherwise.
    fn resolve(&self, id: &str) -> Option<&Gate>;
}

/// Simple resolver implementation using a HashMap.
impl GateResolver for std::collections::HashMap<Arc<str>, Gate> {
    fn resolve(&self, id: &str) -> Option<&Gate> {
        self.get(id)
    }
}

/// Simple resolver implementation using a slice of gates.
impl<'a> GateResolver for [(&'a str, &'a Gate)] {
    fn resolve(&self, id: &str) -> Option<&Gate> {
        self.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
    }
}

/// Resolves a [`MaskSource`] to event indices at filter time.
///
/// Mask gates delegate containment to this trait instead of performing geometric
/// calculations. The application layer implements this to load precomputed masks
/// (e.g. QC results) from disk or cache.
pub trait MaskResolver {
    /// Resolve a mask source to the set of event indices that pass.
    ///
    /// `n_events` is the total number of events in the file, needed to
    /// correctly interpret bit-packed mask files.
    fn resolve_mask(
        &self,
        source: &crate::types::MaskSource,
        n_events: usize,
    ) -> crate::error::Result<Vec<usize>>;
}

pub mod cache;
pub use cache::{FilterCache, FilterCacheKey};

/// Spatial index for efficient event filtering using R*-tree data structure.
///
/// `EventIndex` provides O(log n) spatial queries for point-in-gate operations,
/// making it ideal for repeated filtering operations on the same dataset.
///
/// The index is built once from coordinate arrays and can then be reused for
/// multiple gate filtering operations, significantly improving performance
/// compared to linear scans.
///
/// # Performance
///
/// - **Build time**: O(n log n) - one-time cost
/// - **Query time**: O(log n) per gate - much faster than O(n) linear scan
/// - **Memory**: O(n) - stores all event points
///
/// # Example
///
/// ```rust
/// use flow_gates::{EventIndex, Gate, GateGeometry, GateNode, GateCoordinateSpace};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Build index from coordinate arrays
/// let x_values: Vec<f32> = (0..10000).map(|i| i as f32).collect();
/// let y_values: Vec<f32> = (0..10000).map(|i| (i * 2) as f32).collect();
/// let index = EventIndex::build("x", &x_values, "y", &y_values)?;
///
/// // Create a gate
/// let min = GateNode::new("min").with_coordinate("x", 100.0).with_coordinate("y", 200.0);
/// let max = GateNode::new("max").with_coordinate("x", 500.0).with_coordinate("y", 600.0);
/// let gate = Gate::new(
///     "rect",
///     "Rectangle",
///     GateGeometry::Rectangle { min, max },
///     "x",
///     "y",
///     GateCoordinateSpace::Raw,
/// );
///
/// // Filter events (fast!)
/// let filtered_indices = index.filter_by_gate(&gate)?;
/// # Ok(())
/// # }
/// ```
pub struct EventIndex {
    /// R*-tree for O(log n) spatial queries
    rtree: RTree<GeomWithData<Point<f32>, usize>>,
    /// Total number of events
    event_count: usize,
    /// Channel name the rtree's `x` coordinate represents.
    x_param: Arc<str>,
    /// Channel name the rtree's `y` coordinate represents.
    y_param: Arc<str>,
}

impl EventIndex {
    /// Build a spatial index from x and y coordinate arrays.
    ///
    /// `x_param` and `y_param` are the channel names the values were sourced
    /// from. They are stored on the index so that filtering against gates with
    /// `GateParameters::OneChannel` (range, threshold) can resolve which axis
    /// the bounded channel corresponds to without relying on the gate carrying
    /// a stale companion field.
    ///
    /// This is an O(n log n) operation, but subsequent queries are O(log n).
    pub fn build(
        x_param: impl Into<Arc<str>>,
        x_values: &[f32],
        y_param: impl Into<Arc<str>>,
        y_values: &[f32],
    ) -> Result<Self> {
        if x_values.len() != y_values.len() {
            return Err(GateError::index_error(format!(
                "X and Y arrays must have the same length: {} vs {}",
                x_values.len(),
                y_values.len()
            )));
        }

        let event_count = x_values.len();

        // Create points with their indices
        let points: Vec<GeomWithData<Point<f32>, usize>> = x_values
            .iter()
            .zip(y_values.iter())
            .enumerate()
            .map(|(idx, (&x, &y))| GeomWithData::new(Point::new(x, y), idx))
            .collect();

        // Build R*-tree with bulk loading for better performance
        let rtree = RTree::bulk_load(points);

        Ok(Self {
            rtree,
            event_count,
            x_param: x_param.into(),
            y_param: y_param.into(),
        })
    }

    /// Channel name the index's x-axis represents.
    pub fn x_param(&self) -> &str {
        self.x_param.as_ref()
    }

    /// Channel name the index's y-axis represents.
    pub fn y_param(&self) -> &str {
        self.y_param.as_ref()
    }

    /// Filter events by gate geometry.
    ///
    /// Returns indices of events that fall within the gate.
    ///
    /// **Note**: Boolean gates require a resolver to resolve referenced gates.
    /// Use `filter_by_gate_with_resolver()` for boolean gates, or this will
    /// return an error.
    pub fn filter_by_gate(&self, gate: &Gate) -> Result<Vec<usize>> {
        match &gate.geometry {
            GateGeometry::Polygon { nodes, closed } => {
                if !closed || nodes.len() < 3 {
                    return Ok(Vec::new());
                }
                Ok(self.filter_by_polygon(gate))
            }
            GateGeometry::Rectangle { .. } => Ok(self.filter_by_rectangle(gate)),
            GateGeometry::Ellipse { .. } => Ok(self.filter_by_ellipse(gate)),
            GateGeometry::Range { .. } => Ok(self.filter_by_range(gate)),
            GateGeometry::Threshold { .. } => Ok(self.filter_by_threshold(gate)),
            GateGeometry::QuadrantGate(_) => Err(GateError::filtering_error(
                "QuadrantGate has 4 sub-populations. Use filter_by_quadrant_corner(gate, sub_id) instead.",
            )),
            GateGeometry::Boolean { .. } => Err(GateError::filtering_error(
                "Boolean gates require a resolver. Use filter_by_gate_with_resolver() instead.",
            )),
            GateGeometry::Mask { .. } => Err(GateError::filtering_error(
                "Mask gates require a MaskResolver. Use the hierarchy filter with a mask resolver.",
            )),
        }
    }

    /// Filter events by gate geometry with resolver support for boolean gates.
    ///
    /// This method handles all gate types, including boolean gates that reference
    /// other gates. For boolean gates, the resolver is used to resolve operand
    /// gate IDs to their actual gate references.
    ///
    /// # Arguments
    ///
    /// * `gate` - The gate to filter by
    /// * `fcs` - The FCS file (required for boolean gates to filter operand gates)
    /// * `resolver` - Optional resolver for boolean gate operands
    ///
    /// # Returns
    ///
    /// Indices of events that pass the gate, or an error if:
    /// - Boolean gate operands cannot be resolved
    /// - Filtering fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use flow_gates::{EventIndex, Gate, GateResolver, GateGeometry, GateNode, GateCoordinateSpace};
    /// use flow_fcs::Fcs;
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Build index from coordinate arrays
    /// let x_values: Vec<f32> = vec![100.0, 200.0, 300.0];
    /// let y_values: Vec<f32> = vec![100.0, 200.0, 300.0];
    /// let index = EventIndex::build("x", &x_values, "y", &y_values)?;
    ///
    /// // Create a geometric gate
    /// let min = GateNode::new("min").with_coordinate("x", 50.0).with_coordinate("y", 50.0);
    /// let max = GateNode::new("max").with_coordinate("x", 250.0).with_coordinate("y", 250.0);
    /// let gate = Gate::new("rect", "Rectangle", GateGeometry::Rectangle { min, max }, "x", "y", GateCoordinateSpace::Raw);
    ///
    /// // Works for geometric gates (resolver not needed)
    /// let indices = index.filter_by_gate_with_resolver(&gate, None, None::<&HashMap<_, _>>)?;
    ///
    /// // For boolean gates, you would need:
    /// // let fcs = Fcs::from_file("data.fcs")?;
    /// // let gate_storage: HashMap<Arc<str>, Gate> = HashMap::new();
    /// // let indices = index.filter_by_gate_with_resolver(&boolean_gate, Some(&fcs), Some(&gate_storage))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn filter_by_gate_with_resolver<R: GateResolver>(
        &self,
        gate: &Gate,
        fcs: Option<&Fcs>,
        resolver: Option<&R>,
    ) -> Result<Vec<usize>> {
        match &gate.geometry {
            GateGeometry::Polygon { nodes, closed } => {
                if !closed || nodes.len() < 3 {
                    return Ok(Vec::new());
                }
                Ok(self.filter_by_polygon(gate))
            }
            GateGeometry::Rectangle { .. } => Ok(self.filter_by_rectangle(gate)),
            GateGeometry::Ellipse { .. } => Ok(self.filter_by_ellipse(gate)),
            GateGeometry::Range { .. } => Ok(self.filter_by_range(gate)),
            GateGeometry::Threshold { .. } => Ok(self.filter_by_threshold(gate)),
            GateGeometry::QuadrantGate(_) => Err(GateError::filtering_error(
                "QuadrantGate has 4 sub-populations. Use filter_by_quadrant_corner(gate, sub_id) instead.",
            )),
            GateGeometry::Boolean {
                operation,
                operands,
            } => {
                let resolver = resolver.ok_or_else(|| {
                    GateError::filtering_error(
                        "Boolean gates require a resolver to resolve operand gates",
                    )
                })?;

                let fcs = fcs.ok_or_else(|| {
                    GateError::filtering_error(
                        "Boolean gates require FCS data to filter operand gates",
                    )
                })?;

                // Resolve operand gates
                let resolved_gates: Vec<&Gate> = operands
                    .iter()
                    .filter_map(|id| resolver.resolve(id.as_ref()))
                    .collect();

                if resolved_gates.len() != operands.len() {
                    return Err(GateError::gate_not_found(
                        "one or more boolean gate operands",
                        "could not resolve all operand gate IDs",
                    ));
                }

                // Filter using boolean operation
                filter_events_boolean(*operation, &resolved_gates, fcs, None)
            }
            GateGeometry::Mask { .. } => Err(GateError::filtering_error(
                "Mask gates require a MaskResolver. Use the hierarchy filter with a mask resolver.",
            )),
        }
    }

    /// Filter by polygon gate
    fn filter_by_polygon(&self, gate: &Gate) -> Vec<usize> {
        // Get bounding box for spatial query
        let bbox = match gate
            .geometry
            .bounding_box(self.x_param.as_ref(), self.y_param.as_ref())
        {
            Some(bounds) => bounds,
            None => return Vec::new(),
        };

        // Create AABB for R-tree query
        let aabb = AABB::from_corners(Point::new(bbox.0, bbox.1), Point::new(bbox.2, bbox.3));

        // Query R-tree for candidates within bounding box (fast)
        let candidates: Vec<_> = self.rtree.locate_in_envelope(&aabb).collect();

        // Extract polygon coordinates for batch processing
        let polygon_coords: Vec<(f32, f32)> = match &gate.geometry {
            GateGeometry::Polygon { nodes, .. } => nodes
                .iter()
                .filter_map(|node| {
                    Some((
                        node.get_coordinate(self.x_param.as_ref())?,
                        node.get_coordinate(self.y_param.as_ref())?,
                    ))
                })
                .collect(),
            _ => return Vec::new(),
        };

        if polygon_coords.len() < 3 {
            return Vec::new();
        }

        // Extract candidate points
        let candidate_points: Vec<(f32, f32)> = candidates
            .iter()
            .map(|geom| {
                let point = geom.geom();
                (point.x(), point.y())
            })
            .collect();

        let results =
            crate::batch_filtering::filter_by_polygon_batch(&candidate_points, &polygon_coords)
                .unwrap_or_default();

        // Map results back to indices
        candidates
            .into_iter()
            .zip(results)
            .filter_map(|(geom, inside)| if inside { Some(geom.data) } else { None })
            .collect()
    }

    /// Filter by rectangle gate
    fn filter_by_rectangle(&self, gate: &Gate) -> Vec<usize> {
        if let GateGeometry::Rectangle { min, max } = &gate.geometry {
            let x_param = self.x_param.as_ref();
            let y_param = self.y_param.as_ref();
            let min_x = match min.get_coordinate(x_param) {
                Some(x) => x,
                None => return Vec::new(),
            };
            let min_y = match min.get_coordinate(y_param) {
                Some(y) => y,
                None => return Vec::new(),
            };
            let max_x = match max.get_coordinate(x_param) {
                Some(x) => x,
                None => return Vec::new(),
            };
            let max_y = match max.get_coordinate(y_param) {
                Some(y) => y,
                None => return Vec::new(),
            };

            // Use AABB for fast spatial query
            let aabb = AABB::from_corners(Point::new(min_x, min_y), Point::new(max_x, max_y));
            let candidates: Vec<_> = self.rtree.locate_in_envelope(&aabb).collect();

            // Extract candidate points
            let candidate_points: Vec<(f32, f32)> = candidates
                .iter()
                .map(|geom| {
                    let point = geom.geom();
                    (point.x(), point.y())
                })
                .collect();

            let results = crate::batch_filtering::filter_by_rectangle_batch(
                &candidate_points,
                (min_x, min_y, max_x, max_y),
            )
            .unwrap_or_default();

            // Map results back to indices
            candidates
                .into_iter()
                .zip(results)
                .filter_map(|(geom, inside)| if inside { Some(geom.data) } else { None })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Filter by ellipse gate
    fn filter_by_ellipse(&self, gate: &Gate) -> Vec<usize> {
        if let GateGeometry::Ellipse {
            center,
            radius_x,
            radius_y,
            angle,
        } = &gate.geometry
        {
            let cx = match center.get_coordinate(self.x_param.as_ref()) {
                Some(x) => x,
                None => return Vec::new(),
            };
            let cy = match center.get_coordinate(self.y_param.as_ref()) {
                Some(y) => y,
                None => return Vec::new(),
            };

            let cos_angle = angle.cos();
            let sin_angle = angle.sin();

            let extent_x = ((radius_x * cos_angle).powi(2) + (radius_y * sin_angle).powi(2)).sqrt();
            let extent_y = ((radius_x * sin_angle).powi(2) + (radius_y * cos_angle).powi(2)).sqrt();

            // Use bounding box for spatial query
            let bbox = (cx - extent_x, cy - extent_y, cx + extent_x, cy + extent_y);

            let aabb = AABB::from_corners(Point::new(bbox.0, bbox.1), Point::new(bbox.2, bbox.3));

            // Get candidates from R-tree
            let candidates: Vec<_> = self.rtree.locate_in_envelope(&aabb).collect();

            // Extract candidate points
            let candidate_points: Vec<(f32, f32)> = candidates
                .iter()
                .map(|geom| {
                    let point = geom.geom();
                    (point.x(), point.y())
                })
                .collect();

            let results = crate::batch_filtering::filter_by_ellipse_batch(
                &candidate_points,
                (cx, cy),
                *radius_x,
                *radius_y,
                *angle,
            )
            .unwrap_or_default();

            // Map results back to indices
            candidates
                .into_iter()
                .zip(results)
                .filter_map(|(geom, inside)| if inside { Some(geom.data) } else { None })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Filter by range gate (1D on either plot axis, resolved from the index's stored axes).
    fn filter_by_range(&self, gate: &Gate) -> Vec<usize> {
        if let GateGeometry::Range { min, max } = &gate.geometry {
            let range = crate::range::RangeGateGeometry {
                min: min.clone(),
                max: max.clone(),
            };
            let (axis, lo, hi) =
                match range.resolve_bounds(self.x_param.as_ref(), self.y_param.as_ref()) {
                    Ok(v) => v,
                    Err(_) => return Vec::new(),
                };

            let aabb = match axis {
                crate::range::RangeAxis::X => {
                    AABB::from_corners(Point::new(lo, f32::MIN), Point::new(hi, f32::MAX))
                }
                crate::range::RangeAxis::Y => {
                    AABB::from_corners(Point::new(f32::MIN, lo), Point::new(f32::MAX, hi))
                }
            };
            let candidates: Vec<_> = self.rtree.locate_in_envelope(&aabb).collect();

            let candidate_points: Vec<(f32, f32)> = candidates
                .iter()
                .map(|geom| {
                    let point = geom.geom();
                    (point.x(), point.y())
                })
                .collect();

            let results = match axis {
                crate::range::RangeAxis::X => {
                    crate::batch_filtering::filter_by_range_batch(&candidate_points, (lo, hi))
                }
                crate::range::RangeAxis::Y => {
                    crate::batch_filtering::filter_by_range_y_batch(&candidate_points, (lo, hi))
                }
            }
            .unwrap_or_default();

            candidates
                .into_iter()
                .zip(results)
                .filter_map(|(geom, inside)| if inside { Some(geom.data) } else { None })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn filter_by_threshold(&self, gate: &Gate) -> Vec<usize> {
        if let GateGeometry::Threshold {
            value_node,
            direction,
        } = &gate.geometry
        {
            let t = crate::threshold::ThresholdGateGeometry {
                value_node: value_node.clone(),
                direction: *direction,
            };
            let (axis, val) = match t.resolve_value(self.x_param.as_ref(), self.y_param.as_ref()) {
                Ok(v) => v,
                Err(_) => return Vec::new(),
            };
            let above = matches!(direction, crate::types::ThresholdDirection::Above);

            let aabb = match (axis, above) {
                (crate::threshold::ThresholdAxis::X, true) => {
                    AABB::from_corners(Point::new(val, f32::MIN), Point::new(f32::MAX, f32::MAX))
                }
                (crate::threshold::ThresholdAxis::X, false) => {
                    AABB::from_corners(Point::new(f32::MIN, f32::MIN), Point::new(val, f32::MAX))
                }
                (crate::threshold::ThresholdAxis::Y, true) => {
                    AABB::from_corners(Point::new(f32::MIN, val), Point::new(f32::MAX, f32::MAX))
                }
                (crate::threshold::ThresholdAxis::Y, false) => {
                    AABB::from_corners(Point::new(f32::MIN, f32::MIN), Point::new(f32::MAX, val))
                }
            };
            let candidates: Vec<_> = self.rtree.locate_in_envelope(&aabb).collect();
            let candidate_points: Vec<(f32, f32)> = candidates
                .iter()
                .map(|geom| {
                    let p = geom.geom();
                    (p.x(), p.y())
                })
                .collect();

            let results = match axis {
                crate::threshold::ThresholdAxis::X => {
                    crate::batch_filtering::filter_by_threshold_x_batch(
                        &candidate_points,
                        val,
                        above,
                    )
                }
                crate::threshold::ThresholdAxis::Y => {
                    crate::batch_filtering::filter_by_threshold_y_batch(
                        &candidate_points,
                        val,
                        above,
                    )
                }
            }
            .unwrap_or_default();

            candidates
                .into_iter()
                .zip(results)
                .filter_map(|(geom, inside)| if inside { Some(geom.data) } else { None })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Filter events inside ONE sub-quadrant (corner) of a quadrant gate,
    /// identified by its stable `sub_id`. The intersection of the corner's
    /// position half-planes. Narrow with the corner's bounding box (an AABB),
    /// then confirm each candidate with the AND'd containment test.
    ///
    /// A whole quadrant gate is not one population, so there is no
    /// `filter_by_quadrant(gate)` — callers must say which corner they want.
    pub fn filter_by_quadrant_corner(&self, gate: &Gate, sub_id: &str) -> Result<Vec<usize>> {
        let corner = gate.geometry.quadrant_corner_index(sub_id).ok_or_else(|| {
            GateError::filtering_error("unknown sub-quadrant id or not a quadrant gate")
        })?;
        // The corner-selective geometry view gives us the per-corner box.
        let view = match &gate.geometry {
            GateGeometry::QuadrantGate(q) => crate::quadrant::QuadrantGateGeometry {
                dividers: &q.dividers,
                quadrants: &q.quadrants,
            },
            _ => return Err(GateError::filtering_error("not a quadrant gate")),
        };
        let (min_x, min_y, max_x, max_y) =
            view.corner_bounding_box(corner, self.x_param.as_ref(), self.y_param.as_ref())?;
        // Replace infinities with f32::MIN/MAX for the rtree envelope.
        let env = AABB::from_corners(
            Point::new(
                if min_x.is_finite() { min_x } else { f32::MIN },
                if min_y.is_finite() { min_y } else { f32::MIN },
            ),
            Point::new(
                if max_x.is_finite() { max_x } else { f32::MAX },
                if max_y.is_finite() { max_y } else { f32::MAX },
            ),
        );
        let candidates: Vec<_> = self.rtree.locate_in_envelope(&env).collect();
        let candidate_points: Vec<(f32, f32)> = candidates
            .iter()
            .map(|geom| {
                let p = geom.geom();
                (p.x(), p.y())
            })
            .collect();
        let results = view.contains_points_batch_corner(
            corner,
            &candidate_points,
            self.x_param.as_ref(),
            self.y_param.as_ref(),
        )?;
        Ok(candidates
            .into_iter()
            .zip(results)
            .filter_map(|(geom, inside)| if inside { Some(geom.data) } else { None })
            .collect())
    }

    /// Build a geo::Polygon from gate nodes
    fn _build_geo_polygon(&self, gate: &Gate) -> Option<GeoPolygon<f32>> {
        if let GateGeometry::Polygon { nodes, closed } = &gate.geometry {
            if !closed || nodes.len() < 3 {
                return None;
            }

            let coords: Vec<Coord<f32>> = nodes
                .iter()
                .filter_map(|node| {
                    let x = node.get_coordinate(self.x_param.as_ref())?;
                    let y = node.get_coordinate(self.y_param.as_ref())?;
                    Some(Coord { x, y })
                })
                .collect();

            if coords.len() < 3 {
                return None;
            }

            // Create LineString from coordinates
            let line_string = LineString::new(coords);

            // Create Polygon from LineString (exterior ring)
            Some(GeoPolygon::new(line_string, vec![]))
        } else {
            None
        }
    }

    /// Get total event count
    pub fn len(&self) -> usize {
        self.event_count
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.event_count == 0
    }
}

/// Filter events from an FCS file by a gate.
///
/// This function returns the indices of all events that fall within the specified gate.
/// It uses spatial indexing for efficient filtering when a pre-built index is provided.
///
/// **Note**: Boolean gates require a resolver. Use `filter_events_by_gate_with_resolver()`
/// for boolean gates, or this will return an error.
///
/// # Arguments
///
/// * `fcs` - The FCS file containing event data
/// * `gate` - The gate to filter by (must be a geometric gate, not boolean)
/// * `spatial_index` - Optional pre-built spatial index for performance optimization.
///   If `None`, a temporary index will be built for this operation.
///
/// # Returns
///
/// A vector of event indices (0-based) that pass through the gate.
///
/// # Errors
///
/// Returns an error if:
/// - The gate is a boolean gate (use `filter_events_by_gate_with_resolver()` instead)
/// - Parameter data cannot be retrieved from FCS file
/// - Index building fails
///
/// # Performance
///
/// - **With index**: O(log n) - very fast for repeated operations
/// - **Without index**: O(n log n) - builds index then filters
///
/// # Example
///
/// ```rust,no_run
/// use flow_gates::{filter_events_by_gate, Gate, GateGeometry, GateNode, GateCoordinateSpace, EventIndex};
/// use flow_fcs::Fcs;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load FCS file (implementation depends on your FCS library)
/// // let fcs = /* load FCS file */;
///
/// // Create geometric gate
/// let min = GateNode::new("min")
///     .with_coordinate("FSC-A", 1000.0)
///     .with_coordinate("SSC-A", 2000.0);
/// let max = GateNode::new("max")
///     .with_coordinate("FSC-A", 5000.0)
///     .with_coordinate("SSC-A", 6000.0);
/// let gate = Gate::new(
///     "lymphocytes",
///     "Lymphocytes",
///     GateGeometry::Rectangle { min, max },
///     "FSC-A",
///     "SSC-A",
///     GateCoordinateSpace::Raw,
/// );
///
/// // Filter events (builds temporary index)
/// // let indices = filter_events_by_gate(&fcs, &gate, None)?;
///
/// // Or use a pre-built index for better performance
/// // let x_slice = fcs.get_parameter_events_slice("FSC-A")?;
/// // let y_slice = fcs.get_parameter_events_slice("SSC-A")?;
/// // let index = EventIndex::build("FSC-A", x_slice, "SSC-A", y_slice)?;
/// // let indices = filter_events_by_gate(&fcs, &gate, Some(&index))?;
/// # Ok(())
/// # }
/// ```
pub fn filter_events_by_gate(
    data: EventData<'_>,
    gate: &Gate,
    spatial_index: Option<&EventIndex>,
) -> Result<Vec<usize>> {
    if data.space != gate.coordinate_space {
        return Err(GateError::space_mismatch(gate.coordinate_space, data.space));
    }

    let indices = if let Some(index) = spatial_index {
        index.filter_by_gate(gate)?
    } else {
        let index = EventIndex::build(data.x_param, data.x, data.y_param, data.y)?;
        index.filter_by_gate(gate)?
    };

    Ok(indices)
}

/// Filter events by ONE sub-quadrant (corner) of a quadrant `gate`, identified
/// by its stable `sub_id`. Sibling of [`filter_events_by_gate`] for the quadrant
/// case, where a single gate owns four addressable sub-populations.
pub fn filter_events_by_gate_corner(
    data: EventData<'_>,
    gate: &Gate,
    sub_id: &str,
    spatial_index: Option<&EventIndex>,
) -> Result<Vec<usize>> {
    if data.space != gate.coordinate_space {
        return Err(GateError::space_mismatch(gate.coordinate_space, data.space));
    }

    let indices = if let Some(index) = spatial_index {
        index.filter_by_quadrant_corner(gate, sub_id)?
    } else {
        let index = EventIndex::build(data.x_param, data.x, data.y_param, data.y)?;
        index.filter_by_quadrant_corner(gate, sub_id)?
    };

    Ok(indices)
}

/// Filter events from an FCS file by a gate with resolver support for boolean gates.
///
/// This function handles all gate types, including boolean gates that reference
/// other gates. For boolean gates, the resolver is used to resolve operand
/// gate IDs to their actual gate references.
///
/// # Arguments
///
/// * `fcs` - The FCS file containing event data
/// * `gate` - The gate to filter by (can be geometric or boolean)
/// * `spatial_index` - Optional pre-built spatial index for performance optimization.
///   If `None`, a temporary index will be built for this operation.
/// * `resolver` - Optional resolver for boolean gate operands. Required if `gate` is a boolean gate.
///
/// # Returns
///
/// A vector of event indices (0-based) that pass through the gate.
///
/// # Errors
///
/// Returns an error if:
/// - Boolean gate operands cannot be resolved
/// - Parameter data cannot be retrieved from FCS file
/// - Index building fails
/// - Filtering fails
///
/// # Performance
///
/// - **With index**: O(log n) - very fast for repeated operations
/// - **Without index**: O(n log n) - builds index then filters
///
/// # Example
///
/// ```rust,no_run
/// use flow_gates::{filter_events_by_gate_with_resolver, Gate, GateResolver, GateGeometry, GateNode, GateCoordinateSpace};
/// use flow_fcs::Fcs;
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load FCS file (implementation depends on your FCS library)
/// // let fcs = /* load FCS file */;
/// let gate_storage: HashMap<Arc<str>, Gate> = HashMap::new();
/// // ... populate gate_storage ...
///
/// // Create a geometric gate
/// let min = GateNode::new("min").with_coordinate("FSC-A", 1000.0).with_coordinate("SSC-A", 2000.0);
/// let max = GateNode::new("max").with_coordinate("FSC-A", 5000.0).with_coordinate("SSC-A", 6000.0);
/// let geometric_gate = Gate::new("rect", "Rectangle", GateGeometry::Rectangle { min, max }, "FSC-A", "SSC-A", GateCoordinateSpace::Raw);
///
/// // Works for geometric gates (resolver not needed)
/// // let indices = filter_events_by_gate_with_resolver(&fcs, &geometric_gate, None, None::<&HashMap<_, _>>)?;
///
/// // For boolean gates, you would need:
/// // let boolean_gate = /* ... */;
/// // let indices = filter_events_by_gate_with_resolver(&fcs, &boolean_gate, None, Some(&gate_storage))?;
/// # Ok(())
/// # }
/// ```
///
/// Currently this helper only supports `coordinate_space == Raw` gates. For
/// compensated/unmixed boolean gates, fetch the appropriate per-operand data at
/// the call site and use `filter_events_by_gate` directly. This reflects the
/// fact that boolean-gate operands can each live in a different parameter space;
/// a single `EventData` wrapper can't represent "heterogeneous" data.
pub fn filter_events_by_gate_with_resolver<R: GateResolver>(
    fcs: &Fcs,
    gate: &Gate,
    spatial_index: Option<&EventIndex>,
    resolver: Option<&R>,
) -> Result<Vec<usize>> {
    if gate.coordinate_space != GateCoordinateSpace::Raw {
        return Err(GateError::filtering_error(format!(
            "filter_events_by_gate_with_resolver only supports Raw gates; got {:?}. \
             Use filter_events_by_gate with EventData for compensated/unmixed gates.",
            gate.coordinate_space
        )));
    }

    // Handle boolean gates separately (they need resolver and filter operand gates).
    if matches!(gate.geometry, GateGeometry::Boolean { .. }) {
        return filter_boolean_gate_with_resolver(fcs, gate, resolver);
    }

    // Mask gates cannot be resolved here — they require a MaskResolver.
    if matches!(gate.geometry, GateGeometry::Mask { .. }) {
        return Err(GateError::filtering_error(
            "Mask gates require a MaskResolver. Resolve at the application layer.",
        ));
    }

    let (x_param, y_param) = gate_axis_pair_for_raw_filter(gate);
    let data = EventData::raw_from_fcs(fcs, x_param, y_param)?;

    let indices = if let Some(index) = spatial_index {
        index.filter_by_gate(gate)?
    } else {
        let index = EventIndex::build(data.x_param, data.x, data.y_param, data.y)?;
        index.filter_by_gate(gate)?
    };

    Ok(indices)
}

/// Pick the `(x_param, y_param)` to fetch from FCS when filtering raw-space events
/// for `gate`. Two-channel gates use the gate's `(x, y)`; one-channel gates use the
/// bounded channel for both slots (the batch filter only inspects the bounded axis,
/// so the second slice is harmless and lets us reuse the 2-D `EventData` shape).
/// NoChannels (mask) gates should never reach this function — they are resolved by
/// the MaskResolver before geometric filtering.
fn gate_axis_pair_for_raw_filter(gate: &Gate) -> (&str, &str) {
    match &gate.parameters {
        crate::types::GateParameters::TwoChannels { x, y } => (x.as_ref(), y.as_ref()),
        crate::types::GateParameters::OneChannel { channel } => {
            let c = channel.as_ref();
            (c, c)
        }
        crate::types::GateParameters::NoChannels => ("", ""),
    }
}

/// Helper function to filter boolean gates with resolver
fn filter_boolean_gate_with_resolver<R: GateResolver>(
    fcs: &Fcs,
    gate: &Gate,
    resolver: Option<&R>,
) -> Result<Vec<usize>> {
    if let GateGeometry::Boolean {
        operation,
        operands,
    } = &gate.geometry
    {
        let resolver = resolver.ok_or_else(|| {
            GateError::filtering_error("Boolean gates require a resolver to resolve operand gates")
        })?;

        // Resolve operand gates
        let resolved_gates: Vec<&Gate> = operands
            .iter()
            .filter_map(|id| resolver.resolve(id.as_ref()))
            .collect();

        if resolved_gates.len() != operands.len() {
            return Err(GateError::gate_not_found(
                "one or more boolean gate operands",
                "could not resolve all operand gate IDs",
            ));
        }

        // Filter using boolean operation
        filter_events_boolean(*operation, &resolved_gates, fcs, None)
    } else {
        Err(GateError::filtering_error("Expected boolean gate geometry"))
    }
}

/// Internal helper: filter a gate that must be in `Raw` space, fetching data
/// from the FCS as raw slices. Used by the `_with_resolver` and `combine_*`
/// variants that still operate on `&Fcs`.
///
/// Errors if the gate is not in Raw space; callers wanting to filter compensated
/// or unmixed gates should construct an `EventData` and call
/// `filter_events_by_gate` directly.
fn filter_events_by_gate_raw(fcs: &Fcs, gate: &Gate) -> Result<Vec<usize>> {
    if gate.coordinate_space != GateCoordinateSpace::Raw {
        return Err(GateError::filtering_error(format!(
            "This filter path requires Raw gates; got {:?}. Use filter_events_by_gate \
             with EventData for compensated/unmixed gates.",
            gate.coordinate_space
        )));
    }
    let (x_param, y_param) = gate_axis_pair_for_raw_filter(gate);
    let data = EventData::raw_from_fcs(fcs, x_param, y_param)?;
    filter_events_by_gate(data, gate, None)
}

/// Filter events through a hierarchy of gates with caching support.
///
/// This function applies a chain of gates sequentially, where each gate filters
/// the results of the previous gate. This is useful for hierarchical gating
/// strategies where child gates are applied to events that pass parent gates.
///
/// **Note**: If any gate in the chain is a boolean gate, a resolver must be provided.
/// Use `filter_events_by_hierarchy_with_resolver()` for chains containing boolean gates.
///
/// # Arguments
///
/// * `fcs` - The FCS file containing event data
/// * `gate_chain` - Chain of gates to filter through, ordered from parent to child.
///   Events must pass all gates in the chain. All gates must be geometric (not boolean).
/// * `filter_cache` - Optional filter cache for caching results. This can significantly
///   improve performance when filtering the same gate hierarchies repeatedly.
/// * `file_guid` - File GUID for cache key generation. Required if `filter_cache` is provided.
///
/// # Returns
///
/// A vector of event indices that pass all gates in the hierarchy.
///
/// # Errors
///
/// Returns an error if any gate in the chain is a boolean gate (use `filter_events_by_hierarchy_with_resolver()` instead).
///
/// # Example
///
/// ```rust,no_run
/// use flow_gates::{filter_events_by_hierarchy, Gate, GateHierarchy};
/// use flow_fcs::Fcs;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load FCS file (implementation depends on your FCS library)
/// // let fcs = /* load FCS file */;
///
/// // Build gate chain from hierarchy (all geometric gates)
/// let hierarchy = GateHierarchy::new();
/// // ... populate hierarchy ...
///
/// // Get gate chain for a specific gate
/// // In practice, you would resolve IDs from hierarchy to gates:
/// // let gate_chain: Vec<&Gate> = hierarchy
/// //     .get_chain_to_root("child-gate")
/// //     .iter()
/// //     .filter_map(|id| storage.get(id.as_ref()))
/// //     .collect();
///
/// // Filter through hierarchy (geometric gates only)
/// // let indices = filter_events_by_hierarchy(&fcs, &gate_chain, None, None)?;
/// # Ok(())
/// # }
/// ```
/// Filter events through a hierarchy of geometric gates.
///
/// The `filter_one_gate` callback is invoked once per gate in the chain. It must
/// fetch whatever event data the gate needs (matching the gate's
/// `coordinate_space`), build an `EventData`, and call `filter_events_by_gate`
/// itself — returning the resulting indices. This callback-style avoids
/// lifetime gymnastics where a caller would otherwise try to return a borrowed
/// `EventData` from owned Vecs.
pub fn filter_events_by_hierarchy<F>(
    total_event_count: usize,
    gate_chain: &[&Gate],
    mut filter_one_gate: F,
    filter_cache: Option<&dyn FilterCache>,
    file_guid: Option<&str>,
) -> Result<Vec<usize>>
where
    F: FnMut(&Gate) -> Result<Vec<usize>>,
{
    // Delegate to the corner-aware variant with no corner selectors. Each step
    // is a whole gate; the callback ignores the (always-`None`) corner arg.
    let steps: Vec<(&Gate, Option<&str>)> = gate_chain.iter().map(|g| (*g, None)).collect();
    filter_events_by_hierarchy_steps(
        total_event_count,
        &steps,
        |gate, _corner| filter_one_gate(gate),
        filter_cache,
        file_guid,
    )
}

/// Corner-aware variant of [`filter_events_by_hierarchy`]. Each step is a gate
/// plus an optional sub-quadrant id: when present, the step filters only that
/// corner of a quadrant gate (the population a child gate parents under).
///
/// The `filter_one_step` callback receives `(gate, corner)` and must filter
/// accordingly — calling [`filter_events_by_gate`] for `None`, or
/// [`filter_events_by_gate_corner`] for `Some(sub_id)`.
///
/// Cache-keying uses each step's **effective id** — the sub-quadrant id when the
/// step is a corner, otherwise the gate id — so two corners of the same quadrant
/// never collide on one cache entry.
pub fn filter_events_by_hierarchy_steps<F>(
    total_event_count: usize,
    gate_chain: &[(&Gate, Option<&str>)],
    mut filter_one_step: F,
    filter_cache: Option<&dyn FilterCache>,
    file_guid: Option<&str>,
) -> Result<Vec<usize>>
where
    F: FnMut(&Gate, Option<&str>) -> Result<Vec<usize>>,
{
    if gate_chain.is_empty() {
        return Ok((0..total_event_count).collect());
    }

    // A step's effective id for cache-keying: the corner sub-id if present,
    // else the gate id.
    fn effective_id<'a>(step: &(&'a Gate, Option<&'a str>)) -> Arc<str> {
        match step.1 {
            Some(sub_id) => Arc::from(sub_id),
            None => step.0.id.clone(),
        }
    }

    // Try cache.
    if let (Some(cache), Some(guid)) = (filter_cache, file_guid)
        && let Some(last_step) = gate_chain.last()
    {
        let parent_chain: Vec<Arc<str>> = gate_chain[..gate_chain.len() - 1]
            .iter()
            .map(effective_id)
            .collect();
        let last_id = effective_id(last_step);

        let cache_key = if parent_chain.is_empty() {
            FilterCacheKey::simple(guid, last_id.as_ref())
        } else {
            FilterCacheKey::new(guid, last_id.as_ref(), parent_chain)
        };

        if let Some(cached_indices) = cache.get(&cache_key) {
            return Ok((*cached_indices).clone());
        }
    }

    // Cache miss — filter each step and intersect.
    let mut current_indices: Option<Vec<usize>> = None;
    for step in gate_chain {
        let (gate, corner) = *step;
        if matches!(gate.geometry, GateGeometry::Boolean { .. }) {
            return Err(GateError::filtering_error(
                "Hierarchy contains boolean gates. Use filter_events_by_hierarchy_with_resolver() instead.",
            ));
        }
        // Mask gates are handled by the caller's closure (which has access to
        // the MaskResolver). Don't reject them here.

        let gate_indices = filter_one_step(gate, corner)?;

        current_indices = Some(match current_indices {
            None => gate_indices,
            Some(prev) => {
                let prev_set: std::collections::HashSet<_> = prev.iter().copied().collect();
                gate_indices
                    .into_iter()
                    .filter(|idx| prev_set.contains(idx))
                    .collect()
            }
        });
    }

    let result = current_indices.unwrap_or_default();

    if let (Some(cache), Some(guid)) = (filter_cache, file_guid)
        && let Some(last_step) = gate_chain.last()
    {
        let parent_chain: Vec<Arc<str>> = gate_chain[..gate_chain.len() - 1]
            .iter()
            .map(effective_id)
            .collect();
        let last_id = effective_id(last_step);

        let cache_key = if parent_chain.is_empty() {
            FilterCacheKey::simple(guid, last_id.as_ref())
        } else {
            FilterCacheKey::new(guid, last_id.as_ref(), parent_chain)
        };

        cache.insert(cache_key, Arc::new(result.clone()));
    }

    Ok(result)
}

/// Filter events through a hierarchy of gates with resolver support for boolean gates.
///
/// This function applies a chain of gates sequentially, where each gate filters
/// the results of the previous gate. Supports both geometric and boolean gates.
///
/// # Arguments
///
/// * `fcs` - The FCS file containing event data
/// * `gate_chain` - Chain of gates to filter through, ordered from parent to child.
///   Events must pass all gates in the chain. Can include boolean gates.
/// * `filter_cache` - Optional filter cache for caching results. This can significantly
///   improve performance when filtering the same gate hierarchies repeatedly.
/// * `file_guid` - File GUID for cache key generation. Required if `filter_cache` is provided.
/// * `resolver` - Optional resolver for boolean gate operands. Required if any gate in the chain is boolean.
///
/// # Returns
///
/// A vector of event indices that pass all gates in the hierarchy.
///
/// # Errors
///
/// Returns an error if:
/// - Boolean gate operands cannot be resolved
/// - Filtering fails for any gate
///
/// # Example
///
/// ```rust,no_run
/// use flow_gates::{filter_events_by_hierarchy_with_resolver, Gate, GateResolver};
/// use flow_fcs::Fcs;
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load FCS file (implementation depends on your FCS library)
/// // let fcs = /* load FCS file */;
/// let gate_storage: HashMap<Arc<str>, Gate> = HashMap::new();
/// // ... populate gate_storage ...
///
/// // Build gate chain (may include boolean gates)
/// // In practice, you would get these from your gate storage
/// // let gate1 = gate_storage.get("gate1").unwrap();
/// // let boolean_gate = gate_storage.get("boolean").unwrap();
/// // let gate2 = gate_storage.get("gate2").unwrap();
/// // let gate_chain: Vec<&Gate> = vec![gate1, boolean_gate, gate2];
///
/// // Filter through hierarchy with resolver
/// // let indices = filter_events_by_hierarchy_with_resolver(
/// //     &fcs,
/// //     &gate_chain,
/// //     None,
/// //     None,
/// //     Some(&gate_storage),
/// // )?;
/// # Ok(())
/// # }
/// ```
pub fn filter_events_by_hierarchy_with_resolver<R: GateResolver>(
    fcs: &Fcs,
    gate_chain: &[&Gate],
    filter_cache: Option<&dyn FilterCache>,
    file_guid: Option<&str>,
    resolver: Option<&R>,
) -> Result<Vec<usize>> {
    filter_events_by_hierarchy_with_resolvers(
        fcs,
        gate_chain,
        filter_cache,
        file_guid,
        resolver,
        None::<&NoopMaskResolver>,
    )
}

/// No-op mask resolver that always errors (used when no mask resolver is provided).
struct NoopMaskResolver;
impl MaskResolver for NoopMaskResolver {
    fn resolve_mask(
        &self,
        _source: &crate::types::MaskSource,
        _n_events: usize,
    ) -> crate::error::Result<Vec<usize>> {
        Err(GateError::filtering_error(
            "No MaskResolver provided for mask gate",
        ))
    }
}

/// Like [`filter_events_by_hierarchy_with_resolver`] but also accepts a
/// [`MaskResolver`] for handling mask gates in the chain.
pub fn filter_events_by_hierarchy_with_resolvers<R: GateResolver, M: MaskResolver>(
    fcs: &Fcs,
    gate_chain: &[&Gate],
    filter_cache: Option<&dyn FilterCache>,
    file_guid: Option<&str>,
    resolver: Option<&R>,
    mask_resolver: Option<&M>,
) -> Result<Vec<usize>> {
    if gate_chain.is_empty() {
        // No gates - return all indices
        let event_count = fcs.data_frame.height();
        return Ok((0..event_count).collect());
    }

    // Check if any gate requires external resolution
    let has_boolean = gate_chain
        .iter()
        .any(|g| matches!(g.geometry, GateGeometry::Boolean { .. }));
    if has_boolean && resolver.is_none() {
        return Err(GateError::filtering_error(
            "Hierarchy contains boolean gates. A GateResolver is required.",
        ));
    }
    let has_mask = gate_chain
        .iter()
        .any(|g| matches!(g.geometry, GateGeometry::Mask { .. }));
    if has_mask && mask_resolver.is_none() {
        return Err(GateError::filtering_error(
            "Hierarchy contains mask gates. A MaskResolver is required.",
        ));
    }

    // Try to get from cache if cache is provided
    if let (Some(cache), Some(guid)) = (filter_cache, file_guid) {
        // For hierarchical gates, use the last gate ID and parent chain
        if let Some(last_gate) = gate_chain.last() {
            let parent_chain: Vec<Arc<str>> = gate_chain[..gate_chain.len() - 1]
                .iter()
                .map(|g| g.id.clone())
                .collect();

            let cache_key = if parent_chain.is_empty() {
                FilterCacheKey::simple(guid, last_gate.id.as_ref())
            } else {
                FilterCacheKey::new(guid, last_gate.id.as_ref(), parent_chain)
            };

            // Try to get from cache
            if let Some(cached_indices) = cache.get(&cache_key) {
                return Ok((*cached_indices).clone());
            }
        }
    }

    // Cache miss or no cache - compute the result
    let mut current_indices: Option<Vec<usize>> = None;

    for gate in gate_chain {
        let gate_indices = if matches!(gate.geometry, GateGeometry::Boolean { .. }) {
            // Boolean gate - use gate resolver
            filter_boolean_gate_with_resolver(fcs, gate, resolver)?
        } else if let GateGeometry::Mask { source } = &gate.geometry {
            // Mask gate - use mask resolver
            let mr = mask_resolver.ok_or_else(|| {
                GateError::filtering_error("Mask gate encountered but no MaskResolver provided")
            })?;
            let n_events = fcs.data_frame.height();
            mr.resolve_mask(source, n_events)?
        } else {
            // Geometric gate - use standard filtering
            filter_events_by_gate_raw(fcs, gate)?
        };

        if let Some(indices) = &current_indices {
            // Intersect with current indices
            let indices_set: std::collections::HashSet<_> = indices.iter().copied().collect();
            current_indices = Some(
                gate_indices
                    .into_iter()
                    .filter(|idx| indices_set.contains(idx))
                    .collect(),
            );
        } else {
            // First gate - filter all events
            current_indices = Some(gate_indices);
        }
    }

    let result = current_indices.unwrap_or_default();

    // Store in cache if cache is provided
    if let (Some(cache), Some(guid)) = (filter_cache, file_guid)
        && let Some(last_gate) = gate_chain.last()
    {
        let parent_chain: Vec<Arc<str>> = gate_chain[..gate_chain.len() - 1]
            .iter()
            .map(|g| g.id.clone())
            .collect();

        let cache_key = if parent_chain.is_empty() {
            FilterCacheKey::simple(guid, last_gate.id.as_ref())
        } else {
            FilterCacheKey::new(guid, last_gate.id.as_ref(), parent_chain)
        };

        cache.insert(cache_key, Arc::new(result.clone()));
    }

    Ok(result)
}

/// Combine gates using AND operation
///
/// Returns event indices that pass ALL of the specified gates.
/// This is equivalent to the intersection of all gate results.
///
/// # Arguments
/// * `gates` - Slice of gates to combine with AND
/// * `fcs` - The FCS file containing event data
/// * `cache` - Optional filter cache for performance
///
/// # Returns
/// A vector of event indices that pass all gates
///
/// # Errors
/// Returns an error if filtering fails for any gate
///
/// # Example
/// ```rust,no_run
/// use flow_gates::{combine_gates_and, Gate};
/// use flow_fcs::Fcs;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load FCS file (implementation depends on your FCS library)
/// // let fcs = /* load FCS file */;
/// // In practice, you would get gates from storage
/// // let gate1 = /* ... */;
/// // let gate2 = /* ... */;
/// // let gate3 = /* ... */;
/// // let gates = vec![&gate1, &gate2, &gate3];
/// // let indices = combine_gates_and(&gates, &fcs, None)?;
/// # Ok(())
/// # }
/// ```
pub fn combine_gates_and(
    gates: &[&Gate],
    fcs: &Fcs,
    _cache: Option<&dyn FilterCache>,
) -> Result<Vec<usize>> {
    if gates.is_empty() {
        return Err(GateError::empty_operands("and"));
    }

    if gates.len() < 2 {
        return Err(GateError::invalid_boolean_operation("and", gates.len(), 2));
    }

    // Filter with first gate
    let first_indices = filter_events_by_gate_raw(fcs, gates[0])?;
    let mut result_set: std::collections::HashSet<usize> = first_indices.iter().copied().collect();

    // Intersect with remaining gates
    for gate in &gates[1..] {
        let gate_indices = filter_events_by_gate_raw(fcs, gate)?;
        let gate_set: std::collections::HashSet<usize> = gate_indices.iter().copied().collect();

        result_set = result_set.intersection(&gate_set).copied().collect();
    }

    Ok(result_set.into_iter().collect())
}

/// Combine gates using OR operation
///
/// Returns event indices that pass AT LEAST ONE of the specified gates.
/// This is equivalent to the union of all gate results.
///
/// # Arguments
/// * `gates` - Slice of gates to combine with OR
/// * `fcs` - The FCS file containing event data
/// * `cache` - Optional filter cache for performance
///
/// # Returns
/// A vector of event indices that pass at least one gate
///
/// # Errors
/// Returns an error if filtering fails for any gate
///
/// # Example
/// ```rust,no_run
/// use flow_gates::{combine_gates_or, Gate};
/// use flow_fcs::Fcs;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load FCS file (implementation depends on your FCS library)
/// // let fcs = /* load FCS file */;
/// // In practice, you would get gates from storage
/// // let gate1 = /* ... */;
/// // let gate2 = /* ... */;
/// // let gate3 = /* ... */;
/// // let gates = vec![&gate1, &gate2, &gate3];
/// // let indices = combine_gates_or(&gates, &fcs, None)?;
/// # Ok(())
/// # }
/// ```
pub fn combine_gates_or(
    gates: &[&Gate],
    fcs: &Fcs,
    _cache: Option<&dyn FilterCache>,
) -> Result<Vec<usize>> {
    if gates.is_empty() {
        return Err(GateError::empty_operands("or"));
    }

    if gates.len() < 2 {
        return Err(GateError::invalid_boolean_operation("or", gates.len(), 2));
    }

    // Union all gate results
    let mut result_set: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for gate in gates {
        let gate_indices = filter_events_by_gate_raw(fcs, gate)?;
        result_set.extend(gate_indices);
    }

    Ok(result_set.into_iter().collect())
}

/// Combine gates using NOT operation
///
/// Returns event indices that do NOT pass the specified gate.
/// This is the complement of the gate's result.
///
/// # Arguments
/// * `gate` - The gate to negate
/// * `fcs` - The FCS file containing event data
/// * `cache` - Optional filter cache for performance
///
/// # Returns
/// A vector of event indices that do NOT pass the gate
///
/// # Errors
/// Returns an error if filtering fails
///
/// # Example
/// ```rust,no_run
/// use flow_gates::{combine_gates_not, Gate};
/// use flow_fcs::Fcs;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load FCS file (implementation depends on your FCS library)
/// // let fcs = /* load FCS file */;
/// // In practice, you would get gate from storage
/// // let gate = /* ... */;
/// // let indices = combine_gates_not(&gate, &fcs, None)?;
/// # Ok(())
/// # }
/// ```
pub fn combine_gates_not(
    gate: &Gate,
    fcs: &Fcs,
    _cache: Option<&dyn FilterCache>,
) -> Result<Vec<usize>> {
    let gate_indices = filter_events_by_gate_raw(fcs, gate)?;
    let gate_set: std::collections::HashSet<usize> = gate_indices.iter().copied().collect();

    let total_events = fcs.data_frame.height();
    let result: Vec<usize> = (0..total_events)
        .filter(|idx| !gate_set.contains(idx))
        .collect();

    Ok(result)
}

/// Filter events using a boolean operation
///
/// This is a convenience function that dispatches to the appropriate
/// boolean operation function based on the operation type.
///
/// # Arguments
/// * `operation` - The boolean operation to apply
/// * `gates` - Slice of gates to combine (must match operation requirements)
/// * `fcs` - The FCS file containing event data
/// * `cache` - Optional filter cache for performance
///
/// # Returns
/// A vector of event indices based on the boolean operation
///
/// # Errors
/// Returns an error if:
/// - Operation requirements are not met (e.g., NOT with multiple gates)
/// - Filtering fails for any gate
///
/// # Example
/// ```rust,no_run
/// use flow_gates::{filter_events_boolean, BooleanOperation, Gate};
/// use flow_fcs::Fcs;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load FCS file (implementation depends on your FCS library)
/// // let fcs = /* load FCS file */;
/// // In practice, you would get gates from storage
/// // let gate1 = /* ... */;
/// // let gate2 = /* ... */;
/// // let gates = vec![&gate1, &gate2];
/// // let indices = filter_events_boolean(BooleanOperation::And, &gates, &fcs, None)?;
/// # Ok(())
/// # }
/// ```
pub fn filter_events_boolean(
    operation: crate::types::BooleanOperation,
    gates: &[&Gate],
    fcs: &Fcs,
    cache: Option<&dyn FilterCache>,
) -> Result<Vec<usize>> {
    match operation {
        crate::types::BooleanOperation::And => combine_gates_and(gates, fcs, cache),
        crate::types::BooleanOperation::Or => combine_gates_or(gates, fcs, cache),
        crate::types::BooleanOperation::Not => {
            if gates.len() != 1 {
                return Err(GateError::invalid_boolean_operation("not", gates.len(), 1));
            }
            combine_gates_not(gates[0], fcs, cache)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::GateNode;

    fn create_test_index() -> EventIndex {
        // Create a simple 10x10 grid of points
        let x_values: Vec<f32> = (0..100).map(|i| (i % 10) as f32).collect();
        let y_values: Vec<f32> = (0..100).map(|i| (i / 10) as f32).collect();
        EventIndex::build("x", &x_values, "y", &y_values).expect("Failed to build index")
    }

    #[test]
    fn test_build_index() {
        let index = create_test_index();
        assert_eq!(index.len(), 100);
        assert!(!index.is_empty());
    }

    #[test]
    fn test_rectangle_filter() {
        let index = create_test_index();

        let min_node = GateNode::new("min")
            .with_coordinate("x", 2.0)
            .with_coordinate("y", 2.0);
        let max_node = GateNode::new("max")
            .with_coordinate("x", 5.0)
            .with_coordinate("y", 5.0);

        let gate = Gate::new(
            "rect-gate",
            "Rectangle",
            GateGeometry::Rectangle {
                min: min_node,
                max: max_node,
            },
            "x",
            "y",
            GateCoordinateSpace::Raw,
        );

        let filtered = index.filter_by_gate(&gate).expect("filter should succeed");

        // Should include points (2,2), (2,3), ..., (5,5)
        // That's 4x4 = 16 points
        assert!(filtered.len() >= 12 && filtered.len() <= 20); // Allow some tolerance
    }

    #[test]
    fn test_polygon_filter() {
        let index = create_test_index();

        // Create a triangle
        let nodes = vec![
            GateNode::new("n1")
                .with_coordinate("x", 0.0)
                .with_coordinate("y", 0.0),
            GateNode::new("n2")
                .with_coordinate("x", 5.0)
                .with_coordinate("y", 0.0),
            GateNode::new("n3")
                .with_coordinate("x", 2.5)
                .with_coordinate("y", 5.0),
        ];

        let gate = Gate::new(
            "poly-gate",
            "Triangle",
            GateGeometry::Polygon {
                nodes,
                closed: true,
            },
            "x",
            "y",
            GateCoordinateSpace::Raw,
        );

        let filtered = index.filter_by_gate(&gate).expect("filter should succeed");

        // Should have some points inside the triangle
        assert!(!filtered.is_empty());
    }
}
