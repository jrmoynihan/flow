//! Time primitive kernels and write `docs/dev/PERF_HOST.md`.
//!
//! ```bash
//! cargo run -p flow-perf-calibrate --release --example snapshot_host
//! ```
//!
//! This is a single pass, not an A/B. Do not treat session drift as a regression
//! (see beads memory `benchmark-a-b-on-this-machine-apple-m5`).

use anyhow::{Context, Result};
use flow_perf_calibrate::{
    F32_SCAN_BYTES, GATHER_F32_ELEMS, GATHER_PROBES, MEMCPY_LARGE, MEMCPY_SMALL, WIDTH_ELEMS,
    filled_bytes, filled_f32, filled_f64, filled_u16, gather_sum_f32, hashmap_from_keys, hashmap_sum,
    memcpy_bytes, n_f32, random_indices, rayon_scale_sum, seq_scale_sum, sequential_indices,
    slice_sum_f32, sort_f32_clone, sum_f32, sum_f64, sum_u16, vec_push_f32,
};
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

struct Row {
    group: &'static str,
    name: String,
    median: Duration,
    extra: String,
}

fn median_time<T, F: FnMut() -> T>(mut body: F, inner: u32, samples: usize) -> Duration {
    for _ in 0..3 {
        black_box(body());
    }
    let mut times = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        for _ in 0..inner {
            black_box(body());
        }
        times.push(start.elapsed() / inner);
    }
    times.sort();
    times[samples / 2]
}

fn fmt_duration(d: Duration) -> String {
    let ns = d.as_nanos();
    if ns >= 1_000_000_000 {
        format!("{:.3} s", d.as_secs_f64())
    } else if ns >= 1_000_000 {
        format!("{:.3} ms", d.as_secs_f64() * 1e3)
    } else if ns >= 1_000 {
        format!("{:.1} µs", d.as_secs_f64() * 1e6)
    } else {
        format!("{ns} ns")
    }
}

fn gb_s(bytes: usize, d: Duration) -> String {
    let s = d.as_secs_f64().max(1e-12);
    format!("{:.1} GB/s", (bytes as f64 / s) / 1e9)
}

fn ns_each(count: usize, d: Duration) -> String {
    let ns = d.as_secs_f64() * 1e9 / (count as f64).max(1.0);
    format!("{ns:.2} ns/elem")
}

fn sysctl(key: &str) -> Option<String> {
    let out = Command::new("sysctl").arg("-n").arg(key).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn rustc_short() -> String {
    let out = Command::new("rustc").arg("-vV").output();
    match out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            let hash = text
                .lines()
                .find_map(|l| l.strip_prefix("commit-hash: "))
                .map(|h| &h[..h.len().min(9)])
                .unwrap_or("unknown");
            let ver = text
                .lines()
                .find_map(|l| l.strip_prefix("release: "))
                .unwrap_or("unknown");
            format!("{ver} ({hash})")
        }
        Err(_) => "unknown".into(),
    }
}

fn host_docs_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/dev/PERF_HOST.md")
}

