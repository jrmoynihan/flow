# peacoqc-rs alloc micro-opt A/B

Protocol: [`docs/dev/UNSAFE_MICROOPT_AB.md`](../../docs/dev/UNSAFE_MICROOPT_AB.md) Campaign 2.
Bench: `cargo bench -p peacoqc-rs --no-default-features --features flow-fcs --bench peaks_alloc_micro`.

## Peak-detection bin windows: slice versus copy

**Problem.** Peak detection copies each time-bin’s intensity window into a new `Vec` (`to_vec`) before kernel density estimation (KDE). At 100,000 events that is one heap allocation per bin.

**Solution tried.** Pass a slice of the existing channel buffer into KDE when zeros are not removed (`!remove_zeros`), so the bin window is a view instead of a copy.

**What changed in operation.**

- Before: each bin allocates and fills a new vector, then KDE runs on that copy.
- After: KDE reads the bin as a slice of the parent buffer (no copy when `remove_zeros` is false).
- Difference: median 1.1084 ms → 1.1166 ms (**+0.7%**, noise) at 100,000 events. KDE still dominates wall time, so removing the copies did not move the needle past the 5% keep rule.

**Decision:** reverted. The `to_vec` path remains. Code: `peaks_alloc_micro` bench; peak-detection bin windows in `peacoqc-rs`.

## Cost model (napkin vs measured)

Workspace protocol: [`docs/dev/PERF_PGD.md`](../../docs/dev/PERF_PGD.md). Index: [`docs/dev/PERF_GAP.md`](../../docs/dev/PERF_GAP.md).

**Peaks bin windows** (100,000 events): **1.11 ms**. Skipping `to_vec` did not move the median (KDE-bound). The primitive is FFT KDE on bin-sized windows (`flow-density`), not alloc. Do not retry `workspace-per-worker` here.

**GPU KDE** can win a microbench while full PeacoQC e2e loses to Rayon CPU. Strategy: `gpu-after-amortize` — do not headline GPU for QC-core until e2e wins. Encoding: KDE APIs take `&[f64]` (see `flow-density`); the grid is hundreds of points, so width does not change cache class at typical 1D sizes.
