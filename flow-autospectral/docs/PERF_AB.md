# flow-autospectral performance A/B notes

Follow workspace protocol in `docs/dev/UNSAFE_MICROOPT_AB.md` when micro-optimizing.

For match throughput, interleave baseline/HEAD Criterion runs and keep an untouched control bench (see beads memory `benchmark-a-b-on-this-machine-apple-m5`). The `match_events_nn` / `match_nn_control` groups are the control counterpart to residual matching.

```bash
cargo bench -p flow-autospectral --bench discover_and_match
cargo bench -p flow-autospectral --bench match_matrix
cargo bench -p flow-autospectral --bench scatter_clean --features gpu
```

Primary quality metrics for algorithm changes: OLS residual, population spread — not wall time alone. Use `--example method_comparison --features tru-ols` for quality A/B across discovery backends.

## Parallel / factor-once (2026-08-18, Apple M5 Max, rustc 1.95.0)

Smoke grid: `match_matrix` filter `n=10_000`, `d=8`, `K∈{1,8,32}` plus `scatter_clean` n=10k/50k. Full tables in [`PERF_MATRIX.md`](PERF_MATRIX.md).

| Item | Status | Pre median | Post median | Delta | Primary size | Notes |
|------|--------|------------|-------------|-------|--------------|-------|
| Factor-once residual | kept | 26.19 ms (naive) | 6.17 ms | −76% | n=10k d=8 K=32 | Default `reuse_af_factors=true` |
| Rayon residual | kept | 31.95 ms (seq) | 6.17 ms | −81% | n=10k d=8 K=32 | Threshold 256 |
| Factor-once unmix | kept | 1.096 ms (naive QR) | 0.505 ms | −54% | n=10k d=8 | `OlsUnmixConfig::reuse_factor` |
| Rayon unmix | kept | 0.892 ms (seq) | 0.505 ms | −43% | n=10k d=8 | Same threshold |
| GPU scatter-clean | skipped as default at 10k | 12.58 ms CPU | 12.36 ms GPU | −1.7% | n=10k d=2 | Below 5% rule; overlapping CI |
| GPU scatter-clean | kept optional at 50k | 300.0 ms CPU | 98.0 ms GPU | −67% | n=50k d=2 | `KnnMethod::GpuExact` |
| GPU NnDescent AnnIndex | skipped | — | — | — | — | Needs `&mut` query API |

Naive residual / per-event QR paths stay behind `reuse_af_factors=false` and `OlsUnmixConfig { reuse_factor: false }` for Criterion A/B.
