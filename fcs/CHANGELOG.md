# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.5.1 (2026-08-10)

### Added

- **Lazy column accessors**: `Fcs::column`, `Fcs::columns`, and `Fcs::events` with
  cached layouts via `ColumnLayout` / `extract_columns` for row-major materialization.
- **`Fcs::open_all()`**: walk `$NEXTDATA` chains for multi-dataset FCS (e.g. Beckman `.lmd`).
- **`Fcs::for_testing`**: public test-fixture constructor (feature-gated) for out-of-crate tests.
- **FCS 3.2 conformance work**: CRC, datetime/keyword handling, and related rules.

### Fixed

- **`$PnR` masking** for integer parameters; **bit-packed `$PnB` stride** calculation;
  **data-set-relative offsets** (HEADER / `$BEGIN*` / `$NEXTDATA`) with vendor-absolute disambiguation.
- OTHER-segment offset scan bounded at the first segment; cache emptiness / bounds hardening
  for derived `Fcs` values.

### Bug Fixes (BREAKING)

 - <csr-id-f0b29225fb01d5d2c8060e2b9fdf4b9b87b2dfa7/> resolve offsets data-set-relative, fold OTHER into CRC range
   Every FCS offset is measured from the start of the data set that declares
   it, not from the start of the file: HEADER fields (§2.4.3), $BEGINDATA and
   $BEGINANALYSIS (§3.3.3), and $NEXTDATA (§3.3.31). We were treating them all
   as file-absolute.
   
   The bug stayed invisible because a two-data-set file -- which is what every
   .lmd is -- takes exactly one hop, from byte 0, where relative and absolute
   agree. It takes a three-data-set chain to expose it, and no fixture had one.
   
   Fcs gains a public dataset_start; a private absolutize() in file.rs maps a
   declared offset to a file-absolute one. It disambiguates rather than
   assuming, because vendors do emit file-absolute offsets: an offset below
   dataset_start must be relative, and otherwise the relative reading wins
   unless it runs past EOF, in which case we warn and fall back. That keeps the
   existing vendor-style two-data-set fixture green.

### New Features (BREAKING)

 - <csr-id-b6eb1c2c1f7f3fda501406a830cede1e5cf3913e/> FCS 3.2 conformance — CRC, datetime, keywords, conformance rules
   Closes the fcs half of epic flow-crates-x17. Every file the crate writes is
   now FCS 3.2 conformant, and files past 99,999,999 bytes no longer panic on
   write.
   
   CRC (§3.7, flow-crates-x17.3)
     New fcs/src/crc.rs implements CRC-16/KERMIT. The polynomial text in the spec
     does not pin the algorithm — XMODEM-with-reflected-input reads the same prose
     and disagrees on nearly every message. §3.7's normative vector settles it:
     brute-forcing the CRC-16 parameter space against compute("CatMouse987654321")
     == 49805 yields exactly one match. Both that vector and KERMIT's catalog
     check value are asserted, so drift toward XMODEM fails immediately.
   
     The on-disk field is 8 ASCII bytes of DECIMAL, left-zero-padded ("00049805"),
     not hex — the spec quotes the value in hex in the same sentence, which is the
     trap. Eight ASCII zeros means "not computed"; emitting nothing, which this
     crate did until now, is not a legal encoding, so every file it had ever
     written was non-conformant even under the opt-out.
   
     Read side warns rather than rejects: many vendor files carry absent or wrong
     CRCs, and hard-failing would make them unopenable. StoredCrc distinguishes
     Absent / Value / Malformed / Missing so a pre-CRC file is not called corrupt.
     Fcs::open_verified opts into strict rejection.
   
   HEADER offsets (flow-crates-x17.1)
     build_header wrote format!("{:>8}", offset) into fixed 8-byte slices with no
     width guard, so any segment past 99,999,999 bytes panicked in
     copy_from_slice. A ~768 MB spectral file reaches this in normal use. Per
     §2.2.4 those offsets are now declared 0 and carried in $BEGINDATA/$ENDDATA.
   
     resolve_segment_offsets() is extracted in file.rs so the DataFrame reader and
     the CRC locator resolve segment bounds the same way; reading them off Header
     directly is now documented as wrong.
   
   Conformance rules and 3.2 keywords (flow-crates-x17.4, x17.5)
     New fcs/src/conformance.rs holds per-version rules keyed by Version, ready to
     be lifted into the VersionSpec trait (flow-crates-zmx) rather than scattered.
     Warnings by default, errors under ConformancePolicy::Strict, so existing
     pipelines that write slightly-off files keep working.
   
     Adds $UNSTAINEDINFO/$UNSTAINEDCENTERS and MixedKeyword::MixingMatrix, a
     rectangular detector×endmember matrix that $SPILLOVER's square encoding
     cannot express.
   
     fcs/src/upgrade.rs migrates a 3.0/3.1 TEXT segment to 3.2 in place, keeping
     the deprecated originals so 3.1 readers still work.
   
     estimate_text_segment_size now asks each keyword for its serialized length
     (flow-crates-x17.2): a 64×40 matrix is ~30 KB in one keyword, which the old
     flat 50-bytes-per-keyword estimate undershot badly enough to exhaust the
     offset-convergence budget.
   
   Also fixes write_inline_fcs baking a stale $BEGINDATA into TEXT
   (flow-crates-x17.9), and routes both writers through a shared write_segments()
   so a writer cannot forget the CRC.

### Test

 - <csr-id-7ccf98f5c2c6fd282b1381d00e48b15d2bbe7788/> verify bit-packed fallback rejects lazy column() access
   column() must reject bit-packed layouts rather than attempt a byte-stride
   decode that can't represent them. events() correctness for bit-packed data
   (including $PnR masking) is already covered by
   bit_packed_events_applies_pnr_mask_matching_data_frame_oracle.
 - <csr-id-da20fb28d685f27f0349fd55f8c97d9e4b06a9b3/> exercise non-uniform param widths in ColumnLayout offset test

### Refactor

 - <csr-id-c32a0cf75877d51201ffca309d0373328d9f9f69/> dedupe $PnR masking formula, fix ColumnLayout docs, strengthen cache-emptiness test
 - <csr-id-1d76e633be505604bd3d36996b4cbd4b80679469/> dedupe columns() and have column() delegate to it
   Fix 1: column() reimplemented columns()'s cache-check/decode/populate
   sequence instead of delegating to it for a single-element request.
   Fix 2: columns() could pass duplicate indices to extract_columns when
   the same channel name was requested more than once; dedupe `missing`
   before decoding.
   
   Addresses two Minor findings deferred from the earlier lazy FCS column
   loading task.
 - <csr-id-cdc0f8b085b5d250037fd003050869de968ec797/> widen visibility of parse helpers to pub(crate)

### Performance

 - <csr-id-2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37/> bulk-load KnnGraph IO; record unsafe micro-opt A/B
   Keep the ~100× faster graph load via read_exact + LE bytemuck cast.
   Add Criterion benches and PERF_AB docs for the six-item A/B campaign;
   revert opts that missed the ≥5% keep rule (BSS, FCS columns/write,
   TRU-OLS SyncPtr, exact KNN / PaCMAP unchecked).

