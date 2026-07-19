//! Golden-file parity tests for R's `stats::smooth.spline(spar=…)`.
//!
//! Fixtures in `tests/data/spline_r_n*.txt` were generated with R 4.x:
//! `smooth.spline(seq_len(n), y, spar=0.5)$y`.

use peacoqc_rs::stats::spline::smooth_spline;
use std::fs;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn parse_vector_line(line: &str, prefix: &str) -> Vec<f64> {
    let rest = line
        .strip_prefix(prefix)
        .unwrap_or_else(|| panic!("expected prefix {prefix} in line: {line}"));
    rest.split(',')
        .map(|s| {
            s.trim()
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("bad f64 in {prefix}: {s}"))
        })
        .collect()
}

struct GoldenCase {
    spar: f64,
    y: Vec<f64>,
    r: Vec<f64>,
    y2: Vec<f64>,
    r2: Vec<f64>,
}

fn load_golden(name: &str) -> GoldenCase {
    let text = fs::read_to_string(fixture_path(name)).expect("read golden fixture");
    let mut lines = text.lines();
    let header = lines.next().expect("header");
    let mut n = 0usize;
    let mut spar = 0.5f64;
    for part in header.split_whitespace() {
        if let Some(v) = part.strip_prefix("n=") {
            n = v.parse().expect("n");
        } else if let Some(v) = part.strip_prefix("spar=") {
            spar = v.parse().expect("spar");
        }
    }
    let y = parse_vector_line(lines.next().expect("y"), "y\t");
    let r = parse_vector_line(lines.next().expect("r"), "r\t");
    let y2 = parse_vector_line(lines.next().expect("y2"), "y2\t");
    let r2 = parse_vector_line(lines.next().expect("r2"), "r2\t");
    assert_eq!(y.len(), n);
    assert_eq!(r.len(), n);
    assert_eq!(y2.len(), n);
    assert_eq!(r2.len(), n);
    GoldenCase {
        spar,
        y,
        r,
        y2,
        r2,
    }
}

fn max_abs_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f64, f64::max)
}

fn mean_abs_diff(a: &[f64], b: &[f64]) -> f64 {
    let s: f64 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum();
    s / a.len() as f64
}

fn assert_matches_r(label: &str, y: &[f64], r_ref: &[f64], spar: f64) {
    let x: Vec<f64> = (1..=y.len()).map(|i| i as f64).collect();
    let got = smooth_spline(&x, y, spar).expect("smooth_spline");
    let max_d = max_abs_diff(&got, r_ref);
    let mean_d = mean_abs_diff(&got, r_ref);
    assert!(
        max_d < 1e-3,
        "{label}: max|Rust-R|={max_d:.6e} mean={mean_d:.6e} (want < 1e-3)"
    );
}

#[test]
fn smooth_spline_matches_r_golden_n30() {
    let g = load_golden("spline_r_n30.txt");
    assert_matches_r("n30/linear", &g.y, &g.r, g.spar);
    assert_matches_r("n30/flowlike", &g.y2, &g.r2, g.spar);
}

#[test]
fn smooth_spline_matches_r_golden_n100() {
    let g = load_golden("spline_r_n100.txt");
    assert_matches_r("n100/linear", &g.y, &g.r, g.spar);
    assert_matches_r("n100/flowlike", &g.y2, &g.r2, g.spar);
}

#[test]
fn smooth_spline_matches_r_golden_n520() {
    let g = load_golden("spline_r_n520.txt");
    assert_matches_r("n520/linear", &g.y, &g.r, g.spar);
    assert_matches_r("n520/flowlike", &g.y2, &g.r2, g.spar);
}
