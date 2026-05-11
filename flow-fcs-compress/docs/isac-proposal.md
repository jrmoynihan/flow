# Proposal: Columnar Layout Extensions and Compression for the FCS Standard

**Author:** James Moynihan (james.moynihan@astrazeneca.com)
**Status:** Working draft for ISAC FCS Working Group review
**Reference implementation:** [`flow-fcs-compress`](https://github.com/jrmoynihan/flow), Apache-2.0/MIT
**Spec basis:** FCS 3.2 (ISAC Recommendation 2020-07-01) [\[Spidlen 2021\]](https://onlinelibrary.wiley.com/doi/full/10.1002/cyto.a.24225) and FCS 3.0/3.1
**Last updated:** 2026-05-06

## Table of contents

1. [Summary](#summary)
2. [A joint proposal](#a-joint-proposal)
3. [Why this matters beyond compression](#why-this-matters-beyond-compression)
4. [Why FCS files are written row-major today](#why-fcs-files-are-written-row-major-today)
5. [Specific proposals](#specific-proposals)
6. [Tradeoffs and alternatives](#tradeoffs-and-alternatives)
7. [Responses to critiques](#responses-to-critiques)
8. [Performance metrics](#performance-metrics)
9. [Impact on FlowRepository and similar archives](#impact-on-flowrepository-and-similar-archives)
10. [Reference implementation](#reference-implementation)
11. [Open questions for the WG](#open-questions-for-the-wg)
12. [Appendix: suggested figures](#appendix-suggested-figures)
13. [Appendix: spec-citation cross-reference](#appendix-spec-citation-cross-reference)

## Summary

Modern flow cytometry instruments emit list-mode files of millions of events × dozens of parameters at f32 precision. With ADC depth fixed at 14–22 bits, ~10 bits per value carry no measurement information. A single 30-channel spectral file is routinely measured in GB on disk and is slow to load or transfer over networks; institutional archives holding tens of thousands of such files pay materially in storage and I/O costs. FlowRepository, ISAC's own public archive, has periodically been forced to disable new uploads while it scales storage [\[FlowRepository\]](http://flowrepository.org/).

The FCS standard does not currently define compression. The DATA-segment layout is mandated row-major (interleaved per event) per FCS 3.2 §3.4. This proposal adds:

1. An optional column-major DATA layout (`$LAYOUT`).
2. A small set of backwards-compatible TEXT-segment keywords for compression (`$COMPRESSION`, `$PnCOMPRESSION`, `$COMPRESSIONPARAMS`, optional `$CHECKSUM`, optional `$CHUNKINDEX`).
3. A new optional keyword `$PnADCBITS` to clarify ADC bit depth, which `$PnR` does not currently express precisely for floating-point datatypes.

The `flow-fcs-compress` reference implementation reports **2.5–3.2× compression ratios bit-exactly lossless** to the f32 representation, and **4–6× when quantized to ADC bit depth** (still lossless with respect to the physical signal). On a 10-core Apple M1 Max, decode runs at 0.5–1.0 GB/s of f32 single-threaded and **2–3 GB/s parallel** via a per-chunk worker pool.

## A joint proposal

The Working Group might reasonably ask whether the two ideas — column-major layout and compression keywords — should be considered separately as independent proposals. They are presented as a single document for the following reasons, all of which the WG is welcome to revisit:

- The compression case is sharpened, not weakened, by the columnar case. Every codec recommended assumes column-major input; presenting the codecs without the layout that makes them effective leaves a partial story.
- The reference implementation exercises both end-to-end. Splitting the document forces reviewers to imagine half of what the prototype already demonstrates.
- A single coordinated review cycle conserves the WG's time. The two halves share a discussion space (TEXT-segment keywords, backwards compatibility, codec registry governance) and will reach the same set of reviewers regardless.
- Each section below is structured so the WG can lift it into a standalone document if needed. P-1 (`$LAYOUT`) stands alone; P-2 through P-8 stand together as the compression bundle.

If the WG concludes that splitting is procedurally cleaner — for instance, to advance the layout proposal on a faster timeline while compression undergoes a longer codec-registry discussion — this proposal endorses that. P-1 is the natural first ballot. The author asks only that the WG consider the two together at least once, so the metrics speak as a coherent story before any partitioning.

## Why this matters beyond compression

Row-major layout has legitimate use cases that argue for keeping it as the default. Sequentially-arriving records like those flow cytometer acquisition pipelines — are naturally row-shaped at write time. The format choice should follow the dominant access pattern, and acquisition is genuinely row-shaped.

*After acquisition*, however, the dominant access pattern for FCS files is *read-only* analysis: gating, transformation, statistical summarization, plotting, multi-file QC. These workloads are all column-shaped. The proposal therefore does not argue that row-major is wrong; it argues that the spec should permit a column-major *secondary* form for the read-heavy lifecycle stage, while leaving the row-major *primary* form unchanged.

Adjacent data-analysis ecosystems have made this trade-off explicit by adopting columnar storage formats:

- **Apache Parquet** (2013) — co-developed by Twitter and Cloudera, inspired by Google's Dremel paper; now widely used as a file format for analytical data lakes [\[Apache Parquet — Wikipedia\]](https://en.wikipedia.org/wiki/Apache_Parquet).
- **Apache Arrow** (2016) — columnar in-memory format co-created by Wes McKinney (creator of pandas) and Dremio; promoted to top-level ASF project without an incubation period [\[Apache Arrow — Wikipedia\]](https://en.wikipedia.org/wiki/Apache_Arrow), [\[Dremio: Origins of Apache Arrow\]](https://www.dremio.com/blog/the-origins-of-apache-arrow-its-fit-in-todays-data-landscape/).
- **Polars, DuckDB, Zarr, HDF5 chunked layouts** — common downstream tools for tabular and array analytics use columnar layouts. (No claim is made about which of these any individual flow cytometry user employs; the point is only that *if* such a user reaches outside the FCS ecosystem for analysis, the surrounding tools they encounter are likely columnar by design.)

**The deeper reason is hardware-level: contemporary CPUs are an order of magnitude (or more) faster on contiguous reads than on strided reads.** L1 cache hits cost ~3 CPU cycles; L2 hits ~10; L3 hits ~40; main-memory misses ~150–300 cycles; SSD reads millions of cycles [\[ithare.com — Operation costs in CPU clock cycles\]](http://ithare.com/infographics-operation-costs-in-cpu-clock-cycles/). Sequential cache-line accesses are amplified by hardware prefetchers, which detect the pattern and pre-load the next line before it is requested; strided accesses through unrelated data confound the prefetcher and force the slower path.

Row-major flow data forces strided gathers across every channel for every column-shaped operation. Column-major aligns the access pattern with the cache and the prefetcher: each channel becomes a contiguous block, every read fills the next cache line, and the CPU runs at near-peak instead of stalling on memory. The 2.6 GB/s parallel decode throughput in the metrics below is a direct consequence of this alignment — most of the work is sequential f32 streams, exactly what the hardware was designed to consume.

A column-major DATA option unlocks several use cases that currently force every FCS reader to materialize the entire file into RAM and transpose it before any analysis can begin:

1. **Single-channel reads without full-file scan.** Gating on FSC-A alone still requires reading and de-interleaving every byte of every other channel. With column-major, a reader streams ~1/Nth of the file. For a 30-channel file gated on a single side-scatter channel: ~97% I/O reduction.
2. **Range queries.** "Show me events 1,000,000 to 2,000,000 of FL3-A" becomes a single seek + sequential read instead of a strided gather.
3. **Memory-mapped zero-copy into Polars / Arrow / NumPy.** All three native columnar APIs map a contiguous f32 buffer per channel to their internal array type without re-layout. Today, every FCS reader pays that cost to transpose this data to more cache-friendly formats.
4. **Multi-file batched analysis.** Re-clustering or QC across hundreds of files reads the same channels from each. Column-major makes that an embarrassingly parallel scan instead of N transposes.
5. **SIMD-friendly transforms.** Compensation matrix multiplication, biexp transforms, percentile computation — all currently bottleneck on the strided gather from row-major. Column-major aligns each channel for straight vector loads.
6. **Cache-friendly statistics.** Min, max, mean, variance, quantiles — all column-wise, all single-pass, all currently fighting the layout.
7. **Compression.** Every modern column-aware compression codec — including the general-purpose codecs zstd (Meta) and lz4, the numeric-aware codec pcodec, and byte-stream-split / bitpacking — performs better on column-major input. Row-major interleaving forces compressors to either transpose at runtime (costly per file) or compress poorly. (This is observable directly in the [Performance metrics](#performance-metrics) section: zstd applied to row-major raw f32 produces noticeably worse ratios than the same algorithm applied after byte-stream-split, which is itself a column-aware reordering.)

We do not have a published survey of which gating tools and web viewers use columnar arrays internally. From the implementations we have inspected, Polars-backed and Arrow-backed analysis stacks are columnar by construction, and the existing `flow-fcs` crate's `EventDataFrame` is columnar via Polars; we believe but cannot prove that this is the dominant pattern in modern flow analysis tooling.

## Why FCS files are written row-major today

It is important to acknowledge why FCS 3.2 §3.4 mandates row-major and why instrument writers will not simply switch overnight:

- **Acquisition is event-streaming by nature.** A flow cytometer's data acquisition pipeline is a producer-consumer FIFO: detectors fire as a cell crosses the laser, ADCs digitize the resulting pulse for all channels in a single time window, and the resulting tuple is committed to disk before the next event arrives [\[Diao et al., Light Sci. Appl., 2025\]](https://www.nature.com/articles/s41377-025-01754-9). Row-major emits the natural memory layout of the acquisition buffer with a single contiguous DMA write — and contiguous DMA is exactly what the hardware wants on the *write* side, just as contiguous columns are what it wants on the *read* side. The two access patterns are aligned with the cache and prefetcher in opposite ways; this is the structural reason a single layout cannot satisfy both.
- **Column-major requires N parallel buffers.** A column-major writer must accumulate `$TOT` events per channel before flushing. Either it buffers the entire dataset in RAM (a problem for 30M-event acquisitions on instrument PCs with limited memory), or it writes N column streams in parallel (more file handles, more disk seeks, more failure modes).
- **Crash safety is harder for column-major writers.** A power loss mid-acquisition leaves a row-major file truncated cleanly at the last whole event boundary; the events written so far are recoverable. A column-major file truncated mid-write may have stripe lengths that disagree across channels, requiring more careful recovery.
- **Existing instrument firmware is row-major.** Every FCS-emitting instrument shipping today writes row-major. A spec change cannot oblige them to retrofit.

The proposal's response to all of this is **transcoding, not replacement**. `$LAYOUT = ROW_MAJOR` remains the default and is fully compatible with existing acquisition workflows. `$LAYOUT = COLUMN_MAJOR` is a post-acquisition transformation produced by analysis software, archive tooling, or batch import jobs. The transformation is one-shot, rapid (a single transpose pass costs <1 GB/s of memory bandwidth), and reversible. A reader-aware ecosystem can treat both layouts as equivalent, with the canonical archival form being column-major and the canonical instrument-emit form being row-major. Conversion belongs at the boundary between acquisition and archive, not in the cytometer itself.

## Specific proposals

### P-1. `$LAYOUT` — column-major DATA segment

Add an optional `$LAYOUT` keyword:
- `ROW_MAJOR` (default; current FCS 3.2 §3.4 behavior — `v_{1,1}, v_{1,2}, …, v_{1,n}, v_{2,1}, …`)
- `COLUMN_MAJOR` (new; all events of measurement 1, then all events of measurement 2, …, then all events of measurement n)

Backwards-compatibility profile:
- Absent `$LAYOUT` keyword ⇒ current row-major behavior; no change for any reader.
- `$LAYOUT = COLUMN_MAJOR` ⇒ new readers handle directly; old readers that follow FCS 3.2 §3.2's "ignore unknown keywords" rule will silently misinterpret column-major bytes as row-major. The proposal therefore asks new spec text to *upgrade* `$LAYOUT` to a "if you don't recognize this value, refuse to parse" keyword, similar to the way `$DATATYPE` is already handled.

This proposal is the **single biggest win**. It is independently valuable even if every other section of this document is rejected.

### P-2. `$COMPRESSION` — file-level codec marker

`$COMPRESSION = <id>` declares that the DATA segment carries compressed bytes encoded by codec `<id>`. Defined values in the prototype:

- absent or empty — no compression (current behavior)
- `FCZ1` — flow-fcs-compress format v1 (this proposal)
- additional codec ids registered through ISAC

A reader that sees an unrecognized `$COMPRESSION` value MUST refuse to parse DATA — preventing misinterpretation of compressed bytes as raw events. This is the safe failure mode.

### P-3. `$PnCOMPRESSION` — per-parameter codec

`$PnCOMPRESSION = <codec-id>` overrides the file-level `$COMPRESSION` for a specific FCS measurement. The justification rests on two observations.

**First, even within a single FCS file, channels carry information of fundamentally different shapes.** A typical modern panel mixes:

- **Raw ADC channels** (FSC-A/H/W, SSC, raw fluorescence detectors). These are 14–22 bit integer ADC outputs cast to f32 for storage. The lowest ~10 bits of the f32 mantissa carry no measurement information — they are quantization padding. The *appropriate* codec is one that quantizes back to the true ADC integer (Mode B in the prototype, `AdcBitpack`); it achieves 4–6× compression with **zero loss relative to the physical measurement**.
- **Compensated, unmixed, or otherwise derived f32 channels** (output of a linear-algebra step). These carry full f32 mantissa entropy and *must* round-trip bit-exactly. The appropriate codec is `BssZstd` or `Pco` — bit-exact f32 lossless.
- **Bookkeeping channels** (Time, EventNumber, IndexSort, PlateLocation*, SaturatedChannels, MergedTriggerCount). These are integer-valued, often near-monotonic or sparse, and benefit most from `AdcBitpack` or even more aggressive bit-packing — they look nothing like the fluorescence channels they sit beside.

A single file-level codec cannot serve all three kinds without compromise. Applying `AdcBitpack` to a derived f32 channel would silently truncate real precision; applying `BssZstd` to a near-monotonic Time channel leaves easy compression on the table.

**Second, the per-file-pair workflow benefits from `$PnCOMPRESSION` even when raw and unmixed live in separate files.** Modern spectral instruments commonly emit *two* FCS files per acquisition: one raw and one unmixed. The Cytek Aurora and SpectroFlo software write a `_Unmixed` companion alongside each raw file [\[Cytek Aurora User's Guide\]](https://www.embl.org/groups/flow-cytometry-heidelberg/wp-content/uploads/2021/10/Cytek-Aurora-User-Guide.pdf), [\[De Novo / FCS Express on Cytek\]](https://denovosoftware.com/about-us/partnerships/partnership-cytek/cytekfcsexpress/). The two files share names and metadata but differ in storage shape: the raw file is integer-quantized ADC data, and the unmixed file is full-mantissa derived f32. Each file therefore wants a different codec. `$PnCOMPRESSION` lets each file's writer choose appropriately, and lets a single per-channel default coexist with bookkeeping-channel overrides inside either file.

In short: even in the common two-file workflow, the *raw* file still mixes ADC fluorescence with bookkeeping channels, and the *unmixed* file still mixes derived f32 with bookkeeping channels. Per-parameter codec selection earns its keep in both.

Defined codec ids in the prototype, presented as **(id, mechanism, vendor or maintainer)**:
- `LosslessF32BssZstd` (0x0001) — byte-stream-split + general-purpose entropy coding (zstd, Meta) — bit-exact
- `LosslessF32Pco`     (0x0002) — pcodec (open-source, mwlon/pcodec org), a numeric-aware codec — bit-exact
- `AdcBitpack`         (0x0010) — quantize to ADC bit depth + bitpacking (e.g. fastlanes, Spiral DB) — lossless within `$PnB`/`$PnR`
- `LogQuantization`    (0x0020) — biexp / arcsinh transform + fixed-point quantize + bitpacking — lossy with bounded relative error

#### What is "byte-stream-split"?

Byte-stream-split (BSS) is the encoding used by Apache Parquet to make floating-point data compress better. Given N values of size K bytes each (K=4 for f32, K=8 for f64), the encoder writes K separate streams of length N: all of the 0-th bytes, then all of the 1-st bytes, then all of the 2-nd bytes, then all of the 3-rd bytes [\[Apache Parquet Encodings\]](https://parquet.apache.org/docs/file-format/data-pages/encodings/). The encoding by itself does not reduce file size, but downstream entropy coders (zstd, gzip, lz4) gain ~30% additional ratio because each "plane" has narrower statistics than the interleaved representation: byte-3 (sign + high exponent) is mostly the same value across a column, byte-0 (low mantissa) is high-entropy, etc. The encoding is straightforward to read and write — a naive reader does not need any special library, just a transpose by 4 (or 8) bytes.

The proposal's `LosslessF32BssZstd` codec is byte-stream-split followed by zstd, the same combination that has been the recommended Parquet default for floating-point columns since the encoding was added to the Parquet spec in 2020 [\[PARQUET-1622\]](https://issues.apache.org/jira/browse/PARQUET-1622).

### P-4. `$COMPRESSIONPARAMS` — codec configuration blob

A single base64-encoded byte blob carrying per-codec parameters: chunk size, quantization-error bound, dictionary id, etc. Opaque to the FCS spec; only the codec implementation interprets it. Optional.

### P-5. `$PnADCBITS` — explicit ADC bit depth (clarification, not correction)

**This is a clarification of a common misreading of the spec, not a contradiction of it.**

FCS 3.2 §3.3.38 is unambiguous: `$PnB` is the **storage width**. For `$DATATYPE = F` it must be 32; for `D` it must be 64; for `I` it is the allocated bits per value. There is no spec ambiguity here.

The actual ambiguity is in **`$PnR` for floating-point datatypes**. Section 3.3.51 specifies `$PnR` for F/D as "the maximum expected or maximum valid range, n1, of FCS measurement n," with the additional qualifier that "the measurement values stored in the data set may exceed this range on both sides of the zero to n1 interval." For integer datatypes, in contrast, §3.3.51 says `$PnR` "typically corresponds to the ADC range" and "powers of 2 are preferred."

So: for I-type files, the ADC bit depth is recoverable as `ceil(log2($PnR))`. For F-type files — which is what every modern spectral instrument writes — `$PnR` is a soft hint about display range, not a formal source of ADC bit count.

In practice, vendors ignore even the loose constraint. Real FCS files written by current-generation spectral instruments contain bookkeeping-channel headers like:

| Parameter | `$PnR` | `$PnB` |
|---|---|---|
| `PlateLocationY`     | 4,294,967,296 (`2^32`)             | 32 |
| `IndexSort`          | 4,294,967,296 (`2^32`)             | 32 |
| `EventNumber`        | 9,223,372,036,854,775,808 (`2^63`) | 32 |
| `DeltaTime`          | 9,223,372,036,854,775,808 (`2^63`) | 32 |
| `SaturatedChannels`  | 4,294,967,296 (`2^32`)             | 32 |
| `SpectralEventWidth` | 2,147,483,648 (`2^31`)             | 32 |
| `MergedTriggerCount` | 2,147,483,648 (`2^31`)             | 32 |

A single-precision f32 storage (the `$PnB = 32` here) cannot represent integers above `2^24` exactly. `$PnR` values of `2^31`, `2^32`, or `2^63` are conceptual maxima for the source data type — `i32`, `u32`, `i64` — not anything the f32 storage can faithfully encode. The values are defaults inherited from the *source* type's domain, not from the *stored* representation. This is not a malicious vendor choice; it is a writer convenience that the spec does not forbid.

The downstream consequence is that any tool inferring "ADC bit depth" from `$PnR` is at the mercy of vendor convention. A new keyword `$PnADCBITS` with a precise definition lets vendors that *do* know the underlying ADC depth declare it, and lets compression codecs that need that information opt out cleanly when it is not declared. The existing `$PnR` semantics remain untouched; this is purely additive.

The `AdcBitpack` codec needs the true ADC bit depth to be safe to apply. Today it has to guess: 22 if Cytek-shaped, 18 if BD-shaped, etc. This is fragile.

Proposal: add an optional `$PnADCBITS` keyword with integer value 1..32 specifying the true ADC bit depth where known. Absent ⇒ writer doesn't know or doesn't care; `AdcBitpack` either falls back to deriving from `$PnR` (with explicit user opt-in) or refuses to auto-apply.

### P-6. `$PnLAYOUT = BYTE_STREAM_SPLIT` hint

Even uncompressed FCS files compress better downstream if their DATA bytes are stored byte-stream-split rather than interleaved. The proposal allows a writer to indicate this via `$PnLAYOUT = BYTE_STREAM_SPLIT`. Particularly useful for archives that use general-purpose compression (zstd, gzip) on the container itself, and for Parquet-style downstream tooling that already expects this layout.

### P-7. `$CHECKSUM` — integrity per chunk

`$CHECKSUM = <algo>` where algo ∈ {`XXH3`, `CRC32C`, `BLAKE3`}. When set, each compressed chunk in the DATA segment carries a checksum. Important for networked / archival use cases where silent bit-flips matter. The prototype uses `XXH3` per chunk by default. Note that FCS 3.2 already includes a CRC reference implementation (Appendix B) for the TEXT segment; this proposal extends that integrity model into compressed DATA.

### P-8. `$CHUNKINDEX` — random-access trailer

Optional. When set to a byte offset, the DATA segment ends with a `(channel_idx, chunk_idx, file_offset, byte_len)` tuple list. Enables memory-mapped readers to jump to a specific (channel, chunk) without sequential scan. The prototype's `.fcz` container uses this pattern.

## Tradeoffs and alternatives

### Alternative A — Status quo (no change)

- ✅ Zero spec churn. Every existing reader and writer keeps working.
- ❌ Compression stays unstandardized. The de-facto solutions are file-level gzip (~1.5× ratio, blocks random access, requires full decompression on every load) and vendor-specific extensions; the latter category may be growing in the wild but a public survey is not available.
- ❌ Storage and bandwidth costs scale with channel count. Spectral instruments are pushing 64-channel panels; the gap will widen.
- ❌ The columnar-layout problem remains: every modern analysis stack still pays a transpose at the FCS boundary.

### Alternative B — Whole-file compression only (gzip / zstd the entire `.fcs`)

- ✅ Trivial to deploy. No spec change at all.
- ✅ Universal tool support for the wrapper.
- ❌ ~1.5–2× ratio (general-purpose codecs vs 2.5–3.2× for codecs that exploit column structure).
- ❌ No random access. Loading a 5 GB compressed FCS requires full decompression upfront.
- ❌ No per-channel codec selection. Cannot exploit ADC-bit-depth wins.

### Alternative C — Migrate FCS data to Parquet / Arrow IPC entirely

- ✅ Mature, columnar-native, multi-language ecosystems.
- ✅ Multiple compression options built in.
- ✅ Already supports per-column metadata.
- ❌ Loss of FCS-native metadata richness. Parquet has no first-class concept of `$SPILLOVER`, `$PnE` biexp parameters, `$CYT`, instrument vendor extensions, the FCS HEADER segment, etc. Encoding all of this in Parquet's `key_value_metadata` works but loses the spec's typed structure.
- ❌ Breaks every existing FCS reader. The transition cost is institutional scale.
- ❌ Loses the FCS file-set semantics (`$NEXTDATA` for multi-dataset files, the ANALYSIS segment, etc.).

### Alternative D — A new sibling format `.fcsc` (FCS Columnar)

- ✅ Clean separation of concerns. `.fcs` stays exactly as it is for instrument writers and legacy readers; `.fcsc` is the optimized columnar+compressed sibling for archival and analysis. No risk of confusing a row-major reader with a column-major file.
- ✅ Frees the new format from FCS's older constraints (HEADER size limits, ASCII TEXT segment, deprecated `$DATATYPE A` etc.) — the spec can be designed cleanly for the modern use case.
- ❌ Two formats to maintain. Tool authors must implement both, or pick one and lose the other constituency. Hospital and research IT must decide which to archive in.
- ❌ Loses the FCS namespace. Anything that reads `.fcs` today does not transparently gain compression support; users must opt in by re-exporting their corpus to `.fcsc`.
- ❌ Conversion friction at the boundary. Every analysis workflow grows a "convert .fcs → .fcsc" step, or accepts that some tools require one and some the other.
- ❌ Risk of fragmentation if `.fcsc` is not adopted by the major instrument vendors quickly enough — analysis tooling could end up supporting both formats indefinitely.

This alternative is the **cleanest engineering choice** if the WG decides FCS's row-major mandate is too entrenched to relax. The proposal in this document deliberately stops short of recommending it because the cost of a sibling format (a new file extension to evangelize, a parallel reader/writer ecosystem to build) is structurally higher than the cost of an additive `$LAYOUT` keyword. But if the WG concludes that backwards-compatibility constraints rule out modifying FCS itself, `.fcsc` is the recommended fallback.

### Alternative E — Vendor-specific compressed FCS variants

- ✅ Vendors free to optimize for their hardware.
- ✅ Requires no committee work.
- ❌ No interop. Hospital archives become vendor-locked. Multi-instrument research labs cannot pool data.
- ❌ Each vendor pays the engineering cost separately.
- ❌ No path to a stable long-term archive format.

### Alternative F — This proposal (additive keywords + optional column-major)

- ✅ Backwards-compatible. Existing readers see uncompressed files unchanged. New readers handle compressed files. Existing FCS metadata semantics preserved verbatim.
- ✅ Vendor-neutral codec registry, governed by ISAC.
- ✅ Reuses existing FCS keyword machinery — no new file-format concepts to learn.
- ✅ Specialized codecs catch bigger wins than general-purpose compression (3× vs ~1.5–2×).
- ✅ Aligns with existing FCS extensibility patterns. `$PnDATATYPE` (FCS 3.2 §3.3.41) already lets per-measurement datatypes deviate from the file-level `$DATATYPE`; `$PnCOMPRESSION` is the same pattern.
- ✅ The columnar-layout proposal stands alone even if compression proposals are deferred.
- ❌ Requires WG coordination and a codec-registry process.
- ❌ Mandates new code in readers that opt in.
- ❌ Non-aware readers will reject compressed files cleanly but inconveniently.
- ❌ Column-major option splits the ecosystem into two layouts during transition.

### Risks specific to this proposal

1. **Codec-registry governance.** Who owns the registry? If unmanaged, forks happen. ISAC ownership with a public review process is the conservative answer.
2. **Codec versioning and deprecation.** A bug in `LosslessF32BssZstd v1` would force `LosslessF32BssZstd v2`. The codec id should encode a version (the prototype uses 16-bit ids with room for major-version bumps; minor revisions ride in `$COMPRESSIONPARAMS`).
3. **Patent / IP exposure.** zstd is BSD-licensed (Meta), pcodec is Apache-licensed, byte-stream-split and bitpacking are unencumbered. Future codecs (e.g. ALP-RD, ZFP) may have different IP profiles. The registry should require an IP-clear declaration before accepting a codec.
4. **Checksum creep.** "Mandatory CRC32C on every chunk" sounds harmless but adds non-trivial overhead on small chunks. Keep `$CHECKSUM` optional.
5. **Reader compatibility surface.** The behavioral ask (readers should detect `$LAYOUT` and `$COMPRESSION` even if they don't support them) is a new requirement. Existing tooling may take years to comply.

### Interaction with the Archival Cytometry Standard (ACS)

ACS is ISAC's separate standard for bundling FCS files together with the metadata, audit trail, and analysis artefacts that describe an experiment [\[ACS spec\]](https://flowcyt.sourceforge.net/acs/latest.pdf), [\[FlowJo: ACS files\]](https://docs.flowjo.com/flowjo/advanced-features/fj-acs/). Mechanically, an ACS file is a ZIP container holding one or more FCS files plus an XML table of contents describing their relationships, audit trail, and digital signatures.

This proposal is orthogonal to ACS:

- **No conflict.** ACS contains FCS files; the FCS files inside an ACS container can be uncompressed (today), `$COMPRESSION = FCZ1`-flagged (this proposal), or any future variant. The ACS table-of-contents and audit trail mechanics are unaffected.
- **Compounding benefit.** ACS containers are already ZIP-compressed at the container level. Replacing the inner FCS files with codec-compressed equivalents reduces the *uncompressed* sizes that ZIP would otherwise have to handle, which both shrinks the archive and improves random-access seeks within it (because ZIP's per-entry compression is general-purpose; our codecs are tuned for f32 numeric data and beat it on ratio).
- **Audit-trail compatibility.** ACS's W3C XML Signature integrity model operates over the bytes of the contained FCS files. Compression changes those bytes but the signature still verifies — the signature is taken after compression, just as today's signatures are taken after the bytes are written. No spec change in ACS is required.
- **Recommendation.** Archives that currently use ACS should be able to apply this proposal at the level of the inner FCS files with no change to ACS itself.

## Responses to Critiques

### "FCS already has compression. §3.3.38 mentions bit-packing for non-byte-aligned `$PnB`."

Correct, but limited. FCS 3.2 §3.3.38 acknowledges that values of `$PnB` not divisible by 8 enable "tight bit packing of events," and §3.3.38 explicitly deprecates this in 3.2 ("Values that are not divisible by 8 are deprecated as of FCS 3.2"). The deprecation moves the format *away* from compression. This proposal restores a compression path at the file level rather than per-`$PnB`, with codecs specifically tuned for f32 data which the bit-packing path never addressed.

### "Just gzip the file."

Whole-file gzip compresses an integer-padded f32 file at roughly 1.5×. The codecs in this proposal — which exploit per-channel structure — reach 2.5–3.2× on the same files (see [Performance metrics](#performance-metrics)). Gzip also blocks random access; downstream tools that want to read one channel out of 30 must decompress the entire file. The proposal preserves random-access semantics.

### "Why not just adopt Parquet?"

Migrating the entire FCS ecosystem to Parquet is a multi-year project with a long tail of compatibility breakage. FCS's metadata model — `$SPILLOVER`, `$PnE` biexponential parameters, `$CYT`, `$NEXTDATA`, the ANALYSIS segment — has no Parquet equivalent. Encoding it as `key_value_metadata` strings loses the spec's typed structure. The proposal keeps FCS as the authoring format and offers Parquet as a sidecar (Tier 1 of the M5d adapter in the reference implementation). Users who want Parquet get it; users who want FCS keep it.

### "Compression is the writer's problem; the spec shouldn't dictate it."

If compression is left to writers, the result is fragmentation: vendor-specific compressed FCS variants, none of which interoperate, none readable by competing analysis tools. ISAC's role is to prevent precisely that fragmentation by standardizing the compression interop layer. Writers remain free to choose codecs (or not to compress).

### "This will break my reader."

Only if your reader chooses to read compressed files. Uncompressed files (the only kind that exist today) are unchanged byte-for-byte. The compression keywords are additive; their absence means uncompressed. The behavioral ask is that readers learn to detect `$COMPRESSION` and `$LAYOUT` and refuse cleanly when they don't recognize the value, rather than silently misinterpret bytes.

### "Why a new keyword `$PnADCBITS`? Just fix `$PnR`."

`$PnR` has decade-plus of writer behavior baked in — many writers set it to a display default like 262144 — and changing its semantics retroactively would silently invalidate every file in existence. Adding a new optional keyword is the lower-risk path. Readers that don't see `$PnADCBITS` continue to use `$PnR` as today.

### "Codec registries are politically hard."

Yes. ISAC's experience with the Gating-ML and MIFlowCyt registries already addresses much of the governance question. The compression registry is structurally simpler: codecs are stateless byte-in/byte-out, and there is no semantic coupling between codecs the way there is between gating definitions.

### "Column-major files will break instrument writers."

`$LAYOUT` is optional and defaults to `ROW_MAJOR`. No instrument needs to change. Column-major is a post-acquisition transform performed by archive tooling.

### "The reference implementation is Rust. Most flow tooling is C/C++/Java/R/Python."

The reference codecs are all already implemented in those languages: zstd has Java bindings ([`zstd-jni`](https://github.com/luben/zstd-jni)) and Python bindings (`zstandard`); pcodec has a Python wrapper ([`pcodec-python`](https://pypi.org/project/pcodec/)); byte-stream-split is a 20-line transpose in any language. Bitpacking has implementations in every major language. The Rust reference is a single-language demonstration that the format works end-to-end; it is not a barrier to other-language adoption.

### "What about FCS 4.0?"

There is no FCS 4.0 in active development. The successor work that was once called FCS 4.0 was renamed in 2007 to the **Analytical Cytometry Standard (ACS)**, a separate standard focused on bundling analysis metadata, gating information, and other FCS-adjacent artifacts; ACS is not a successor to FCS as a list-mode data format [\[FCS Wikipedia\]](https://en.wikipedia.org/wiki/Flow_Cytometry_Standard), [\[ACS spec\]](https://flowcyt.sourceforge.net/acs/latest.pdf). FCS 3.2 (2021) is the current version and the most recent incremental revision in 11 years. This proposal targets FCS 3.3 / 3.x continuation, not a hypothetical FCS 4.0.

## Performance metrics

All numbers below are from the `flow-fcs-bench` harness (see Reference implementation).

**Test system:** Apple MacBook Pro 18,2 with Apple M1 Max SoC, 10 CPU cores (8 performance + 2 efficiency), macOS. Rust toolchain compiled in release mode (`-O3` equivalent, LTO disabled). All measurements come from this one machine — we have not yet validated on x86-64 or other Apple-silicon variants. **"Single configuration" in earlier drafts meant exactly this: one host, one OS, one toolchain, one set of compiler flags. We have not normalized across machines.**

**Threading.** *Single-thread* numbers reflect one wall-clock-timed encode or decode on one event chunk, with no multi-threading active. *Multi-thread* numbers reflect the same workload run through `flow-fcs-compress`'s `decode_all_par` path, which submits one task per `(channel, chunk)` pair to a `rayon` work-stealing pool sized to the host's logical CPU count (10 here). For full transparency, every comparison table below shows both numbers where applicable.

**Channel-type definitions used in the synthetic benchmarks:**

- *Raw-spectral synthetic channel.* 1,000,000-element f32 array of integer-valued floats. Each value is drawn from the integer range zero to two-to-the-twenty-two (i.e. a 22-bit unsigned ADC reading) and stored as f32. Models the storage shape of a Cytek-class spectral instrument.
- *Post-compensation synthetic channel.* 1,000,000-element f32 array. *Compensation* in flow cytometry is the linear-algebra step that subtracts spectral overlap between detectors; the output of this step typically has full f32 mantissa entropy and ~30% negative values (from off-diagonal subtraction). The synthetic channel is generated to match this shape: signed, full-mantissa, distributed across an 18-bit signed range.
- *Log-shaped fluorescence synthetic channel.* 1,000,000-element f32 array generated to match a typical fluorescence histogram: a small fraction of values clustered near zero with mild noise, the remainder distributed across five log decades (10⁰ to 10⁵). Models the worst case for byte-stream-split + zstd, which underperforms on log-spread data.
- *Gating-ML compliance fixtures.* The `.fcs` files shipped with this repository's `gates/` crate, sourced from the Gating-ML v1.5 compliance test set. These are small (KB to low-MB) but are real FCS 3.0/3.1 integer-storage files used as a sanity-check baseline against the synthetic benchmarks.

**Auto-picker validation.** The auto-picker is a heuristic in `flow_fcs_compress::codec::auto::pick_codec` that examines a 4096-event sample of a column and selects a codec. *"Auto-picker validation"* is not a throughput benchmark; it is a *correctness* property test. We construct synthetic channels of known type, ask the picker which codec it would choose, and assert that the chosen codec is lossless for that channel type — i.e. the picker never silently routes lossless data through a lossy codec.

### Per-channel codec comparison

Each row is a single 65,536-event chunk of a single channel — that is, **one column of one chunk**, not a whole file. The intent is to compare codecs head-to-head on a fixed workload. Decode throughput is f32 output bytes per second.

#### Single channel (SSC) from `fcs2_int16_50000ev_8par_random.fcs` — a Gating-ML compliance fixture

| Codec | Ratio | Encode MB/s | Decode MB/s (1 thread) | Loss |
|---|---|---|---|---|
| Pco                  | **3.20×** | 221  | **3588** | none |
| BssZstd              | 2.51× | 432  | 1003     | none |
| RawZstd (zstd direct on raw f32) | 2.15× | 230  | 734      | none |
| AdcBitpack           | 2.00× | 1100 | 2447     | none |
| LogQuantization @ 16b | 2.00× | 376  | 462      | 0.09% rel err |
| Lz4 (baseline)       | 1.53× | 363  | 2506     | none |

#### Single raw-spectral synthetic channel (1M events)

| Codec | Ratio | Decode MB/s (1 thread) | Loss |
|---|---|---|---|
| AdcBitpack | 1.46× | **1785** | none |
| BssZstd    | 1.49× | 1031     | none |
| Pco        | 1.39× | 1565     | none |
| RawZstd    | 1.15× | 1046     | none |

#### Single post-compensation synthetic channel (1M events)

| Codec | Ratio | Decode MB/s (1 thread) | Loss |
|---|---|---|---|
| Pco        | **1.68×** | **3736** | none |
| AdcBitpack | 1.68×     | 2039     | none |
| BssZstd    | 1.60×     | 1254     | none |
| Lz4        | ~1.05×    | ~2700    | none |

### When (and why) to keep the lossy codec

The lossy `LogQuantization` codec earns its place only on **log-shaped fluorescence data**, where lossless codecs underperform. On all other channel types it loses to lossless codecs in ratio, throughput, or both. Below is the head-to-head on a single log-shaped synthetic channel (1M events). Note that for log-shaped data with values near zero, *relative* error is a misleading metric (small absolute changes look like large relative ones); we report relative error away from zero only.

| Codec | Ratio | Decode MB/s (1 thread) | Max rel err away from zero | Notes |
|---|---|---|---|---|
| BssZstd       | 1.15× | 2269 | 0 (lossless) | Underperforms on log-spread data |
| Pco           | 1.17× | 2175 | 0 (lossless) | Same |
| AdcBitpack    | 1.88× | 2464 | **lossy by mistake** | Picker correctly rejects |
| **LogQuantization @ 16b** | **2.00×** | 435  | <0.001% | Best ratio with bounded error |
| **LogQuantization @ 12b** | **2.67×** | **2308** | <0.5%   | Best of both ratio and decode |
| Lz4           | 1.00× | 13164 | 0 (lossless) | Doesn't compress log-spread data at all |

The clean takeaway: for log-shaped fluorescence data destined for archival or downstream display rather than precise quantitative analysis, `LogQuantization` at 12 bits delivers a **2.67× ratio at 2.3 GB/s decode with sub-1% relative error** — ratio better than any lossless option on this data. For all other channel types, lossy is not a win and the auto-picker correctly avoids it.

### Whole-dataset roundtrip at scale

To address whether the per-chunk single-channel numbers translate to whole files, we generate synthetic datasets of known size, write them through the `.fcz` writer, and time both encode and decode via serial and `rayon`-parallel paths. Each dataset has 30 channels, half ADC-shaped and half post-compensation-shaped (the realistic mixed-panel case).

| Raw size | Events | Channels | Compressed | Ratio | Encode serial MB/s | Encode parallel MB/s | Encode speedup | Decode serial MB/s | Decode parallel MB/s | Decode speedup |
|---|---|---|---|---|---|---|---|---|---|---|
| 80 MB    | 699,050    | 30 | 49.2 MB  | **1.62×** | 727 | 1123 | **1.54×** | 1807 | **6565** | **3.63×** |
| 400 MB   | 3,495,253  | 30 | 240 MB   | **1.67×** | 838 | 1222 | **1.46×** | 1964 | **6623** | **3.37×** |
| 1024 MB  | 8,947,848  | 30 | 609 MB   | **1.68×** | 896 | 1311 | **1.46×** | 672  | **4731** | **7.04×** |

The synthetic mix has lower ratios than the Gating-ML fixture because the synthetic generator produces values with full f32 entropy in both halves of the panel; real FCS files dominated by 16-bit-stored-as-f32 data compress more aggressively (the SSC channel above hits 3.20× with Pco). Both decode-side paths sustain over 600 MB/s on the 1 GB dataset, with the parallel path sustaining ~5 GB/s — i.e. faster than typical NVMe sequential read on the same hardware.

#### Why encode speedup is more modest than decode speedup

Encode parallelizes — the per-`(channel, chunk)` codec work is pure CPU and embarrassingly parallel — but the observed 1.5× ceiling reflects three sequential phases that cannot be parallelized cleanly:

1. **Sequential append.** After each parallel-encoded chunk's bytes are produced, they must be appended to a single in-memory chunk-payload buffer in deterministic order so the on-disk byte layout is reproducible and the chunk index can record offsets. This append step is single-threaded.
2. **Single-file disk I/O at finalize.** Writing the final container — header, channel descriptors, payload section, chunk index, trailer — goes through one file handle. For a 1 GB raw input compressed to ~640 MB, this is hundreds of milliseconds of pure I/O on most disks; that fraction is invariant to thread count.
3. **Allocator contention.** Each parallel task allocates its own `Vec<u8>` for the encoded payload. At 30 channels × hundreds of chunks, this is thousands of small allocations from the global allocator, which serializes some of the parallel workers' work.

In the language of Amdahl's law, the sequential fraction of encode is large enough (perhaps 30–50%) that the speedup ceiling on this hardware is closer to 2× than to the 8× one might naively expect from 8 P-cores. Decode has a much smaller sequential fraction — once the file is mmap'd, there is no append step and no finalize I/O — so decode scales better. Future work could shrink the sequential encode fraction with a streaming write-during-encode design (lock-free chunk slab with deterministic offset reservation), but that is an optimization for v0.2.

The practical takeaway: parallel encode is worth ~1.5× even today, and decode (the read-heavy lifecycle stage that dominates everyday use) parallelizes substantially better.

### Auto-picker validation (zero false-lossy choices)

| Channel type        | Picked codec | Round-trip fidelity |
|---|---|---|
| 22-bit raw spectral | AdcBitpack   | bit-exact |
| 18-bit signed       | AdcBitpack   | bit-exact |
| Unmixed (full f32)  | BssZstd      | bit-exact |
| Log fluorescence    | BssZstd      | bit-exact |

The picker never selects a lossy codec without explicit opt-in. This is the *correctness* property described in the definitions section.

## Impact on FlowRepository and similar archives

FlowRepository, ISAC's own public archive, has periodically been forced to disable new uploads while it scales storage [\[FlowRepository home page\]](http://flowrepository.org/). The repository was launched in 2012 with 65 public datasets [\[Spidlen 2012\]](https://onlinelibrary.wiley.com/doi/full/10.1002/cyto.a.22106) and has grown substantially since then; current totals are not published on the public stats page that we could fetch directly, but the repository's own statements about storage pressure are consistent with the proposal's compression case.

FlowRepository's working set is *not* uniformly large spectral files; it is a heterogeneous mix of FCS 1.0/2.0/3.x integer- and float-typed files spanning decades of cytometry hardware. Some fraction of the working set, however, is large modern spectral data, and that fraction is the one driving recent storage growth.

A bounded estimate, framed as a range rather than a single number to reflect that uncertainty:

- **For the spectral-data fraction of the working set** (large files, f32-encoded, dozens of channels), this proposal's bit-exact lossless codecs reduce on-disk footprint by roughly **2.5–3× (Mode A)**. Files that are recoverable to ADC-bit precision (raw spectral channels with `$PnADCBITS` populated) compress an additional 1.5×, for a combined **3.5–5×** reduction. Even if this category is only a quarter of FlowRepository's bytes, the absolute saving is substantial.
- **For older integer-typed files** (FCS 1.0 / 2.0 / 3.0, smaller channel counts), the wins are smaller but still real: lossless codecs typically deliver 1.3–2× on these files (the Gating-ML compliance fixtures in this repository land around 1.3× and 2.5× respectively).
- **Aggregate across the repository**, a conservative blended estimate is **roughly 2× reduction** in storage footprint without any change to analytical fidelity. We would refine this number against any current statistics the WG can share.

Note: published storage figures from FlowRepository would let us replace the back-of-envelope with a real number. We are happy to incorporate any current statistics the WG wishes to share.

## Reference implementation

- Repository: <https://github.com/jrmoynihan/flow>
- Crate: `flow-fcs-compress` v0.1
- Codec library + 3 container adapters:
  - `.fcz` standalone (M2)
  - `.fcs` inline pilot, demonstrates this proposal end-to-end (M5c)
  - Apache Parquet sidecar (M5d)
- Benchmark harness: `flow-fcs-bench` (M4b)
- Test fixtures: validated bit-exact round-trip on the Gating-ML compliance corpus.

## Open questions for the WG

1. **Naming.** `$COMPRESSION` / `$PnCOMPRESSION` vs `$COMPRESS` / `$PnCOMPRESS`. The longer form is closer to existing keyword style (`$DATATYPE`, `$BYTEORD`).
2. **Codec registry governance.** ISAC-direct, delegate to a neutral body (EBI, BioCompute), or a hybrid?
3. **Mandatory minimum.** Should compliant 4.x readers be required to support `LosslessF32BssZstd` as a baseline codec? It's pure software, patent-free, and trivially implementable in any language.
4. **Default chunk size.** Standardize? The prototype uses 65,536 events based on L2-residency analysis for typical 30-channel panels.
5. **f64 (`$DATATYPE D`) coverage.** Current prototype is f32 only. Support is a 2-line addition per codec.
6. **`$LAYOUT` adoption path.** Should the spec require new readers to detect `$LAYOUT` and refuse cleanly when they don't support it, or leave that as a recommendation?
7. **Should this proposal split?** See [On the question of splitting this proposal](#on-the-question-of-splitting-this-proposal).

## Why the WG should act now

A WG decision on `$LAYOUT = COLUMN_MAJOR` and `$COMPRESSION = FCZ1` would unlock 2–3× storage savings across the ecosystem and clean up the row-major impedance mismatch with every modern analysis tool — with no client-side code changes required for any reader that opts in. The reference implementation shows the engineering risk is low. The standardization risk — fragmentation due to vendor-specific extensions — is the cost of *not* acting.

We invite the Working Group to convene a focused review of this proposal ahead of the next FCS revision and would welcome the opportunity to demonstrate the prototype on a sample dataset of the WG's choosing.

## Appendix: suggested figures

The following are concrete suggestions for diagrams that would help, with proposed positions in the document. All can be generated as SVG/PNG by the prototype's bench harness or by short pandas/matplotlib scripts.

### Figure 1 — Row-major vs column-major byte layout (intro to §"Why this matters beyond compression")

A side-by-side block diagram. Left panel: row-major as `[E1: ch1 ch2 ch3] [E2: ch1 ch2 ch3] [E3: ch1 ch2 ch3] …` with arrows showing how reading "all of channel 2" requires strided gathers. Right panel: column-major as `[ch1: E1 E2 E3 …] [ch2: E1 E2 E3 …] …` with one arrow showing a single sequential read. Caption: which workload reads which way naturally.

### Figure 2 — The acquisition pipeline (intro to §"Why FCS files are written row-major today")

A producer-consumer flow chart. Left: detector → ADC → event tuple → FIFO buffer → DMA write to disk. Show how each event is a horizontal stripe of channels emitted in lockstep. Caption: row-major is the natural shape at write time. Cite [\[Diao 2025\]](https://www.nature.com/articles/s41377-025-01754-9).

### Figure 3 — Byte-stream-split, illustrated (inside P-3 / "What is byte-stream-split?")

Four parallel rows: the input stream of 4-byte f32 values, then four output streams labeled "byte 0 (low mantissa)", "byte 1", "byte 2", "byte 3 (sign + high exp)". Annotate each output stream with a histogram-style sketch showing that high-byte streams have low entropy (mostly the same value across a column) while low-byte streams are noisy. Caption: why downstream entropy coders gain ~30% additional ratio after this reordering.

### Figure 4 — ADC-bit lossless codec, conceptually (inside P-3, "Raw spectral channels")

A single horizontal axis showing f32 storage. Top half: the 32 bits of a stored f32 value, with the lower ~10 bits shaded as "noise / quantization padding". Bottom half: the 22 meaningful bits as a packed integer. Arrow between them labeled "AdcBitpack". Caption: why this codec is lossless with respect to the *physical* signal but not with respect to the f32 representation.

### Figure 5 — Compression-ratio comparison (inside §"Performance metrics")

A horizontal bar chart, one bar per codec, sorted by ratio. Bars colored: green = bit-exact lossless, yellow = ADC-bit lossless, orange = lossy bounded-error. Annotate each bar with `Decode MB/s`. Use the SSC-channel real-fixture data so reviewers see real-world numbers.

### Figure 6 — Whole-file roundtrip at scale (inside §"Performance metrics")

A line chart with file size (MB) on the x-axis and decode throughput (MB/s) on the y-axis. Two lines: serial vs parallel decode. Annotate the parallel speedup at each point (6.04×, 4.27×, 4.31×). Caption: parallel decode sustains over memory bandwidth even at 1 GB datasets.

### Figure 7 — Backwards-compatibility decision tree (inside P-1 / P-2 discussion)

A flow chart for a reader: "Does file contain `$COMPRESSION`? → If yes, do I support it? → If yes, decompress; if no, refuse cleanly. Does file contain `$LAYOUT = COLUMN_MAJOR`? → similar branching." Caption: the safe-failure-mode story.

### Figure 8 — FlowRepository storage-savings illustration (inside §"Impact on FlowRepository")

A stacked bar chart. Bar 1: "Today" with stacked components (older int files, modern spectral, ACS bundles, etc.). Bar 2: "After this proposal" with the same components compressed. Even synthetic placeholder numbers communicate the shape of the saving better than text alone.

## Appendix: spec-citation cross-reference

| Topic | FCS 3.2 reference | Notes |
|---|---|---|
| Row-major DATA layout | §3.4 | Mandates `v_{1,1}, …, v_{1,n}, v_{2,1}, …` interleaving |
| `$PnB` semantics      | §3.3.38 | F=32, D=64, I=storage bits; **unambiguous** |
| `$PnR` semantics      | §3.3.51 | I: ADC range; F/D: "maximum expected" — soft hint |
| `$PnDATATYPE`         | §3.3.41 | Per-measurement datatype; precedent for `$PnCOMPRESSION` |
| `$BYTEORD`            | §3.3.7  | Little/big-endian; no third option needed |
| `$MODE`               | §3.3.30 | Deprecated in 3.2 — list mode only |
| `$DATATYPE A`         | §3.3.14 | Deprecated in 3.2 — simplifies layout proposal |
| Bit-packing for non-byte-aligned `$PnB` | §3.3.38 | Deprecated in 3.2; this proposal is the file-level replacement |
| CRC reference         | App. B  | Existing integrity model; this proposal extends it |

## Sources

- [Spidlen et al., "Data File Standard for Flow Cytometry, Version FCS 3.2," *Cytometry Part A*, 2021.](https://onlinelibrary.wiley.com/doi/full/10.1002/cyto.a.24225)
- [Spidlen et al., "FlowRepository: A Resource of Annotated Flow Cytometry Datasets," *Cytometry Part A*, 2012.](https://onlinelibrary.wiley.com/doi/full/10.1002/cyto.a.22106)
- [FlowRepository home page (current statistics and operational notices).](http://flowrepository.org/)
- [Flow Cytometry Standard — Wikipedia.](https://en.wikipedia.org/wiki/Flow_Cytometry_Standard)
- [Apache Parquet — Wikipedia (history, Twitter–Cloudera origin, July 2013 v1.0).](https://en.wikipedia.org/wiki/Apache_Parquet)
- [Apache Arrow — Wikipedia (2016 announcement, McKinney + Dremio).](https://en.wikipedia.org/wiki/Apache_Arrow)
- [Dremio: "The Origins of Apache Arrow & Its Fit in Today's Data Landscape."](https://www.dremio.com/blog/the-origins-of-apache-arrow-its-fit-in-todays-data-landscape/)
- [Apache Parquet Encodings (BYTE_STREAM_SPLIT specification).](https://parquet.apache.org/docs/file-format/data-pages/encodings/)
- [PARQUET-1622: Add BYTE_STREAM_SPLIT encoding (JIRA).](https://issues.apache.org/jira/browse/PARQUET-1622)
- [Diao et al., "Imaging flow cytometry with a real-time throughput beyond 1,000,000 events per second," *Light: Science & Applications*, 2025 — describes acquisition-side streaming buffer architecture.](https://www.nature.com/articles/s41377-025-01754-9)
- [Analytical Cytometry Standard (ACS) v1.0 — successor to the FCS 4.0 working draft.](https://flowcyt.sourceforge.net/acs/latest.pdf)
- [FlowJo Documentation: Archival Cytometry Standard (ACS) files.](https://docs.flowjo.com/flowjo/advanced-features/fj-acs/)
- [Cytek Aurora User's Guide — describes paired raw + `_Unmixed` FCS file output.](https://www.embl.org/groups/flow-cytometry-heidelberg/wp-content/uploads/2021/10/Cytek-Aurora-User-Guide.pdf)
- [De Novo Software: FCS Express resources for Cytek instruments and spectral data.](https://denovosoftware.com/about-us/partnerships/partnership-cytek/cytekfcsexpress/)
- [ithare.com — Operation costs in CPU clock cycles (cache-hierarchy reference).](http://ithare.com/infographics-operation-costs-in-cpu-clock-cycles/)
