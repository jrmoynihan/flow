//! `flow-fcs-bench` — synthetic-data benchmark harness for flow-fcs-compress.
//!
//! Three subcommands:
//!
//! - `synth`     — runs every codec on synthetic channels (ADC integer, unmixed
//!                 f32, log-domain fluorescence) and prints a CSV table with
//!                 ratio, encode/decode throughput, and round-trip error.
//! - `auto-pick` — runs the auto picker against each synthetic channel type
//!                 and prints which codec it would have chosen.
//! - `roundtrip` — single-codec round-trip on synthetic data; useful as a
//!                 manual regression while iterating on a codec.
//!
//! Real-FCS-file benchmarks land in M5 once the regression-suite work goes in.

use std::hint::black_box;
use std::time::Instant;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand, ValueEnum};

use flow_fcs_compress::codec::adc_bitpack::AdcBitpack;
use flow_fcs_compress::codec::auto::pick_codec;
use flow_fcs_compress::codec::log_quant::{LogQuantization, LogQuantizationConfig};
use flow_fcs_compress::codec::lossless_f32::{BssZstd, RawNone, RawZstd};
use flow_fcs_compress::codec::lossless_f32_pco::LosslessF32Pco;
use flow_fcs_compress::codec::lz4_baseline::Lz4Block;
use flow_fcs_compress::codec::{ChannelParams, ColumnCodec};

#[derive(Parser, Debug)]
#[command(name = "flow-fcs-bench", version, about = "flow-fcs-compress benchmark harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run every codec against every synthetic channel and print a CSV table.
    Synth {
        /// Events per chunk.
        #[arg(long, default_value_t = 65_536)]
        events: usize,
        /// Repeats per (codec, channel) for throughput averaging.
        #[arg(long, default_value_t = 5)]
        repeats: usize,
    },
    /// Print the codec the auto picker chooses for each synthetic channel.
    AutoPick {
        #[arg(long, default_value_t = 4096)]
        events: usize,
    },
    /// Single-codec round-trip on a single synthetic channel.
    Roundtrip {
        #[arg(long, value_enum)]
        codec: BenchCodec,
        #[arg(long, value_enum, default_value = "adc-22")]
        channel: BenchChannel,
        #[arg(long, default_value_t = 65_536)]
        events: usize,
    },
    /// Run every codec against every channel of a real `.fcs` file.
    File {
        /// Path to a `.fcs` file.
        path: String,
        /// Repeats per (codec, channel) for throughput averaging.
        #[arg(long, default_value_t = 3)]
        repeats: usize,
    },
    /// Whole-file write+read roundtrip via `.fcz`. Reports both serial and
    /// rayon-parallel decode throughput across the entire dataset.
    FileFull {
        /// Path to a `.fcs` file.
        path: String,
        /// Repeats per measurement.
        #[arg(long, default_value_t = 5)]
        repeats: usize,
        /// Events per chunk for the .fcz writer.
        #[arg(long, default_value_t = 65536)]
        chunk_events: u32,
    },
    /// Synthetic whole-dataset roundtrip at chosen size (~MB raw f32). Half
    /// channels ADC-shaped, half unmixed-shaped. Reports serial + parallel decode.
    SynthFull {
        #[arg(long, default_value_t = 80)]
        size_mb: usize,
        #[arg(long, default_value_t = 30)]
        channels: usize,
        #[arg(long, default_value_t = 3)]
        repeats: usize,
        #[arg(long, default_value_t = 65536)]
        chunk_events: u32,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum BenchCodec {
    BssZstd,
    Pco,
    AdcBitpack,
    LogQuantization,
    /// 12-bit LogQuantization — exercises the small-width LUT decode fast path.
    LogQuant12,
    RawZstd,
    Lz4,
    RawNone,
}

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq, Hash)]
enum BenchChannel {
    /// 22-bit ADC, integer-quantized — typical raw spectral channel.
    #[value(name = "adc-22")]
    Adc22,
    /// 18-bit ADC, signed integer — typical post-compensation raw channel.
    #[value(name = "adc-18-signed")]
    Adc18Signed,
    /// Full f32 entropy — typical unmixed/compensated channel.
    Unmixed,
    /// Log-domain fluorescence with negatives, biexp-shaped distribution.
    LogFluorescence,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Synth { events, repeats } => cmd_synth(events, repeats),
        Command::AutoPick { events } => cmd_auto_pick(events),
        Command::Roundtrip {
            codec,
            channel,
            events,
        } => cmd_roundtrip(codec, channel, events),
        Command::File { path, repeats } => cmd_file(path, repeats),
        Command::FileFull {
            path,
            repeats,
            chunk_events,
        } => cmd_file_full(path, repeats, chunk_events),
        Command::SynthFull {
            size_mb,
            channels,
            repeats,
            chunk_events,
        } => cmd_synth_full(size_mb, channels, repeats, chunk_events),
    }
}

