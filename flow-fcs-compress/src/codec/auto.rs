//! Auto codec picker.
//!
//! Given a sample of a column's events, choose the codec that's likely to win
//! on the full column:
//! - **Mode B (`AdcBitpack`)** if every sampled value is exactly representable
//!   in the channel's stated ADC granularity (i.e. raw spectral data).
//! - **Mode A (`BssZstd`)** otherwise (compensated, unmixed, or non-ADC data).
//!
//! Mode C (lossy log-quant) is opt-in only — auto-pick never returns it.
//!
//! The picker is cheap by design: ≤4096 samples, no FFT, no histograms.

use crate::codec::{ChannelParams, CodecId};

/// Recommended codec for a channel given a representative sample.
///
/// `sample` should be drawn from the head of the column (the first N events).
/// The full column does not need to be passed.
pub fn pick_codec(sample: &[f32], params: &ChannelParams) -> CodecId {
    if sample.is_empty() {
        return CodecId::LosslessF32BssZstd;
    }

    if let Some(bits) = params.adc_bits {
        if (1..=32).contains(&bits) && params.range > 0 && all_quantized(sample, params.range, bits)
        {
            return CodecId::AdcBitpack;
        }
    }

    CodecId::LosslessF32BssZstd
}

fn all_quantized(values: &[f32], range: u32, adc_bits: u8) -> bool {
    let denom = if adc_bits >= 32 {
        u32::MAX as f64 + 1.0
    } else {
        (1u32 << adc_bits) as f64
    };
    let scale = range as f64 / denom;
    if scale == 0.0 || !scale.is_finite() {
        return false;
    }
    // A value is "quantized" if x/scale is within 1/4 ULP of an integer.
    // We compare in f64 to avoid bouncing on f32 rounding noise.
    let tolerance = 1e-3_f64;
    values.iter().all(|&x| {
        let q = (x as f64) / scale;
        (q - q.round()).abs() <= tolerance
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params_with(range: u32, adc_bits: Option<u8>) -> ChannelParams {
        ChannelParams {
            name: "ch".into(),
            stored_bits: 32,
            range,
            log_decades: (0.0, 0.0),
            adc_bits,
            signed: false,
        }
    }

    #[test]
    fn picks_adc_bitpack_for_quantized_data() {
        // 22-bit integer-quantized values
        let scale = (1u32 << 22) as f64 / (1u32 << 22) as f64; // 1.0
        let sample: Vec<f32> = (0..256).map(|i| (i as f64 * scale) as f32).collect();
        let p = params_with(1 << 22, Some(22));
        assert_eq!(pick_codec(&sample, &p), CodecId::AdcBitpack);
    }

    #[test]
    fn falls_back_to_bss_for_unmixed_data() {
        // Non-integer values — typical of unmixed spectral output
        let sample: Vec<f32> = (0..256)
            .map(|i| (i as f32) * 0.123_456 + 1.7)
            .collect();
        let p = params_with(1 << 22, Some(22));
        assert_eq!(pick_codec(&sample, &p), CodecId::LosslessF32BssZstd);
    }

    #[test]
    fn falls_back_when_adc_bits_unknown() {
        let sample: Vec<f32> = (0..32).map(|i| i as f32).collect();
        let p = params_with(1 << 22, None);
        assert_eq!(pick_codec(&sample, &p), CodecId::LosslessF32BssZstd);
    }

    #[test]
    fn empty_sample_picks_lossless_default() {
        let p = params_with(1 << 22, Some(22));
        assert_eq!(pick_codec(&[], &p), CodecId::LosslessF32BssZstd);
    }
}
