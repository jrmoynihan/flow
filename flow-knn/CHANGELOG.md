# Changelog

## [0.1.2] — 2026-11-03

### Added

- [`AnnIndex`] / [`build_ann_index`] for query-vs-library nearest-neighbour search
  (Exact, usearch HNSW, ann-search-rs; GPU Exact/IVF behind `gpu`). Used by
  stained-vs-AF-library matching in `flow-autospectral`.

### Performance

- Keep bulk [`KnnGraph`] IO: staged `write_all` and little-endian typed
  `read_exact` load buffers (see crate `docs/PERF_AB.md`).

### Documentation

- README covers `AnnIndex` for query ≠ database workloads alongside self-query
  [`compute_knn`].

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
