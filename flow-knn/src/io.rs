//! Versioned on-disk encoding for [`KnnGraph`].
//!
//! Dense packed layout (little-endian):
//! - magic `b"FKNN"` (4)
//! - version `u32` (currently 1)
//! - `n` `u64`, `k` `u64`
//! - metric discriminant `u8` (0=Euclidean, 1=EuclideanSq, 2=Cosine, 3=Manhattan)
//! - provenance length `u32` + UTF-8 bytes (0 = None)
//! - `n * k` `u32` indices (row-major)
//! - `n * k` `f32` distances (row-major)

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::config::DistanceMetric;
use crate::error::KnnError;
use crate::graph::{KnnGraph, NeighborList};

const MAGIC: &[u8; 4] = b"FKNN";
const VERSION: u32 = 1;

fn metric_to_u8(metric: DistanceMetric) -> u8 {
    match metric {
        DistanceMetric::Euclidean => 0,
        DistanceMetric::EuclideanSq => 1,
        DistanceMetric::Cosine => 2,
        DistanceMetric::Manhattan => 3,
    }
}

fn metric_from_u8(v: u8) -> Result<DistanceMetric, KnnError> {
    match v {
        0 => Ok(DistanceMetric::Euclidean),
        1 => Ok(DistanceMetric::EuclideanSq),
        2 => Ok(DistanceMetric::Cosine),
        3 => Ok(DistanceMetric::Manhattan),
        _ => Err(KnnError::Io(format!(
            "unknown distance metric discriminant {v}"
        ))),
    }
}

fn write_u32<W: Write>(w: &mut W, v: u32) -> Result<(), KnnError> {
    w.write_all(&v.to_le_bytes())
        .map_err(|e| KnnError::Io(e.to_string()))
}

fn write_u64<W: Write>(w: &mut W, v: u64) -> Result<(), KnnError> {
    w.write_all(&v.to_le_bytes())
        .map_err(|e| KnnError::Io(e.to_string()))
}

fn read_exact_arr<R: Read, const N: usize>(r: &mut R) -> Result<[u8; N], KnnError> {
    let mut buf = [0u8; N];
    r.read_exact(&mut buf)
        .map_err(|e| KnnError::Io(e.to_string()))?;
    Ok(buf)
}

fn read_u32<R: Read>(r: &mut R) -> Result<u32, KnnError> {
    Ok(u32::from_le_bytes(read_exact_arr(r)?))
}

fn read_u64<R: Read>(r: &mut R) -> Result<u64, KnnError> {
    Ok(u64::from_le_bytes(read_exact_arr(r)?))
}

/// Write a [`KnnGraph`] to `path` (creates/overwrites the file).
pub fn write_knn_graph(path: &Path, graph: &KnnGraph) -> Result<(), KnnError> {
    if graph.neighbors.len() != graph.n {
        return Err(KnnError::GraphSizeMismatch {
            graph_n: graph.n,
            neighbors_len: graph.neighbors.len(),
            data_n: graph.n,
        });
    }
    for (i, nbr) in graph.neighbors.iter().enumerate() {
        if nbr.indices.len() != graph.k || nbr.distances.len() != graph.k {
            return Err(KnnError::Io(format!(
                "neighbor list {i} has indices={} distances={} but graph.k={}",
                nbr.indices.len(),
                nbr.distances.len(),
                graph.k
            )));
        }
    }

    let mut file = File::create(path).map_err(|e| KnnError::Io(e.to_string()))?;
    file.write_all(MAGIC)
        .map_err(|e| KnnError::Io(e.to_string()))?;
    write_u32(&mut file, VERSION)?;
    write_u64(&mut file, graph.n as u64)?;
    write_u64(&mut file, graph.k as u64)?;
    file.write_all(&[metric_to_u8(graph.metric)])
        .map_err(|e| KnnError::Io(e.to_string()))?;

    let prov = graph.provenance.as_deref().unwrap_or("");
    let prov_bytes = prov.as_bytes();
    if prov_bytes.len() > u32::MAX as usize {
        return Err(KnnError::Io(
            "provenance string exceeds u32 length".to_string(),
        ));
    }
    write_u32(&mut file, prov_bytes.len() as u32)?;
    file.write_all(prov_bytes)
        .map_err(|e| KnnError::Io(e.to_string()))?;

    let total = graph
        .n
        .checked_mul(graph.k)
        .ok_or_else(|| KnnError::Io("n*k overflow".to_string()))?;
    let byte_len = total
        .checked_mul(4)
        .ok_or_else(|| KnnError::Io("n*k*4 overflow".to_string()))?;

    // Stage packed LE payloads, then two bulk write_all calls (mirrors read path).
    let mut idx_bytes = Vec::with_capacity(byte_len);
    let mut dist_bytes = Vec::with_capacity(byte_len);
    if cfg!(target_endian = "little") {
        let mut indices = Vec::with_capacity(total);
        let mut distances = Vec::with_capacity(total);
        for nbr in &graph.neighbors {
            indices.extend_from_slice(&nbr.indices);
            distances.extend_from_slice(&nbr.distances);
        }
        idx_bytes.extend_from_slice(bytemuck::cast_slice::<u32, u8>(&indices));
        dist_bytes.extend_from_slice(bytemuck::cast_slice::<f32, u8>(&distances));
    } else {
        for nbr in &graph.neighbors {
            for &idx in &nbr.indices {
                idx_bytes.extend_from_slice(&idx.to_le_bytes());
            }
        }
        for nbr in &graph.neighbors {
            for &dist in &nbr.distances {
                dist_bytes.extend_from_slice(&dist.to_le_bytes());
            }
        }
    }
    file.write_all(&idx_bytes)
        .map_err(|e| KnnError::Io(e.to_string()))?;
    file.write_all(&dist_bytes)
        .map_err(|e| KnnError::Io(e.to_string()))?;
    file.sync_all().map_err(|e| KnnError::Io(e.to_string()))?;
    Ok(())
}

