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

| Codec | ID | Description | Typical ratio |
|-------|-----|-------------|---------------|
| **Mode A** (Lossless f32) | `LosslessF32` | Byte-stream-split + zstd | 2.5–3.2× |
| **Mode B** (ADC bitpack) | `AdcBitpack` | Bit-reservoir packing at ADC resolution | 3–4× |
| **Mode C** (Log-quant) | `LogQuant` | Arcsinh transform + fixed-point quantize | 4–6× |
| Pco | `LosslessF32Pco` | Piecewise coding (alternative to Mode A) | 2.8–3.5× |
| LZ4 | `Lz4Baseline` | LZ4 frame (baseline comparison) | 1.5–2× |

Mode A and B are **bit-exact lossless** to the f32 representation. Mode C is lossless with respect to ADC bit depth (quantization matches instrument resolution).

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

Performance on Apple M1 Max (10-core):

| Operation | Throughput |
|-----------|-----------|
| Mode A decode (single-threaded) | 0.5–1.0 GB/s |
| Mode A decode (parallel) | 2–3 GB/s |
| Mode B decode | 1.5–2.5 GB/s |
| Mode C decode (LUT path, ≤14 bits) | ~4× faster than naive |

## Tests

```bash
cargo test -p flow-fcs-compress
```

37 unit tests covering codec roundtrips, chunk splitting, container I/O, transform correctness, and auto-selection logic.

## ISAC Proposal

This crate includes a draft proposal for the ISAC FCS Working Group to standardize compression and column-major layout in the FCS specification. See [`docs/isac-proposal.md`](docs/isac-proposal.md).

## License

MIT
