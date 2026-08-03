# peacoqc-rs alloc micro-opt A/B

Protocol: [`docs/dev/UNSAFE_MICROOPT_AB.md`](../../docs/dev/UNSAFE_MICROOPT_AB.md) Campaign 2.
Bench: `cargo bench -p peacoqc-rs --no-default-features --features flow-fcs --bench peaks_alloc_micro`.

## Peaks bin-window copies

| Item | Status | Pre median | Post median | Delta | Primary size | Machine | rustc | Date | Notes |
|------|--------|------------|-------------|-------|--------------|---------|-------|------|-------|
| peaks_alloc_micro | reverted | 1.1084 ms | 1.1166 ms | +0.7% (noise) | 100k events | arm64 Apple | 59807616e | 2026-08-02 | slice-into-KDE when `!remove_zeros`; KDE dominates; `to_vec` kept |
