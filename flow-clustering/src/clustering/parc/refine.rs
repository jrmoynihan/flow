//! Too-big recursive split and small-population reassignment.

use super::{run_subparc_on_subset, ParcConfig};
use crate::clustering::ClusteringResult;
use std::collections::{HashMap, HashSet};

/// Renumber labels to contiguous `0..k`.
pub fn renumber_labels(labels: Vec<usize>) -> Vec<usize> {
    let mut map: HashMap<usize, usize> = HashMap::new();
    let mut next = 0usize;
    labels
        .into_iter()
        .map(|lab| {
            *map.entry(lab).or_insert_with(|| {
                let id = next;
                next += 1;
                id
            })
        })
        .collect()
}

fn cluster_sizes(labels: &[usize]) -> HashMap<usize, usize> {
    let mut sizes = HashMap::new();
    for &lab in labels {
        *sizes.entry(lab).or_insert(0) += 1;
    }
    sizes
}

fn majority_label(labels: &[usize]) -> Option<usize> {
    if labels.is_empty() {
        return None;
    }
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for &lab in labels {
        *counts.entry(lab).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(lab, count)| (*count, usize::MAX - *lab))
        .map(|(lab, _)| lab)
}

/// Reassign points in communities smaller than `small_pop` using original k-NN labels.
pub fn reassign_small_populations(
    mut labels: Vec<usize>,
    neighbor_indices: &[Vec<u32>],
    small_pop: usize,
    max_iters: usize,
) -> Vec<usize> {
    let n = labels.len();
    if small_pop == 0 || n == 0 {
        return labels;
    }

    // First pass: prefer neighbours in large communities only.
    {
        let sizes = cluster_sizes(&labels);
        let small_clusters: HashSet<usize> = sizes
            .iter()
            .filter(|(_, sz)| **sz < small_pop)
            .map(|(&lab, _)| lab)
            .collect();
        if !small_clusters.is_empty() {
            let mut updates: Vec<(usize, usize)> = Vec::new();
            for (i, &lab) in labels.iter().enumerate() {
                if !small_clusters.contains(&lab) {
                    continue;
                }
                let nbr_labs: Vec<usize> = neighbor_indices
                    .get(i)
                    .into_iter()
                    .flatten()
                    .filter_map(|&j| {
                        let j = j as usize;
                        if j < n {
                            Some(labels[j])
                        } else {
                            None
                        }
                    })
                    .collect();
                let available: Vec<usize> = nbr_labs
                    .iter()
                    .copied()
                    .filter(|l| !small_clusters.contains(l))
                    .collect();
                if let Some(best) = majority_label(&available) {
                    updates.push((i, best));
                }
            }
            for (i, lab) in updates {
                labels[i] = lab;
            }
        }
    }

    // Subsequent passes: majority among all neighbours (Python while-loop).
    for _ in 0..max_iters {
        let sizes = cluster_sizes(&labels);
        let small_clusters: HashSet<usize> = sizes
            .iter()
            .filter(|(_, sz)| **sz < small_pop)
            .map(|(&lab, _)| lab)
            .collect();
        if small_clusters.is_empty() {
            break;
        }
        let mut updates: Vec<(usize, usize)> = Vec::new();
        for (i, &lab) in labels.iter().enumerate() {
            if !small_clusters.contains(&lab) {
                continue;
            }
            let nbr_labs: Vec<usize> = neighbor_indices
                .get(i)
                .into_iter()
                .flatten()
                .filter_map(|&j| {
                    let j = j as usize;
                    if j < n {
                        Some(labels[j])
                    } else {
                        None
                    }
                })
                .collect();
            if let Some(best) = majority_label(&nbr_labs) {
                updates.push((i, best));
            }
        }
        if updates.is_empty() {
            break;
        }
        for (i, lab) in updates {
            labels[i] = lab;
        }
    }

    labels
}

/// Recursively recluster communities larger than `too_big_factor * n`.
pub fn split_too_big_clusters(
    data_f32: &[f32],
    n: usize,
    d: usize,
    mut labels: Vec<usize>,
    _neighbor_indices: &[Vec<u32>],
    config: &ParcConfig,
) -> ClusteringResult<Vec<usize>> {
    let threshold = (config.too_big_factor * n as f64) as usize;
    if threshold == 0 {
        return Ok(labels);
    }

    let mut expanded_pops: HashSet<usize> = HashSet::new();
    loop {
        labels = renumber_labels(labels);
        let sizes = cluster_sizes(&labels);
        // Prefer largest first (Python expands cluster 0 when it is the largest).
        let mut candidates: Vec<(usize, usize)> = sizes.into_iter().collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        let mut target: Option<(usize, usize)> = None;
        for (lab, pop) in candidates {
            if pop > threshold && !expanded_pops.contains(&pop) {
                target = Some((lab, pop));
                break;
            }
        }
        let Some((lab, pop)) = target else {
            break;
        };
        expanded_pops.insert(pop);

        let members: Vec<usize> = labels
            .iter()
            .enumerate()
            .filter_map(|(i, &l)| if l == lab { Some(i) } else { None })
            .collect();
        if members.len() < 3 {
            break;
        }

        let mut sub_data = Vec::with_capacity(members.len() * d);
        for &idx in &members {
            let start = idx * d;
            sub_data.extend_from_slice(&data_f32[start..start + d]);
        }

        let sub_labels = run_subparc_on_subset(&sub_data, members.len(), d, config)?;
        // Offset so new labels do not collide before renumber.
        for (j, &global_i) in members.iter().enumerate() {
            labels[global_i] = sub_labels[j].saturating_add(100_000);
        }
    }

    Ok(renumber_labels(labels))
}