fn cmd_synth_full(
    size_mb: usize,
    channels: usize,
    repeats: usize,
    chunk_events: u32,
) -> Result<()> {
    use flow_fcs_compress::codec::CodecId;
    use flow_fcs_compress::container::fcz::{FczReader, FczWriteOptions, FczWriter};

    if repeats == 0 {
        bail!("--repeats must be >= 1");
    }
    if channels == 0 {
        bail!("--channels must be >= 1");
    }
    let target_bytes = size_mb.saturating_mul(1024 * 1024);
    let n_events = target_bytes / (channels * 4);
    let total_input_bytes = (n_events * channels * 4) as u64;
    eprintln!(
        "# synth: {} events × {} channels = {:.1} MB raw f32",
        n_events,
        channels,
        total_input_bytes as f64 / (1024.0 * 1024.0)
    );

    let mut columns: Vec<Vec<f32>> = Vec::with_capacity(channels);
    for ch in 0..channels {
        let kind = if ch % 2 == 0 {
            BenchChannel::Adc22
        } else {
            BenchChannel::Unmixed
        };
        let (data, _) = synthesize(kind, n_events);
        columns.push(data);
    }

    let tmp = std::env::temp_dir().join(format!(
        "flow-fcs-bench-synth-{}-{}.fcz",
        std::process::id(),
        size_mb
    ));

    // Helper: register channels on a fresh writer.
    let register = |writer: &mut FczWriter| -> Result<()> {
        for i in 0..channels {
            let kind = if i % 2 == 0 {
                BenchChannel::Adc22
            } else {
                BenchChannel::Unmixed
            };
            let (_, mut params) = synthesize(kind, 1);
            params.name = format!("ch{}", i);
            let codec = if i % 2 == 0 {
                CodecId::AdcBitpack
            } else {
                CodecId::LosslessF32BssZstd
            };
            writer
                .add_channel(params, codec)
                .map_err(|e| anyhow::anyhow!("add_channel({i}): {e}"))?;
        }
        Ok(())
    };

    // Serial encode (current default path).
    let mut encode_serial_total = 0u128;
    let mut compressed_size = 0u64;
    for _ in 0..repeats {
        let _ = std::fs::remove_file(&tmp);
        let opts = FczWriteOptions {
            events_per_chunk: chunk_events,
        };
        let t = Instant::now();
        let mut writer = FczWriter::create(&tmp, opts)
            .map_err(|e| anyhow::anyhow!("FczWriter::create: {e}"))?;
        register(&mut writer)?;
        let mut chunk_idx = 0u32;
        let mut start = 0usize;
        while start < n_events {
            let end = (start + chunk_events as usize).min(n_events);
            for (i, col) in columns.iter().enumerate() {
                writer
                    .write_chunk(i as u16, chunk_idx, &col[start..end])
                    .map_err(|e| anyhow::anyhow!("write_chunk({i},{chunk_idx}): {e}"))?;
            }
            start = end;
            chunk_idx += 1;
        }
        writer.finish().map_err(|e| anyhow::anyhow!("finish: {e}"))?;
        encode_serial_total += t.elapsed().as_nanos();
        compressed_size = std::fs::metadata(&tmp)?.len();
    }
    let encode_serial_ns = (encode_serial_total / repeats as u128).max(1) as f64;
    let encode_serial_mb_s =
        (total_input_bytes as f64) / encode_serial_ns * 1e9 / (1024.0 * 1024.0);

    // Parallel encode: par_iter over (channel, chunk) tasks producing
    // EncodedChunk payloads, then drain serially into the writer's buffer.
    use rayon::prelude::*;
    let n_chunks = n_events.div_ceil(chunk_events as usize);
    let mut encode_par_total = 0u128;
    for _ in 0..repeats {
        let _ = std::fs::remove_file(&tmp);
        let opts = FczWriteOptions {
            events_per_chunk: chunk_events,
        };
        let t = Instant::now();
        let mut writer = FczWriter::create(&tmp, opts)
            .map_err(|e| anyhow::anyhow!("FczWriter::create: {e}"))?;
        register(&mut writer)?;
        // Build all (channel, chunk) tasks.
        let tasks: Vec<(usize, u32, std::ops::Range<usize>)> = (0..n_chunks)
            .flat_map(|c| {
                (0..channels).map(move |i| {
                    let start = c * chunk_events as usize;
                    let end = (start + chunk_events as usize).min(n_events);
                    (i, c as u32, start..end)
                })
            })
            .collect();
        // Parallel encode.
        let writer_ref = &writer;
        let encoded: Vec<(u32, _)> = tasks
            .par_iter()
            .map(|(i, c, r)| {
                let payload = writer_ref
                    .encode_chunk_payload(*i as u16, &columns[*i][r.clone()])
                    .map_err(|e| anyhow::anyhow!("encode_chunk_payload: {e}"))?;
                Ok::<_, anyhow::Error>((*c, payload))
            })
            .collect::<Result<Vec<_>>>()?;
        // Serial append (mutates writer state).
        for (chunk_idx, payload) in encoded {
            writer
                .append_encoded_chunk(chunk_idx, payload)
                .map_err(|e| anyhow::anyhow!("append_encoded_chunk: {e}"))?;
        }
        writer.finish().map_err(|e| anyhow::anyhow!("finish: {e}"))?;
        encode_par_total += t.elapsed().as_nanos();
    }
    let encode_par_ns = (encode_par_total / repeats as u128).max(1) as f64;
    let encode_par_mb_s =
        (total_input_bytes as f64) / encode_par_ns * 1e9 / (1024.0 * 1024.0);

    let mut decode_serial_total = 0u128;
    for _ in 0..repeats {
        let r = FczReader::open(&tmp)
            .map_err(|e| anyhow::anyhow!("FczReader::open: {e}"))?;
        let t = Instant::now();
        for ch in 0..r.n_channels() {
            let v = r
                .read_full_channel(ch)
                .map_err(|e| anyhow::anyhow!("read_full_channel({ch}): {e}"))?;
            std::hint::black_box(v);
        }
        decode_serial_total += t.elapsed().as_nanos();
    }
    let decode_serial_ns = (decode_serial_total / repeats as u128).max(1) as f64;
    let decode_serial_mb_s =
        (total_input_bytes as f64) / decode_serial_ns * 1e9 / (1024.0 * 1024.0);

    let mut decode_par_total = 0u128;
    for _ in 0..repeats {
        let r = FczReader::open(&tmp)
            .map_err(|e| anyhow::anyhow!("FczReader::open: {e}"))?;
        let n_ch = r.n_channels();
        let total_events = r.total_events() as usize;
        let mut buffers: Vec<Vec<f32>> = (0..n_ch).map(|_| vec![0.0f32; total_events]).collect();
        let t = Instant::now();
        r.decode_all_par(&mut buffers)
            .map_err(|e| anyhow::anyhow!("decode_all_par: {e}"))?;
        decode_par_total += t.elapsed().as_nanos();
        std::hint::black_box(buffers);
    }
    let decode_par_ns = (decode_par_total / repeats as u128).max(1) as f64;
    let decode_par_mb_s =
        (total_input_bytes as f64) / decode_par_ns * 1e9 / (1024.0 * 1024.0);

    let _ = std::fs::remove_file(&tmp);

    println!(
        "size_mb,events,channels,raw_bytes,compressed_bytes,ratio,encode_serial_mb_s,encode_parallel_mb_s,encode_speedup,decode_serial_mb_s,decode_parallel_mb_s,decode_speedup"
    );
    println!(
        "{},{},{},{},{},{:.3},{:.1},{:.1},{:.2},{:.1},{:.1},{:.2}",
        size_mb,
        n_events,
        channels,
        total_input_bytes,
        compressed_size,
        total_input_bytes as f64 / compressed_size.max(1) as f64,
        encode_serial_mb_s,
        encode_par_mb_s,
        encode_par_mb_s / encode_serial_mb_s.max(1.0),
        decode_serial_mb_s,
        decode_par_mb_s,
        decode_par_mb_s / decode_serial_mb_s.max(1.0),
    );
    Ok(())
}

