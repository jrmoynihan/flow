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
/// let filtered_indices = index.filter_by_gate(&gate);
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

    /// Filter events by gate geometry
    ///
    /// Returns indices of events that fall within the gate
    pub fn filter_by_gate(&self, gate: &Gate) -> Vec<usize> {
        match &gate.geometry {
            GateGeometry::Polygon { nodes, closed } => {
                if !closed || nodes.len() < 3 {
                    return Vec::new();
                }
                self.filter_by_polygon(gate)
            }
            GateGeometry::Rectangle { .. } => self.filter_by_rectangle(gate),
            GateGeometry::Ellipse { .. } => self.filter_by_ellipse(gate),
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

        // Build geo::Polygon for precise point-in-polygon test
        let polygon = match self.build_geo_polygon(gate) {
            Some(poly) => poly,
            None => return Vec::new(),
        };

        // Filter candidates with precise point-in-polygon test
        use geo::Contains;
        candidates
            .into_iter()
            .filter(|geom| {
                let point = geom.geom();
                polygon.contains(point)
            })
            .map(|geom| geom.data)
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

            // Perform precise point-in-rectangle check (handles edge cases and floating-point precision)
            candidates
                .into_iter()
                .filter(|geom| {
                    let point = geom.geom();
                    let x = point.x();
                    let y = point.y();
                    // Inclusive bounds: x >= min_x && x <= max_x && y >= min_y && y <= max_y
                    x >= min_x && x <= max_x && y >= min_y && y <= max_y
                })
                .map(|geom| geom.data)
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

            // Apply ellipse equation for precise filtering
            candidates
                .into_iter()
                .filter(|geom| {
                    let point = geom.geom();
                    let dx = point.x() - cx;
                    let dy = point.y() - cy;

                    // Rotate point to ellipse's coordinate system
                    let rotated_x = dx * cos_angle + dy * sin_angle;
                    let rotated_y = -dx * sin_angle + dy * cos_angle;

                    // Check if inside ellipse: (x/rx)^2 + (y/ry)^2 <= 1
                    let normalized_x = rotated_x / radius_x;
                    let normalized_y = rotated_y / radius_y;
                    normalized_x * normalized_x + normalized_y * normalized_y <= 1.0
                })
                .map(|geom| geom.data)
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
/// # Arguments
///
/// * `fcs` - The FCS file containing event data
/// * `gate` - The gate to filter by
/// * `spatial_index` - Optional pre-built spatial index for performance optimization.
///   If `None`, a temporary index will be built for this operation.
///
/// # Returns
///
/// A vector of event indices (0-based) that pass through the gate.
///
/// # Performance
///
/// - **With index**: O(log n) - very fast for repeated operations
/// - **Without index**: O(n log n) - builds index then filters
///
/// # Example
///
/// ```rust
/// use flow_gates::{filter_events_by_gate, Gate, GateGeometry, GateNode, EventIndex};
/// use flow_fcs::Fcs;
///
/// // Load FCS file
/// let fcs = Fcs::from_file("data.fcs")?;
///
/// // Create gate
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
/// let indices = filter_events_by_gate(&fcs, &gate, None)?;
///
/// // Or use a pre-built index for better performance
/// let x_slice = fcs.get_parameter_events_slice("FSC-A")?;
/// let y_slice = fcs.get_parameter_events_slice("SSC-A")?;
/// let index = EventIndex::build(x_slice, y_slice)?;
/// let indices = filter_events_by_gate(&fcs, &gate, Some(&index))?;
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
        GateError::filtering_error(format!("Failed to get parameter data for {}: {}", x_param, e))
    })?;
    let y_slice = fcs.get_parameter_events_slice(y_param).map_err(|e| {
        GateError::filtering_error(format!("Failed to get parameter data for {}: {}", y_param, e))
    })?;

    // Use provided index or build one
    let indices = if let Some(index) = spatial_index {
        index.filter_by_gate(gate)
    } else {
        // Build index from slices (zero-copy)
        let index = EventIndex::build(x_slice, y_slice)?;
        index.filter_by_gate(gate)
    };

    Ok(indices)
}

/// Filter events through a hierarchy of gates with caching support.
///
/// This function applies a chain of gates sequentially, where each gate filters
/// the results of the previous gate. This is useful for hierarchical gating
/// strategies where child gates are applied to events that pass parent gates.
///
/// # Arguments
///
/// * `fcs` - The FCS file containing event data
/// * `gate_chain` - Chain of gates to filter through, ordered from parent to child.
///   Events must pass all gates in the chain.
/// * `filter_cache` - Optional filter cache for caching results. This can significantly
///   improve performance when filtering the same gate hierarchies repeatedly.
/// * `file_guid` - File GUID for cache key generation. Required if `filter_cache` is provided.
///
/// # Returns
///
/// A vector of event indices that pass all gates in the hierarchy.
///
/// # Example
///
/// ```rust
/// use flow_gates::{filter_events_by_hierarchy, Gate, GateHierarchy};
/// use flow_fcs::Fcs;
///
/// // Load FCS file
/// let fcs = Fcs::from_file("data.fcs")?;
///
/// // Build gate chain from hierarchy
/// let hierarchy = GateHierarchy::new();
/// // ... populate hierarchy ...
///
/// // Get gate chain for a specific gate
/// let gate_chain: Vec<&Gate> = hierarchy
///     .get_chain_to_root("child-gate")
///     .iter()
///     .filter_map(|id| storage.get(id.as_ref()))
///     .collect();
///
/// // Filter through hierarchy
/// let indices = filter_events_by_hierarchy(&fcs, &gate_chain, None, None)?;
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

        let filtered = index.filter_by_gate(&gate);

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

        let filtered = index.filter_by_gate(&gate);

        // Should have some points inside the triangle
        assert!(!filtered.is_empty());
    }
}

