# flow-fcs-compress

Column-oriented compression codecs and container formats for FCS flow cytometry data.

[![crates.io](https://img.shields.io/crates/v/flow-fcs-compress.svg)](https://crates.io/crates/flow-fcs-compress)
[![docs.rs](https://docs.rs/flow-fcs-compress/badge.svg)](https://docs.rs/flow-fcs-compress)
[![MIT](https://img.shields.io/crates/l/flow-fcs-compress.svg)](LICENSE)

## Overview

FCS files store event data row-major (all parameters for one event contiguous), which is optimal for acquisition but suboptimal for analysis — reading a single channel requires touching every row. `flow-fcs-compress` provides column-major codecs that exploit per-channel statistical structure for 2.5–6× compression while enabling parallel, single-channel decode at 0.5–3 GB/s.

## Features

| Feature | Description |
|---------|-------------|
| `multithread` *(default)* | Rayon-parallel encode/decode |
| `pco-backend` | Alternative lossless codec via `pco` (Piecewise Coding) |
| `lz4-baseline` | LZ4 frame codec for comparison benchmarking |

## Codecs

| Codec | ID | Fidelity | Ratio | Encode | Decode (1T) | Decode (MT) |
|-------|-----|----------|-------|--------|-------------|-------------|
| **Mode A** | `LosslessF32` | Lossless (bit-exact f32) | 2.5–3.2× | 400–450 MB/s | 0.7–1.0 GB/s | 2–3 GB/s |
| **Mode B** | `AdcBitpack` | Lossless (bit-exact f32) | 2–4× | 1.0–1.1 GB/s | 1.5–2.5 GB/s | 3–5 GB/s |
| **Mode C** | `LogQuant` | Lossy (≤ 0.09% rel error, ±0.5 ADC bin) | 4–6× | 350–400 MB/s | 2–3 GB/s | 4–6 GB/s |
| Pco | `LosslessF32Pco` | Lossless (bit-exact f32) | 2.8–3.5× | 200–250 MB/s | 0.8–1.2 GB/s | 2–3 GB/s |
| LZ4 | `Lz4Baseline` | Lossless (bit-exact f32) | 1.5–2× | 350–400 MB/s | 2–3 GB/s | 4–6 GB/s |

- **Mode A** (byte-stream-split + zstd): best ratio among lossless codecs for floating-point channels.
- **Mode B** (bit-reservoir bitpack): packs values at ADC resolution (`$PnR` bits). Fastest encoder; ~3.5× faster decode than Mode A.
- **Mode C** (arcsinh + fixed-point quantize): lossy — precision loss is ≤ ±0.5 of the least-significant ADC bit (sub-0.1% relative error away from zero). Uses sinh LUT for decode when bit width ≤ 14. Appropriate when downstream analysis already applies arcsinh transforms.
- **Pco**: highest ratio on integer-stored-as-f32 data (common in 16-bit instruments), slower encode.
- **LZ4**: fast baseline with modest ratio; useful for streaming/transient storage.

Throughput measured on Apple M1 Max (10-core), 80–1024 MB datasets. 1T = single-threaded, MT = rayon parallel.

## Container Formats

### `.fcz` Native Container

Memory-mapped, chunk-indexed format for zero-copy random access:

```rust
use flow_fcs_compress::container::fcz::{FczWriter, FczReader, FczWriteOptions};
use flow_fcs_compress::codec::{CodecId, ChannelParams};

// Write
let mut writer = FczWriter::create("output.fcz", FczWriteOptions::default())?;
writer.set_fcs_text(text_segment)?;
let ch_idx = writer.add_channel(ChannelParams::linear_unsigned("FSC-A", 262144), CodecId::LosslessF32)?;
writer.write_chunk(ch_idx, &events)?;
writer.finish()?;

// Read
let reader = FczReader::open("output.fcz")?;
reader.warm_cache();  // prefault pages for benchmarking
let fsc_a = reader.read_full_channel(0)?;

// Parallel decode all channels
let mut buffers = vec![vec![]; reader.n_channels()];
reader.decode_all_par(&mut buffers)?;
```

### Inline FCS Payload

Embeds compressed column data inside a standard FCS file's DATA segment with a `$COMPRESSION = FCZ1` keyword:

```rust
use flow_fcs_compress::container::inline::{encode_inline, decode_inline};

let payload = encode_inline(&channels, &params, &codec_ids)?;
// payload bytes go into the FCS DATA segment

let decoded = decode_inline(&payload)?;
for ch in &decoded {
    println!("{}: {} events", ch.name, ch.data.len());
}
```

## Auto Codec Selection

```rust
use flow_fcs_compress::codec::auto::pick_codec;

let codec_id = pick_codec(&params, allow_lossy);
// Never selects a lossy codec unless allow_lossy = true
```

## Architecture

```
┌─────────────────────────────────────────────┐
│  Container layer (.fcz / inline FCS)        │
│  - Chunk indexing, mmap, parallel I/O       │
├─────────────────────────────────────────────┤
│  Codec layer (ColumnCodec trait)            │
│  - encode_chunk / decode_chunk              │
│  - Per-channel, per-chunk granularity       │
├─────────────────────────────────────────────┤
│  Transform layer (pre-processing)           │
│  - Byte-stream split (f32 → 4 streams)     │
│  - Arcsinh log-space mapping               │
└─────────────────────────────────────────────┘
```

## Scope

This crate owns:

- Column-oriented compression codecs for f32 event data
- `.fcz` container format (write, read, mmap, parallel decode)
- Inline FCS DATA-segment compression payload
- Pre-compression transforms (byte-stream split, arcsinh)
- Codec auto-selection based on channel characteristics
- *(Future)* Streaming encode for acquisition pipelines
- *(Future)* Parquet sidecar integration

It does **not** own: FCS file parsing/writing (see `flow-fcs`), analysis algorithms, or visualization.

## Benchmarks

```bash
# Codec microbenchmarks (Criterion)
cargo bench -p flow-fcs-compress

# Full-file benchmarks (requires FCS test data)
cargo run -p flow-fcs-bench -- file path/to/data.fcs
cargo run -p flow-fcs-bench -- synth
```

## Tests

```bash
cargo test -p flow-fcs-compress
```

37 unit tests covering codec roundtrips, chunk splitting, container I/O, transform correctness, and auto-selection logic.

## ISAC Proposal

This crate includes a draft proposal for the ISAC FCS Working Group to standardize compression and column-major layout in the FCS specification. See [`docs/isac-proposal.md`](docs/isac-proposal.md).

## License

MIT
