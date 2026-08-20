# Napkin vs measured (workspace index)

Protocol: [`PERF_PGD.md`](PERF_PGD.md). Primitive costs: [`PERF_LATENCIES.md`](PERF_LATENCIES.md).
Host snapshot: [`PERF_HOST.md`](PERF_HOST.md). Retry tactics: [`PERF_STRATEGIES.md`](PERF_STRATEGIES.md).

Ratios use published Criterion medians and an explicit `T_lower`. They are
**order-of-magnitude** checks, not a second bench grid. “On roofline” means 1–3×.
Parent epic: `flow-crates-0ap`.

| Crate | Hot path | Encoding | Measured | Napkin `T_lower` | Ratio | Bucket | Strategy | Bead | Detail |
|-------|----------|----------|----------|------------------|-------|--------|----------|------|--------|
| flow-autospectral | joint unmix 10k×20×F8×K8 | `f64` faer | 2.096 ms keep (1.658 ms MATRIX) | ~1 ms FMA | ~2× | 1–3× occupancy | `workspace-per-worker` (kept) | `flow-crates-0ap.1` | [`PERF_MATRIX.md`](../../flow-autospectral/docs/PERF_MATRIX.md). Remaining lever is width (`f64` vs `f32`), not alloc. |
| flow-autospectral | residual match factored K=32 | `f64` | 6.17 ms | ~2 ms apply | ~3× | 3–10× | `hoist-factor-once` (kept) | — | Naive QR-per-pair was 26 ms (complexity, already hoisted). |
| flow-autospectral | OLS factor-once 10k×8 | `f64` | 0.505 ms | ~0.2–0.5 ms | ~1–3× | 1–3× | `hoist-factor-once` | — | 50 ns/event. |
| flow-autospectral | scatter-clean Exact 50k×2 | `f32` KNN | CPU 300 ms / GPU 98 ms | ~50 ms `n²` FMA | ~6× / ~2× | 3–10× / 1–3× | `gpu-after-amortize` | — | GPU skip at 10k (launch). |
| flow-knn | exact 100k×20 k=60 | `f32` | 10.1 s | ~2 s GEMM / gather-bound | ~5× vs FMA, ~1–3× vs gather | 1–3× gather | HNSW / `ivf_gpu` | — | [`PERF_MATRIX.md`](../../flow-knn/docs/PERF_MATRIX.md). Do not `get_unchecked`. |
| flow-knn | HNSW ann-search 100k×20 | `f32` | 2.52 s | `O(n log n)` ANN | n/a (right complexity) | — | `recommend_method` | — | Intended CPU path above ~5–10k events. |
| flow-knn | graph write 100k×k=60 | `u32`+`f32` | 17.6 ms | ~2.6 ms memcpy 64 MiB | ~7× | 3–10× | `bulk-syscall-io` (kept) | — | Pre-keep 13.9 s was >100× syscalls. |
| flow-tru-ols | `TruOls::unmix` 100k | panel `f64`/`f32` mix | 30.4 ms | ~7 ms one-shot NE | ~4× | 3–10× extra solves | `unmix-cache` + `parallel-after-precomp` | — | [`PROFILING.md`](../../tru-ols/docs/PROFILING.md). Variable inner LS, not a missed memcpy. |
| flow-tru-ols | CPU NE 100k 10×10 | — | ~7 ms | ~1–4 ms | ~2–7× | 3–10× | `hoist-factor-once` (kept) | — | GPU RHS slower on this host. |
| flow-fcs | column extract 1M×20 | `f32` strided | 14.8 ms | ~8 ms gather | ~2× | 1–3× | none (stride is the cost) | — | [`PERF_AB.md`](../../fcs/docs/PERF_AB.md). |
| flow-fcs | LE serialize 1M×20 | `f32` | 18.0 ms | ~3 ms memcpy 80 MiB | ~6× | 3–10× | none — new tactic | — | Per-value `write_f32`; bytemuck A/B did not keep at primary size. |
| flow-fcs-compress | BSS split 1M f32 | BSS planes | 119 µs | ~150 µs memcpy 4 MiB | ~1× | 1–3× | — | — | [`PERF_AB.md`](../../flow-fcs-compress/docs/PERF_AB.md). |
| flow-fcs-compress | encode 16×64k BSS+zstd | packed + zstd | 1.57 ms | zstd-bound | ~1× vs codec | 1–3× | do-not-retry scratch | — | Payload reuse regressed. |
| flow-pacmap | gradient_micro 50k | `f32` low-d | 1.33 ms | ~0.5–1 ms | ~1–3× | 1–3× | do-not-retry `get_unchecked` | — | [`PERFORMANCE_NOTES.md`](../../flow-pacmap/docs/PERFORMANCE_NOTES.md). |
| flow-pacmap | GPU optimize 50k×10 450 iter | `f32` | 1.09 s vs 9.49 s CPU | launch amortized | GPU **wins** | — | `gpu-after-amortize` (kept) | — | Zero-copy Burn↔cubeCL. |
| peacoqc-rs | peaks_alloc_micro 100k | bin windows → KDE `f64` | 1.11 ms | KDE-bound | ~1× vs KDE | 1–3× | do-not-retry slice vs `to_vec` | — | [`PERF_AB.md`](../../peacoqc-rs/docs/PERF_AB.md). |
| flow-density | FFT KDE 1M events, 512-grid | `&[f64]` | (FFT, µs–ms class) | vs naive O(n²) KDE | FFT is the hoist | >100× avoided | FFT path (kept by design) | — | [`README.md`](../../flow-density/README.md). Grid width is not a cache miss. |

Rows without a bead are 1–10× after the **correct** primitive (gather vs FMA vs memcpy vs extra LS). The one filed miss is **encoding width** on the joint-unmix hot path (`flow-crates-0ap.1`).
