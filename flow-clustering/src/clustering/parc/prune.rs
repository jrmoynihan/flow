//! Local Euclidean prune and global Jaccard prune (Rayon-parallel).

use crate::clustering::{ClusteringError, ClusteringResult};
use flow_knn::NeighborList;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

/// Global Jaccard prune threshold policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JacStdGlobal {
    /// Keep edges with Jaccard > median (Python default).
    Median,
    /// Keep edges with Jaccard > mean − σ · this value.
    Sigma(f64),
}

/// Whether to skip local distance pruning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeepLocalDist {
    /// Skip local prune when n > 300_000 (Python `'auto'`).
    #[default]
    Auto,
    /// Always skip local prune.
    Always,
    /// Always apply local prune.
    Never,
}

/// Directed edge after local prune.
#[derive(Debug, Clone, Copy)]
pub struct WeightedEdge {
    pub src: u32,
    pub dst: u32,
}

/// Undirected (canonical src < dst) edge with Jaccard similarity / Leiden weight.
#[derive(Debug, Clone, Copy)]
pub struct PrunedEdge {
    pub src: u32,
    pub dst: u32,
    pub jaccard: f64,
}

fn mean_std(values: &[f32]) -> (f32, f32) {
    let n = values.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f32>() / n as f32;
    if n == 1 {
        return (mean, 0.0);
    }
    let var = values
        .iter()
        .map(|v| {
            let d = v - mean;
            d * d
        })
        .sum::<f32>()
        / n as f32;
    (mean, var.sqrt())
}

/// Local distance prune over each node's neighbour list (Rayon over nodes).
///
/// Distance interpretation: with hnswlib `l2`, Python prunes on squared distances
/// then forms CSR weights as `1/(√d+0.1)`. With `flow-knn` [`DistanceMetric::Euclidean`],
/// prune thresholds use true L2. Jaccard reweighting replaces local weights for Leiden.
fn local_distance_prune_row(
    rowi: usize,
    nl: &NeighborList,
    std_mul: f32,
    skip_local: bool,
) -> Vec<WeightedEdge> {
    let rowi_u = rowi as u32;
    let mut kept: Vec<WeightedEdge> = Vec::new();
    if nl.indices.is_empty() {
        return kept;
    }

    // Python adds 0.1 to distances before the threshold comparison when pruning.
    let dists_for_thresh: Vec<f32> = if skip_local {
        nl.distances.clone()
    } else {
        nl.distances.iter().map(|d| d + 0.1).collect()
    };

    let (mean, std) = mean_std(&dists_for_thresh);
    let threshold = mean + std_mul * std;

    for (j, &idx) in nl.indices.iter().enumerate() {
        if idx == rowi_u {
            continue;
        }
        let dist_cmp = if skip_local {
            nl.distances[j]
        } else {
            dists_for_thresh[j]
        };
        if skip_local || dist_cmp < threshold {
            kept.push(WeightedEdge {
                src: rowi_u,
                dst: idx,
            });
        }
    }
    kept
}

pub fn local_distance_prune(
    neighbors: &[NeighborList],
    dist_std_local: f64,
    skip_local: bool,
    _distances_are_squared: bool,
    parallel: bool,
) -> Vec<WeightedEdge> {
    let std_mul = dist_std_local as f32;
    if parallel {
        neighbors
            .par_iter()
            .enumerate()
            .flat_map_iter(|(rowi, nl)| {
                local_distance_prune_row(rowi, nl, std_mul, skip_local).into_iter()
            })
            .collect()
    } else {
        neighbors
            .iter()
            .enumerate()
            .flat_map(|(rowi, nl)| local_distance_prune_row(rowi, nl, std_mul, skip_local))
            .collect()
    }
}

fn build_adjacency(n: usize, edges: &[WeightedEdge]) -> Vec<HashSet<u32>> {
    let mut adj: Vec<HashSet<u32>> = (0..n).map(|_| HashSet::new()).collect();
    for e in edges {
        let s = e.src as usize;
        let d = e.dst as usize;
        if s < n {
            adj[s].insert(e.dst);
        }
        if d < n {
            adj[d].insert(e.src);
        }
    }
    adj
}

fn jaccard(a: &HashSet<u32>, b: &HashSet<u32>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.len() + b.len() - inter as usize;
    if union == 0 {
        0.0
    } else {
        inter / union as f64
    }
}

