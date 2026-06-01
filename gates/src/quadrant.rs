//! Quadrant gate geometry — one gate owning dividers + 4 sub-quadrants.
//!
//! A quadrant gate splits a plot into four populations (the corners) using two
//! dividers (one boundary per channel). Unlike other geometries it is NOT a
//! single population: each [`crate::types::QuadrantSub`] is an addressable
//! sub-population with its own stable id. This mirrors GatingML's
//! `<gating:QuadrantGate>` (N dividers + M `<gating:Quadrant>` sub-elements),
//! where child gates reference a quadrant's id as their `parent_id`.
//!
//! [`QuadrantGateGeometry`] is a borrowed *view* over the dividers and
//! sub-quadrants owned by a [`crate::types::GateGeometry::QuadrantGate`]. Its
//! methods are **corner-selective**: callers pass which sub-quadrant they want
//! (by index — resolve an id via [`QuadrantGateGeometry::corner_index`]). This
//! is the "call a method on the gate, passing the corner as a parameter" model.
//!
//! Like [`crate::threshold::ThresholdGateGeometry`], a corner is *axis-agnostic*:
//! each divider stores its boundary under a channel name, and the axis it maps
//! to is resolved at containment time from the plot's `(x, y)` parameters — so a
//! quadrant renders correctly even on transposed plots. Containment of a corner
//! is the AND of one threshold half-plane per position; `Above` is inclusive
//! (`>=`) and `Below` exclusive (`<`), so every point lands in exactly one
//! corner, including points exactly on a divider line.

use super::error::{GateError, Result};
use super::threshold::{ThresholdAxis, ThresholdGateGeometry};
use super::traits::{GateBounds, GateContainment};
use super::types::{GateNode, QuadrantDivider, QuadrantSub, ThresholdDirection};

/// A borrowed view over a quadrant gate's dividers and sub-quadrants.
/// See the module docs for the corner-selective containment model.
pub struct QuadrantGateGeometry<'a> {
    pub dividers: &'a [QuadrantDivider],
    pub quadrants: &'a [QuadrantSub],
}

