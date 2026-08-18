//! Leiden community detection adapter for PARC.

use super::prune::PrunedEdge;
use crate::clustering::{ClusteringError, ClusteringResult};
use leiden_rs::{GraphDataBuilder, Leiden, LeidenConfig, QualityType};

/// Leiden quality function used by PARC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParcPartition {
    /// Newman–Girvan modularity (`ModularityVertexPartition`).
    #[default]
    Modularity,
    /// Reichardt–Bornholdt configuration model (`RBConfigurationVertexPartition`).
    RbConfiguration,
}

/// Run Leiden on an undirected pruned edge list.
pub fn run_leiden(
    n: usize,
    edges: &[PrunedEdge],
    weighted: bool,
    partition: ParcPartition,
    resolution: f64,
    n_iter: usize,
    seed: u64,
) -> ClusteringResult<Vec<usize>> {
    let mut builder = GraphDataBuilder::new(n);
    for e in edges {
        let w = if weighted {
            e.jaccard.max(0.0)
        } else {
            1.0
        };
        if !w.is_finite() || w <= 0.0 {
            continue;
        }
        builder
            .add_edge(e.src as usize, e.dst as usize, w)
            .map_err(|err| {
                ClusteringError::ClusteringFailed(format!("Leiden graph edge error: {err}"))
            })?;
    }
    let graph = builder.build().map_err(|err| {
        ClusteringError::ClusteringFailed(format!("Leiden graph build failed: {err}"))
    })?;

    let quality = match partition {
        ParcPartition::Modularity => QualityType::Modularity,
        ParcPartition::RbConfiguration => QualityType::RBConfiguration,
    };

    let config = LeidenConfig {
        max_iterations: n_iter.max(1),
        resolution,
        seed: Some(seed),
        quality,
        ..LeidenConfig::default()
    };

    let leiden = Leiden::new(config);
    let result = leiden
        .run(&graph)
        .map_err(|err| ClusteringError::ClusteringFailed(format!("Leiden failed: {err}")))?;

    Ok(result.partition.as_slice().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clustering::parc::prune::PrunedEdge;

    #[test]
    fn leiden_finds_two_cliques() {
        // Two complete K4 components, no bridge.
        let mut edges = Vec::new();
        for a in 0..4u32 {
            for b in (a + 1)..4u32 {
                edges.push(PrunedEdge {
                    src: a,
                    dst: b,
                    jaccard: 1.0,
                });
            }
        }
        for a in 4..8u32 {
            for b in (a + 1)..8u32 {
                edges.push(PrunedEdge {
                    src: a,
                    dst: b,
                    jaccard: 1.0,
                });
            }
        }
        let labels = run_leiden(
            8,
            &edges,
            true,
            ParcPartition::Modularity,
            1.0,
            10,
            42,
        )
        .expect("leiden");
        let set_a: std::collections::HashSet<_> = labels[..4].iter().copied().collect();
        let set_b: std::collections::HashSet<_> = labels[4..].iter().copied().collect();
        assert_eq!(set_a.len(), 1, "first clique should be one community: {labels:?}");
        assert_eq!(set_b.len(), 1, "second clique should be one community: {labels:?}");
        assert!(set_a.is_disjoint(&set_b), "cliques must differ: {labels:?}");
    }
}
