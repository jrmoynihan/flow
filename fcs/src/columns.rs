//! Lazy, per-column extraction from row-major FCS DATA bytes.
//!
//! FCS event data is stored interleaved: `event0_p0, event0_p1, …, event1_p0, …`.
//! `ColumnLayout` precomputes the fixed per-parameter byte offsets and widths
//! from metadata; `extract_columns` walks the bytes exactly once per call,
//! decoding only the requested parameter indices.

use crate::byteorder::ByteOrder;
use crate::datatype::FcsDataType;
use crate::metadata::Metadata;
use anyhow::Result;

/// Precomputed per-parameter byte layout for one FCS file's DATA segment,
/// derived from metadata. Built fresh on each `column()`/`columns()`/
/// `events()` call — it is the *decoded values* that get cached (in
/// `Fcs.columns`), not this layout.
#[derive(Debug, Clone)]
pub(crate) struct ColumnLayout {
    /// Number of events (`$TOT`) this layout was computed for.
    pub num_events: usize,
    /// Total bytes consumed by one full event record — the sum of `bytes_per_parameter`.
    pub bytes_per_event: usize,
    /// Byte width of each parameter (`$PnB / 8`), in parameter order.
    pub bytes_per_parameter: Vec<usize>,
    /// Running-sum byte offset of each parameter within one event record.
    /// Not `param_idx * width` — widths vary per parameter.
    pub param_offsets: Vec<usize>,
    /// Declared FCS data type (`$DATATYPE` or a `$PnDATATYPE` override) of
    /// each parameter, in parameter order.
    pub data_types: Vec<FcsDataType>,
    /// Byte order (`$BYTEORD`), shared by every parameter in this file.
    pub byte_order: ByteOrder,
    /// `$PnR`-derived mask for integer parameters whose storage width
    /// (`$PnB`) exceeds their declared ADC resolution. `None` for float/double
    /// parameters, which aren't bit-packed ADC values and are exempt per spec.
    pub range_masks: Vec<Option<u32>>,
    /// True if any `$PnB` isn't a multiple of 8. The byte-stride traversal in
    /// `extract_columns` can't represent bit-packed records.
    pub is_bit_packed: bool,
}