impl QuadrantGateGeometry<'_> {
    /// Index of the sub-quadrant with the given stable id, or `None` if unknown.
    pub fn corner_index(&self, sub_id: &str) -> Option<usize> {
        self.quadrants.iter().position(|q| q.id.as_ref() == sub_id)
    }

    /// Build the per-position threshold half-planes for one sub-quadrant.
    /// Each position references a divider (for the channel) and carries its own
    /// boundary value + side.
    fn corner_thresholds(&self, corner: usize) -> Result<Vec<ThresholdGateGeometry>> {
        let sub = self
            .quadrants
            .get(corner)
            .ok_or_else(|| GateError::invalid_geometry("sub-quadrant index out of range"))?;
        let mut thresholds = Vec::with_capacity(sub.positions.len());
        for pos in &sub.positions {
            let divider = self
                .dividers
                .iter()
                .find(|d| d.id == pos.divider_ref)
                .ok_or_else(|| {
                    GateError::invalid_geometry("sub-quadrant references unknown divider")
                })?;
            thresholds.push(ThresholdGateGeometry {
                value_node: GateNode::new(pos.divider_ref.clone())
                    .with_coordinate(divider.channel.clone(), pos.location),
                direction: pos.direction,
            });
        }
        Ok(thresholds)
    }

    /// Corner-selective containment: is `(x, y)` inside sub-quadrant `corner`?
    /// AND of every position's threshold half-plane.
    pub fn contains_point_corner(
        &self,
        corner: usize,
        x: f32,
        y: f32,
        x_param: &str,
        y_param: &str,
    ) -> Result<bool> {
        for t in self.corner_thresholds(corner)? {
            if !t.contains_point(x, y, x_param, y_param)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Corner-selective batch containment for sub-quadrant `corner`.
    /// AND of every position's batch result.
    pub fn contains_points_batch_corner(
        &self,
        corner: usize,
        points: &[(f32, f32)],
        x_param: &str,
        y_param: &str,
    ) -> Result<Vec<bool>> {
        let thresholds = self.corner_thresholds(corner)?;
        let mut acc = vec![true; points.len()];
        for t in thresholds {
            let (axis, val) = t.resolve_value(x_param, y_param)?;
            let above = matches!(t.direction, ThresholdDirection::Above);
            let mask = match axis {
                ThresholdAxis::X => {
                    crate::batch_filtering::filter_by_threshold_x_batch(points, val, above)?
                }
                ThresholdAxis::Y => {
                    crate::batch_filtering::filter_by_threshold_y_batch(points, val, above)?
                }
            };
            for (a, m) in acc.iter_mut().zip(mask) {
                *a = *a && m;
            }
        }
        Ok(acc)
    }

    /// Center of sub-quadrant `corner`: the crosshair intersection of its
    /// position boundaries placed on their resolved axes.
    pub fn corner_center(&self, corner: usize, x_param: &str, y_param: &str) -> Result<(f32, f32)> {
        let mut x = 0.0;
        let mut y = 0.0;
        for t in self.corner_thresholds(corner)? {
            let (axis, val) = t.resolve_value(x_param, y_param)?;
            match axis {
                ThresholdAxis::X => x = val,
                ThresholdAxis::Y => y = val,
            }
        }
        Ok((x, y))
    }

    /// Bounding box of sub-quadrant `corner`: intersection of its position
    /// half-plane boxes.
    pub fn corner_bounding_box(
        &self,
        corner: usize,
        x_param: &str,
        y_param: &str,
    ) -> Result<(f32, f32, f32, f32)> {
        let mut min_x = f32::NEG_INFINITY;
        let mut min_y = f32::NEG_INFINITY;
        let mut max_x = f32::INFINITY;
        let mut max_y = f32::INFINITY;
        for t in self.corner_thresholds(corner)? {
            let (b_min_x, b_min_y, b_max_x, b_max_y) = t.bounding_box(x_param, y_param)?;
            min_x = min_x.max(b_min_x);
            min_y = min_y.max(b_min_y);
            max_x = max_x.min(b_max_x);
            max_y = max_y.min(b_max_y);
        }
        Ok((min_x, min_y, max_x, max_y))
    }

    /// Gate-level "center": the crosshair where the two dividers intersect.
    /// Useful for placing the divider lines when rendering. Each divider's first
    /// value is placed on the axis its channel resolves to.
    pub fn dividers_center(&self, x_param: &str, y_param: &str) -> Result<(f32, f32)> {
        let mut x = 0.0;
        let mut y = 0.0;
        for divider in self.dividers {
            let value = divider.values.first().copied().unwrap_or(0.0);
            if divider.channel.as_ref() == x_param {
                x = value;
            } else if divider.channel.as_ref() == y_param {
                y = value;
            }
        }
        Ok((x, y))
    }

    /// The quadrant is valid when it has dividers on two distinct axes (one X,
    /// one Y) with finite values, and at least one sub-quadrant.
    pub fn dividers_valid(&self, x_param: &str, y_param: &str) -> Result<bool> {
        if self.quadrants.is_empty() || self.dividers.is_empty() {
            return Ok(false);
        }
        let mut on_x = false;
        let mut on_y = false;
        for divider in self.dividers {
            let value = divider.values.first().copied();
            let Some(value) = value else {
                return Ok(false);
            };
            if !value.is_finite() {
                return Ok(false);
            }
            if divider.channel.as_ref() == x_param {
                on_x = true;
            } else if divider.channel.as_ref() == y_param {
                on_y = true;
            }
        }
        Ok(on_x && on_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // X channel = CD8 (divider value 1000), Y channel = CD4 (divider value 2000).
    // Q1 top-left:     x < 1000, y >= 2000  (Below, Above)
    // Q2 top-right:    x >= 1000, y >= 2000 (Above, Above)
    // Q3 bottom-right: x >= 1000, y < 2000  (Above, Below)
    // Q4 bottom-left:  x < 1000, y < 2000   (Below, Below)
    const X: &str = "CD8";
    const Y: &str = "CD4";

    fn divider(id: &str, channel: &str, value: f32) -> QuadrantDivider {
        QuadrantDivider {
            id: Arc::from(id),
            channel: Arc::from(channel),
            values: vec![value],
        }
    }

    fn sub(id: &str, x_dir: ThresholdDirection, y_dir: ThresholdDirection) -> QuadrantSub {
        QuadrantSub {
            id: Arc::from(id),
            label: id.to_string(),
            positions: vec![
                super::super::types::QuadrantPosition {
                    divider_ref: Arc::from("div_x"),
                    location: 1000.0,
                    direction: x_dir,
                },
                super::super::types::QuadrantPosition {
                    divider_ref: Arc::from("div_y"),
                    location: 2000.0,
                    direction: y_dir,
                },
            ],
            label_position: None,
        }
    }

    fn dividers() -> Vec<QuadrantDivider> {
        vec![divider("div_x", X, 1000.0), divider("div_y", Y, 2000.0)]
    }

    fn quads() -> Vec<QuadrantSub> {
        use ThresholdDirection::{Above, Below};
        vec![
            sub("Q1", Below, Above),
            sub("Q2", Above, Above),
            sub("Q3", Above, Below),
            sub("Q4", Below, Below),
        ]
    }

    fn view<'a>(d: &'a [QuadrantDivider], q: &'a [QuadrantSub]) -> QuadrantGateGeometry<'a> {
        QuadrantGateGeometry {
            dividers: d,
            quadrants: q,
        }
    }

    #[test]
    fn point_belongs_to_exactly_one_corner() {
        let d = dividers();
        let q = quads();
        let g = view(&d, &q);
        let points = [
            (500.0, 3000.0),  // Q1
            (1500.0, 3000.0), // Q2
            (1500.0, 100.0),  // Q3
            (500.0, 100.0),   // Q4
            (1000.0, 2000.0), // on both lines -> Q2 (Above/Above inclusive)
            (1000.0, 100.0),  // on x line, below y -> Q3
            (500.0, 2000.0),  // below x, on y line -> Q1
        ];
        for (px, py) in points {
            let hits = (0..g.quadrants.len())
                .filter(|&c| g.contains_point_corner(c, px, py, X, Y).unwrap())
                .count();
            assert_eq!(
                hits, 1,
                "point ({px},{py}) should belong to exactly one corner"
            );
        }
    }

    #[test]
    fn corner_index_resolves_stable_ids() {
        let d = dividers();
        let q = quads();
        let g = view(&d, &q);
        assert_eq!(g.corner_index("Q1"), Some(0));
        assert_eq!(g.corner_index("Q3"), Some(2));
        assert_eq!(g.corner_index("nope"), None);
    }

    #[test]
    fn axis_agnostic_when_transposed() {
        let d = dividers();
        let q = quads();
        let g = view(&d, &q);
        let q1 = g.corner_index("Q1").unwrap();
        // Transposed plot: x_param = CD4, y_param = CD8.
        // Point CD4=3000 (x), CD8=500 (y): CD8<1000 and CD4>=2000 -> in Q1.
        assert!(g.contains_point_corner(q1, 3000.0, 500.0, Y, X).unwrap());
        assert!(!g.contains_point_corner(q1, 1000.0, 1500.0, Y, X).unwrap());
    }

    #[test]
    fn batch_matches_single() {
        let d = dividers();
        let q = quads();
        let g = view(&d, &q);
        let points = [(500.0, 3000.0), (1500.0, 3000.0), (1500.0, 100.0)];
        for c in 0..g.quadrants.len() {
            let batch = g.contains_points_batch_corner(c, &points, X, Y).unwrap();
            for (i, (px, py)) in points.iter().enumerate() {
                let single = g.contains_point_corner(c, *px, *py, X, Y).unwrap();
                assert_eq!(batch[i], single, "corner {c} point {i}");
            }
        }
    }

    #[test]
    fn corner_bounding_box_is_intersection() {
        let d = dividers();
        let q = quads();
        let g = view(&d, &q);
        // Q2: x >= 1000, y >= 2000 -> [1000, 2000, +inf, +inf]
        let q2 = g.corner_index("Q2").unwrap();
        let (min_x, min_y, max_x, max_y) = g.corner_bounding_box(q2, X, Y).unwrap();
        assert_eq!(min_x, 1000.0);
        assert_eq!(min_y, 2000.0);
        assert_eq!(max_x, f32::INFINITY);
        assert_eq!(max_y, f32::INFINITY);
    }

    #[test]
    fn dividers_center_is_crosshair() {
        let d = dividers();
        let q = quads();
        let g = view(&d, &q);
        assert_eq!(g.dividers_center(X, Y).unwrap(), (1000.0, 2000.0));
        // Transposed: crosshair swaps too.
        assert_eq!(g.dividers_center(Y, X).unwrap(), (2000.0, 1000.0));
    }

    #[test]
    fn valid_requires_two_distinct_axes() {
        let d = dividers();
        let q = quads();
        assert!(view(&d, &q).dividers_valid(X, Y).unwrap());
        // Both dividers on the same channel -> not a real 2-D quadrant.
        let bad_d = vec![divider("div_x", X, 1000.0), divider("div_y", X, 2000.0)];
        assert!(!view(&bad_d, &q).dividers_valid(X, Y).unwrap());
    }
}
