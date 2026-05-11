# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.0 (2026-05-11)

### Documentation

 - <csr-id-9012270efea8a503c72c32bc9bda717e0dde0a48/> fix section heading in ISAC proposal

### New Features

 - <csr-id-53b4eba342397dffb258f37c1a80a430683955b7/> add FczReader::warm_cache for page fault elimination before timed reads
 - <csr-id-a4a5e18e06b55de252b74110118ac72aa2fc0891/> add compression crate, benchmarks, and ISAC proposal
   Introduce two new workspace crates and an ISAC FCS WG proposal targeting
   compression and column-major DATA layout for the FCS standard.
   
   flow-fcs-compress (new crate, codec library + container adapters):
   - ColumnCodec trait with chunked encode/decode and zero-copy semantics
   - Mode A lossless f32: byte-stream-split + zstd (default), pco backend
     (optional, behind `pco-backend` feature)
   - Mode B lossless within ADC bit depth: bit-reservoir bitpack with
     per-chunk signed offset, ~3.5x decode speedup vs naive bit-extract
   - Mode C lossy log-quantization: arcsinh transform + fixed-point quantize
     with sinh LUT for bits <= 14 (~4x decode speedup at small widths)
   - Auto codec picker: never selects a lossy codec without explicit opt-in
   - lz4_flex baseline (optional, behind `lz4-baseline` feature)
   - .fcz native container with mmap + rayon parallel decode
   - Inline FCS DATA-segment payload format (codec-payload bytes intended
     to live inside an FCS file with `$COMPRESSION = FCZ1` keyword)
   - New EncodedChunk API splits parallel encode (`encode_chunk_payload`,
     &self) from sequential append (`append_encoded_chunk`, &mut self)
   
   flow-fcs-bench (new bin crate):
   - synth: per-codec/per-channel CSV table on synthetic channels
   - file: per-codec table on real .fcs files, with auto-picker validation
   - file-full / synth-full: whole-dataset roundtrip with both serial and
     rayon-parallel encode and decode throughput
   
   flow-fcs (existing crate, gains optional `compress` and `parquet-sidecar`
   features):
   - Fcs::write_fcz / Fcs::events_from_fcz round-trip via .fcz container
   - Fcs::write_inline_fcs / Fcs::events_from_inline_fcs (FCS-inline pilot
     with `$COMPRESSION` extension keyword)
   - Fcs::write_parquet / Fcs::events_from_parquet (Parquet sidecar Tier 1)
   - Expose write::serialize_metadata, write::build_header,
     write::estimate_text_segment_size as pub(crate) helpers for the
     inline-FCS writer
   
   ISAC proposal (flow-fcs-compress/docs/isac-proposal.md):
   - Verified against FCS 3.2 spec (Spidlen 2021): $PnB unambiguous
     storage width per S3.3.38; $PnR for F/D types is the soft hint
     per S3.3.51; row-major DATA layout mandated per S3.4
   - New keywords proposed: $LAYOUT (column-major option), $COMPRESSION,
     $PnCOMPRESSION, $COMPRESSIONPARAMS, $PnADCBITS, $PnLAYOUT,
     $CHECKSUM, $CHUNKINDEX
   - Sections: cache-friendliness rationale (cite ithare.com cycle
     costs), why FCS is row-major (acquisition FIFO/DMA),
     tradeoffs vs alternatives (status quo, file-level gzip, Parquet
     migration, vendor variants), reviewer critiques and responses,
     performance metrics (M1 Max 10-core, single + multi-threaded
     encode + decode at 80 MB / 400 MB / 1024 MB), FlowRepository
     impact, FCS 4.0 / ACS status (no FCS 4.0 in active development;
     the 2007 working draft was renamed to ACS)
   - Scope explicitly excludes HEADER (58 fixed bytes) and TEXT
     (<0.01% of large files, bootstraps DATA offsets)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release over the course of 3 calendar days.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Fix section heading in ISAC proposal ([`9012270`](https://github.com/jrmoynihan/flow/commit/9012270efea8a503c72c32bc9bda717e0dde0a48))
    - Add FczReader::warm_cache for page fault elimination before timed reads ([`53b4eba`](https://github.com/jrmoynihan/flow/commit/53b4eba342397dffb258f37c1a80a430683955b7))
    - Add compression crate, benchmarks, and ISAC proposal ([`a4a5e18`](https://github.com/jrmoynihan/flow/commit/a4a5e18e06b55de252b74110118ac72aa2fc0891))
</details>

