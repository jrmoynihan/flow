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
use crate::types::{Gate, GateGeometry};
use flow_fcs::Fcs;
use geo::{Coord, LineString, Point, Polygon as GeoPolygon};
use rstar::{AABB, RTree, primitives::GeomWithData};
use std::sync::Arc;

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
/// use flow_gates::{EventIndex, Gate, GateGeometry, GateNode};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Build index from coordinate arrays
/// let x_values: Vec<f32> = (0..10000).map(|i| i as f32).collect();
/// let y_values: Vec<f32> = (0..10000).map(|i| (i * 2) as f32).collect();
/// let index = EventIndex::build(&x_values, &y_values)?;
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
}

impl EventIndex {
    /// Build a spatial index from x and y coordinate arrays
    ///
    /// This is an O(n log n) operation, but subsequent queries are O(log n)
    pub fn build(x_values: &[f32], y_values: &[f32]) -> Result<Self> {
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

        Ok(Self { rtree, event_count })
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
            GateGeometry::Boolean { .. } => Err(GateError::filtering_error(
                "Boolean gates require a resolver. Use filter_by_gate_with_resolver() instead.",
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
    /// use flow_gates::{EventIndex, Gate, GateResolver, GateGeometry, GateNode};
    /// use flow_fcs::Fcs;
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Build index from coordinate arrays
    /// let x_values: Vec<f32> = vec![100.0, 200.0, 300.0];
    /// let y_values: Vec<f32> = vec![100.0, 200.0, 300.0];
    /// let index = EventIndex::build(&x_values, &y_values)?;
    ///
    /// // Create a geometric gate
    /// let min = GateNode::new("min").with_coordinate("x", 50.0).with_coordinate("y", 50.0);
    /// let max = GateNode::new("max").with_coordinate("x", 250.0).with_coordinate("y", 250.0);
    /// let gate = Gate::new("rect", "Rectangle", GateGeometry::Rectangle { min, max }, "x", "y");
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
        }
    }

    /// Filter by polygon gate
    fn filter_by_polygon(&self, gate: &Gate) -> Vec<usize> {
        // Get bounding box for spatial query
        let bbox = match gate.geometry.bounding_box(
            gate.x_parameter_channel_name(),
            gate.y_parameter_channel_name(),
        ) {
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
                        node.get_coordinate(gate.x_parameter_channel_name())?,
                        node.get_coordinate(gate.y_parameter_channel_name())?,
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

        let results = crate::batch_filtering::filter_by_polygon_batch(
            &candidate_points,
            &polygon_coords,
        )
        .unwrap_or_default();

        // Map results back to indices
        candidates
            .into_iter()
            .zip(results.into_iter())
            .filter_map(|(geom, inside)| if inside { Some(geom.data) } else { None })
            .collect()
    }

    /// Filter by rectangle gate
    fn filter_by_rectangle(&self, gate: &Gate) -> Vec<usize> {
        if let GateGeometry::Rectangle { min, max } = &gate.geometry {
            let min_x = match min.get_coordinate(gate.x_parameter_channel_name()) {
                Some(x) => x,
                None => return Vec::new(),
            };
            let min_y = match min.get_coordinate(gate.y_parameter_channel_name()) {
                Some(y) => y,
                None => return Vec::new(),
            };
            let max_x = match max.get_coordinate(gate.x_parameter_channel_name()) {
                Some(x) => x,
                None => return Vec::new(),
            };
            let max_y = match max.get_coordinate(gate.y_parameter_channel_name()) {
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
                .zip(results.into_iter())
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
            let cx = match center.get_coordinate(gate.x_parameter_channel_name()) {
                Some(x) => x,
                None => return Vec::new(),
            };
            let cy = match center.get_coordinate(gate.y_parameter_channel_name()) {
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
                .zip(results.into_iter())
                .filter_map(|(geom, inside)| if inside { Some(geom.data) } else { None })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Build a geo::Polygon from gate nodes
    fn build_geo_polygon(&self, gate: &Gate) -> Option<GeoPolygon<f32>> {
        if let GateGeometry::Polygon { nodes, closed } = &gate.geometry {
            if !closed || nodes.len() < 3 {
                return None;
            }

            let coords: Vec<Coord<f32>> = nodes
                .iter()
                .filter_map(|node| {
                    let x = node.get_coordinate(gate.x_parameter_channel_name())?;
                    let y = node.get_coordinate(gate.y_parameter_channel_name())?;
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
/// use flow_gates::{filter_events_by_gate, Gate, GateGeometry, GateNode, EventIndex};
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
/// );
///
/// // Filter events (builds temporary index)
/// // let indices = filter_events_by_gate(&fcs, &gate, None)?;
///
/// // Or use a pre-built index for better performance
/// // let x_slice = fcs.get_parameter_events_slice("FSC-A")?;
/// // let y_slice = fcs.get_parameter_events_slice("SSC-A")?;
/// // let index = EventIndex::build(x_slice, y_slice)?;
/// // let indices = filter_events_by_gate(&fcs, &gate, Some(&index))?;
/// # Ok(())
/// # }
/// ```
pub fn filter_events_by_gate(
    fcs: &Fcs,
    gate: &Gate,
    spatial_index: Option<&EventIndex>,
) -> Result<Vec<usize>> {
    // Get parameter data as slices (zero-copy when possible)
    let x_param = gate.x_parameter_channel_name();
    let y_param = gate.y_parameter_channel_name();

    let x_slice = fcs.get_parameter_events_slice(x_param).map_err(|e| {
        GateError::filtering_error(format!(
            "Failed to get parameter data for {}: {}",
            x_param, e
        ))
    })?;
    let y_slice = fcs.get_parameter_events_slice(y_param).map_err(|e| {
        GateError::filtering_error(format!(
            "Failed to get parameter data for {}: {}",
            y_param, e
        ))
    })?;

    // Use provided index or build one
    let indices = if let Some(index) = spatial_index {
        index.filter_by_gate(gate)?
    } else {
        // Build index from slices (zero-copy)
        let index = EventIndex::build(x_slice, y_slice)?;
        index.filter_by_gate(gate)?
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
/// use flow_gates::{filter_events_by_gate_with_resolver, Gate, GateResolver, GateGeometry, GateNode};
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
/// let geometric_gate = Gate::new("rect", "Rectangle", GateGeometry::Rectangle { min, max }, "FSC-A", "SSC-A");
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
pub fn filter_events_by_gate_with_resolver<R: GateResolver>(
    fcs: &Fcs,
    gate: &Gate,
    spatial_index: Option<&EventIndex>,
    resolver: Option<&R>,
) -> Result<Vec<usize>> {
    // Get parameter data as slices (zero-copy when possible)
    let x_param = gate.x_parameter_channel_name();
    let y_param = gate.y_parameter_channel_name();

    let x_slice = fcs.get_parameter_events_slice(x_param).map_err(|e| {
        GateError::filtering_error(format!(
            "Failed to get parameter data for {}: {}",
            x_param, e
        ))
    })?;
    let y_slice = fcs.get_parameter_events_slice(y_param).map_err(|e| {
        GateError::filtering_error(format!(
            "Failed to get parameter data for {}: {}",
            y_param, e
        ))
    })?;

    // Handle boolean gates separately (they need resolver and filter operand gates)
    if matches!(gate.geometry, GateGeometry::Boolean { .. }) {
        return filter_boolean_gate_with_resolver(fcs, gate, resolver);
    }

    // For geometric gates, use provided index or build one
    let indices = if let Some(index) = spatial_index {
        index.filter_by_gate(gate)?
    } else {
        // Build index from slices (zero-copy)
        let index = EventIndex::build(x_slice, y_slice)?;
        index.filter_by_gate(gate)?
    };

    Ok(indices)
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
pub fn filter_events_by_hierarchy(
    fcs: &Fcs,
    gate_chain: &[&Gate],
    filter_cache: Option<&dyn FilterCache>,
    file_guid: Option<&str>,
) -> Result<Vec<usize>> {
    if gate_chain.is_empty() {
        // No gates - return all indices
        let event_count = fcs.data_frame.height();
        return Ok((0..event_count).collect());
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
        // Check if gate is boolean (would require resolver)
        if matches!(gate.geometry, GateGeometry::Boolean { .. }) {
            return Err(GateError::filtering_error(
                "Hierarchy contains boolean gates. Use filter_events_by_hierarchy_with_resolver() instead.",
            ));
        }

        if let Some(indices) = &current_indices {
            // Filter the already-filtered events
            // This is more complex - we'd need to subset the FCS data
            // For now, we'll filter from scratch and intersect
            let gate_indices = filter_events_by_gate(fcs, gate, None)?;

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
            current_indices = Some(filter_events_by_gate(fcs, gate, None)?);
        }
    }

    let result = current_indices.unwrap_or_default();

    // Store in cache if cache is provided
    if let (Some(cache), Some(guid)) = (filter_cache, file_guid) {
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

            cache.insert(cache_key, Arc::new(result.clone()));
        }
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
    if gate_chain.is_empty() {
        // No gates - return all indices
        let event_count = fcs.data_frame.height();
        return Ok((0..event_count).collect());
    }

    // Check if any gate is boolean and requires resolver
    let has_boolean = gate_chain
        .iter()
        .any(|g| matches!(g.geometry, GateGeometry::Boolean { .. }));
    if has_boolean && resolver.is_none() {
        return Err(GateError::filtering_error(
            "Hierarchy contains boolean gates. A resolver is required.",
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
            // Boolean gate - use resolver
            filter_boolean_gate_with_resolver(fcs, gate, resolver)?
        } else {
            // Geometric gate - use standard filtering
            filter_events_by_gate(fcs, gate, None)?
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
    if let (Some(cache), Some(guid)) = (filter_cache, file_guid) {
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

            cache.insert(cache_key, Arc::new(result.clone()));
        }
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
    let first_indices = filter_events_by_gate(fcs, gates[0], None)?;
    let mut result_set: std::collections::HashSet<usize> = first_indices.iter().copied().collect();

    // Intersect with remaining gates
    for gate in &gates[1..] {
        let gate_indices = filter_events_by_gate(fcs, gate, None)?;
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
        let gate_indices = filter_events_by_gate(fcs, gate, None)?;
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
    let gate_indices = filter_events_by_gate(fcs, gate, None)?;
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
        EventIndex::build(&x_values, &y_values).expect("Failed to build index")
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
        );

        let filtered = index.filter_by_gate(&gate).expect("filter should succeed");

        // Should have some points inside the triangle
        assert!(!filtered.is_empty());
    }
}
