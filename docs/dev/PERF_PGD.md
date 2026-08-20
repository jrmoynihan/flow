# Performance-guided development

Compare a **cost model** (from [`PERF_LATENCIES.md`](PERF_LATENCIES.md) and
[`PERF_HOST.md`](PERF_HOST.md)) to a **Criterion median**. The ratio is the
signal. Do not start with a 5% `unsafe` A/B when the path is 50× off a roofline.

Related:

- Gap index: [`PERF_GAP.md`](PERF_GAP.md)
- Retry shortlist: [`PERF_STRATEGIES.md`](PERF_STRATEGIES.md)
- Keep/revert for small hot-path A/Bs: [`UNSAFE_MICROOPT_AB.md`](UNSAFE_MICROOPT_AB.md)

## Loop

1. Name the hot path and the Criterion id (or example) that times it.
2. Count bytes, FLOPs, allocs, and syscalls per event. Include temps.
3. Compute `T_lower` with the compounding recipe in `PERF_LATENCIES.md`.
4. Ratio = measured median / `T_lower`.
5. Bucket (below). If 3× or worse, pick a tactic from `PERF_STRATEGIES.md`
   **before** inventing a new one.
6. A/B only if the tactic applies. Keep if ≥5% on the primary size
   (`UNSAFE_MICROOPT_AB.md`).
7. If still ≥10×, or a concrete encoding miss, **file a Beads issue** under the
   PGD epic. Put the id on the `PERF_GAP.md` row. Do not bury it in prose.

On this machine, interleave baseline/HEAD when A/B-ing (beads memory
`benchmark-a-b-on-this-machine-apple-m5`). Host calibration snapshots are a
single pass, not an A/B; do not treat session drift as a regression.

## Gap buckets

| Ratio `measured / T_lower` | Meaning | Action |
|----------------------------|---------|--------|
| **1–3×** | On the roofline | Stop. A/B only if ≥5% on the primary size |
| **3–10×** | Cache, occupancy, SIMD, type-width/packing, or parallel efficiency | Try a `PERF_STRATEGIES` layout/occupancy/encoding card. Stay in `PERF_GAP.md` unless the encoding rule below fires |
| **10–100×** | Wrong primitive (alloc, extra copies, syscalls) | File a Beads task. Try `workspace-per-worker`, `bulk-syscall-io`, `hoist-factor-once` |
| **>100×** | Wrong complexity or a hoist that was never done | File a Beads task. Exact `n²` vs HNSW, QR-per-cell vs factor-once |

## When to file Beads

Create a child of the PGD epic when **any** of:

- Ratio **≥10×**
- Ratio **>100×**
- A concrete **encoding/width miss** that would change cache level or SIMD width
  on a primary size (`f64` where `f32` is enough; unpacked `f32` when packed
  `u16` would stay in L2) — even if the time ratio is only 3–10×

Do **not** file Beads for 1–3× rows.

Each issue: `--type=task` (or `bug` if it is a regression), priority 2 unless the
path is a published QC-core claim (then 1), parent = PGD epic. Description must
include crate, Criterion id, napkin vs measured, suspected primitive, which
`PERF_STRATEGIES` id to try first (or “none”), and the next experiment.

## Cost-model subsection (per crate)

Add a short **Cost model** block to the crate’s `PERF_MATRIX.md` / `PERF_AB.md` /
`PROFILING.md` (do not replace measured tables):

- Operation mix and encoding (`f32` vs `f64`, packed vs unpacked)
- `T_lower` and the measured median
- Ratio, bucket, strategy id (or “none — new tactic”)
- Bead id if filed

## Provenance

Same fields as [`UNSAFE_MICROOPT_AB.md`](UNSAFE_MICROOPT_AB.md): date, host/chip,
`rustc` short hash, `RUSTFLAGS`, features, Criterion group + ids, primary size.
