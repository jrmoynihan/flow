# flow-pacmap Criterion benches

## `pacmap_compare`

Three-way (plus optional fourth) throughput track vs `manifolds-rs` and `oxicuda-manifold`.

**Build:** `cargo bench -p flow-pacmap --features ann-search --bench pacmap_compare …`
(`ann-search` is required so the `flow_ann_hnsw` arm links.)

### Fairness caveats

- **Weight schedules** still differ across crates. Wall-time A/B remains meaningful when neighbor counts, iteration budget, learning rate, PCA init, and seed are aligned.
- **oxicuda PaCMAP is f64** end-to-end. Conversion from the shared f32 fixture happens **outside** timed regions.
- **oxicuda `pacmap` e2e uses brute-force kNN internally**. The `oxicuda_hnsw` arm only appears in the KNN group for `n ≤ 50k`.
- **manifolds default Adam lr is 0.01** — benches override to `1.0`.
- **`flow_ann_hnsw`** uses `KnnMethod::AnnSearchHnsw` (same `ann-search-rs` stack as manifolds) via the shared `flow-knn` crate.

### Size grid

| Mode | Sizes `(n, d)` |
|------|----------------|
| Default | `(10k,10)`, `(50k,10)`, `(50k,20)`, `(100k,15)`, `(250k,20)`, `(500k,20)` |
| `FLOW_PACMAP_BENCH_MATRIX=1` | Cartesian `{10k…500k} × {10,15,20}` |
| `FLOW_PACMAP_BENCH_1M=1` | adds `(1M,20)` |
| `FLOW_PACMAP_BENCH_MAX_N=<n>` | keep only cells with `n ≤` that cap |
| `FLOW_PACMAP_BENCH_SMOKE=1` | `(1k,10)` with short phase iters |

For a growing JSONL matrix used by `flow_knn::recommend_method`, prefer
`flow-knn`’s `collect_matrix` example (see `flow-knn/docs/PERF_MATRIX.md`).

Exact / exhaustive KNN arms run only when `n ≤ 50_000`. oxicuda KNN arms only when smoke or `n ≤ 50k`. oxicuda **e2e** is smoke-only.

### GPU optimize A/B

```bash
cargo bench -p flow-pacmap --features "cubecl,ann-search" --bench pacmap_optimize_gpu
```

Requires a WGPU adapter (Metal on Apple). Set `PaCMAPConfig::optimize_backend = OptimizeBackend::Gpu`.

### Commands

```bash
# KNN-only grid (recommended first)
cargo bench -p flow-pacmap --features ann-search --bench pacmap_compare -- \
  pacmap_knn --warm-up-time 1 --measurement-time 6 --sample-size 10

# Full track (slow — hours at 250k+ × 450 iters × arms)
cargo bench -p flow-pacmap --features ann-search --bench pacmap_compare

# Smoke
FLOW_PACMAP_BENCH_SMOKE=1 cargo bench -p flow-pacmap --features ann-search --bench pacmap_compare -- \
  --warm-up-time 1 --measurement-time 1 --sample-size 10
```
