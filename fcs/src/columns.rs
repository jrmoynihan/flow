//! Lazy, per-column extraction from row-major FCS DATA bytes.
//!
//! FCS event data is stored interleaved: `event0_p0, event0_p1, …, event1_p0, …`.
//! `ColumnLayout` precomputes the fixed per-parameter byte offsets and widths
//! from metadata once; `extract_columns` (added in the next task) walks the
//! bytes exactly once per call, decoding only the requested parameter indices.

use crate::byteorder::ByteOrder;
use crate::datatype::FcsDataType;
use crate::metadata::Metadata;
use anyhow::Result;

/// Precomputed per-parameter byte layout for one FCS file's DATA segment,
/// derived once from metadata. Reused across every `column()`/`columns()`/
/// `events()` call so repeated access doesn't re-walk `$PnB`/`$PnR`/etc.
#[derive(Debug, Clone)]
pub(crate) struct ColumnLayout {
    pub num_events: usize,
    pub bytes_per_event: usize,
    pub bytes_per_parameter: Vec<usize>,
    /// Running-sum byte offset of each parameter within one event record.
    /// Not `param_idx * width` — widths vary per parameter.
    pub param_offsets: Vec<usize>,
    pub data_types: Vec<FcsDataType>,
    pub byte_order: ByteOrder,
    /// `$PnR`-derived mask for integer parameters whose storage width
    /// (`$PnB`) exceeds their declared ADC resolution. `None` for float/double
    /// parameters, which aren't bit-packed ADC values and are exempt per spec.
    pub range_masks: Vec<Option<u32>>,
    /// True if any `$PnB` isn't a multiple of 8. The byte-stride traversal in
    /// `extract_columns` can't represent bit-packed records.
    pub is_bit_packed: bool,
}

