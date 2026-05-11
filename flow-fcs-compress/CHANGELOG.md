# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.2 (2026-05-11)

### Documentation

 - <csr-id-c5bda8ca7e17807268121b5027cfdc7849a91dd4/> consolidate codec table with encode/decode throughput and fidelity columns

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Consolidate codec table with encode/decode throughput and fidelity columns ([`c5bda8c`](https://github.com/jrmoynihan/flow/commit/c5bda8ca7e17807268121b5027cfdc7849a91dd4))
</details>

## v0.1.1 (2026-05-11)

<csr-id-90e3ee26926e8df26e30fdf12ac25aae632d1dd9/>

### Chore

 - <csr-id-90e3ee26926e8df26e30fdf12ac25aae632d1dd9/> bump to 0.1.1 with readme field for crates.io display

### Documentation

 - <csr-id-e14b4799997458dba9110f4723e88dc50c8dce5b/> add README.md to flow-linalg, flow-density, flow-clustering, flow-fcs-compress
   Each README describes the crate's purpose, public API, algorithms,
   scope boundaries, features, tests, and benchmarks for prospective
   users and contributors.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-linalg v0.1.1, flow-density v0.1.1, flow-clustering v0.1.1, flow-fcs-compress v0.1.1 ([`966d22a`](https://github.com/jrmoynihan/flow/commit/966d22ae4fbdd6114dc3862d45648fce7ebf53cc))
    - Bump to 0.1.1 with readme field for crates.io display ([`90e3ee2`](https://github.com/jrmoynihan/flow/commit/90e3ee26926e8df26e30fdf12ac25aae632d1dd9))
    - Add README.md to flow-linalg, flow-density, flow-clustering, flow-fcs-compress ([`e14b479`](https://github.com/jrmoynihan/flow/commit/e14b4799997458dba9110f4723e88dc50c8dce5b))
    - Merge branch 'feat/flow-fcs-compress' ([`ef239b2`](https://github.com/jrmoynihan/flow/commit/ef239b24dbacfabc1e68dfa5f4dc8baa49f9704a))
    - Merge pull request #20 from jrmoynihan/feat/flow-fcs-compress ([`f953bc5`](https://github.com/jrmoynihan/flow/commit/f953bc5df8f6978e3fe511538cb2943730a35eff))
</details>

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

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release over the course of 3 calendar days.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-linalg v0.1.0, flow-density v0.1.0, flow-clustering v0.1.0, flow-fcs-compress v0.1.0, flow-fcs v0.4.0 ([`e8c908e`](https://github.com/jrmoynihan/flow/commit/e8c908ef92fb68b8e2d01d3c1e8d6a294c8c6bda))
    - Fix section heading in ISAC proposal ([`9012270`](https://github.com/jrmoynihan/flow/commit/9012270efea8a503c72c32bc9bda717e0dde0a48))
    - Add FczReader::warm_cache for page fault elimination before timed reads ([`53b4eba`](https://github.com/jrmoynihan/flow/commit/53b4eba342397dffb258f37c1a80a430683955b7))
    - Add compression crate, benchmarks, and ISAC proposal ([`a4a5e18`](https://github.com/jrmoynihan/flow/commit/a4a5e18e06b55de252b74110118ac72aa2fc0891))
</details>

<csr-unknown>
Mode A lossless f32: byte-stream-split + zstd (default), pco backend(optional, behind pco-backend feature)Mode B lossless within ADC bit depth: bit-reservoir bitpack withper-chunk signed offset, ~3.5x decode speedup vs naive bit-extractMode C lossy log-quantization: arcsinh transform + fixed-point quantizewith sinh LUT for bits <= 14 (~4x decode speedup at small widths)Auto codec picker: never selects a lossy codec without explicit opt-inlz4_flex baseline (optional, behind lz4-baseline feature).fcz native container with mmap + rayon parallel decodeInline FCS DATA-segment payload format (codec-payload bytes intendedto live inside an FCS file with $COMPRESSION = FCZ1 keyword)New EncodedChunk API splits parallel encode (encode_chunk_payload,&self) from sequential append (append_encoded_chunk, &mut self)synth: per-codec/per-channel CSV table on synthetic channelsfile: per-codec table on real .fcs files, with auto-picker validationfile-full / synth-full: whole-dataset roundtrip with both serial andrayon-parallel encode and decode throughputFcs::write_fcz / Fcs::events_from_fcz round-trip via .fcz containerFcs::write_inline_fcs / Fcs::events_from_inline_fcs (FCS-inline pilotwith $COMPRESSION extension keyword)Fcs::write_parquet / Fcs::events_from_parquet (Parquet sidecar Tier 1)Expose write::serialize_metadata, write::build_header,write::estimate_text_segment_size as pub(crate) helpers for theinline-FCS writerVerified against FCS 3.2 spec (Spidlen 2021): $PnB unambiguousstorage width per S3.3.38; $PnR for F/D types is the soft hintper S3.3.51; row-major DATA layout mandated per S3.4New keywords proposed: $LAYOUT (column-major option), $COMPRESSION,$PnCOMPRESSION, $COMPRESSIONPARAMS, $PnADCBITS, $PnLAYOUT,$CHECKSUM, $CHUNKINDEXSections: cache-friendliness rationale (cite ithare.com cyclecosts), why FCS is row-major (acquisition FIFO/DMA),tradeoffs vs alternatives (status quo, file-level gzip, Parquetmigration, vendor variants), reviewer critiques and responses,performance metrics (M1 Max 10-core, single + multi-threadedencode + decode at 80 MB / 400 MB / 1024 MB), FlowRepositoryimpact, FCS 4.0 / ACS status (no FCS 4.0 in active development;the 2007 working draft was renamed to ACS)Scope explicitly excludes HEADER (58 fixed bytes) and TEXT(<0.01% of large files, bootstraps DATA offsets)<csr-unknown/>

