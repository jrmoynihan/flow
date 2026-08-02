//! Structured KNN throughput records for method selection.
//!
//! Append-only JSONL lives at `data/knn_perf_matrix.jsonl`. Regenerate or extend
//! with `cargo run -p flow-knn --example collect_matrix --features "hnsw,ann-search"`.

use crate::config::{HnswParams, KnnMethod};
use serde::{Deserialize, Serialize};

/// One timed cell in the (n, d, method) performance matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfRecord {
    /// Backend id: `exact`, `hnsw_usearch`, `hnsw_ann_search`, `exact_gpu`, `ivf_gpu`, `nndescent_gpu`.
    pub method: String,
    pub n: usize,
    pub d: usize,
    pub k: usize,
    /// Median wall time in seconds for build+self-query (or exact pass).
    pub median_secs: f64,
    /// Elements/sec = n / median_secs.
    pub throughput_elem_per_s: f64,
    /// Optional host / CI label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// ISO-8601 capture time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at: Option<String>,
}

/// Preferences that bias automatic method choice.
#[derive(Debug, Clone, Default)]
pub struct RecommendOpts {
    /// Prefer exact when it is within this factor of the best ANN (default 1.25).
    pub exact_ok_factor: Option<f64>,
    /// Force usearch even when ann-search is available (quantization / no faer).
    pub prefer_usearch: bool,
    /// Neighbours requested (affects exact vs ANN crossover slightly).
    pub k: Option<usize>,
    /// Include GPU method ids when the `gpu` feature is on and an adapter is up.
    /// Default `false` so headless CI stays on CPU.
    pub allow_gpu: bool,
}

/// Committed matrix shipped with the crate (`data/knn_perf_matrix.jsonl`).
const SHIPPED_MATRIX_JSONL: &str = include_str!("../data/knn_perf_matrix.jsonl");

/// Built-in snapshot used when JSONL parse fails.
pub fn builtin_matrix() -> Vec<PerfRecord> {
    parse_matrix_jsonl(SHIPPED_MATRIX_JSONL).unwrap_or_else(|_| {
        // Hard-coded fallback from Criterion pacmap_knn grid (2026-07-23).
        vec![
            rec("hnsw_usearch", 50_000, 10, 60, 0.803),
            rec("hnsw_ann_search", 50_000, 10, 60, 0.529),
            rec("exact", 50_000, 10, 60, 1.265),
            rec("hnsw_usearch", 100_000, 15, 60, 1.817),
            rec("hnsw_ann_search", 100_000, 15, 60, 1.626),
            rec("hnsw_usearch", 250_000, 20, 60, 9.254),
            rec("hnsw_ann_search", 250_000, 20, 60, 5.757),
            rec("hnsw_usearch", 500_000, 20, 60, 20.967),
            rec("hnsw_ann_search", 500_000, 20, 60, 13.467),
        ]
    })
}

fn rec(method: &str, n: usize, d: usize, k: usize, median_secs: f64) -> PerfRecord {
    PerfRecord {
        method: method.to_string(),
        n,
        d,
        k,
        median_secs,
        throughput_elem_per_s: n as f64 / median_secs,
        machine: Some("local-criterion".into()),
        captured_at: Some("2026-07-23".into()),
    }
}

/// Parse JSONL text into records (skips blank / comment lines).
pub fn parse_matrix_jsonl(text: &str) -> Result<Vec<PerfRecord>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let rec: PerfRecord = serde_json::from_str(line)
            .map_err(|e| format!("jsonl line {}: {e}", i + 1))?;
        out.push(rec);
    }
    Ok(out)
}

/// Load matrix: prefer `path` if readable, else the shipped JSONL / [`builtin_matrix`].
pub fn load_matrix(path: Option<&std::path::Path>) -> Vec<PerfRecord> {
    if let Some(p) = path
        && let Ok(text) = std::fs::read_to_string(p)
        && let Ok(recs) = parse_matrix_jsonl(&text)
        && !recs.is_empty()
    {
        return recs;
    }
    builtin_matrix()
}

/// Recommend a [`KnnMethod`] for `n` points of dimension `d`.
///
/// Uses nearest measured cells in the performance matrix (log-distance in n×d),
/// falling back to feature-aware heuristics when the matrix is sparse.
pub fn recommend_method(n: usize, d: usize, opts: &RecommendOpts) -> KnnMethod {
    recommend_method_with_matrix(n, d, opts, &load_matrix(None))
}

/// Same as [`recommend_method`] but with an explicit matrix (tests / custom datasets).
pub fn recommend_method_with_matrix(
    n: usize,
    d: usize,
    opts: &RecommendOpts,
    matrix: &[PerfRecord],
) -> KnnMethod {
    let factor = opts.exact_ok_factor.unwrap_or(1.25);

    // Very small problems: exact is fine and avoids index build.
    if n <= 5_000 {
        return KnnMethod::Exact;
    }

    let candidates = available_method_ids(opts.prefer_usearch, opts.allow_gpu, n);
    let scored: Vec<(&str, f64)> = candidates
        .iter()
        .filter_map(|&method| estimate_secs(method, n, d, matrix).map(|secs| (method, secs)))
        .collect();

    if scored.is_empty() {
        return heuristic_fallback(n, opts.prefer_usearch);
    }

    let best = scored
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .copied();

    if let Some((best_id, best_secs)) = best {
        if best_id != "exact"
            && let Some((_, exact_secs)) = scored.iter().find(|(id, _)| *id == "exact")
            && *exact_secs <= best_secs * factor
        {
            return KnnMethod::Exact;
        }
        return method_from_id(best_id);
    }

    heuristic_fallback(n, opts.prefer_usearch)
}

