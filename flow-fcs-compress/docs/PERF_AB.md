# flow-fcs-compress unsafe micro-opt A/B

Protocol: [`docs/dev/UNSAFE_MICROOPT_AB.md`](../../docs/dev/UNSAFE_MICROOPT_AB.md).
Bench: `cargo bench -p flow-fcs-compress --bench byte_stream_split`.

## BSS split / unsplit (`get_unchecked`)

| Item | Status | Pre median | Post median | Delta | Primary size | Machine | rustc | Date | Notes |
|------|--------|------------|-------------|-------|--------------|---------|-------|------|-------|
| bss_split | reverted | 118.90 µs | 120.96 µs | +1.7% (noise) | 1M f32 | arm64 Apple | 59807616e | 2026-08-02 | Criterion: no change vs `unsafe-ab-bss-pre`; &lt;5% keep rule |
| bss_unsplit | reverted | 170.68 µs | 169.64 µs | −0.6% | 1M f32 | arm64 Apple | 59807616e | 2026-08-02 | Same; LLVM already elides/amortizes checks |

Secondary sizes (post vs pre, change within noise): split 64k ≈ −2%, 256k ≈ −0.4%; unsplit 64k ≈ +2%, 256k ≈ +2%.

**Decision:** keep safe indexed implementation; retain Criterion bench for future regressions.
