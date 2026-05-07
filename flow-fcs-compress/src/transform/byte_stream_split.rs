//! Byte-stream split: regroup the bytes of a `[f32]` slice into 4 byte planes.
//!
//! For a slice `[f0, f1, f2, ...]` where each `fi` has bytes `(b0, b1, b2, b3)`
//! (little-endian), the output is `[b0_0, b0_1, ..., b0_n, b1_0, b1_1, ..., b3_n]`.
//! This dramatically improves entropy-coder ratio on numerical data because the
//! exponent and high-mantissa byte planes have much narrower distributions than
//! the raw interleaved bytes.
//!
//! See Parquet's `BYTE_STREAM_SPLIT` encoding (PARQUET-1622) for the same idea.

/// Split `input` into 4 byte planes, appending to `out`. `out` is grown by
/// `4 * input.len()` bytes.
pub fn split_f32_le(input: &[f32], out: &mut Vec<u8>) {
    let n = input.len();
    out.reserve(n * 4);
    let start = out.len();
    out.resize(start + n * 4, 0);
    let dst = &mut out[start..];
    for (i, v) in input.iter().enumerate() {
        let b = v.to_le_bytes();
        dst[i] = b[0];
        dst[n + i] = b[1];
        dst[2 * n + i] = b[2];
        dst[3 * n + i] = b[3];
    }
}

/// Inverse of [`split_f32_le`]. `planes.len()` must equal `4 * out.len()`.
pub fn unsplit_f32_le(planes: &[u8], out: &mut [f32]) {
    let n = out.len();
    debug_assert_eq!(planes.len(), n * 4);
    for i in 0..n {
        let b0 = planes[i];
        let b1 = planes[n + i];
        let b2 = planes[2 * n + i];
        let b3 = planes[3 * n + i];
        out[i] = f32::from_le_bytes([b0, b1, b2, b3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_unsplit_roundtrips() {
        let input: Vec<f32> = (0..1024).map(|i| (i as f32) * 0.5 - 1.0).collect();
        let mut planes = Vec::new();
        split_f32_le(&input, &mut planes);
        assert_eq!(planes.len(), input.len() * 4);

        let mut out = vec![0.0f32; input.len()];
        unsplit_f32_le(&planes, &mut out);
        assert_eq!(out, input);
    }

    #[test]
    fn split_handles_specials() {
        let input = vec![
            0.0f32,
            -0.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            f32::MIN_POSITIVE,
            f32::EPSILON,
        ];
        let mut planes = Vec::new();
        split_f32_le(&input, &mut planes);
        let mut out = vec![0.0f32; input.len()];
        unsplit_f32_le(&planes, &mut out);
        for (a, b) in input.iter().zip(out.iter()) {
            if a.is_nan() {
                assert!(b.is_nan());
            } else {
                assert_eq!(a.to_bits(), b.to_bits());
            }
        }
    }
}
