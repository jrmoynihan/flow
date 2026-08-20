//! Primitive kernels for host calibration. Criterion and `snapshot_host` share these.
//!
//! This crate is unpublished (`publish = false`). It exists to fill
//! `docs/dev/PERF_HOST.md`.

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use rayon::prelude::*;
use std::collections::HashMap;
use std::hint::black_box;

/// Sequential `f32` working sets: 32 KiB, 256 KiB, 8 MiB, 256 MiB.
pub const F32_SCAN_BYTES: [usize; 4] = [
    32 * 1024,
    256 * 1024,
    8 * 1024 * 1024,
    256 * 1024 * 1024,
];

/// Matched element count for `u16` / `f32` / `f64` width comparison (8,388,608 values).
pub const WIDTH_ELEMS: usize = 8_388_608;

/// Random vs sequential gather buffer (64 MiB of `f32`).
pub const GATHER_F32_ELEMS: usize = (64 * 1024 * 1024) / 4;

/// How many gather probes to issue into the 64 MiB buffer.
pub const GATHER_PROBES: usize = 1_048_576;

pub const MEMCPY_SMALL: usize = 1024 * 1024;
pub const MEMCPY_LARGE: usize = 64 * 1024 * 1024;

pub fn n_f32(bytes: usize) -> usize {
    bytes / 4
}

pub fn filled_f32(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    if n > 1_048_576 {
        return (0..n).map(|i| ((i as u32).wrapping_mul(0x9e37_79b9) >> 16) as f32 * 1e-4).collect();
    }
    (0..n).map(|_| rng.random_range(0.5f32..1.5)).collect()
}

pub fn filled_u16(n: usize, seed: u64) -> Vec<u16> {
    let mut rng = StdRng::seed_from_u64(seed);
    if n > 1_048_576 {
        return (0..n).map(|i| (i as u16).wrapping_mul(13).wrapping_add(7)).collect();
    }
    (0..n).map(|_| rng.random_range(1u16..1000)).collect()
}

pub fn filled_f64(n: usize, seed: u64) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    if n > 1_048_576 {
        return (0..n)
            .map(|i| ((i as u32).wrapping_mul(0x9e37_79b9) >> 16) as f64 * 1e-4)
            .collect();
    }
    (0..n).map(|_| rng.random_range(0.5f64..1.5)).collect()
}

pub fn filled_bytes(n: usize, seed: u64) -> Vec<u8> {
    if n > 1_048_576 {
        return (0..n).map(|i| i as u8).collect();
    }
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n).map(|_| rng.random_range(0u8..=255)).collect()
}

pub fn sequential_indices(n: usize) -> Vec<usize> {
    (0..n).collect()
}

pub fn random_indices(n_buf: usize, n_probe: usize, seed: u64) -> Vec<usize> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n_probe)
        .map(|_| rng.random_range(0..n_buf))
        .collect()
}

#[inline]
pub fn sum_f32(data: &[f32]) -> f32 {
    let mut acc = 0.0f32;
    for &x in data {
        acc += black_box(x);
    }
    acc
}

#[inline]
pub fn sum_u16(data: &[u16]) -> u64 {
    let mut acc = 0u64;
    for &x in data {
        acc += u64::from(black_box(x));
    }
    acc
}

#[inline]
pub fn sum_f64(data: &[f64]) -> f64 {
    let mut acc = 0.0f64;
    for &x in data {
        acc += black_box(x);
    }
    acc
}

#[inline]
pub fn gather_sum_f32(data: &[f32], idx: &[usize]) -> f32 {
    let mut acc = 0.0f32;
    for &i in idx {
        acc += black_box(data[i]);
    }
    acc
}

#[inline]
pub fn memcpy_bytes(dst: &mut [u8], src: &[u8]) {
    dst.copy_from_slice(src);
}

pub fn vec_push_f32(n: usize, with_capacity: bool) -> Vec<f32> {
    let mut v = if with_capacity {
        Vec::with_capacity(n)
    } else {
        Vec::new()
    };
    for i in 0..n {
        v.push(black_box(i as f32));
    }
    v
}

pub fn hashmap_from_keys(n: usize) -> (HashMap<u64, f32>, Vec<u64>) {
    let mut map = HashMap::with_capacity(n);
    let mut keys = Vec::with_capacity(n);
    for i in 0..n {
        let k = i as u64;
        map.insert(k, i as f32);
        keys.push(k);
    }
    (map, keys)
}

#[inline]
pub fn hashmap_sum(map: &HashMap<u64, f32>, keys: &[u64]) -> f32 {
    let mut acc = 0.0f32;
    for &k in keys {
        acc += black_box(*map.get(&k).unwrap_or(&0.0));
    }
    acc
}

#[inline]
pub fn slice_sum_f32(data: &[f32]) -> f32 {
    sum_f32(data)
}

pub fn sort_f32_clone(mut data: Vec<f32>) -> Vec<f32> {
    data.sort_unstable_by(|a, b| a.total_cmp(b));
    data
}

pub fn rayon_scale_sum(data: &[f32]) -> f32 {
    data.par_iter().map(|x| black_box(*x) * 2.0).sum()
}

pub fn seq_scale_sum(data: &[f32]) -> f32 {
    data.iter().map(|x| black_box(*x) * 2.0).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_sum_is_positive() {
        let data = filled_f32(1024, 1);
        assert!(sum_f32(&data) > 0.0);
    }

    #[test]
    fn hashmap_agrees_with_slice_len() {
        let n = 64;
        let (map, keys) = hashmap_from_keys(n);
        assert_eq!(map.len(), n);
        assert_eq!(keys.len(), n);
        assert!(hashmap_sum(&map, &keys) > 0.0);
    }
}