fn cmd_file_full(path: String, repeats: usize, chunk_events: u32) -> Result<()> {
    use flow_fcs::Fcs;
    use flow_fcs_compress::container::fcz::{FczReader, FczWriteOptions};

    if repeats == 0 {
        bail!("--repeats must be >= 1");
    }
    let fcs = Fcs::open(&path).map_err(|e| anyhow::anyhow!("open {path}: {e}"))?;
    let n_events = fcs.data_frame.height();
    let n_params = fcs.data_frame.width();
    let total_input_bytes = (n_events * n_params * 4) as u64;

    eprintln!(
        "# loaded {} events × {} parameters from {}",
        n_events, n_params, path
    );
    eprintln!(
        "# raw f32 footprint: {} bytes ({:.1} MB)",
        total_input_bytes,
        total_input_bytes as f64 / (1024.0 * 1024.0)
    );

    // Write once, time it.
    let tmp = std::env::temp_dir().join(format!("flow-fcs-bench-{}.fcz", std::process::id()));
    let mut encode_total = 0u128;
    let mut compressed_size = 0u64;
    for _ in 0..repeats {
        let _ = std::fs::remove_file(&tmp);
        let opts = FczWriteOptions {
            events_per_chunk: chunk_events,
        };
        let t = Instant::now();
        fcs.write_fcz(&tmp, opts)
            .map_err(|e| anyhow::anyhow!("write_fcz: {e}"))?;
        encode_total += t.elapsed().as_nanos();
        compressed_size = std::fs::metadata(&tmp)?.len();
    }
    let encode_avg_ns = (encode_total / repeats as u128).max(1) as f64;
    let encode_mb_s = (total_input_bytes as f64) / encode_avg_ns * 1e9 / (1024.0 * 1024.0);

    // Serial decode via Fcs::events_from_fcz (per-channel sequential read).
    let mut decode_serial_total = 0u128;
    for _ in 0..repeats {
        let t = Instant::now();
        let df = Fcs::events_from_fcz(&tmp)
            .map_err(|e| anyhow::anyhow!("events_from_fcz: {e}"))?;
        decode_serial_total += t.elapsed().as_nanos();
        std::hint::black_box(df);
    }
    let decode_serial_ns = (decode_serial_total / repeats as u128).max(1) as f64;
    let decode_serial_mb_s =
        (total_input_bytes as f64) / decode_serial_ns * 1e9 / (1024.0 * 1024.0);

    // Parallel decode via FczReader::decode_all_par (rayon, per (channel, chunk) tasks).
    let mut decode_par_total = 0u128;
    for _ in 0..repeats {
        let reader = FczReader::open(&tmp)
            .map_err(|e| anyhow::anyhow!("FczReader::open: {e}"))?;
        let n_ch = reader.n_channels();
        let total_events = reader.total_events() as usize;
        let mut buffers: Vec<Vec<f32>> = (0..n_ch).map(|_| vec![0.0f32; total_events]).collect();
        let t = Instant::now();
        reader
            .decode_all_par(&mut buffers)
            .map_err(|e| anyhow::anyhow!("decode_all_par: {e}"))?;
        decode_par_total += t.elapsed().as_nanos();
        std::hint::black_box(buffers);
    }
    let decode_par_ns = (decode_par_total / repeats as u128).max(1) as f64;
    let decode_par_mb_s =
        (total_input_bytes as f64) / decode_par_ns * 1e9 / (1024.0 * 1024.0);

    let _ = std::fs::remove_file(&tmp);

    println!(
        "events,parameters,raw_bytes,compressed_bytes,ratio,encode_mb_s,decode_serial_mb_s,decode_parallel_mb_s,parallel_speedup"
    );
    println!(
        "{},{},{},{},{:.3},{:.1},{:.1},{:.1},{:.2}",
        n_events,
        n_params,
        total_input_bytes,
        compressed_size,
        total_input_bytes as f64 / compressed_size.max(1) as f64,
        encode_mb_s,
        decode_serial_mb_s,
        decode_par_mb_s,
        decode_par_mb_s / decode_serial_mb_s.max(1.0),
    );
    Ok(())
}