fn available_method_ids(prefer_usearch: bool, allow_gpu: bool, n: usize) -> Vec<&'static str> {
    let mut ids = Vec::new();
    // Exact only competes up to FCS mid-scale; beyond that ANN build dominates.
    if n <= 80_000 {
        ids.push("exact");
    }
    #[cfg(feature = "hnsw")]
    ids.push("hnsw_usearch");
    #[cfg(feature = "ann-search")]
    if !prefer_usearch {
        ids.push("hnsw_ann_search");
    }
    #[cfg(feature = "gpu")]
    if allow_gpu && crate::gpu_adapter_available() {
        if n <= 50_000 {
            ids.push("exact_gpu");
        }
        ids.push("ivf_gpu");
        if n >= 50_000 {
            ids.push("nndescent_gpu");
        }
    }
    #[cfg(not(feature = "gpu"))]
    let _ = allow_gpu;
    ids
}

fn method_from_id(id: &str) -> KnnMethod {
    match id {
        "exact" => KnnMethod::Exact,
        #[cfg(feature = "hnsw")]
        "hnsw_usearch" => KnnMethod::Hnsw(HnswParams::default()),
        #[cfg(feature = "ann-search")]
        "hnsw_ann_search" => KnnMethod::AnnSearchHnsw(HnswParams::default()),
        #[cfg(feature = "gpu")]
        "exact_gpu" => KnnMethod::GpuExact,
        #[cfg(feature = "gpu")]
        "ivf_gpu" => KnnMethod::GpuIvf(crate::IvfGpuParams::default()),
        #[cfg(feature = "gpu")]
        "nndescent_gpu" => KnnMethod::GpuNnDescent(crate::NnDescentGpuParams::default()),
        _ => heuristic_fallback(100_000, false),
    }
}

fn heuristic_fallback(n: usize, prefer_usearch: bool) -> KnnMethod {
    if n <= 5_000 {
        return KnnMethod::Exact;
    }
    #[cfg(feature = "ann-search")]
    if !prefer_usearch {
        return KnnMethod::AnnSearchHnsw(HnswParams::default());
    }
    #[cfg(feature = "hnsw")]
    {
        return KnnMethod::Hnsw(HnswParams::default());
    }
    #[allow(unreachable_code)]
    KnnMethod::Exact
}

/// Inverse-distance weighted estimate of median seconds at (n,d) for `method`.
fn estimate_secs(method: &str, n: usize, d: usize, matrix: &[PerfRecord]) -> Option<f64> {
    let mut num = 0.0;
    let mut den = 0.0;
    for r in matrix.iter().filter(|r| r.method == method) {
        let dn = (n as f64).ln() - (r.n as f64).ln();
        let dd = (d as f64).ln() - (r.d as f64).ln();
        let dist = (dn * dn + dd * dd).sqrt();
        let w = if dist < 1e-9 {
            return Some(r.median_secs * (n as f64 / r.n as f64));
        } else {
            1.0 / (dist * dist)
        };
        // Scale observed time roughly with n (and mildly with d).
        let scaled = r.median_secs * (n as f64 / r.n as f64) * ((d as f64 / r.d as f64).sqrt());
        num += w * scaled;
        den += w;
    }
    if den > 0.0 {
        Some(num / den)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_n_picks_exact() {
        let m = recommend_method(1_000, 10, &RecommendOpts::default());
        assert!(matches!(m, KnnMethod::Exact));
    }

    #[test]
    fn large_n_prefers_ann_when_available() {
        let m = recommend_method(250_000, 20, &RecommendOpts::default());
        #[cfg(feature = "ann-search")]
        assert!(matches!(m, KnnMethod::AnnSearchHnsw(_)));
        #[cfg(all(not(feature = "ann-search"), feature = "hnsw"))]
        assert!(matches!(m, KnnMethod::Hnsw(_)));
    }

    #[test]
    fn prefer_usearch_honored() {
        let opts = RecommendOpts {
            prefer_usearch: true,
            ..Default::default()
        };
        let m = recommend_method(250_000, 20, &opts);
        #[cfg(feature = "hnsw")]
        assert!(matches!(m, KnnMethod::Hnsw(_)));
    }

    #[test]
    fn shipped_matrix_parses() {
        let recs = builtin_matrix();
        assert!(recs.len() >= 5);
        assert!(recs.iter().any(|r| r.method == "hnsw_ann_search"));
    }

    #[test]
    fn jsonl_roundtrip() {
        let text = concat!(
            r#"{"method":"exact","n":1000,"d":10,"k":10,"median_secs":0.01,"throughput_elem_per_s":100000}"#,
            "\n"
        );
        let recs = parse_matrix_jsonl(text).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].method, "exact");
    }
}
