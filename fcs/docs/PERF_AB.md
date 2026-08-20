# flow-fcs unsafe micro-opt A/B

Protocol: [`docs/dev/UNSAFE_MICROOPT_AB.md`](../../docs/dev/UNSAFE_MICROOPT_AB.md).

## Column de-interleave

Bench: `cargo bench -p flow-fcs --bench column_extract`.

**Problem.** FCS DATA is stored event-major: detector values for one event sit together, so one parameter’s column is a strided view (every *d*-th value). Extracting a column with iterator `skip` / `step_by` / `collect` does a bounds-checked stride.

**Solution tried.** `get_unchecked` plus `set_len` on a pre-sized buffer, skipping Rust’s per-index checks.

**What changed in operation.**

- Before: safe strided iterator into a new `Vec`.
- After: unchecked indexed writes into a buffer whose length is set after the loop.
- Difference: at the primary size (1,000,000 events × 20 parameters) median **regressed** 14.831 ms → 15.135 ms (**+2.0%**). Secondary: 100,000 events × 20 parameters **+14%**; 1,000,000 events × 40 parameters **+4%**. The strided loads are the cost; removing bounds checks did not help.

**Decision:** reverted. `extract_param_column` remains for benches; the implementation is the original iterator path.

| What we changed | Status | Before | After | Delta | Size | Date |
|-----------------|--------|--------|-------|-------|------|------|
| Unchecked strided column extract | reverted | 14.831 ms | 15.135 ms | +2.0% (regressed) | 1,000,000 events × 20 parameters | 2026-08-02, arm64 Apple, rustc 59807616e |

## Little-endian `serialize_data` (bulk cast)

Bench: `cargo bench -p flow-fcs --bench serialize_data`.

**Problem.** Writing DATA as little-endian `f32` loops `write_f32` (or equivalent) per value: many small stores rather than one contiguous copy of the float payload.

**Solution tried.** Build one `Vec<u8>` via [bytemuck](https://docs.rs/bytemuck) by reinterpreting the `f32` slice on little-endian hosts. A second variant buffered floats first, then cast (double buffer).

**What changed in operation.**

- Before: per-value endian writes.
- After (single buffer): one cast of the existing float storage to bytes, then one write.
- Difference: at 1,000,000 events × 20 parameters, 17.992 ms → 18.522 ms (**+2.9%**, under the 5% keep rule). At 100,000 events the single-buffer path was about **−13%**, but the primary size did not keep. The double-buffer path **regressed ~25%** (extra copy).

**Decision:** reverted the serialize hot path. `serialize_f32_columns` and the bench remain; implementation still uses the `write_f32` endian path.

| What we changed | Status | Before | After | Delta | Size | Date |
|-----------------|--------|--------|-------|-------|------|------|
| bytemuck single-buffer LE serialize | reverted | 17.992 ms | 18.522 ms | +2.9% (noise) | 1,000,000 events × 20 parameters | 2026-08-02, arm64 Apple, rustc 59807616e |

## Cost model (napkin vs measured)

Workspace protocol: [`docs/dev/PERF_PGD.md`](../../docs/dev/PERF_PGD.md). Index: [`docs/dev/PERF_GAP.md`](../../docs/dev/PERF_GAP.md).

**Column extract** (1,000,000 events × 20 parameters): **14.8 ms**. Payload 80 MiB. This is a **strided gather** (event-major DATA → one parameter). Host random gather ~8 ns × 1e6 ≈ 8 ms; sequential `f32` sum would be ~1 ms. Ratio vs gather **~2×** (1–3×). `get_unchecked` reverted. Encoding: decoded `f32` columns — correct once many kernels reuse them.

**LE serialize** (same size): **18.0 ms**. `memcpy` 64 MiB is 2.6 ms on this host → **~6×** vs bulk move (**3–10×**, per-value `write_f32`). Not a syscall storm. Stay in the gap table; no bead.
