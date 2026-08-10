//! Per-column value decoders for byte-aligned FCS DATA.
//!
//! `(datatype, width, byteorder)` is fixed for a whole column by its metadata,
//! so the dispatch belongs at column-resolution time, not per value. Resolving
//! once also moves all fallibility out of the inner loop: `resolve` runs
//! `wanted.len()` times and can fail; `read` runs `num_events * wanted.len()`
//! times and cannot.

use crate::byteorder::ByteOrder;
use crate::datatype::FcsDataType;
use anyhow::{Result, anyhow};
use byteorder::{BigEndian as BE, ByteOrder as BO, LittleEndian as LE};

/// One resolved `(datatype, width, byteorder)` combination.
///
/// `Copy` and one byte wide, so a `ColumnPlan` holding one costs nothing to
/// pass into a parallel closure. The eight variants are every combination FCS
/// permits for byte-aligned data; bit-packed records take a different path
/// entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decoder {
    U16Le,
    U16Be,
    U32Le,
    U32Be,
    F32Le,
    F32Be,
    F64Le,
    F64Be,
}

impl Decoder {
    /// Resolve a column's decoder from its metadata, once.
    ///
    /// Error messages are reproduced verbatim from
    /// `Fcs::parse_parameter_value_to_f32`, which this replaces in the column
    /// path — a caller matching on the old text keeps working.
    ///
    /// # Errors
    /// Returns `Err` for any `(data_type, bytes_per_param)` pair FCS does not
    /// define, including any use of `$DATATYPE A`.
    pub(crate) fn resolve(
        data_type: FcsDataType,
        bytes_per_param: usize,
        byte_order: &ByteOrder,
    ) -> Result<Self> {
        let little = matches!(byte_order, ByteOrder::LittleEndian);
        Ok(match (data_type, bytes_per_param) {
            (FcsDataType::I, 2) => if little { Self::U16Le } else { Self::U16Be },
            (FcsDataType::I, 4) => if little { Self::U32Le } else { Self::U32Be },
            (FcsDataType::F, 4) => if little { Self::F32Le } else { Self::F32Be },
            (FcsDataType::D, 8) => if little { Self::F64Le } else { Self::F64Be },
            (FcsDataType::I, _) => {
                return Err(anyhow!(
                    "Unsupported integer size: {} bytes (expected 2 or 4)",
                    bytes_per_param
                ));
            }
            (FcsDataType::F, _) => {
                return Err(anyhow!(
                    "Invalid float32 size: {} bytes (expected 4)",
                    bytes_per_param
                ));
            }
            (FcsDataType::D, _) => {
                return Err(anyhow!(
                    "Invalid float64 size: {} bytes (expected 8)",
                    bytes_per_param
                ));
            }
            (FcsDataType::A, _) => return Err(anyhow!("ASCII data type not supported")),
        })
    }

