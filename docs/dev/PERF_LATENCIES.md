# Primitive costs (napkin math)

Use this file for **order-of-magnitude** estimates before proposing an optimization.
Host-measured numbers live in [`PERF_HOST.md`](PERF_HOST.md). The feedback loop is
[`PERF_PGD.md`](PERF_PGD.md). Retry tactics that already kept here:
[`PERF_STRATEGIES.md`](PERF_STRATEGIES.md). Gap index: [`PERF_GAP.md`](PERF_GAP.md).

Times are **per operation**, not per FCS file. Convert with:

```text
ns ≈ cycles / GHz
T_mem ≈ bytes / bandwidth
T_roofline ≈ max(flops / peak_flops, bytes / peak_bw)
T ≈ T_fixed + n × (T_arith + T_mem + T_alloc)
working_set ≈ n × d × sizeof(T)
```

The **~4 GHz** column is `cycles / 4`. Apple Silicon does not expose
`hw.cpufrequency` via `sysctl`; use that column for conversion, then scale if you
know the core’s boost clock. This development host is documented in
[`PERF_HOST.md`](PERF_HOST.md) (Apple M5 Max, 6 performance + 12 efficiency cores,
128-byte cache line, 128 GiB unified memory).

## Sources

Order-of-magnitude rows follow the widely cited “latency numbers every programmer
should know” tables (Dean 2009 / Norvig; updated interactive set by
[Colin Scott](https://colin-scott.github.io/personal_website/research/interactive_latency.html)),
instruction-class costs from [Agner Fog](https://www.agner.org/optimize/), and
Apple cache geometry from `sysctl` on this machine. They are **floors**, not SLAs.
A 2× miss vs these numbers is noise; a 10× miss is a different primitive.

Apple Silicon notes used below:

- Performance cores share a 16 MiB L2; efficiency cores share 8 MiB L2 per 6-core
  cluster. There is no Intel-style L3 in `sysctl`; a system-level cache (SLC) sits
  above DRAM and is not sized here.
- Cache line is **128 bytes** on this host (`hw.cachelinesize`), not the 64-byte
  x86 default.
- GPU is **unified memory**. Discrete PCIe H2D/D2H rows do not apply on this Mac;
  a wgpu/cubeCL dispatch still pays launch + cache warmup. Keep the PCIe row for
  machines with a discrete GPU.

---

## 1. CPU arithmetic

| Operation | Order of magnitude | ~ns @ 4 GHz | ~cycles | Notes |
|-----------|-------------------|-------------|---------|-------|
| Integer add / `f32` FMA (throughput) | 0.1–1 ns | 0.25 | 1 | Dependent chain is a few cycles of latency |
| `f64` FMA | 0.3–1 ns | 0.5 | 2 | Half the NEON lanes vs `f32` |
| `f32`/`f64` divide or `sqrt` | 3–10 ns | 3–8 | 10–30 | Not FMA; avoid in inner loops |
| Predicted branch | ~0.3 ns | 0.25 | 1 | |
| Branch mispredict | 3–5 ns | 3–5 | ~10–20 | Random AF-index branches hurt |
| Atomic CAS (uncontended) | 5–20 ns | 5–15 | 20–60 | Contended: 100 ns–µs |
| NEON `f32x4` FMA | 4 FLOPs / cycle / core | — | 1 | Peak ≈ `4 × GHz × cores` FLOP/s |
| NEON `f64x2` FMA | 2 FLOPs / cycle / core | — | 1 | Same issue width, half the values |

Peak arithmetic for one 4 GHz P-core at 4×`f32` FMA is on the order of
**16 GFLOP/s**. Six P-cores ≈ 100 GFLOP/s if the loop is FMA-dense, vectorized,
and not memory-bound — which most FCS loops are not.

## 2. Memory hierarchy

| Operation | Order of magnitude | ~ns @ 4 GHz | ~cycles | Notes |
|-----------|-------------------|-------------|---------|-------|
| L1 hit (P-core 128 KiB D) | 1 ns | 1 | ~4 | Sequential `f32` scans live here up to ~32 KiB–128 KiB |
| L2 hit (P-cluster 16 MiB) | 3–10 ns | 4–8 | 15–30 | 1M events × 8 `f32` detectors ≈ 32 MiB → not L2 |
| SLC / “L3-class” | 10–30 ns | 10–20 | 40–80 | Not in `sysctl`; treat as between L2 and DRAM |
| DRAM (unified) | 50–150 ns | 80 | 200–400 | Random access; sequential is bandwidth-bound |
| TLB miss (with page still in RAM) | 10–100 ns | — | — | Huge pages rarely help FCS-sized buffers |
| Cache line fill | 128 B here | — | — | One miss brings 32 `f32` values |

Sequential vs random: a sequential `f32` sum is prefetcher- and bandwidth-bound
(GB/s). A random index into a 64 MiB buffer is latency-bound (ns per load ×
misses). Hash maps and `gather` from event-major FCS both look like the random
case once the working set leaves L2.

## 3. Memory-efficiency / encoding

**Bytes per value** decides which row of the hierarchy you live in, and how many
SIMD lanes you fill. Packing only wins when it **avoids a miss class** (L2→DRAM,
or DRAM→disk). Extra unpack arithmetic is cheap compared to DRAM, expensive
compared to L1.

| Encoding | Bytes | SIMD `f32`-equivalent lanes (NEON) | When it wins |
|----------|------:|-----------------------------------:|--------------|
| `u8` / bitpacked flags | 1 or &lt;1 | 16× bytes vs `f32` | Masks, gates, “this fluor is on” |
| `u16` (typical FCS DATA integer) | 2 | 8 | Keep packed until a kernel needs floats |
| `u32` / `i32` | 4 | 4 | Indices, hashes; same width as `f32` |
| `f32` | 4 | 4 | Default for unmix / KNN in this workspace |
| `u64` / `i64` | 8 | 2 | IDs; do not use as a numeric payload |
| `f64` | 8 | 2 | Need the extra mantissa; otherwise 2× DRAM and half the lanes |
| `i128` / bigint | 16+ | 0–1 | Almost never in an event loop |

Worked size: **1,000,000 events × 64 detectors**

| Type | Working set | Likely home on this host |
|------|-------------|--------------------------|
| `u16` | 128 MiB | DRAM (but half the traffic of `f32`) |
| `f32` | 256 MiB | DRAM |
| `f64` | 512 MiB | DRAM, 2× the fills, half the NEON occupancy |

Other layout choices:

- **AoS vs SoA.** Event-major FCS (AoS) is one cache line per event for small `d`;
  a column extract is a stride of `d` values. Detector-major (SoA) makes KDE and
  per-parameter scans sequential.
- **BSS / bitpacking** ([`flow-fcs-compress`](../../flow-fcs-compress/docs/PERF_AB.md))
  helps **codecs and IO**, not the unmix inner loop, unless you decode in-register
  and the alternative is a DRAM-sized `f32` table.
- **Dictionary / interned strings.** Panel names belong in a side table; the hot
  path should carry `u32` ids.
- **Padding / alignment.** A 24-byte struct in a 32-byte slot is a 33% bandwidth
  tax. Prefer packed arrays of scalars over arrays of structs in event loops.

FCS on disk is often 16-bit integers. Decoding once into `f32` columns is the
right trade when many kernels reuse the table. Decoding in the inner loop wins
only if the `f32` table would miss and the kernel is a single pass.

## 4. Alloc and syscalls

| Operation | Order of magnitude | ~ns @ 4 GHz | Notes |
|-----------|-------------------|-------------|-------|
| `Vec` index, in-cache | 0.3–1 ns | 1–4 cycles | Sequential |
| Small `malloc` / `free` (jemalloc/system) | 20–100 ns | — | Plus later cache misses on first touch |
| `Vec` grow (`realloc` + copy) | 100 ns–µs | — | Amortize with `with_capacity` |
| `clone` of an `n×d` matrix | memcpy-bound | — | `n × d × 4 / bandwidth` |
| `mmap` already-resident page | ~RAM | — | Hot FCS mmap ≈ DRAM |
| Major page fault (cold, from SSD) | 10–100 µs | — | First touch after open |
| Syscall (uncontended) | 100–1000 ns | — | `write` per `u32` will dominate any pack |
| Mutex lock (uncontended) | 15–30 ns | — | |
| Thread wake / Rayon steal | 1–20 µs | — | Do not parallelize 10 µs of work |

At 10,000 events, **20 `Vec` constructions per event** at ~50 ns is ~10 ms — the
same order as the whole joint-unmix Criterion median before scratch reuse. That
is the alloc-bound signature.

## 5. Software structures

Treat big-O as a **multiplier**, not a time. Cost ≈ `ops(n) × T_op`.

| Structure / algorithm | `ops` | `T_op` class | Notes |
|----------------------|-------|--------------|-------|
| Slice index, sequential | 1 | L1 load | Baseline |
| Slice index, random | 1 | L2–DRAM | |
| Binary search | `log2 n` | random load + branch | 1M keys ≈ 20 probes |
| `HashMap` get (hot) | ~1 | L1 + hash | ~10–30 ns |
| `HashMap` get (cold, 100k keys) | ~1 | DRAM | ~50–150 ns |
| `BTreeMap` get | `log n` | random | Slower than HashMap for this workload |
| `sort_unstable` `f32` | `n log n` compares | L1–DRAM | 100k ≈ 1.6e6 compares × ~5 ns ≈ few ms |
| Linear scan vs HashMap | `n` vs ~1 | — | Scan wins while `n × T_seq < T_hash` (often n &lt; ~10–30) |

Exact k-NN is `O(n² d)` distance evaluations. At 100,000 events × 20 detectors ×
~2 ns per fused multiply-add over 20 dims (~40 ns/distance) you are in **seconds**,
which matches the exact-KNN matrices. HNSW is `O(n log n)` build with a large
constant; it wins once `n` makes `n²` leave L2 and then DRAM for minutes.

## 6. Bulk moves (CPU memcpy, disk)

| Operation | Order of magnitude | Notes |
|-----------|-------------------|-------|
| `memcpy` L1-resident | 50–200 GB/s / core | Tiny buffers |
| `memcpy` DRAM (this class of Mac) | tens–hundreds GB/s | Measure in `PERF_HOST.md`; do not quote PCIe |
| NVMe sequential | 2–7 GB/s | FCS load of 256 MiB ≈ 40–130 ms |
| NVMe random 4 KiB | 10–100 µs | |
| HDD seek | 1–10 ms | Unlikely for FCS on this host |

1,000,000 events × 20 `f32` detectors is 80 MiB. A sequential read is tens of
milliseconds; a parse that does one syscall per value is seconds
([`bulk-syscall-io`](PERF_STRATEGIES.md#bulk-syscall-io)).

## 7. Parallel and GPU

| Operation | Order of magnitude | Notes |
|-----------|-------------------|-------|
| Rayon pool already warm, useful chunk | ~µs overhead | Wins when per-chunk work ≫ 10–100 µs |
| Rayon over 256 tiny events | net **loss** | `parallel_event_threshold` exists for this |
| Nested Rayon + threaded BLAS | slowdown | Set `OMP_NUM_THREADS=1` under an event pool |
| wgpu/cubeCL kernel launch | 5–50 µs | Plus first-dispatch pipeline compile (ms–s, once) |
| Discrete GPU H2D (PCIe 4.0 x16) | ~25 GB/s | **Not this Mac** |
| Apple unified GPU access | memcpy-class + cache | Still not free: scatter-clean 10k is a wash vs CPU |

Evidence already in-tree: GPU scatter-clean at 10,000 events is +1.7% (skip);
at 50,000 events it is 3.1× ([`gpu-after-amortize`](PERF_STRATEGIES.md#gpu-after-amortize)).
PeacoQC GPU KDE microbenches can win while full QC e2e loses to Rayon CPU.

## 8. Compounding recipe

1. Count **bytes** and **FLOPs** per event (include temps, not only inputs).
2. `T_mem = bytes / DRAM_or_cache_bw` using the hierarchy row for `working_set`.
3. `T_arith = flops / (SIMD_width × GHz × useful_cores)`.
4. `T_alloc = (allocs per event) × 50 ns` (or host malloc from `PERF_HOST.md`).
5. `T_fixed` = factorization, index build, kernel launch, FCS parse.
6. `T_lower = T_fixed + n × max(T_arith, T_mem) + n × T_alloc`.
7. Ratio `measured / T_lower` → gap bucket in [`PERF_PGD.md`](PERF_PGD.md).

### Worked example: joint unmix alloc tax

Recorded in [`flow-autospectral/docs/PERF_AB.md`](../../flow-autospectral/docs/PERF_AB.md):
10,000 events, 20 detectors, 8 fluorophores, 8 AF spectra.

Arithmetic floor (very rough): a few thousand FLOPs/event × 10,000 ≈ 2e7 FLOPs.
One P-core at 16 GFLOP/s → **~1 ms** if fully vectorized. Memory: 10,000 × 20 × 4 B
= 800 KiB, L2-resident. Alloc floor if each event builds ~20 short `Vec`s at 50 ns:
10,000 × 20 × 50 ns = **10 ms**.

Measured: 4.464 ms before workspace reuse, 2.096 ms after (−56%). The keep moved
the path from **alloc-bound** toward **compute/occupancy**. It is still a few× the
pure FMA floor (3–10× bucket: layout / SIMD / Rayon), not a 100× complexity bug.

### Worked example: `f64` vs `f32` at 1M × 64

`working_set` doubles (512 MiB vs 256 MiB). DRAM traffic doubles. NEON occupancy
halves. Napkin: **~2×** slower on a memory-bound scan, before counting extra cache
pressure. Use `f64` when the algorithm needs it (some KDE/FFT paths);
do not inherit it into unmix/KNN by default.

### Worked example: exact KNN vs HNSW

100,000 events, 20 detectors, k=60. Exact: `n² d` ≈ 2e11 FLOP-equivalents of
distance work → **seconds** (measured exact CPU 10.1 s at 100k×20). HNSW measured
2.5–3.9 s. The gap is **complexity**, not a missed `get_unchecked`.