/// Read a [`KnnGraph`] previously written by [`write_knn_graph`].
pub fn read_knn_graph(path: &Path) -> Result<KnnGraph, KnnError> {
    let mut file = File::open(path).map_err(|e| KnnError::Io(e.to_string()))?;
    let magic = read_exact_arr::<_, 4>(&mut file)?;
    if &magic != MAGIC {
        return Err(KnnError::Io(format!(
            "bad knn.bin magic: expected FKNN, got {:?}",
            String::from_utf8_lossy(&magic)
        )));
    }
    let version = read_u32(&mut file)?;
    if version != VERSION {
        return Err(KnnError::Io(format!(
            "unsupported knn.bin version {version} (expected {VERSION})"
        )));
    }
    let n = read_u64(&mut file)? as usize;
    let k = read_u64(&mut file)? as usize;
    let metric = metric_from_u8(read_exact_arr::<_, 1>(&mut file)?[0])?;
    let prov_len = read_u32(&mut file)? as usize;
    let provenance = if prov_len == 0 {
        None
    } else {
        let mut buf = vec![0u8; prov_len];
        file.read_exact(&mut buf)
            .map_err(|e| KnnError::Io(e.to_string()))?;
        let s = String::from_utf8(buf).map_err(|e| KnnError::Io(e.to_string()))?;
        Some(s)
    };

    let total = n
        .checked_mul(k)
        .ok_or_else(|| KnnError::Io("n*k overflow".to_string()))?;
    let byte_len = total
        .checked_mul(4)
        .ok_or_else(|| KnnError::Io("n*k*4 overflow".to_string()))?;

    // Bulk-read directly into typed buffers (avoids u8 staging + second copy on LE).
    let (indices, distances) = if cfg!(target_endian = "little") {
        let mut indices = vec![0u32; total];
        file.read_exact(bytemuck::cast_slice_mut(&mut indices))
            .map_err(|e| KnnError::Io(e.to_string()))?;
        let mut distances = vec![0f32; total];
        file.read_exact(bytemuck::cast_slice_mut(&mut distances))
            .map_err(|e| KnnError::Io(e.to_string()))?;
        (indices, distances)
    } else {
        let mut idx_bytes = vec![0u8; byte_len];
        file.read_exact(&mut idx_bytes)
            .map_err(|e| KnnError::Io(e.to_string()))?;
        let mut dist_bytes = vec![0u8; byte_len];
        file.read_exact(&mut dist_bytes)
            .map_err(|e| KnnError::Io(e.to_string()))?;
        let indices: Vec<u32> = idx_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let distances: Vec<f32> = dist_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        (indices, distances)
    };

    let mut neighbors = Vec::with_capacity(n);
    for i in 0..n {
        let start = i * k;
        let end = start + k;
        neighbors.push(NeighborList {
            indices: indices[start..end].to_vec(),
            distances: distances[start..end].to_vec(),
        });
    }

    Ok(KnnGraph {
        neighbors,
        n,
        k,
        metric,
        provenance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KnnMethod, compute_knn};
    use std::env::temp_dir;

    fn make_grid(n: usize) -> Vec<f32> {
        (0..n).flat_map(|i| [i as f32, 0.0]).collect()
    }

    #[test]
    fn round_trip_preserves_graph() {
        let data = make_grid(12);
        let graph = compute_knn(
            &data,
            12,
            2,
            4,
            &KnnMethod::Exact,
            DistanceMetric::Euclidean,
        )
        .unwrap();
        let path = temp_dir().join(format!(
            "flow-knn-roundtrip-{}.bin",
            std::process::id()
        ));
        write_knn_graph(&path, &graph).unwrap();
        let loaded = read_knn_graph(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.n, graph.n);
        assert_eq!(loaded.k, graph.k);
        assert_eq!(loaded.metric, graph.metric);
        assert_eq!(loaded.provenance, graph.provenance);
        for (a, b) in loaded.neighbors.iter().zip(graph.neighbors.iter()) {
            assert_eq!(a.indices, b.indices);
            assert_eq!(a.distances, b.distances);
        }
    }

    #[test]
    fn rejects_bad_magic() {
        let path = temp_dir().join(format!("flow-knn-bad-magic-{}.bin", std::process::id()));
        std::fs::write(&path, b"XXXX\0\0\0\x01").unwrap();
        let err = read_knn_graph(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(matches!(err, KnnError::Io(_)));
        assert!(err.to_string().contains("magic"));
    }

    #[test]
    fn rejects_bad_version() {
        let path = temp_dir().join(format!("flow-knn-bad-ver-{}.bin", std::process::id()));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&99u32.to_le_bytes());
        std::fs::write(&path, &bytes).unwrap();
        let err = read_knn_graph(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.to_string().contains("version"));
    }

    #[test]
    fn round_trip_preserves_metric_and_provenance() {
        let data = make_grid(8);
        let mut graph = compute_knn(
            &data,
            8,
            2,
            2,
            &KnnMethod::Exact,
            DistanceMetric::Manhattan,
        )
        .unwrap();
        graph.provenance = Some("unit-test".to_string());
        let path = temp_dir().join(format!("flow-knn-metric-{}.bin", std::process::id()));
        write_knn_graph(&path, &graph).unwrap();
        let loaded = read_knn_graph(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.metric, DistanceMetric::Manhattan);
        assert_eq!(loaded.provenance.as_deref(), Some("unit-test"));
    }
}