fn cmd_file(path: String, repeats: usize) -> Result<()> {
    use flow_fcs::Fcs;

    if repeats == 0 {
        bail!("--repeats must be >= 1");
    }
    let fcs = Fcs::open(&path).map_err(|e| anyhow::anyhow!("open {path}: {e}"))?;
    eprintln!(
        "# loaded {} events × {} parameters from {}",
        fcs.data_frame.height(),
        fcs.data_frame.width(),
        path
    );

    println!(
        "codec,channel,events,bytes_in,bytes_out,ratio,encode_mb_s,decode_mb_s,max_abs_err,max_rel_err,picked_by_auto"
    );

    for name in fcs.get_parameter_names_from_dataframe() {
        let input = fcs.get_parameter_events_slice(&name)?;
        let param_num = fcs
            .get_parameter_names_from_dataframe()
            .iter()
            .position(|n| n == &name)
            .map(|i| i + 1)
            .unwrap_or(1);
        let stored_bits = fcs
            .metadata
            .get_bytes_per_parameter(param_num)
            .map(|b| (b.saturating_mul(8)).min(255) as u8)
            .unwrap_or(32);
        let range = fcs
            .metadata
            .get_parameter_numeric_metadata(param_num, "R")
            .ok()
            .and_then(|kw| match kw {
                flow_fcs::keyword::IntegerKeyword::PnR(v) => {
                    Some((*v).min(u32::MAX as usize) as u32)
                }
                _ => None,
            })
            .unwrap_or(262_144);
        // Promote stored_bits to ADC bits when stored_bits ≤ 24 — typical of
        // FCS files where $PnB matches the ADC depth.
        let adc_bits = if (1..=32).contains(&stored_bits) {
            Some(stored_bits)
        } else {
            None
        };
        let params = ChannelParams {
            name: name.clone(),
            stored_bits,
            range,
            log_decades: (0.0, 0.0),
            adc_bits,
            signed: input.iter().any(|&x| x < 0.0),
        };

        // Use first chunk-sized window of the channel to keep the bench bounded.
        let window = input.len().min(65_536);
        let slice = &input[..window];
        let picked = pick_codec(slice, &params);

        for &codec in BenchCodec::all() {
            let result = match run_codec(codec, slice, &params, repeats) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("# {codec:?} on {name}: skipped ({e})");
                    continue;
                }
            };
            let picked_by_auto = matches!(
                (codec, picked),
                (BenchCodec::BssZstd, flow_fcs_compress::CodecId::LosslessF32BssZstd)
                    | (BenchCodec::AdcBitpack, flow_fcs_compress::CodecId::AdcBitpack)
            );
            println!(
                "{:?},{},{},{},{},{:.3},{:.1},{:.1},{:.4e},{:.4e},{}",
                codec,
                name,
                window,
                window * 4,
                result.output_bytes,
                result.ratio,
                result.encode_mb_s,
                result.decode_mb_s,
                result.max_abs_err,
                result.max_rel_err,
                picked_by_auto,
            );
        }
    }
    Ok(())
}