/// Applies the `$PnR`-derived integer range mask to a decoded value, if one
/// applies. `None` (float/double parameters, or integer parameters without a
/// usable `$PnR`) passes the value through unchanged.
pub(crate) fn apply_range_mask(value: f32, mask: Option<u32>) -> f32 {
    match mask {
        Some(mask) => ((value as u32) & mask) as f32,
        None => value,
    }
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

use crate::file::Fcs;
use anyhow::anyhow;
use rayon::prelude::*;

/// Decode the requested parameter indices from row-major FCS event bytes in
/// a single pass over `data_bytes`. Extracting 1 column and extracting all of
/// them cost the same per-event traversal — the DATA segment is interleaved,
/// so there's no strided-vs-sequential choice to make, only how many values
/// to keep per event. Callers should batch every column they need into one
/// call rather than calling this once per column.
///
/// # Errors
/// Returns `Err` if `layout.is_bit_packed` (bit-packed records aren't
/// byte-aligned, so this stride-based traversal can't represent them — use
/// the existing `parse_bit_packed_data` path instead), or if a value fails to
/// decode for its declared data type/width.
pub(crate) fn extract_columns(
    data_bytes: &[u8],
    layout: &ColumnLayout,
    wanted: &[usize],
) -> Result<Vec<Box<[f32]>>> {
    if layout.is_bit_packed {
        return Err(anyhow!(
            "bit-packed FCS records don't support lazy single-column access; call events() instead"
        ));
    }

    let total_event_bytes = layout.num_events * layout.bytes_per_event;
    let event_bytes = data_bytes
        .get(..total_event_bytes)
        .ok_or_else(|| anyhow!(
            "data segment ({} bytes) is shorter than {} events x {} bytes/event",
            data_bytes.len(), layout.num_events, layout.bytes_per_event
        ))?;

    let decode_row = |event: &[u8]| -> Result<Vec<f32>> {
        wanted
            .iter()
            .map(|&idx| {
                let offset = layout.param_offsets[idx];
                let width = layout.bytes_per_parameter[idx];
                let mut value = Fcs::parse_parameter_value_to_f32(
                    &event[offset..offset + width],
                    width,
                    &layout.data_types[idx],
                    &layout.byte_order,
                )?;
                value = apply_range_mask(value, layout.range_masks[idx]);
                Ok(value)
            })
            .collect()
    };

    let rows: Vec<Vec<f32>> = if layout.num_events * wanted.len() >= crate::file::PARALLEL_THRESHOLD {
        event_bytes
            .par_chunks_exact(layout.bytes_per_event)
            .map(decode_row)
            .collect::<Result<Vec<_>>>()?
    } else {
        event_bytes
            .chunks_exact(layout.bytes_per_event)
            .map(decode_row)
            .collect::<Result<Vec<_>>>()?
    };

    let mut columns: Vec<Vec<f32>> = wanted.iter().map(|_| Vec::with_capacity(layout.num_events)).collect();
    for row in rows {
        for (column, value) in columns.iter_mut().zip(row) {
            column.push(value);
        }
    }
    Ok(columns.into_iter().map(Vec::into_boxed_slice).collect())
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

    /// 3 events x 2 f32 params, little-endian, matching `synthetic_metadata_2f32`.
    /// event0 = (1.0, 2.0), event1 = (3.0, 4.0), event2 = (5.0, 6.0).
    fn synthetic_f32_bytes() -> Vec<u8> {
        let values: [f32; 6] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn extract_columns_decodes_requested_indices_only() {
        let metadata = synthetic_metadata_2f32();
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");
        let data_bytes = synthetic_f32_bytes();

        let param1_only = super::extract_columns(&data_bytes, &layout, &[1]).expect("extract");
        assert_eq!(param1_only.len(), 1);
        assert_eq!(&*param1_only[0], &[2.0f32, 4.0, 6.0]);

        let both = super::extract_columns(&data_bytes, &layout, &[0, 1]).expect("extract");
        assert_eq!(&*both[0], &[1.0f32, 3.0, 5.0]);
        assert_eq!(&*both[1], &[2.0f32, 4.0, 6.0]);
    }

    #[test]
    fn extract_columns_applies_range_mask_for_integer_params() {
        let mut metadata = synthetic_metadata_2f32();
        metadata
            .keywords
            .insert("$DATATYPE".to_string(), Keyword::Byte(ByteKeyword::DATATYPE(FcsDataType::I)));
        for n in 1..=2 {
            metadata
                .keywords
                .insert(format!("$P{n}R"), Keyword::Int(IntegerKeyword::PnR(16)));
        }
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");

        // One event, param0 = 0xFF (255), param1 = 0x0A (10), both u32 LE in 4-byte fields.
        let mut data_bytes = Vec::new();
        data_bytes.extend_from_slice(&255u32.to_le_bytes());
        data_bytes.extend_from_slice(&10u32.to_le_bytes());
        let mut layout = layout;
        layout.num_events = 1;

        let columns = super::extract_columns(&data_bytes, &layout, &[0, 1]).expect("extract");
        assert_eq!(&*columns[0], &[15.0f32], "0xFF & (16.next_power_of_two()-1 = 15) = 15");
        assert_eq!(&*columns[1], &[10.0f32], "0x0A & 15 = 10, unaffected");
    }

    #[test]
    fn extract_columns_rejects_bit_packed_layout() {
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

        let err = super::extract_columns(&[], &layout, &[0]).unwrap_err();
        assert!(
            err.to_string().contains("bit-packed"),
            "error should name the unsupported case, got: {err}"
        );
    }
}