fn main() -> Result<()> {
    let mut rows: Vec<Row> = Vec::new();

    for &bytes in &F32_SCAN_BYTES {
        let n = n_f32(bytes);
        let data = filled_f32(n, 1);
        let inner = if bytes >= 64 * 1024 * 1024 { 1 } else { 8 };
        let d = median_time(|| black_box(sum_f32(&data)), inner, 11);
        rows.push(Row {
            group: "seq_sum_f32",
            name: format!("{bytes} B ({n} f32)"),
            median: d,
            extra: format!("{}; {}", ns_each(n, d), gb_s(bytes, d)),
        });
    }

    let n = WIDTH_ELEMS;
    let u16s = filled_u16(n, 2);
    let f32s = filled_f32(n, 2);
    let f64s = filled_f64(n, 2);
    let d_u16 = median_time(|| black_box(sum_u16(&u16s)), 4, 11);
    let d_f32 = median_time(|| black_box(sum_f32(&f32s)), 4, 11);
    let d_f64 = median_time(|| black_box(sum_f64(&f64s)), 4, 11);
    rows.push(Row {
        group: "width_scan",
        name: format!("{n} × u16"),
        median: d_u16,
        extra: format!("{}; {}", ns_each(n, d_u16), gb_s(n * 2, d_u16)),
    });
    rows.push(Row {
        group: "width_scan",
        name: format!("{n} × f32"),
        median: d_f32,
        extra: format!("{}; {}", ns_each(n, d_f32), gb_s(n * 4, d_f32)),
    });
    rows.push(Row {
        group: "width_scan",
        name: format!("{n} × f64"),
        median: d_f64,
        extra: format!("{}; {}", ns_each(n, d_f64), gb_s(n * 8, d_f64)),
    });

    let buf = filled_f32(GATHER_F32_ELEMS, 3);
    let seq = sequential_indices(GATHER_PROBES.min(GATHER_F32_ELEMS));
    let rnd = random_indices(GATHER_F32_ELEMS, GATHER_PROBES, 4);
    let d_seq = median_time(|| black_box(gather_sum_f32(&buf, &seq)), 2, 11);
    let d_rnd = median_time(|| black_box(gather_sum_f32(&buf, &rnd)), 1, 11);
    rows.push(Row {
        group: "gather_64mib",
        name: format!("sequential {} probes", seq.len()),
        median: d_seq,
        extra: ns_each(seq.len(), d_seq),
    });
    rows.push(Row {
        group: "gather_64mib",
        name: format!("random {} probes", rnd.len()),
        median: d_rnd,
        extra: ns_each(rnd.len(), d_rnd),
    });

    for &nbytes in &[MEMCPY_SMALL, MEMCPY_LARGE] {
        let src = filled_bytes(nbytes, 5);
        let mut dst = vec![0u8; nbytes];
        let d = median_time(
            || {
                memcpy_bytes(&mut dst, &src);
                black_box(&dst);
            },
            if nbytes > 8 * 1024 * 1024 { 2 } else { 8 },
            11,
        );
        rows.push(Row {
            group: "memcpy",
            name: format!("{nbytes} B"),
            median: d,
            extra: gb_s(nbytes, d),
        });
    }

    for n in [10_000usize, 100_000] {
        let d_cap = median_time(|| black_box(vec_push_f32(n, true)), 8, 11);
        let d_grow = median_time(|| black_box(vec_push_f32(n, false)), 8, 11);
        rows.push(Row {
            group: "vec_push_f32",
            name: format!("{n} with_capacity"),
            median: d_cap,
            extra: ns_each(n, d_cap),
        });
        rows.push(Row {
            group: "vec_push_f32",
            name: format!("{n} grow"),
            median: d_grow,
            extra: ns_each(n, d_grow),
        });
    }

    for n in [1_000usize, 100_000] {
        let (map, keys) = hashmap_from_keys(n);
        let slice: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let d_h = median_time(|| black_box(hashmap_sum(&map, &keys)), 4, 11);
        let d_s = median_time(|| black_box(slice_sum_f32(&slice)), 8, 11);
        rows.push(Row {
            group: "lookup",
            name: format!("HashMap get n={n}"),
            median: d_h,
            extra: ns_each(n, d_h),
        });
        rows.push(Row {
            group: "lookup",
            name: format!("slice index n={n}"),
            median: d_s,
            extra: ns_each(n, d_s),
        });
    }

    for n in [10_000usize, 100_000] {
        let data = filled_f32(n, 6);
        let d = median_time(|| black_box(sort_f32_clone(data.clone())), 4, 11);
        rows.push(Row {
            group: "sort_unstable_f32",
            name: format!("{n} elems"),
            median: d,
            extra: ns_each(n, d),
        });
    }

    for n in [256usize, 10_000] {
        let data = filled_f32(n, 7);
        let d_seq = median_time(|| black_box(seq_scale_sum(&data)), 32, 11);
        let d_par = median_time(|| black_box(rayon_scale_sum(&data)), 8, 11);
        rows.push(Row {
            group: "rayon_scale",
            name: format!("seq n={n}"),
            median: d_seq,
            extra: ns_each(n, d_seq),
        });
        rows.push(Row {
            group: "rayon_scale",
            name: format!("par n={n}"),
            median: d_par,
            extra: ns_each(n, d_par),
        });
    }

    let date = chrono_like_date();
    let brand = sysctl("machdep.cpu.brand_string").unwrap_or_else(|| "unknown".into());
    let ncpu = sysctl("hw.ncpu").unwrap_or_else(|| "?".into());
    let line = sysctl("hw.cachelinesize").unwrap_or_else(|| "?".into());
    let p_l1d = sysctl("hw.perflevel0.l1dcachesize").unwrap_or_else(|| "?".into());
    let p_l2 = sysctl("hw.perflevel0.l2cachesize").unwrap_or_else(|| "?".into());
    let e_l1d = sysctl("hw.perflevel1.l1dcachesize").unwrap_or_else(|| "?".into());
    let e_l2 = sysctl("hw.perflevel1.l2cachesize").unwrap_or_else(|| "?".into());
    let mem = sysctl("hw.memsize").unwrap_or_else(|| "?".into());
    let rustc = rustc_short();

    let mut md = String::new();
    md.push_str("# Host primitive snapshot\n\n");
    md.push_str("Generated by `cargo run -p flow-perf-calibrate --release --example snapshot_host`.\n");
    md.push_str("Single pass (not an A/B). Order-of-magnitude floors stay in [`PERF_LATENCIES.md`](PERF_LATENCIES.md).\n\n");
    md.push_str("## Provenance\n\n");
    md.push_str("| Field | Value |\n|-------|-------|\n");
    md.push_str(&format!("| Date | {date} |\n"));
    md.push_str(&format!("| Machine | {brand} |\n"));
    md.push_str(&format!("| Logical CPUs | {ncpu} |\n"));
    md.push_str(&format!("| Cache line | {line} B |\n"));
    md.push_str(&format!("| P-core L1D / L2 | {p_l1d} / {p_l2} B |\n"));
    md.push_str(&format!("| E-core L1D / L2 | {e_l1d} / {e_l2} B |\n"));
    md.push_str(&format!("| DRAM (`hw.memsize`) | {mem} B |\n"));
    md.push_str(&format!("| rustc | {rustc} |\n"));
    md.push_str("| RUSTFLAGS | *(unset unless you exported them)* |\n");
    md.push_str("| Features | default (CPU only) |\n\n");
    md.push_str("Medians of 11 samples after 3 warmups. Times are wall time for the kernel only (setup excluded).\n\n");
    md.push_str("`seq_sum_*` is a scalar `black_box` add per element (not SIMD). Use `memcpy` for a bulk-move roofline. `rayon_scale` at n=256 is the pool-wake tax on this host.\n\n");
    md.push_str("| Group | Case | Median | Notes |\n|-------|------|--------|-------|\n");
    for r in &rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            r.group,
            r.name,
            fmt_duration(r.median),
            r.extra
        ));
    }
    md.push('\n');

    let path = host_docs_path();
    std::fs::write(&path, &md).with_context(|| format!("write {}", path.display()))?;
    print!("{md}");
    eprintln!("wrote {}", path.canonicalize().unwrap_or(path).display());
    Ok(())
}

fn chrono_like_date() -> String {
    // Local calendar date without chrono::Local (RUSTSEC-2020-0159).
    let out = Command::new("date").arg("+%Y-%m-%d").output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".into(),
    }
}
