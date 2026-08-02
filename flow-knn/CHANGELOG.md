# Changelog

## [0.1.1] — 2026-07-25

### Added

- Versioned portable disk format for [`KnnGraph`]: [`write_knn_graph`] / [`read_knn_graph`]
  (`knn.bin`: magic `FKNN`, packed indices + distances).
- [`KnnError::Io`] for serialize/deserialize failures.

## [0.1.0] — 2026-07-23

### Added

- Initial `flow-knn` crate: portable [`KnnGraph`] / [`NeighborList`], [`compute_knn`].
- Backends: exact (Rayon), usearch HNSW (`hnsw` feature), optional `ann-search-rs` HNSW
  (`ann-search` feature; faer 0.23 pinned to match that stack), kiddo stub (`kdtree`).
