# CPU vs GPU bench notes (TRU-OLS)

From `FLOW_TRU_OLS_BENCH_PRESSURE=1 cargo bench -p flow-tru-ols --no-default-features --features cubecl --bench ols_method_compare` (2026-07-24).

Fixture: 10 detectors × 10 endmembers. Median wall times:

| n events | QR (parallel) | NE CPU f64 | NE GPU RHS f32 |
|----------|---------------|------------|----------------|
| 50k | 11.8 ms | **4.9 ms** | 7.9 ms |
| 200k | 41.7 ms | **14.1 ms** | 23.2 ms |
| 500k | 98.9 ms | **31.1 ms** | 175 ms* |
| 1M | 676 ms* | 127 ms* | **75.3 ms** |

\*High variance / outliers at 500k–1M (GPU warm / thermal). At 1M the GPU RHS path can beat noisy CPU NE samples; at mid scale CPU NE remains faster for this panel size (GEMM is not the bottleneck until events ≫ detectors×endmembers).

Raw log: `bench_results/ols_cpu_vs_gpu.txt`.
