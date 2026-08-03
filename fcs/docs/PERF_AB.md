# flow-fcs unsafe micro-opt A/B

Protocol: [`docs/dev/UNSAFE_MICROOPT_AB.md`](../../docs/dev/UNSAFE_MICROOPT_AB.md).

## Column de-interleave

Bench: `cargo bench -p flow-fcs --bench column_extract`.

| Item | Status | Pre median | Post median | Delta | Primary size | Machine | rustc | Date | Notes |
|------|--------|------------|-------------|-------|--------------|---------|-------|------|-------|
| column_extract | reverted | 14.831 ms | 15.135 ms | +2.0% (regressed) | 1M×20 | arm64 Apple | 59807616e | 2026-08-02 | Strided `get_unchecked`+`set_len` slower than `skip`/`step_by`/`collect`; kept collect |

Secondary: 100k×20 +14% regress; 1M×40 +4% regress. Helper `extract_param_column` retained for benches; implementation is the original iterator path.

## FCS LE serialize_data (bulk)

Bench: `cargo bench -p flow-fcs --bench serialize_data`.

| Item | Status | Pre median | Post median | Delta | Primary size | Machine | rustc | Date | Notes |
|------|--------|------------|-------------|-------|--------------|---------|-------|------|-------|
| serialize_data_le | reverted | 17.992 ms | 18.522 ms | +2.9% (noise) | 1M×20 | arm64 Apple | 59807616e | 2026-08-02 | bytemuck single `Vec<u8>` cast: 100k −13% but primary &lt;5%; double-buffer float then cast regressed ~25% |

Kept `serialize_f32_columns` helper + bench; implementation remains `write_f32` endian path.