impl ColumnLayout {
    pub(crate) fn from_metadata(metadata: &Metadata) -> Result<Self> {
        let number_of_parameters = *metadata.get_number_of_parameters()?;
        let number_of_events = *metadata.get_number_of_events()?;

        let bits_per_parameter: Vec<usize> = (1..=number_of_parameters)
            .map(|n| metadata.get_bits_per_parameter(n))
            .collect::<Result<Vec<_>>>()?;
        let is_bit_packed = bits_per_parameter.iter().any(|&bits| bits % 8 != 0);

        let bytes_per_parameter: Vec<usize> = (1..=number_of_parameters)
            .map(|n| metadata.get_bytes_per_parameter(n))
            .collect::<Result<Vec<_>>>()?;

        let data_types: Vec<FcsDataType> = (1..=number_of_parameters)
            .map(|n| metadata.get_data_type_for_channel(n))
            .collect::<Result<Vec<_>>>()?;

        let byte_order = metadata.get_byte_order()?.clone();

        let mut running = 0usize;
        let param_offsets: Vec<usize> = bytes_per_parameter
            .iter()
            .map(|&width| {
                let offset = running;
                running += width;
                offset
            })
            .collect();
        let bytes_per_event = running;

        let range_masks: Vec<Option<u32>> = (1..=number_of_parameters)
            .zip(&data_types)
            .map(|(n, &dtype)| {
                if dtype == FcsDataType::I {
                    metadata
                        .get_range_for_channel(n)
                        .ok()
                        .map(|range| range.next_power_of_two().saturating_sub(1) as u32)
                } else {
                    None
                }
            })
            .collect();

        Ok(Self {
            num_events: number_of_events,
            bytes_per_event,
            bytes_per_parameter,
            param_offsets,
            data_types,
            byte_order,
            range_masks,
            is_bit_packed,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::datatype::FcsDataType;
    use crate::keyword::{ByteKeyword, IntegerKeyword, Keyword};
    use crate::metadata::Metadata;
    use crate::byteorder::ByteOrder;

    /// 3 events x 2 parameters, both `$DATATYPE F`, `$PnB 32` (byte-aligned,
    /// not bit-packed). Mirrors the synthetic-metadata construction style used
    /// in `fcs/src/write.rs`'s existing bit-packed test.
    fn synthetic_metadata_2f32() -> Metadata {
        let mut metadata = Metadata::new();
        metadata.delimiter = '\u{000c}';
        metadata
            .keywords
            .insert("$BYTEORD".to_string(), Keyword::Byte(ByteKeyword::BYTEORD(ByteOrder::LittleEndian)));
        metadata
            .keywords
            .insert("$DATATYPE".to_string(), Keyword::Byte(ByteKeyword::DATATYPE(FcsDataType::F)));
        metadata
            .keywords
            .insert("$PAR".to_string(), Keyword::Int(IntegerKeyword::PAR(2)));
        metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(3)));
        for n in 1..=2 {
            metadata.insert_string_keyword(format!("$P{n}N"), format!("P{n}"));
            metadata
                .keywords
                .insert(format!("$P{n}B"), Keyword::Int(IntegerKeyword::PnB(32)));
        }
        metadata
    }

    #[test]
    fn layout_computes_offsets_and_stride_for_uniform_f32() {
        let metadata = synthetic_metadata_2f32();
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");

        assert_eq!(layout.num_events, 3);
        assert_eq!(layout.bytes_per_event, 8, "2 params x 4 bytes = 8 bytes/event");
        assert_eq!(layout.bytes_per_parameter, vec![4, 4]);
        assert_eq!(layout.param_offsets, vec![0, 4], "param 0 starts at byte 0, param 1 at byte 4");
        assert_eq!(layout.data_types, vec![FcsDataType::F, FcsDataType::F]);
        assert!(!layout.is_bit_packed);
        assert_eq!(layout.range_masks, vec![None, None], "F32 params are never range-masked");
    }

    /// 1 event x 3 parameters with deliberately non-uniform `$PnB` widths
    /// (64, 16, 32 bits -> 8, 2, 4 bytes). `$DATATYPE I` since arbitrary
    /// byte-aligned integer widths are unremarkable, unlike float widths.
    /// Distinguishes a correct running-sum offset calculation from a buggy
    /// `param_idx * constant_width` one: under running-sum the offsets are
    /// `[0, 8, 10]` and the stride is `14`; `idx * 8` would wrongly give
    /// `[0, 8, 16]`, and `idx * 4` would wrongly give `[0, 4, 8]` — both
    /// distinguishable from the correct answer.
    fn synthetic_metadata_varying_widths() -> Metadata {
        let mut metadata = Metadata::new();
        metadata.delimiter = '\u{000c}';
        metadata
            .keywords
            .insert("$BYTEORD".to_string(), Keyword::Byte(ByteKeyword::BYTEORD(ByteOrder::LittleEndian)));
        metadata
            .keywords
            .insert("$DATATYPE".to_string(), Keyword::Byte(ByteKeyword::DATATYPE(FcsDataType::I)));
        metadata
            .keywords
            .insert("$PAR".to_string(), Keyword::Int(IntegerKeyword::PAR(3)));
        metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(1)));
        let widths = [64, 16, 32];
        for (i, &bits) in widths.iter().enumerate() {
            let n = i + 1;
            metadata.insert_string_keyword(format!("$P{n}N"), format!("P{n}"));
            metadata
                .keywords
                .insert(format!("$P{n}B"), Keyword::Int(IntegerKeyword::PnB(bits)));
        }
        metadata
    }

    #[test]
    fn layout_computes_running_sum_offsets_for_varying_widths() {
        let metadata = synthetic_metadata_varying_widths();
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");

        assert_eq!(layout.bytes_per_parameter, vec![8, 2, 4]);
        assert_eq!(
            layout.param_offsets,
            vec![0, 8, 10],
            "running-sum offsets must reflect each preceding parameter's actual width, \
             not param_idx * a constant width (idx*8 would wrongly give [0,8,16], \
             idx*4 would wrongly give [0,4,8])"
        );
        assert_eq!(layout.bytes_per_event, 14, "8 + 2 + 4 = 14 bytes/event");
        assert!(!layout.is_bit_packed);
    }

    #[test]
    fn layout_computes_range_mask_for_integer_params() {
        let mut metadata = synthetic_metadata_2f32();
        metadata
            .keywords
            .insert("$DATATYPE".to_string(), Keyword::Byte(ByteKeyword::DATATYPE(FcsDataType::I)));
        for n in 1..=2 {
            metadata
                .keywords
                .insert(format!("$P{n}R"), Keyword::Int(IntegerKeyword::PnR(1024)));
        }

        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");
        assert_eq!(
            layout.range_masks,
            vec![Some(1023), Some(1023)],
            "$PnR=1024 masks down to 10 bits (1024.next_power_of_two() - 1 = 1023)"
        );
    }

    #[test]
    fn layout_detects_bit_packed_records() {
        let mut metadata = synthetic_metadata_2f32();
        metadata
            .keywords
            .insert("$DATATYPE".to_string(), Keyword::Byte(ByteKeyword::DATATYPE(FcsDataType::I)));
        for n in 1..=2 {
            metadata
                .keywords
                .insert(format!("$P{n}B"), Keyword::Int(IntegerKeyword::PnB(10)));
        }

        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");
        assert!(layout.is_bit_packed, "$PnB=10 is not a multiple of 8");
    }
}