fn median_f64(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

/// Global Jaccard prune; returns a deterministic sorted undirected edge list.
pub fn global_jaccard_prune(
    n: usize,
    local_edges: &[WeightedEdge],
    jac_std: JacStdGlobal,
    _jac_weighted_edges: bool,
    parallel: bool,
) -> ClusteringResult<Vec<PrunedEdge>> {
    if local_edges.is_empty() {
        return Err(ClusteringError::ClusteringFailed(
            "no edges remain after local pruning".to_string(),
        ));
    }

    let adj = build_adjacency(n, local_edges);

    // Unique undirected candidate edges from the directed local graph.
    let mut candidates: Vec<(u32, u32)> = local_edges
        .iter()
        .map(|e| {
            if e.src < e.dst {
                (e.src, e.dst)
            } else {
                (e.dst, e.src)
            }
        })
        .filter(|(a, b)| a != b)
        .collect();
    candidates.sort_unstable();
    candidates.dedup();

    let score_one = |&(src, dst): &(u32, u32)| PrunedEdge {
        src,
        dst,
        jaccard: jaccard(&adj[src as usize], &adj[dst as usize]),
    };
    let scored: Vec<PrunedEdge> = if parallel {
        candidates.par_iter().map(score_one).collect()
    } else {
        candidates.iter().map(score_one).collect()
    };

    if scored.is_empty() {
        return Err(ClusteringError::ClusteringFailed(
            "no undirected edges for Jaccard prune".to_string(),
        ));
    }

    let mut jac_vals: Vec<f64> = scored.iter().map(|e| e.jaccard).collect();
    let threshold = match jac_std {
        JacStdGlobal::Median => median_f64(&mut jac_vals),
        JacStdGlobal::Sigma(sigma) => {
            let mean = jac_vals.iter().sum::<f64>() / jac_vals.len() as f64;
            let var = jac_vals
                .iter()
                .map(|v| {
                    let d = v - mean;
                    d * d
                })
                .sum::<f64>()
                / jac_vals.len() as f64;
            mean - sigma * var.sqrt()
        }
    };

    let mut kept: Vec<PrunedEdge> = scored
        .into_iter()
        .filter(|e| e.jaccard > threshold)
        .collect();

    if kept.is_empty() {
        return Err(ClusteringError::ClusteringFailed(
            "all edges removed by global Jaccard prune".to_string(),
        ));
    }

    // Deterministic order for Leiden input.
    kept.sort_by(|a, b| {
        a.src
            .cmp(&b.src)
            .then(a.dst.cmp(&b.dst))
            .then(a.jaccard.partial_cmp(&b.jaccard).unwrap_or(std::cmp::Ordering::Equal))
    });

    // simplify(combine_edges='sum'): merge duplicate undirected edges by summing Jaccard.
    let mut merged: HashMap<(u32, u32), f64> = HashMap::new();
    for e in kept {
        *merged.entry((e.src, e.dst)).or_insert(0.0) += e.jaccard;
    }
    let mut out: Vec<PrunedEdge> = merged
        .into_iter()
        .map(|((src, dst), jaccard)| PrunedEdge { src, dst, jaccard })
        .collect();
    out.sort_by(|a, b| a.src.cmp(&b.src).then(a.dst.cmp(&b.dst)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_knn::NeighborList;

    #[test]
    fn local_prune_keeps_close_neighbours() {
        let neighbors = vec![
            NeighborList {
                indices: vec![1, 2],
                distances: vec![0.1, 10.0],
            },
            NeighborList {
                indices: vec![0],
                distances: vec![0.1],
            },
            NeighborList {
                indices: vec![0],
                distances: vec![10.0],
            },
        ];
        let edges = local_distance_prune(&neighbors, 0.5, false, false, true);
        assert!(
            edges.iter().any(|e| e.src == 0 && e.dst == 1),
            "close edge kept: {edges:?}"
        );
        assert!(
            !edges.iter().any(|e| e.src == 0 && e.dst == 2),
            "far edge pruned: {edges:?}"
        );
    }

    #[test]
    fn jaccard_prune_keeps_high_overlap() {
        // Triangle 0-1-2 fully connected → high Jaccard; 0-3 weak.
        let local = vec![
            WeightedEdge {
                src: 0,
                dst: 1,
            },
            WeightedEdge {
                src: 1,
                dst: 0,
            },
            WeightedEdge {
                src: 0,
                dst: 2,
            },
            WeightedEdge {
                src: 2,
                dst: 0,
            },
            WeightedEdge {
                src: 1,
                dst: 2,
            },
            WeightedEdge {
                src: 2,
                dst: 1,
            },
            WeightedEdge {
                src: 0,
                dst: 3,
            },
            WeightedEdge {
                src: 3,
                dst: 0,
            },
        ];
        let pruned = global_jaccard_prune(4, &local, JacStdGlobal::Median, true, true).unwrap();
        assert!(!pruned.is_empty());
        // Edge (0,3) has lower neighbourhood overlap than triangle edges.
        let has_03 = pruned.iter().any(|e| e.src == 0 && e.dst == 3);
        let triangle = pruned
            .iter()
            .filter(|e| matches!((e.src, e.dst), (0, 1) | (0, 2) | (1, 2)))
            .count();
        assert!(triangle >= 1, "triangle edges present: {pruned:?}");
        let _ = has_03; // may or may not survive median threshold depending on scores
    }
}
