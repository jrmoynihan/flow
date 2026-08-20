# flow-fcs-compress unsafe / alloc micro-opt A/B

Protocol: [`docs/dev/UNSAFE_MICROOPT_AB.md`](../../docs/dev/UNSAFE_MICROOPT_AB.md).

## Byte-stream split / unsplit: bounds checks versus `get_unchecked`

Bench: `cargo bench -p flow-fcs-compress --bench byte_stream_split`.

[Byte stream split](https://arrow.apache.org/docs/format/Columnar.html#byte-stream-split-layout) (BSS) rewrites each `f32` as four byte planes (all byte 0s, then all byte 1s, …). Split and unsplit are tight indexed loops over 1,000,000 `f32` values in the primary size.

**Problem.** Safe indexing (`bytes[i]`) theoretically pays a bounds check per access.

**Solution tried.** `get_unchecked` in the split and unsplit loops.

**What changed in operation.**

- Before: safe indexing; LLVM can hoist or eliminate many checks in a counted loop.
- After: unchecked indexing with a `# Safety` argument that `i` stays in range.
- Difference: split 118.90 µs → 120.96 µs (**+1.7%**); unsplit 170.68 µs → 169.64 µs (**−0.6%**). Secondary sizes (64k and 256k elements) stayed within a few percent. The checks were not the bottleneck.

**Decision:** keep the safe indexed implementation. Leave the Criterion bench for regressions.

| What we changed | Status | Before | After | Delta | Size | Date |
|-----------------|--------|--------|-------|-------|------|------|
| BSS split `get_unchecked` | reverted | 118.90 µs | 120.96 µs | +1.7% (noise) | 1,000,000 `f32` values | 2026-08-02, arm64 Apple, rustc 59807616e |
| BSS unsplit `get_unchecked` | reverted | 170.68 µs | 169.64 µs | −0.6% | 1,000,000 `f32` values | 2026-08-02, arm64 Apple, rustc 59807616e |

## Encode path: reuse one payload buffer across chunks

Bench: `cargo bench -p flow-fcs-compress --bench chunk_encode_scratch`.

**Problem.** Each compressed chunk allocated a fresh `Vec<u8>` for the BSS+zstd payload. Sixteen chunks of 64k values means sixteen heap allocations on the encode path.

**Solution tried.** Reuse one `payload` vector across chunks (clear and refill; keep capacity).

**What changed in operation.**

- Before: `Vec::new` (or equivalent) per chunk.
- After: one vector, `clear` between chunks.
- Difference: 1.5652 ms → 1.6425 ms (**+8.1%**, regression) for 16 × 64k BSS+zstd. [zstd](https://facebook.github.io/zstd/) compression dominates; the extra allocator traffic was not the limiter, and reuse added bookkeeping without a win.

**Decision:** reverted. Fresh `Vec` per chunk remains.

| What we changed | Status | Before | After | Delta | Size | Date |
|-----------------|--------|--------|-------|-------|------|------|
| Reuse encode `payload` Vec across chunks | reverted | 1.5652 ms | 1.6425 ms | +8.1% (regressed) | 16 chunks × 64k values, BSS+zstd | 2026-08-02, arm64 Apple, rustc 59807616e |

## Cost model (napkin vs measured)

Workspace protocol: [`docs/dev/PERF_PGD.md`](../../docs/dev/PERF_PGD.md). Index: [`docs/dev/PERF_GAP.md`](../../docs/dev/PERF_GAP.md).

**BSS split** (1,000,000 `f32`, 4 MiB): **119 µs**. `memcpy` of 4 MiB at ~26 GB/s ≈ 150 µs. Ratio **~0.8–1×** vs bulk move (on the roofline). `get_unchecked` cannot help. Encoding: BSS is an IO/codec layout, not an unmix inner-loop format.

**Chunk encode BSS+zstd** (16 × 64k): **1.57 ms**. zstd dominates; payload `Vec` reuse **regressed**. Do not retry `workspace-per-worker` until a profile shows alloc, not zstd, in the sample.
