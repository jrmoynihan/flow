//! Agglomerative (hierarchical) clustering for small row matrices.
//!
//! Intended for merging over-clusters (e.g. e-means centroids) into
//! metaclusters. The dendrogram shape mirrors fast-flow's TypeScript
//! `ClusterResult` / `DendrogramMerge` so the app can switch later.
//!
//! Backend: [`kodama`](https://docs.rs/kodama) 0.3 (MIT) on a condensed
//! pairwise distance matrix.

use kodama::{Method, linkage};

use crate::clustering::{ClusteringError, ClusteringResult};

/// Linkage criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkageMethod {
    #[default]
    Average,
    Complete,
    Single,
    Ward,
}

impl LinkageMethod {
    fn to_kodama(self) -> Method {
        match self {
            Self::Average => Method::Average,
            Self::Complete => Method::Complete,
            Self::Single => Method::Single,
            Self::Ward => Method::Ward,
        }
    }
}

/// Pairwise metric for the condensed distance matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Metric {
    #[default]
    Cosine,
    Euclidean,
}

/// Configuration for [`Agglomerative::fit`].
#[derive(Debug, Clone)]
pub struct AgglomerativeConfig {
    pub linkage: LinkageMethod,
    pub metric: Metric,
    /// Reject inputs with more than this many rows (default 8192).
    pub max_rows: usize,
}

impl Default for AgglomerativeConfig {
    fn default() -> Self {
        Self {
            linkage: LinkageMethod::Average,
            metric: Metric::Cosine,
            max_rows: 8_192,
        }
    }
}

/// One merge step in the dendrogram.
#[derive(Debug, Clone, PartialEq)]
pub struct DendrogramMerge {
    /// New cluster id (`n + merge_index`).
    pub id: usize,
    pub left: usize,
    pub right: usize,
    /// Linkage height (dissimilarity at merge).
    pub height: f64,
    /// Number of original leaves under this node.
    pub size: usize,
}

/// Full dendrogram with leaf order and cut helper.
#[derive(Debug, Clone)]
pub struct Dendrogram {
    /// Number of input rows (leaves).
    pub n_leaves: usize,
    pub merges: Vec<DendrogramMerge>,
    /// Leaf visit order for plotting (length `n_leaves`).
    pub order: Vec<usize>,
    pub max_height: f64,
}

impl Dendrogram {
    /// Cut the tree into `n_clusters` flat labels (length `n_leaves`).
    pub fn cut(&self, n_clusters: usize) -> ClusteringResult<Vec<usize>> {
        if n_clusters == 0 || n_clusters > self.n_leaves {
            return Err(ClusteringError::InvalidConfig(format!(
                "n_clusters must be in 1..= {}, got {n_clusters}",
                self.n_leaves
            )));
        }
        if n_clusters == self.n_leaves {
            return Ok((0..self.n_leaves).collect());
        }
        // Start from each leaf as its own cluster; apply the first
        // (n_leaves - n_clusters) merges.
        let n_merges = self.n_leaves - n_clusters;
        let mut parent: Vec<usize> = (0..self.n_leaves + self.merges.len()).collect();
        let find = |parent: &mut [usize], mut x: usize| -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        };
        for m in self.merges.iter().take(n_merges) {
            let a = find(&mut parent, m.left);
            let b = find(&mut parent, m.right);
            parent[a] = m.id;
            parent[b] = m.id;
            parent[m.id] = m.id;
        }
        let mut root_to_label = std::collections::HashMap::new();
        let mut next = 0usize;
        let mut labels = vec![0usize; self.n_leaves];
        for (i, label) in labels.iter_mut().enumerate() {
            let r = find(&mut parent, i);
            let entry = root_to_label.entry(r).or_insert_with(|| {
                let v = next;
                next += 1;
                v
            });
            *label = *entry;
        }
        Ok(labels)
    }
}

/// Result of [`Agglomerative::fit`].
#[derive(Debug, Clone)]
pub struct AgglomerativeResult {
    pub dendrogram: Dendrogram,
}

/// Metacluster labels for centroids and (optionally) events.
#[derive(Debug, Clone)]
pub struct MetaclusterResult {
    /// Label per input centroid / over-cluster.
    pub centroid_labels: Vec<usize>,
    /// Label per event when assignments were supplied.
    pub event_labels: Option<Vec<usize>>,
    pub n_metaclusters: usize,
    pub dendrogram: Dendrogram,
}

/// Hierarchical clustering on a small row-major matrix.
pub struct Agglomerative;

impl Agglomerative {
    /// Fit on row-major `f32` data of shape `n × d` (n ≤ `max_rows`).
    pub fn fit(
        data: &[f32],
        n: usize,
        d: usize,
        config: &AgglomerativeConfig,
    ) -> ClusteringResult<AgglomerativeResult> {
        if n == 0 {
            return Err(ClusteringError::EmptyData);
        }
        if d == 0 {
            return Err(ClusteringError::InvalidConfig(
                "feature dimension must be > 0".into(),
            ));
        }
        if data.len() != n.saturating_mul(d) {
            return Err(ClusteringError::InvalidConfig(format!(
                "data length {} != n·d = {n}·{d}",
                data.len()
            )));
        }
        if n > config.max_rows {
            return Err(ClusteringError::InvalidConfig(format!(
                "n = {n} exceeds max_rows = {}",
                config.max_rows
            )));
        }
        if matches!(config.linkage, LinkageMethod::Ward)
            && matches!(config.metric, Metric::Cosine)
        {
            return Err(ClusteringError::InvalidConfig(
                "Ward linkage requires Euclidean metric".into(),
            ));
        }
        if n == 1 {
            return Ok(AgglomerativeResult {
                dendrogram: Dendrogram {
                    n_leaves: 1,
                    merges: Vec::new(),
                    order: vec![0],
                    max_height: 0.0,
                },
            });
        }

        let mut condensed = condensed_distances(data, n, d, config.metric)?;
        let dend = linkage(&mut condensed, n, config.linkage.to_kodama());
        let steps = dend.steps();
        let mut merges = Vec::with_capacity(steps.len());
        let mut max_height = 0.0f64;
        for (i, step) in steps.iter().enumerate() {
            let height = step.dissimilarity;
            max_height = max_height.max(height);
            merges.push(DendrogramMerge {
                id: n + i,
                left: step.cluster1,
                right: step.cluster2, // kodama Step fields
                height,
                size: step.size,
            });
        }
        let order = leaf_order(n, &merges);
        Ok(AgglomerativeResult {
            dendrogram: Dendrogram {
                n_leaves: n,
                merges,
                order,
                max_height,
            },
        })
    }
}