fn cmd_synth(events: usize, repeats: usize) -> Result<()> {
    if repeats == 0 {
        bail!("--repeats must be >= 1");
    }
    println!("codec,channel,events,ratio,encode_mb_s,decode_mb_s,max_abs_err,max_rel_err");

    for &ch in BenchChannel::all() {
        let (input, params) = synthesize(ch, events);
        for &codec in BenchCodec::all() {
            let result = match run_codec(codec, &input, &params, repeats) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("# {codec:?} on {ch:?}: skipped ({e})");
                    continue;
                }
            };
            println!(
                "{:?},{:?},{},{:.3},{:.1},{:.1},{:.4e},{:.4e}",
                codec,
                ch,
                events,
                result.ratio,
                result.encode_mb_s,
                result.decode_mb_s,
                result.max_abs_err,
                result.max_rel_err,
            );
        }
    }
    Ok(())
}

fn cmd_auto_pick(events: usize) -> Result<()> {
    println!("channel,picked_codec");
    for &ch in BenchChannel::all() {
        let (input, params) = synthesize(ch, events);
        let picked = pick_codec(&input, &params);
        println!("{:?},{:?}", ch, picked);
    }
    Ok(())
}

fn cmd_roundtrip(codec: BenchCodec, channel: BenchChannel, events: usize) -> Result<()> {
    let (input, params) = synthesize(channel, events);
    let result = run_codec(codec, &input, &params, 1)?;
    println!("codec        : {codec:?}");
    println!("channel      : {channel:?}");
    println!("events       : {events}");
    println!("input bytes  : {}", input.len() * 4);
    println!("output bytes : {}", result.output_bytes);
    println!("ratio        : {:.3}x", result.ratio);
    println!("encode MB/s  : {:.1}", result.encode_mb_s);
    println!("decode MB/s  : {:.1}", result.decode_mb_s);
    println!("max abs err  : {:.4e}", result.max_abs_err);
    println!("max rel err  : {:.4e}", result.max_rel_err);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct RunResult {
    output_bytes: usize,
    ratio: f64,
    encode_mb_s: f64,
    decode_mb_s: f64,
    max_abs_err: f32,
    max_rel_err: f32,
}

fn run_codec(
    codec: BenchCodec,
    input: &[f32],
    params: &ChannelParams,
    repeats: usize,
) -> Result<RunResult> {
    let codec_box = make_codec(codec);
    let input_bytes = input.len() * 4;

    // Encode (timed, repeated).
    let mut last_payload = Vec::new();
    let mut encode_total = 0u128;
    for _ in 0..repeats {
        last_payload.clear();
        let t = Instant::now();
        codec_box
            .encode_chunk(input, params, &mut last_payload)
            .map_err(|e| anyhow::anyhow!("encode failed: {e}"))?;
        encode_total += t.elapsed().as_nanos();
    }
    let encode_avg_ns = (encode_total / repeats as u128).max(1) as f64;
    let encode_mb_s = (input_bytes as f64) / encode_avg_ns * 1e9 / (1024.0 * 1024.0);

    // Decode (timed, repeated).
    let mut decoded = vec![0.0f32; input.len()];
    let mut decode_total = 0u128;
    for _ in 0..repeats {
        let t = Instant::now();
        codec_box
            .decode_chunk(&last_payload, params, &mut decoded)
            .map_err(|e| anyhow::anyhow!("decode failed: {e}"))?;
        decode_total += t.elapsed().as_nanos();
        black_box(&decoded);
    }
    let decode_avg_ns = (decode_total / repeats as u128).max(1) as f64;
    let decode_mb_s = (input_bytes as f64) / decode_avg_ns * 1e9 / (1024.0 * 1024.0);

    // Round-trip error metrics.
    let mut max_abs = 0f32;
    let mut max_rel = 0f32;
    for (a, b) in input.iter().zip(decoded.iter()) {
        let err = (a - b).abs();
        max_abs = max_abs.max(err);
        if a.abs() > 1e-3 {
            max_rel = max_rel.max(err / a.abs());
        }
    }

    Ok(RunResult {
        output_bytes: last_payload.len(),
        ratio: input_bytes as f64 / last_payload.len().max(1) as f64,
        encode_mb_s,
        decode_mb_s,
        max_abs_err: max_abs,
        max_rel_err: max_rel,
    })
}

fn make_codec(c: BenchCodec) -> Box<dyn ColumnCodec> {
    match c {
        BenchCodec::BssZstd => Box::new(BssZstd::default()),
        BenchCodec::Pco => Box::new(LosslessF32Pco::default()),
        BenchCodec::AdcBitpack => Box::new(AdcBitpack),
        BenchCodec::LogQuantization => Box::new(LogQuantization::new(LogQuantizationConfig {
            cofactor: 150.0,
            bits: 16,
        })),
        BenchCodec::LogQuant12 => Box::new(LogQuantization::new(LogQuantizationConfig {
            cofactor: 150.0,
            bits: 12,
        })),
        BenchCodec::RawZstd => Box::new(RawZstd::default()),
        BenchCodec::Lz4 => Box::new(Lz4Block),
        BenchCodec::RawNone => Box::new(RawNone),
    }
}

impl BenchCodec {
    fn all() -> &'static [BenchCodec] {
        &[
            BenchCodec::BssZstd,
            BenchCodec::Pco,
            BenchCodec::AdcBitpack,
            BenchCodec::LogQuantization,
            BenchCodec::LogQuant12,
            BenchCodec::RawZstd,
            BenchCodec::Lz4,
            BenchCodec::RawNone,
        ]
    }
}

