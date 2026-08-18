# flow-autospectral performance A/B notes

Follow workspace protocol in `docs/dev/UNSAFE_MICROOPT_AB.md` when micro-optimizing.

For match throughput, interleave baseline/HEAD Criterion runs and keep an untouched control bench (see beads memory `benchmark-a-b-on-this-machine-apple-m5`).

Primary quality metrics for algorithm changes: OLS residual, population spread — not wall time alone.