### Other

 - <csr-id-eec97b1b3512332223985c0dadf268fd8d3a9eba/> compare lazy column/events access against the eager baseline
   Adds a criterion benchmark on a real compliance-corpus file comparing the
   new lazy .columns()/.events() paths (Tasks 2-5) against the existing eager
   data_frame parse, so a CPU-time regression wouldn't slip in silently while
   the Stage A memory-savings design is measured.

### Bug Fixes

 - <csr-id-0257d38c13049921f84a0f9630f4d4327138f8c9/> bound the OTHER offset scan at the first segment, not at TEXT
 - <csr-id-a565fdf4b372fe74eb6393eb61218a8ea159b6fe/> address final whole-branch review findings (bounds check, cache warning, feature scoping, version bump, benchmark docs)
   - Fcs::columns() now returns a descriptive Err instead of panicking when a
     parameter's cache index falls outside the column cache (can happen on a
     derived Fcs whose parameters were replaced without resizing the cache,
     e.g. tru-ols's spectral-unmixing output). Added a regression test.
   - Added a `# Warning` doc section to column()/columns()/events() noting the
     cache is only meaningful on an Fcs from open()/open_all() (flow-crates-rkq).
   - Moved flow-fcs's `test-util` feature enablement from [dependencies] to
     [dev-dependencies] in gates, tru-ols, and peacoqc-rs so it's no longer
     forced on in release builds via feature unification.
   - Bumped flow-fcs to 0.5.1 (test-util didn't exist in the published 0.5.0)
     and its dependents' version constraints to ^0.5.1.
   - Documented the benchmark's actual ~8x events_uncached/open_eager_baseline
     gap (extract_columns lacking a uniform-width fast path, not double work
     from open()) in the benchmark source and amended the Stage A plan doc.
     Tracked as flow-crates-3si.
 - <csr-id-6e3d7233683f7c18b858829c83844171fa6adfd1/> add Fcs::for_testing constructor, restore cross-crate test-fixture construction
   Task 4's pub(crate) columns field broke every out-of-crate struct-literal
   construction of Fcs, since a pub(crate) field can't be named externally at
   all. Adds a public, feature-gated constructor and migrates every known
   broken call site (tru-ols, peacoqc-rs, gates, plus flow-fcs's own
   compress-feature tests) to use it instead.
 - <csr-id-a2aca5e30fd669ab239cba065e66ea0eda1308ed/> apply $PnR masking in events() bit-packed branch
 - <csr-id-6986541e936967c566b3c6caca42c9e0cbf5678f/> apply $PnR masking, fix bit-packed stride, add $NEXTDATA traversal
   Fixes four parsing gaps reported in jrmoynihan/flow#21:
   
   - $PnR masking (flow-crates-d35, P0): integer parameters now mask off
     unused high bits per their declared $PnR range before column
     extraction, fixing silently-wrong channel values on instruments
     (Beckman FC500/Gallios/Navios, older BD) that store sub-16-bit ADC
     resolution in wider fields.
   - Bit-packed $PnB stride (flow-crates-bk6, P2): calculate_bytes_per_event
     now sums raw bit widths before rounding once, instead of rounding each
     parameter first — correct for both byte-aligned and bit-packed layouts.
   - $NEXTDATA traversal (flow-crates-1mg, P2): new Fcs::open_all() walks
     the $NEXTDATA chain to read every dataset in a multi-dataset FCS file
     (all Beckman .lmd files use this). open() is unchanged and still
     returns only the first dataset, so existing callers are unaffected.
   - $DATATYPE A (flow-crates-ee0, P3, won't-fix): documented the existing
     Err behavior as a deliberate spec-driven decision (ASCII was
     deprecated due to cross-vendor bit-order disagreement) rather than an
     oversight, and added a test confirming it.
   
   Bumps flow-fcs 0.4.1 -> 0.5.0 and the paired version requirement in
   every workspace crate that depends on it via path (Cargo enforces that
   constraint even for path deps).
 - <csr-id-968561c2e8c75d77efe1bfbb3b9db4dbb74ba213/> converge TEXT and header data offsets when writing
   Iterate $BEGINDATA/$ENDDATA until header layout and TEXT keywords agree so
   digit-length changes cannot leave stale offsets in the written file.

### New Features

 - <csr-id-61704e6f5337a92da20c7a5ca3dbc26aa5e28c52/> add Fcs::events single-pass materialization, cache-free
 - <csr-id-254049e92dcb1662c9f5d0cc3e6abf12ef46decc/> add Fcs::column and Fcs::columns lazy cached accessors
 - <csr-id-07e2d9b2612ed225c7f17e2b55205729367e15f3/> add extract_columns row-major traversal primitive
 - <csr-id-f75d2a30a5921e6b50a6819a7d11b12c8fa2b798/> add ColumnLayout, precomputed per-parameter byte layout
 - <csr-id-c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926/> add shared KNN crate and unify Burn/cubeCL workspace deps
   Introduce flow-knn for portable graphs and ANN backends, pin Burn 0.21 with
   cubeCL 0.10 across GPU consumers, and path-patch pastey/bit-vec for sandbox builds.

### Documentation

 - <csr-id-a897c611a577b16aa12b99809cd5e49134256fd8/> changelog for unpublished 0.5.1 release notes
 - <csr-id-92e31b03dc632230809d10422be0c1062e6e9e1b/> consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate
   Rewrites READMEs across the workspace (fcs, flow-clustering,
   flow-control-detection, flow-density, flow-fcs-compress, flow-knn,
   flow-linalg, flow-pacmap, flow-peak-detection, gates, peacoqc-cli,
   peacoqc-rs, tru-ols, tru-ols-cli) to lead with install/quick-start/perf
   for downstream consumers, and adds a new flow-fcs-bench README.
   
   Adds a concrete usage example to peacoqc-py/README.md mirroring the
   docstring in peacoqc/__init__.py.
   
   Removes the superseded utils/ crate (clustering, KDE, and PCA helpers
   now live in their dedicated crates) and syncs beads issue/interaction
   export state.

### Changed

- Manifest version **0.5.1** (includes `test-util` availability for dependents).

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 23 commits contributed to the release over the course of 8 calendar days.
 - 22 days passed between releases.
 - 22 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Changelog for unpublished 0.5.1 release notes ([`a897c61`](https://github.com/jrmoynihan/flow/commit/a897c611a577b16aa12b99809cd5e49134256fd8))
    - Merge branch 'main' into worktree-lazy-fcs-column-loading-stage-a ([`52b5c50`](https://github.com/jrmoynihan/flow/commit/52b5c508956b9888bebe7a1279b47c26932afc7d))
    - Dedupe $PnR masking formula, fix ColumnLayout docs, strengthen cache-emptiness test ([`c32a0cf`](https://github.com/jrmoynihan/flow/commit/c32a0cf75877d51201ffca309d0373328d9f9f69))
    - Bound the OTHER offset scan at the first segment, not at TEXT ([`0257d38`](https://github.com/jrmoynihan/flow/commit/0257d38c13049921f84a0f9630f4d4327138f8c9))
    - Resolve offsets data-set-relative, fold OTHER into CRC range ([`f0b2922`](https://github.com/jrmoynihan/flow/commit/f0b29225fb01d5d2c8060e2b9fdf4b9b87b2dfa7))
    - Address final whole-branch review findings (bounds check, cache warning, feature scoping, version bump, benchmark docs) ([`a565fdf`](https://github.com/jrmoynihan/flow/commit/a565fdf4b372fe74eb6393eb61218a8ea159b6fe))
    - FCS 3.2 conformance — CRC, datetime, keywords, conformance rules ([`b6eb1c2`](https://github.com/jrmoynihan/flow/commit/b6eb1c2c1f7f3fda501406a830cede1e5cf3913e))
    - Compare lazy column/events access against the eager baseline ([`eec97b1`](https://github.com/jrmoynihan/flow/commit/eec97b1b3512332223985c0dadf268fd8d3a9eba))
    - Add Fcs::for_testing constructor, restore cross-crate test-fixture construction ([`6e3d723`](https://github.com/jrmoynihan/flow/commit/6e3d7233683f7c18b858829c83844171fa6adfd1))
    - Verify bit-packed fallback rejects lazy column() access ([`7ccf98f`](https://github.com/jrmoynihan/flow/commit/7ccf98f5c2c6fd282b1381d00e48b15d2bbe7788))
    - Dedupe columns() and have column() delegate to it ([`1d76e63`](https://github.com/jrmoynihan/flow/commit/1d76e633be505604bd3d36996b4cbd4b80679469))
    - Apply $PnR masking in events() bit-packed branch ([`a2aca5e`](https://github.com/jrmoynihan/flow/commit/a2aca5e30fd669ab239cba065e66ea0eda1308ed))
    - Add Fcs::events single-pass materialization, cache-free ([`61704e6`](https://github.com/jrmoynihan/flow/commit/61704e6f5337a92da20c7a5ca3dbc26aa5e28c52))
    - Add Fcs::column and Fcs::columns lazy cached accessors ([`254049e`](https://github.com/jrmoynihan/flow/commit/254049e92dcb1662c9f5d0cc3e6abf12ef46decc))
    - Add extract_columns row-major traversal primitive ([`07e2d9b`](https://github.com/jrmoynihan/flow/commit/07e2d9b2612ed225c7f17e2b55205729367e15f3))
    - Exercise non-uniform param widths in ColumnLayout offset test ([`da20fb2`](https://github.com/jrmoynihan/flow/commit/da20fb28d685f27f0349fd55f8c97d9e4b06a9b3))
    - Add ColumnLayout, precomputed per-parameter byte layout ([`f75d2a3`](https://github.com/jrmoynihan/flow/commit/f75d2a30a5921e6b50a6819a7d11b12c8fa2b798))
    - Widen visibility of parse helpers to pub(crate) ([`cdc0f8b`](https://github.com/jrmoynihan/flow/commit/cdc0f8b085b5d250037fd003050869de968ec797))
    - Apply $PnR masking, fix bit-packed stride, add $NEXTDATA traversal ([`6986541`](https://github.com/jrmoynihan/flow/commit/6986541e936967c566b3c6caca42c9e0cbf5678f))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Bulk-load KnnGraph IO; record unsafe micro-opt A/B ([`2d3c6fc`](https://github.com/jrmoynihan/flow/commit/2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37))
    - Converge TEXT and header data offsets when writing ([`968561c`](https://github.com/jrmoynihan/flow/commit/968561c2e8c75d77efe1bfbb3b9db4dbb74ba213))
    - Add shared KNN crate and unify Burn/cubeCL workspace deps ([`c9223d6`](https://github.com/jrmoynihan/flow/commit/c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926))
</details>

## 0.4.1 (2026-07-19)

### New Features

 - <csr-id-cf0df0a44cf8ea82aab571f4bfe3684d99aaf213/> specta derives, matrix-context gate fields, Polars 0.54
   Restore the pre-peacoqc WIP so path consumers (fast-flow) get optional
   `specta` features, Embedding parameter category, gate spillover/data-context
   ids, and a Polars 0.54 workspace pin compatible with chrono 0.4.42.

### Changed

- Depend on workspace `polars` `0.54.4` so crates.io consumers (notably `peacoqc-rs` + `plotters`) resolve without the Polars 0.53 / `chrono<=0.4.41` conflict.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release over the course of 69 calendar days.
 - 69 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.4.1 ([`597f21b`](https://github.com/jrmoynihan/flow/commit/597f21bef7ea787437071685fc3cce9d2269270f))
    - Specta derives, matrix-context gate fields, Polars 0.54 ([`cf0df0a`](https://github.com/jrmoynihan/flow/commit/cf0df0a44cf8ea82aab571f4bfe3684d99aaf213))
    - Release flow-fcs-compress v0.1.2 ([`0eb992c`](https://github.com/jrmoynihan/flow/commit/0eb992c3d8e97e305a0a957d0a8bbbecb6e56467))
    - Release flow-linalg v0.1.1, flow-density v0.1.1, flow-clustering v0.1.1, flow-fcs-compress v0.1.1 ([`966d22a`](https://github.com/jrmoynihan/flow/commit/966d22ae4fbdd6114dc3862d45648fce7ebf53cc))
    - Merge branch 'feat/flow-fcs-compress' ([`ef239b2`](https://github.com/jrmoynihan/flow/commit/ef239b24dbacfabc1e68dfa5f4dc8baa49f9704a))
    - Merge pull request #20 from jrmoynihan/feat/flow-fcs-compress ([`f953bc5`](https://github.com/jrmoynihan/flow/commit/f953bc5df8f6978e3fe511538cb2943730a35eff))
</details>

## 0.4.0 (2026-05-11)

<csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/>
<csr-id-dd4dcbc9dd999b59155db42b0ad0db52712231bd/>
<csr-id-006ba79325f7ea81d54af94224e81d3862cdbdb2/>

### Chore

 - <csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/> bump flow-fcs to 0.4.0, add publish metadata to new crates
   - flow-fcs 0.3.0 → 0.4.0 (new compensation feature + public API)
   - flow-linalg, flow-density, flow-clustering: add repository field
     and smart-release scripts for first publish
   - Update all workspace consumers to ^0.4.0

### New Features

 - <csr-id-a58674a3f1d42ca3a81b273602f8706aa01f9900/> add get_compensated_parameters_with_matrix backed by flow-linalg
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

### Bug Fixes

 - <csr-id-bc1223f9f76fbf073d531945991b93f613fe84cc/> pass-through non-matrix channels, hard-error on missing matrix channel
 - <csr-id-f6172992ac40dc8acbdceedabf9c894aaa63a69c/> gate arcsinh inverse debug logging behind debug_assertions

### Refactor

 - <csr-id-dd4dcbc9dd999b59155db42b0ad0db52712231bd/> remove debug eprintln from arcsinh inverse_transform
 - <csr-id-006ba79325f7ea81d54af94224e81d3862cdbdb2/> improve parameter mapping and metadata for unmixing integration
   Refines parameter mapping for unmixed FCS reconstruction, updates
   matrix/metadata handling, and adjusts benchmarks and write path.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release over the course of 53 calendar days.
 - 7 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-linalg v0.1.0, flow-density v0.1.0, flow-clustering v0.1.0, flow-fcs-compress v0.1.0, flow-fcs v0.4.0 ([`e8c908e`](https://github.com/jrmoynihan/flow/commit/e8c908ef92fb68b8e2d01d3c1e8d6a294c8c6bda))
    - Remove debug eprintln from arcsinh inverse_transform ([`dd4dcbc`](https://github.com/jrmoynihan/flow/commit/dd4dcbc9dd999b59155db42b0ad0db52712231bd))
    - Bump flow-fcs to 0.4.0, add publish metadata to new crates ([`74956f9`](https://github.com/jrmoynihan/flow/commit/74956f94c544d1fa83f6fffbb18e2d4f5e6072ff))
    - Pass-through non-matrix channels, hard-error on missing matrix channel ([`bc1223f`](https://github.com/jrmoynihan/flow/commit/bc1223f9f76fbf073d531945991b93f613fe84cc))
    - Add get_compensated_parameters_with_matrix backed by flow-linalg ([`a58674a`](https://github.com/jrmoynihan/flow/commit/a58674a3f1d42ca3a81b273602f8706aa01f9900))
    - Add compression crate, benchmarks, and ISAC proposal ([`a4a5e18`](https://github.com/jrmoynihan/flow/commit/a4a5e18e06b55de252b74110118ac72aa2fc0891))
    - Improve parameter mapping and metadata for unmixing integration ([`006ba79`](https://github.com/jrmoynihan/flow/commit/006ba79325f7ea81d54af94224e81d3862cdbdb2))
    - Gate arcsinh inverse debug logging behind debug_assertions ([`f617299`](https://github.com/jrmoynihan/flow/commit/f6172992ac40dc8acbdceedabf9c894aaa63a69c))
</details>

## 0.2.2 (2026-02-26)

<csr-id-6d8f95797fdd97e7fa1ffa34050cf3fcccb7a1f0/>
<csr-id-ec0fcf8823f4d35e47d7da935f1e70d1927f0f0c/>

### New Features

<csr-id-37f1bccf0cabd2a0a6360afaf064827fbbafb972/>

 - <csr-id-d3498c26ef3b77015f87fbe9fbc154d94e7fec41/> enhance hashing for TransformType and add consistency tests
   - Updated the Hash implementation for TransformType to include additional fields for Arcsinh and Biexponential variants.

### Refactor

 - <csr-id-6d8f95797fdd97e7fa1ffa34050cf3fcccb7a1f0/> remove deprecated attributes from MixedKeyword and StringKeyword enums
   - Eliminated deprecated attributes from the MixedKeyword and StringKeyword enums to clean up the codebase.
   - This change enhances code clarity and prepares for future updates by removing outdated references.

### Chore

 - <csr-id-ec0fcf8823f4d35e47d7da935f1e70d1927f0f0c/> update changelogs and READMEs for flow-fcs, flow-plots, peacoqc-rs patch release

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 9 commits contributed to the release over the course of 11 calendar days.
 - 11 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`d8a6922`](https://github.com/jrmoynihan/flow/commit/d8a6922a47b2196a6dcf8362bab067b176757908))
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`cb7b98e`](https://github.com/jrmoynihan/flow/commit/cb7b98ecbc3d012df79c2e70bd2aad2f89d9c303))
    - Update changelogs and READMEs for flow-fcs, flow-plots, peacoqc-rs patch release ([`ec0fcf8`](https://github.com/jrmoynihan/flow/commit/ec0fcf8823f4d35e47d7da935f1e70d1927f0f0c))
    - Remove deprecated attributes from MixedKeyword and StringKeyword enums ([`6d8f957`](https://github.com/jrmoynihan/flow/commit/6d8f95797fdd97e7fa1ffa34050cf3fcccb7a1f0))
    - Enhance hashing for TransformType and add consistency tests ([`d3498c2`](https://github.com/jrmoynihan/flow/commit/d3498c26ef3b77015f87fbe9fbc154d94e7fec41))
    - Merge PR #15: add PartialEq to Gate, GateNode, GateGeometry and TransformType ([`98455bc`](https://github.com/jrmoynihan/flow/commit/98455bc69a3789f5c8eb9741a3cc024451e63a3e))
    - Add spillover channel name resolution for compensation ([`37f1bcc`](https://github.com/jrmoynihan/flow/commit/37f1bccf0cabd2a0a6360afaf064827fbbafb972))
    - Add partialeq ([`e2ac3ec`](https://github.com/jrmoynihan/flow/commit/e2ac3ecf031a6a265c482a08f33ebed5c1f35bdd))
    - Merge pull request #14 from jrmoynihan/gpu-acceleration ([`01edbec`](https://github.com/jrmoynihan/flow/commit/01edbecfc222685a8e052eb26b001d3fae4dfe13))
</details>

## 0.2.1 (2026-02-15)

<csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/>
<csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/>
<csr-id-b8128fcd93659ca86ee2f1d8dc43eed25616c9f1/>
<csr-id-c4e6b792b31e293d865ebba6a3c58c5e8dde9bd8/>
<csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/>

### Chore

 - <csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/> update dependencies and align workspace configurations
   - Updated various dependencies in Cargo.toml files across multiple crates to their latest versions for improved functionality and compatibility.
   - Changed several dependencies to use workspace references for consistency and to reduce duplication.
   - Notable updates include polars to version 0.53.0, faer to version 0.24, and ndarray-linalg to version 0.18.1.
   - Adjusted dev-dependencies to utilize workspace settings for better management.
 - <csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/> clean up unused imports and variables
   - Remove unused imports in clustering and gating modules
   - Fix unreachable code warning in DBSCAN
   - Remove unused mut keywords
   - Clean up warnings for better code quality

### Chore

 - <csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/> update changelogs prior to release

### New Features

 - <csr-id-f494ea2ab401071aab661a0ce691cef547ebde75/> improve metadata, file handling, and FCS write
   - Extend metadata and keyword parsing

### Bug Fixes

 - <csr-id-43fc966c577feccbd92c9d95fefc101add697d97/> categorize $PnDATATYPE as ByteKeyword per FCS 3.2 spec
   According to FCS 3.2 specification, $PnDATATYPE uses the same character format
   as $DATATYPE ("F", "D", "I", "A"), not numeric codes. Move PnDATATYPE
   from IntegerKeyword to ByteKeyword enum to match the specification.
 - <csr-id-c860514ecd21eaa9a724a5d9baec4977b90b0d46/> parse $PnR keyword with float values
   Some cytometers output float values for the $PnR (parameter range) keyword
   instead of integers (e.g., "1.1" instead of "1"). Update parsing to handle
   both float and integer formats by attempting float parsing first, then falling
   back to integer parsing.
   
   Adds tests for float parsing of $PnR with both small and large parameter
   numbers (P5R, P61R).

### Refactor

 - <csr-id-b8128fcd93659ca86ee2f1d8dc43eed25616c9f1/> update DataFrame creation and series replacement to align with polars 0.51 -> 0.53 breaking change
   - Modified DataFrame creation to include the number of events as a parameter for better clarity.
   - Updated series replacement calls to ensure proper type conversion with `.into()` for consistency across the codebase.
   - Adjusted test cases to reflect changes in DataFrame initialization.
 - <csr-id-c4e6b792b31e293d865ebba6a3c58c5e8dde9bd8/> replace ndarray with faer for matrix operations
   - Use faer Mat/MatRef for invert_matrix, batch_matvec, compensation
   - get_spillover_matrix returns Option<(Mat<f32>, Vec<String>)>
   - apply_compensation, apply_spectral_unmixing take MatRef<f32>
   - Add optional blas feature for ndarray-linalg
   - Update tests and benchmarks to faer mat! macro
   - Update README and doc examples to faer

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 11 commits contributed to the release over the course of 24 calendar days.
 - 24 days passed between releases.
 - 8 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`b758024`](https://github.com/jrmoynihan/flow/commit/b7580243ad5dfba389d80f55d9d2b0a0adf26348))
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`1e3ae1e`](https://github.com/jrmoynihan/flow/commit/1e3ae1e2a91b53f70120cb96987ba5a8f02dc21e))
    - Update changelogs prior to release ([`089feff`](https://github.com/jrmoynihan/flow/commit/089feff624625a5ddf0b1da570e4f60b6fedf09b))
    - Update DataFrame creation and series replacement to align with polars 0.51 -> 0.53 breaking change ([`b8128fc`](https://github.com/jrmoynihan/flow/commit/b8128fcd93659ca86ee2f1d8dc43eed25616c9f1))
    - Update dependencies and align workspace configurations ([`46bee42`](https://github.com/jrmoynihan/flow/commit/46bee42d4f28d185b38446c0d950c2579c422f43))
    - Replace ndarray with faer for matrix operations ([`c4e6b79`](https://github.com/jrmoynihan/flow/commit/c4e6b792b31e293d865ebba6a3c58c5e8dde9bd8))
    - Improve metadata, file handling, and FCS write ([`f494ea2`](https://github.com/jrmoynihan/flow/commit/f494ea2ab401071aab661a0ce691cef547ebde75))
    - Clean up unused imports and variables ([`c987a22`](https://github.com/jrmoynihan/flow/commit/c987a225570c2afae480800327d0072ab4b4e4ad))
    - Categorize $PnDATATYPE as ByteKeyword per FCS 3.2 spec ([`43fc966`](https://github.com/jrmoynihan/flow/commit/43fc966c577feccbd92c9d95fefc101add697d97))
    - Parse $PnR keyword with float values ([`c860514`](https://github.com/jrmoynihan/flow/commit/c860514ecd21eaa9a724a5d9baec4977b90b0d46))
    - Merge pull request #10 from jrmoynihan/gpu-acceleration ([`69363eb`](https://github.com/jrmoynihan/flow/commit/69363eb3a664b1aa6cd0be9b980ec08fc03b7955))
</details>

## 0.2.0 (2026-01-21)

<csr-id-fec1c6d2c50730d98771b7cdc101bad5071baf29/>
<csr-id-2d2660406806bdb259dbf66fefa3576fa1a611f3/>

### Refactor (BREAKING)

 - <csr-id-fec1c6d2c50730d98771b7cdc101bad5071baf29/> remove GPU acceleration implementation
   - Remove GPU module and all GPU-related code
   - Remove GPU dependencies (burn, cubecl, bytemuck)
   - Remove GPU feature flags from Cargo.toml
   - Update batch functions to use CPU-only implementation

### Refactor

 - <csr-id-2d2660406806bdb259dbf66fefa3576fa1a611f3/> remove GPU acceleration implementation
   - Remove GPU module and all GPU-related code
   - Remove GPU dependencies (burn, cubecl, cubecl-wgpu)
   - Remove GPU feature flags from Cargo.toml
   - Reorganize matrix operations into dedicated matrix module
   - Update benchmarks to use CPU-only MatrixOps API
   - Add GPU_BENCHMARKING.md documenting benchmark results
   
   Benchmarks showed CPU implementations are 1.2-21× faster for typical
   flow cytometry workloads due to GPU transfer overhead and kernel launch
   costs. See GPU_BENCHMARKING.md for detailed analysis.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 9 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.0, flow-plots v0.2.0 ([`3620154`](https://github.com/jrmoynihan/flow/commit/3620154c694500bb2ff2edbdf0848076287d77d3))
    - Release flow-fcs v0.2.0 ([`f2fc722`](https://github.com/jrmoynihan/flow/commit/f2fc72250da69b63cacdea28f561db60732c0a39))
    - Release flow-fcs v0.2.0, safety bump 4 crates ([`cd26a89`](https://github.com/jrmoynihan/flow/commit/cd26a8970fc25dbe70c1cc9ac342b367613bcda6))
    - Remove GPU acceleration implementation ([`2d26604`](https://github.com/jrmoynihan/flow/commit/2d2660406806bdb259dbf66fefa3576fa1a611f3))
    - Remove GPU acceleration implementation ([`fec1c6d`](https://github.com/jrmoynihan/flow/commit/fec1c6d2c50730d98771b7cdc101bad5071baf29))
    - Release flow-fcs v0.1.6 ([`bd1ebad`](https://github.com/jrmoynihan/flow/commit/bd1ebad7b940f9c46f3e54202730b1f117a1d70b))
    - Release flow-fcs v0.1.6 ([`3343b32`](https://github.com/jrmoynihan/flow/commit/3343b32dbfeda6e2f0e1efa05c1b903bf457d5be))
    - Adjusting changelogs prior to release of flow-fcs v0.1.6 ([`37f1d61`](https://github.com/jrmoynihan/flow/commit/37f1d61dcb790b63c2ef0ea148b4fde57a6414b2))
    - Adjusting changelogs prior to release of flow-fcs v0.1.6 ([`7fb88db`](https://github.com/jrmoynihan/flow/commit/7fb88db9ede05b317a03d367cea18a3b8b73c5a1))
</details>

## 0.1.6 (2026-01-21)

### Removed

 - Remove GPU acceleration implementations
   - Removed GPU matrix operations module (`gpu/`) after benchmarking showed CPU implementations are 1.2-21× faster for typical flow cytometry workloads
   - GPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)
   - CPU BLAS/LAPACK implementations are highly optimized for these matrix sizes
   - See `GPU_BENCHMARKING.md` for detailed benchmark results and analysis
- GPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)
- CPU BLAS/LAPACK implementations are highly optimized for these matrix sizes
- See `GPU_BENCHMARKING.md` for detailed benchmark results and analysis

### Refactor

 - Reorganize matrix operations into dedicated `matrix` module
   - Moved CPU matrix operations from `gpu/fallback` to new `matrix` module
   - Simplified codebase by removing GPU dependencies (`burn`, `cubecl`)
   - Updated benchmarks to use new `MatrixOps` API

<csr-unknown>
GPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysis<csr-unknown/>
<csr-unknown/>

## 0.1.5 (2026-01-21)

### New Features

 - <csr-id-da12f8bdda2def063a9469ff921250a1d8a91aef/> expand parameter exports in lib.rs
   - Added EventDataFrame, EventDatum, and LabelName to the exported parameters in lib.rs for enhanced functionality.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release over the course of 1 calendar day.
 - 3 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.5, flow-gates v0.1.2 ([`4106abc`](https://github.com/jrmoynihan/flow/commit/4106abc5ae2d35328ec470daf9b0a9a549ebd6ba))
    - Expand parameter exports in lib.rs ([`da12f8b`](https://github.com/jrmoynihan/flow/commit/da12f8bdda2def063a9469ff921250a1d8a91aef))
</details>

## 0.1.4 (2026-01-18)

<csr-id-6da76b758d02b9da1abcd3052323f81992dc3fdd/>

### Refactor

 - <csr-id-6da76b758d02b9da1abcd3052323f81992dc3fdd/> clean up unused imports and improve code readability
   - Removed unused imports from write.rs and peaks.rs.
   - Updated the loop in isolation_tree.rs to ignore unused variables for clarity.
   - Standardized string conversion in plots.rs for consistency.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.4, peacoqc-rs v0.1.2 ([`140a59a`](https://github.com/jrmoynihan/flow/commit/140a59af3c1ca751672e66c9cc69708f45ac8453))
    - Clean up unused imports and improve code readability ([`6da76b7`](https://github.com/jrmoynihan/flow/commit/6da76b758d02b9da1abcd3052323f81992dc3fdd))
</details>

## 0.1.3 (2026-01-18)

<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>
<csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/>
<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-339d07ac60343b172cd5962310abbc7899fdc770/>

### Chore

 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
 - <csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/> update Cargo.toml files for consistency and improvements
   - Standardize formatting in Cargo.toml files across multiple crates
   - Update repository URLs to reflect new structure
   - Enhance keywords and categories for better discoverability
   - Ensure consistent dependency declarations and script commands

### Chore

 - <csr-id-339d07ac60343b172cd5962310abbc7899fdc770/> update categories in Cargo.toml files
   - Simplify categories in fcs and plots to remove redundant entries.
   - Change peacoqc-cli category to reflect its command-line utility nature.
   - Add algorithms category to peacoqc-rs for better classification.

### Refactor

 - <csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/> improve code quality and add features
   - Improve QC algorithm implementations
   - Add plot generation functionality
   - Enhance error handling
   - Update dependencies
   - Improve code organization

### Chore

 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.

### New Features

 - <csr-id-31bd355c1457beae0a9852adfc9dd1bdab7a3cf4/> add FCS file writing and modification utilities
   Add comprehensive FCS file writing capabilities to the previously read-only `flow-fcs` crate.
   
   New functions:
   - `write_fcs_file`: Write Fcs struct to disk

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 11 commits contributed to the release over the course of 3 calendar days.
 - 4 days passed between releases.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`f08823c`](https://github.com/jrmoynihan/flow/commit/f08823cabcae5223efe4250471dd75ea7fcaa936))
    - Update categories in Cargo.toml files ([`339d07a`](https://github.com/jrmoynihan/flow/commit/339d07ac60343b172cd5962310abbc7899fdc770))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`607fcae`](https://github.com/jrmoynihan/flow/commit/607fcae78304d51ce8d156e82e5dba48a1b6dbfa))
    - Update Cargo.toml scripts and dependency versions ([`d3aa6cd`](https://github.com/jrmoynihan/flow/commit/d3aa6cdc5a806703131a3ffac63506142f052da9))
    - Release flow-fcs v0.1.3 ([`e79b57f`](https://github.com/jrmoynihan/flow/commit/e79b57f8fd7613fbdcc682863fef44178f14bed8))
    - Update publish command in Cargo.toml files to include --update-crates-index ([`8d232b2`](https://github.com/jrmoynihan/flow/commit/8d232b2838f65aa621a81031183d4c954d787543))
    - Merge pull request #8 from jrmoynihan/peacoqc-rs ([`fbeaab2`](https://github.com/jrmoynihan/flow/commit/fbeaab262dc1a72832dba3d6c4708bf95c941929))
    - Merge branch 'main' into peacoqc-rs ([`c52af3c`](https://github.com/jrmoynihan/flow/commit/c52af3c09ae547a7e1ce2c62e9999590314e8f97))
    - Improve code quality and add features ([`5bd48e4`](https://github.com/jrmoynihan/flow/commit/5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7))
    - Add FCS file writing and modification utilities ([`31bd355`](https://github.com/jrmoynihan/flow/commit/31bd355c1457beae0a9852adfc9dd1bdab7a3cf4))
    - Update Cargo.toml files for consistency and improvements ([`4649c7a`](https://github.com/jrmoynihan/flow/commit/4649c7af16150d05880ddab4e732e9dee374d01b))
</details>

## 0.1.2 (2026-01-13)

<csr-id-9c44f94e6b8e0236a47361a7dc7156b90d25f37c/>
<csr-id-f64872e441add42bc9d19280d4411df628ff853e/>
<csr-id-661e8e00088c6bee38bc02a8a2830f284cd49ac4/>
<csr-id-2fc9efdd0a9bfeadd0613dd309d811067acc709f/>
<csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/>
<csr-id-037f74e0e364ebfc8d68cf672dca0f758a3f2952/>
<csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/>

### Chore

 - <csr-id-9c44f94e6b8e0236a47361a7dc7156b90d25f37c/> bump version number in Cargo.toml for flow-fcs

### Chore

 - <csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.
   - Added comprehensive documentation and R helper functions for improved usability.

### Chore

 - <csr-id-037f74e0e364ebfc8d68cf672dca0f758a3f2952/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.

### Chore

 - <csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.

### Chore

 - <csr-id-2fc9efdd0a9bfeadd0613dd309d811067acc709f/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, new features, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Added new FCS specification PDF and example QC plot to documentation.
   - Refactored folder names and updated test module imports for better organization and error handling.

### Documentation

 - <csr-id-42a6b5d7214e1ecc6fbad2c74572f9974c4f6a9f/> add FCS specification PDF and example QC plot
   - Add FCS 3.1 implementation guidance PDF for reference

### New Features

<csr-id-590dfaa8e0c551591ea3b2ff98f893df34f6251c/>
<csr-id-c92c76434e9a2bf957040821c246eaef261e80f8/>

 - <csr-id-4d234b204ade5acd6f1cf1f87c36c5e709fd2d4a/> improve FCS file parsing, keyword handling, and transforms
   - Enhance file parsing with better error handling

### Refactor

 - <csr-id-f64872e441add42bc9d19280d4411df628ff853e/> :truck: Rnamed folders without the `flow-` prefix.
   Just shorter to type paths.  We'll keep the crates named with the `flow-` prefix when we publish.
 - <csr-id-661e8e00088c6bee38bc02a8a2830f284cd49ac4/> update test module imports and function signatures
   - Refactored import paths in the polars_tests module to streamline access to parameters and keywords.
   - Updated the create_test_fcs function signature to return a Result with a boxed error type for better error handling.
   - Consolidated related imports.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 20 commits contributed to the release over the course of 5 calendar days.
 - 5 days passed between releases.
 - 11 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`3292c46`](https://github.com/jrmoynihan/flow/commit/3292c46b282d226aa48c2a83bc17c50896bb8341))
    - Update CHANGELOG for upcoming release ([`037f74e`](https://github.com/jrmoynihan/flow/commit/037f74e0e364ebfc8d68cf672dca0f758a3f2952))
    - Update CHANGELOG for upcoming release ([`621d3ad`](https://github.com/jrmoynihan/flow/commit/621d3aded59ff51f953c6acdb75027c4541a8b97))
    - Update CHANGELOG for upcoming release ([`2fc9efd`](https://github.com/jrmoynihan/flow/commit/2fc9efdd0a9bfeadd0613dd309d811067acc709f))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Merge pull request #5 from jrmoynihan/peacoqc-rs ([`198f659`](https://github.com/jrmoynihan/flow/commit/198f659aed1a8ad7a362ebcfc615e1983c6a4ade))
    - Add FCS specification PDF and example QC plot ([`42a6b5d`](https://github.com/jrmoynihan/flow/commit/42a6b5d7214e1ecc6fbad2c74572f9974c4f6a9f))
    - Improve FCS file parsing, keyword handling, and transforms ([`4d234b2`](https://github.com/jrmoynihan/flow/commit/4d234b204ade5acd6f1cf1f87c36c5e709fd2d4a))
    - Merge branch 'flow-gates' into main ([`c2f2d13`](https://github.com/jrmoynihan/flow/commit/c2f2d13a61854f93687cdfd2f6a1b4b12e0d9810))
    - :truck: Rnamed folders without the `flow-` prefix. ([`f64872e`](https://github.com/jrmoynihan/flow/commit/f64872e441add42bc9d19280d4411df628ff853e))
    - Update test module imports and function signatures ([`661e8e0`](https://github.com/jrmoynihan/flow/commit/661e8e00088c6bee38bc02a8a2830f284cd49ac4))
    - Enhance benchmarking and data parsing capabilities ([`590dfaa`](https://github.com/jrmoynihan/flow/commit/590dfaa8e0c551591ea3b2ff98f893df34f6251c))
    - Enhance FCS data handling and metadata processing ([`c92c764`](https://github.com/jrmoynihan/flow/commit/c92c76434e9a2bf957040821c246eaef261e80f8))
    - Merge branch 'main' into flow-plots ([`5977fb3`](https://github.com/jrmoynihan/flow/commit/5977fb309ee7e726e5e7cefca902278f155b79f8))
    - Merge branch 'main' into flow-plots ([`d7b6226`](https://github.com/jrmoynihan/flow/commit/d7b62269232f1bc6a8b155fd44d905e0a6233887))
    - Bump version number in Cargo.toml for flow-fcs ([`9c44f94`](https://github.com/jrmoynihan/flow/commit/9c44f94e6b8e0236a47361a7dc7156b90d25f37c))
    - Merge pull request #2 from jrmoynihan:flow-fcs ([`987314d`](https://github.com/jrmoynihan/flow/commit/987314dd1120fb723aad0946d8bfb0e882d39454))
    - Merge pull request #2 from jrmoynihan:flow-fcs ([`46431c0`](https://github.com/jrmoynihan/flow/commit/46431c0431afb4b7fa7de240595ac5726e693242))
    - Release flow-fcs v0.1.1 ([`c3413e1`](https://github.com/jrmoynihan/flow/commit/c3413e1a46a64f0a798ea0fe4d08134117a8c1ca))
</details>

## 0.1.1 (2026-01-08)

<csr-id-3691bf612ae11ac243fdcc6e3af927d2d3b3780a/>

### Refactor

 - <csr-id-3691bf612ae11ac243fdcc6e3af927d2d3b3780a/> export Transformable and Formattable traits

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.1 ([`e0e16cc`](https://github.com/jrmoynihan/flow/commit/e0e16ccaa87b5f5d8413a3eb6198257e2d052ac8))
    - Merge pull request #1 from jrmoynihan:flow-plots ([`3d994a8`](https://github.com/jrmoynihan/flow/commit/3d994a81aa585e6d5263c5f9d1db7d36106698d2))
    - Merge pull request #1 from jrmoynihan:flow-plots ([`708ddca`](https://github.com/jrmoynihan/flow/commit/708ddca0149fe7f5c6627e052207d78f06b55ed6))
    - Export Transformable and Formattable traits ([`3691bf6`](https://github.com/jrmoynihan/flow/commit/3691bf612ae11ac243fdcc6e3af927d2d3b3780a))
</details>

## 0.1.0 (2026-01-07)

<csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/>
<csr-id-d0455271e8573fa035dab1cf9af4448b5e67373b/>
<csr-id-ae41dccd0a40e182ad251439e6191bf6f2db0aa2/>
<csr-id-ea0456e94b12e17eaea070b942e52287423e88e0/>
<csr-id-4d8fc2267ad20d7fc1ddbdea5e69549b978c1eda/>
<csr-id-eb923b039da61abb83b35f527c096aecbf84739e/>
<csr-id-9c184b0cce3e4d8a662b02ac544ea3659cde68f3/>
<csr-id-48e26f4253ec16f5d49ffbbf1b7bb34c595e2c17/>
<csr-id-7b5c00622d44ad9bd5791c7fe2f6e4aaaa57b277/>
<csr-id-ea242306bd6e5c9211c58fb15971c8277ad7abdd/>
<csr-id-9a522b748fbf62fbb2d3638dd0627c40f400acaa/>
<csr-id-d194503be414fe7b7214f65d0f6c06010a884e69/>

### Chore

 - <csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/> reorganize workspace into separate crates

### Chore

 - <csr-id-d194503be414fe7b7214f65d0f6c06010a884e69/> change category tag for crates.io

### Refactor

 - <csr-id-ae41dccd0a40e182ad251439e6191bf6f2db0aa2/> update deprecated keyword documentation and parsing
   - Added `#[allow(deprecated)]` attributes to suppress warnings for deprecated keywords in `keyword/mod.rs` and `parsing.rs`.
   - Enhanced documentation for deprecated keywords to improve clarity and maintainability.
   - Ensured consistent handling of deprecated keywords in the parsing functions.
 - <csr-id-ea0456e94b12e17eaea070b942e52287423e88e0/> remove unused match arm in MixedKeyword implementation
   - Eliminated the unused match arm in the StringableKeyword implementation for MixedKeyword to enhance code clarity and maintainability.
 - <csr-id-4d8fc2267ad20d7fc1ddbdea5e69549b978c1eda/> clean up imports and remove unused code in flow-fcs
   - Removed unused imports from file.rs, header.rs, and keyword/mod.rs to enhance code clarity and maintainability.
   - Consolidated import statements for better organization and readability.
   - Added `#[allow(deprecated)]` attributes to certain enum implementations in keyword/mod.rs to suppress warnings for deprecated features.
 - <csr-id-eb923b039da61abb83b35f527c096aecbf84739e/> remove ColumnStore struct and related methods from file.rs
   - Deleted the ColumnStore struct and its associated methods, which were previously used for managing column-oriented data storage for FCS files.
   - This change simplifies the codebase by removing unused functionality, streamlining the file handling process.
 - <csr-id-9c184b0cce3e4d8a662b02ac544ea3659cde68f3/> add unused attribute to traits and functions for clarity
   - Added `#[allow(unused)]` attribute to the `validate_number_of_parameters` function in `metadata.rs` to suppress warnings for unused code.
   - Introduced `#[allow(unused)]` to the `Transformable` and `Formattable` traits in `transform.rs` to indicate potential future use.
   - Added `#[allow(unused)]` to the `FloatableKeyword` trait in `keyword/mod.rs` to clarify its intended future implementation.
 - <csr-id-48e26f4253ec16f5d49ffbbf1b7bb34c595e2c17/> rename tests to reflect suffix extraction changes
   - Updated test function names to align with the new `extract_parameter_suffix` function.
   - Simplified tests by removing unnecessary assertions related to parameter numbers.
   - Ensured consistency in testing invalid inputs for suffix extraction.
 - <csr-id-7b5c00622d44ad9bd5791c7fe2f6e4aaaa57b277/> simplify parameter keyword handling in flow-fcs
   - Renamed `extract_parameter_parts` to `extract_parameter_suffix` to focus on suffix extraction.
   - Consolidated logic for checking parameter keywords into a single function using known prefixes.
   - Updated documentation to reflect changes in parameter keyword handling and improved clarity.
   - Enhanced error handling in parsing functions to return `UnableToParse` for invalid inputs.
 - <csr-id-ea242306bd6e5c9211c58fb15971c8277ad7abdd/> remove unnecessary cloning of channel and label names in FCS builder

### Chore

 - <csr-id-9a522b748fbf62fbb2d3638dd0627c40f400acaa/> update dependencies to use memmap3 and add lazy_static
   - Replaced `memmap2` with `memmap3` in Cargo.toml and flow-fcs/Cargo.toml for improved safety.
   - Added `lazy_static` as a dependency in Cargo.lock.
   - Updated file.rs to utilize `memmap3` with enhanced safety guarantees.

### Documentation

 - <csr-id-3014b0af9cac746cf8728a33d4bf7fd0a1124ec0/> added root readme ad updated flow-fcs readme
 - <csr-id-e63e03c98834a3280be7d2f3f32fb4fe93272d53/> :memo: Added a changelog
   Used cargo smart-release to generate a changelog
 - <csr-id-8c420b9f03ce918f7c7e710f622073c66ed0bc64/> :memo: Update changelog

### Chore

 - <csr-id-d0455271e8573fa035dab1cf9af4448b5e67373b/> add script metadata for automated release and changelog generation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 18 commits contributed to the release.
 - 15 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.0 ([`18ab133`](https://github.com/jrmoynihan/flow/commit/18ab1338cacc10f8856409097bca33ce1914f248))
    - Change category tag for crates.io ([`d194503`](https://github.com/jrmoynihan/flow/commit/d194503be414fe7b7214f65d0f6c06010a884e69))
    - :memo: Update changelog ([`8c420b9`](https://github.com/jrmoynihan/flow/commit/8c420b9f03ce918f7c7e710f622073c66ed0bc64))
    - Update deprecated keyword documentation and parsing ([`ae41dcc`](https://github.com/jrmoynihan/flow/commit/ae41dccd0a40e182ad251439e6191bf6f2db0aa2))
    - Remove unused match arm in MixedKeyword implementation ([`ea0456e`](https://github.com/jrmoynihan/flow/commit/ea0456e94b12e17eaea070b942e52287423e88e0))
    - Clean up imports and remove unused code in flow-fcs ([`4d8fc22`](https://github.com/jrmoynihan/flow/commit/4d8fc2267ad20d7fc1ddbdea5e69549b978c1eda))
    - Remove ColumnStore struct and related methods from file.rs ([`eb923b0`](https://github.com/jrmoynihan/flow/commit/eb923b039da61abb83b35f527c096aecbf84739e))
    - Update dependencies to use memmap3 and add lazy_static ([`9a522b7`](https://github.com/jrmoynihan/flow/commit/9a522b748fbf62fbb2d3638dd0627c40f400acaa))
    - Add unused attribute to traits and functions for clarity ([`9c184b0`](https://github.com/jrmoynihan/flow/commit/9c184b0cce3e4d8a662b02ac544ea3659cde68f3))
    - Rename tests to reflect suffix extraction changes ([`48e26f4`](https://github.com/jrmoynihan/flow/commit/48e26f4253ec16f5d49ffbbf1b7bb34c595e2c17))
    - Simplify parameter keyword handling in flow-fcs ([`7b5c006`](https://github.com/jrmoynihan/flow/commit/7b5c00622d44ad9bd5791c7fe2f6e4aaaa57b277))
    - Remove unnecessary cloning of channel and label names in FCS builder ([`ea24230`](https://github.com/jrmoynihan/flow/commit/ea242306bd6e5c9211c58fb15971c8277ad7abdd))
    - Reduce keywords to satisfy crates.io ([`343ec47`](https://github.com/jrmoynihan/flow/commit/343ec47bd3bc81aa0c35e068db8af7d71d9bf71b))
    - Update CHANGELOG.md to reflect recent changes, including added documentation for root and flow-fcs readme, automated release script metadata, and a generated changelog. Consolidated commit statistics to show contributions from multiple commits. ([`1879470`](https://github.com/jrmoynihan/flow/commit/1879470acab8a43fcdde844938a6bb67688a4666))
    - Add script metadata for automated release and changelog generation ([`d045527`](https://github.com/jrmoynihan/flow/commit/d0455271e8573fa035dab1cf9af4448b5e67373b))
    - Added root readme ad updated flow-fcs readme ([`3014b0a`](https://github.com/jrmoynihan/flow/commit/3014b0af9cac746cf8728a33d4bf7fd0a1124ec0))
    - :memo: Added a changelog ([`e63e03c`](https://github.com/jrmoynihan/flow/commit/e63e03c98834a3280be7d2f3f32fb4fe93272d53))
    - Reorganize workspace into separate crates ([`fd12ce3`](https://github.com/jrmoynihan/flow/commit/fd12ce3ff00c02e75c9ea84848adb58b32c4d66f))
</details>