/// Cluster centroids with agglomerative linkage, then map event assignments
/// through the resulting metacluster labels.
pub fn metacluster(
    centroids: &[f32],
    n_centroids: usize,
    d: usize,
    n_metaclusters: usize,
    config: &AgglomerativeConfig,
    event_assignments: Option<&[u32]>,
) -> ClusteringResult<MetaclusterResult> {
    let fitted = Agglomerative::fit(centroids, n_centroids, d, config)?;
    let centroid_labels = fitted.dendrogram.cut(n_metaclusters)?;
    let event_labels = event_assignments.map(|assign| {
        assign
            .iter()
            .map(|&a| {
                let i = a as usize;
                if i < centroid_labels.len() {
                    centroid_labels[i]
                } else {
                    0
                }
            })
            .collect()
    });
    Ok(MetaclusterResult {
        centroid_labels,
        event_labels,
        n_metaclusters,
        dendrogram: fitted.dendrogram,
    })
}

fn condensed_distances(
    data: &[f32],
    n: usize,
    d: usize,
    metric: Metric,
) -> ClusteringResult<Vec<f64>> {
    let mut out = Vec::with_capacity(n * (n - 1) / 2);
    match metric {
        Metric::Euclidean => {
            for i in 0..n {
                let a = &data[i * d..(i + 1) * d];
                for j in (i + 1)..n {
                    let b = &data[j * d..(j + 1) * d];
                    let mut s = 0.0f64;
                    for t in 0..d {
                        let diff = a[t] as f64 - b[t] as f64;
                        s += diff * diff;
                    }
                    out.push(s.sqrt());
                }
            }
        }
        Metric::Cosine => {
            let mut norms = vec![0.0f64; n];
            for i in 0..n {
                let row = &data[i * d..(i + 1) * d];
                let mut s = 0.0f64;
                for &v in row {
                    s += (v as f64) * (v as f64);
                }
                norms[i] = s.sqrt();
            }
            for i in 0..n {
                let a = &data[i * d..(i + 1) * d];
                for j in (i + 1)..n {
                    let b = &data[j * d..(j + 1) * d];
                    let mut dot = 0.0f64;
                    for t in 0..d {
                        dot += a[t] as f64 * b[t] as f64;
                    }
                    let denom = norms[i] * norms[j];
                    let cos = if denom > 0.0 { dot / denom } else { 0.0 };
                    out.push(1.0 - cos.clamp(-1.0, 1.0));
                }
            }
        }
    }
    Ok(out)
}

fn leaf_order(n: usize, merges: &[DendrogramMerge]) -> Vec<usize> {
    if merges.is_empty() {
        return (0..n).collect();
    }
    // Children of each node; leaves have none.
    let mut left = vec![None; n + merges.len()];
    let mut right = vec![None; n + merges.len()];
    for m in merges {
        left[m.id] = Some(m.left);
        right[m.id] = Some(m.right);
    }
    let root = merges.last().map(|m| m.id).unwrap_or(0);
    let mut order = Vec::with_capacity(n);
    fn walk(
        node: usize,
        n_leaves: usize,
        left: &[Option<usize>],
        right: &[Option<usize>],
        order: &mut Vec<usize>,
    ) {
        if node < n_leaves {
            order.push(node);
            return;
        }
        if let Some(l) = left[node] {
            walk(l, n_leaves, left, right, order);
        }
        if let Some(r) = right[node] {
            walk(r, n_leaves, left, right, order);
        }
    }
    walk(root, n, &left, &right, &mut order);
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_linkage_two_pairs() {
        // Four points: two tight pairs far apart.
        let data: Vec<f32> = vec![
            0.0, 0.0, // 0
            0.1, 0.0, // 1
            10.0, 0.0, // 2
            10.1, 0.0, // 3
        ];
        let cfg = AgglomerativeConfig {
            linkage: LinkageMethod::Average,
            metric: Metric::Euclidean,
            ..Default::default()
        };
        let result = Agglomerative::fit(&data, 4, 2, &cfg).expect("fit");
        assert_eq!(result.dendrogram.merges.len(), 3);
        let labels = result.dendrogram.cut(2).expect("cut");
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[2], labels[3]);
        assert_ne!(labels[0], labels[2]);
    }

    #[test]
    fn rejects_ward_with_cosine() {
        let data = vec![1.0f32, 0.0, 0.0, 1.0];
        let cfg = AgglomerativeConfig {
            linkage: LinkageMethod::Ward,
            metric: Metric::Cosine,
            ..Default::default()
        };
        let err = Agglomerative::fit(&data, 2, 2, &cfg).unwrap_err();
        assert!(matches!(err, ClusteringError::InvalidConfig(_)));
    }
}