    /// Decode one value. Infallible by construction: [`resolve`](Self::resolve)
    /// already proved the combination is legal, and the caller slices exactly
    /// the declared width.
    ///
    /// `bytes` must be at least as long as this decoder's width; callers pass
    /// `&event[offset..offset + width]`.
    #[inline(always)]
    pub(crate) fn read(self, bytes: &[u8]) -> f32 {
        match self {
            Self::U16Le => LE::read_u16(bytes) as f32,
            Self::U16Be => BE::read_u16(bytes) as f32,
            Self::U32Le => LE::read_u32(bytes) as f32,
            Self::U32Be => BE::read_u32(bytes) as f32,
            Self::F32Le => LE::read_f32(bytes),
            Self::F32Be => BE::read_f32(bytes),
            Self::F64Le => LE::read_f64(bytes) as f32,
            Self::F64Be => BE::read_f64(bytes) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Decoder;
    use crate::byteorder::ByteOrder;
    use crate::datatype::FcsDataType;

    #[test]
    fn resolves_the_eight_legal_combinations() {
        let cases = [
            (FcsDataType::I, 2, ByteOrder::LittleEndian, Decoder::U16Le),
            (FcsDataType::I, 2, ByteOrder::BigEndian, Decoder::U16Be),
            (FcsDataType::I, 4, ByteOrder::LittleEndian, Decoder::U32Le),
            (FcsDataType::I, 4, ByteOrder::BigEndian, Decoder::U32Be),
            (FcsDataType::F, 4, ByteOrder::LittleEndian, Decoder::F32Le),
            (FcsDataType::F, 4, ByteOrder::BigEndian, Decoder::F32Be),
            (FcsDataType::D, 8, ByteOrder::LittleEndian, Decoder::F64Le),
            (FcsDataType::D, 8, ByteOrder::BigEndian, Decoder::F64Be),
        ];
        for (data_type, width, order, expected) in cases {
            let got = Decoder::resolve(data_type, width, &order)
                .unwrap_or_else(|e| panic!("{data_type:?}/{width}/{order:?}: {e}"));
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn rejects_unsupported_combinations_before_touching_bytes() {
        let order = ByteOrder::LittleEndian;
        let cases = [
            (FcsDataType::I, 3usize, "Unsupported integer size"),
            (FcsDataType::F, 8, "Invalid float32 size"),
            (FcsDataType::D, 4, "Invalid float64 size"),
            (FcsDataType::A, 1, "ASCII data type not supported"),
        ];
        for (data_type, width, expected) in cases {
            let err = Decoder::resolve(data_type, width, &order)
                .expect_err("should reject")
                .to_string();
            assert!(
                err.contains(expected),
                "error for {data_type:?}/{width} should contain {expected:?}, got: {err}"
            );
        }
    }

    #[test]
    fn reads_little_and_big_endian_integers() {
        assert_eq!(Decoder::U16Le.read(&[0x34, 0x12]), 0x1234_u16 as f32);
        assert_eq!(Decoder::U16Be.read(&[0x12, 0x34]), 0x1234_u16 as f32);
        assert_eq!(Decoder::U32Le.read(&[0x78, 0x56, 0x34, 0x12]), 0x1234_5678_u32 as f32);
        assert_eq!(Decoder::U32Be.read(&[0x12, 0x34, 0x56, 0x78]), 0x1234_5678_u32 as f32);
    }

    #[test]
    fn reads_little_and_big_endian_floats() {
        let value = 1234.5678_f32;
        assert_eq!(Decoder::F32Le.read(&value.to_le_bytes()), value);
        assert_eq!(Decoder::F32Be.read(&value.to_be_bytes()), value);

        let wide = 1234.5678_f64;
        assert_eq!(Decoder::F64Le.read(&wide.to_le_bytes()), wide as f32);
        assert_eq!(Decoder::F64Be.read(&wide.to_be_bytes()), wide as f32);
    }

    /// The rewrite must be bit-exact against the function it replaces, or
    /// every downstream analysis silently shifts.
    #[test]
    fn matches_parse_parameter_value_to_f32_bit_for_bit() {
        use crate::file::Fcs;
        let bytes: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        let cases = [
            (FcsDataType::I, 2usize),
            (FcsDataType::I, 4),
            (FcsDataType::F, 4),
            (FcsDataType::D, 8),
        ];
        for order in [ByteOrder::LittleEndian, ByteOrder::BigEndian] {
            for (data_type, width) in cases {
                let expected =
                    Fcs::parse_parameter_value_to_f32(&bytes[..width], width, &data_type, &order)
                        .expect("reference decode");
                let got = Decoder::resolve(data_type, width, &order)
                    .expect("resolve")
                    .read(&bytes[..width]);
                assert_eq!(
                    got.to_bits(),
                    expected.to_bits(),
                    "{data_type:?}/{width}/{order:?} must match the reference bit-for-bit"
                );
            }
        }
    }
}
