# Unsafe micro-optimization A/B protocol

Criterion baseline → optimize → re-measure → keep or revert for small `unsafe`
hot-path changes. Pattern matches [`tru-ols/docs/PROFILING.md`](../../tru-ols/docs/PROFILING.md).

## Keep / revert rule

**Keep** if median wall time improves by **≥5%** on the item’s primary size and
`cargo test -p <crate> --lib` / `cargo check -p <crate>` pass. Otherwise
**revert** the code change and leave a `reverted` row in the result table.

## Per-item workflow

1. Add or extend a Criterion bench that isolates the hot path.
2. Save baseline: `--save-baseline unsafe-ab-<item>-pre`.
3. Record pre numbers + provenance in the crate doc.
4. Implement the optimization (small `unsafe` + `# Safety`).
5. Rebench: `--baseline unsafe-ab-<item>-pre`.
6. Record post; keep or revert per the rule above.

## Provenance fields

Date, host/chip, `rustc` commit (short), `RUSTFLAGS` if set, Cargo features,
Criterion group + function ids, primary input size.

## Result table template

| Item | Status | Pre median | Post median | Delta | Primary size | Machine | rustc | Date | Notes |
|------|--------|------------|-------------|-------|--------------|---------|-------|------|-------|
| … | kept/reverted | … | … | …% | … | … | … | … | … |

## Index (crate result sections)

| Item | Crate | Bench | Primary size | Results |
|------|-------|-------|--------------|---------|
| 1 BSS split/unsplit | `flow-fcs-compress` | `byte_stream_split` | 1M f32 | [PERF_AB.md](../../flow-fcs-compress/docs/PERF_AB.md) |
| 2 FCS column extract | `flow-fcs` (`fcs`) | `column_extract` | 1M×20 | [PERF_AB.md](../../fcs/docs/PERF_AB.md) |
| 3 FCS LE serialize | `flow-fcs` (`fcs`) | `serialize_data` | 1M×20 | [PERF_AB.md](../../fcs/docs/PERF_AB.md) |
| 4 KNN graph IO | `flow-knn` | `knn_graph_io` | 100k×k=60 | [PERF_MATRIX.md](../../flow-knn/docs/PERF_MATRIX.md) |
| 5 TRU-OLS SyncPtr scatter | `flow-tru-ols` | `unmixing_benchmark` | 100k events | [PROFILING.md](../../tru-ols/docs/PROFILING.md) |
| 6a exact KNN unchecked | `flow-knn` | `exact_knn_micro` | 10k×20 | [PERF_MATRIX.md](../../flow-knn/docs/PERF_MATRIX.md) |
| 6b PaCMAP gradient unchecked | `flow-pacmap` | `gradient_micro` | 50k | [PERFORMANCE_NOTES.md](../../flow-pacmap/docs/PERFORMANCE_NOTES.md) |

## Execution order

Run items **1 → 6** one at a time. Do not batch multiple `unsafe` changes before measuring.

## Campaign results (2026-08-02, arm64 Apple, rustc 59807616e)

| Item | Status | Headline |
|------|--------|----------|
| 1 BSS `get_unchecked` | reverted | noise / &lt;5% @ 1M |
| 2 FCS column extract | reverted | strided unchecked **regressed** vs collect |
| 3 FCS LE serialize | reverted | bytemuck single-buffer &lt;5% @ 1M×20 |
| 4 KNN graph IO bulk | **kept** | −99.8% @ 100k×60 (3.51s → 8.06ms) |
| 5 TRU-OLS SyncPtr scatter | reverted | solver-bound; +1% noise @ 100k |
| 6a exact KNN unchecked | reverted | −4% noise @ 10k×20 |
| 6b PaCMAP gradient unchecked | reverted | **regressed** +6% @ 50k |