impl BenchChannel {
    fn all() -> &'static [BenchChannel] {
        &[
            BenchChannel::Adc22,
            BenchChannel::Adc18Signed,
            BenchChannel::Unmixed,
            BenchChannel::LogFluorescence,
        ]
    }
}

fn synthesize(channel: BenchChannel, n: usize) -> (Vec<f32>, ChannelParams) {
    match channel {
        BenchChannel::Adc22 => {
            let bits = 22u8;
            let range = 1u32 << bits;
            let scale = range as f64 / (1u64 << bits) as f64; // 1.0
            let mut s = 0xC0FFEE_u64;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                s = lcg(s);
                let q = (s as u64) % (1u64 << bits);
                v.push((q as f64 * scale) as f32);
            }
            (
                v,
                ChannelParams {
                    name: "raw-22".into(),
                    stored_bits: 32,
                    range,
                    log_decades: (0.0, 0.0),
                    adc_bits: Some(bits),
                    signed: false,
                },
            )
        }
        BenchChannel::Adc18Signed => {
            let bits = 18u8;
            let range = 1u32 << bits;
            let scale = range as f64 / (1u64 << bits) as f64;
            let mut s = 0xBADBEEF_u64;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                s = lcg(s);
                let q = ((s as i64) % (1i64 << bits)) - (1i64 << (bits - 1));
                v.push((q as f64 * scale) as f32);
            }
            (
                v,
                ChannelParams {
                    name: "raw-18-signed".into(),
                    stored_bits: 32,
                    range,
                    log_decades: (0.0, 0.0),
                    adc_bits: Some(bits),
                    signed: true,
                },
            )
        }
        BenchChannel::Unmixed => {
            // Full-mantissa f32 spread, mimicking unmixed channel output.
            let mut s = 0xDEADC0DE_u64;
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                s = lcg(s);
                let u = ((s >> 32) as u32) as f32 / u32::MAX as f32;
                v.push((i as f32) * 0.123_456 + (u - 0.5) * 100.0);
            }
            (
                v,
                ChannelParams {
                    name: "unmixed".into(),
                    stored_bits: 32,
                    range: 262_144,
                    log_decades: (0.0, 0.0),
                    adc_bits: None,
                    signed: true,
                },
            )
        }
        BenchChannel::LogFluorescence => {
            let mut s = 0xFEEDFACE_u64;
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                s = lcg(s);
                let u = ((s >> 32) as u32) as f32 / u32::MAX as f32;
                let base = if i % 7 == 0 {
                    (u - 0.5) * 50.0
                } else {
                    10f32.powf(u * 5.0)
                };
                v.push(base);
            }
            (
                v,
                ChannelParams {
                    name: "log-fluo".into(),
                    stored_bits: 32,
                    range: 262_144,
                    log_decades: (5.0, 0.0),
                    adc_bits: None,
                    signed: true,
                },
            )
        }
    }
}

#[inline]
fn lcg(s: u64) -> u64 {
    s.wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
}
