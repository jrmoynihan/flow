# Lazy FCS Column Loading — Stage A Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a lazy, per-column, cached data-extraction path to `Fcs` — `.column()`, `.columns()`, `.events()` — that coexists with the existing eager `data_frame` field and is verified against it as a correctness oracle, without changing any existing public API.

**Architecture:** A new `fcs/src/columns.rs` module holds a `ColumnLayout` (precomputed byte offsets/widths/types/masks from metadata) and a single row-major traversal primitive, `extract_columns`, parameterized by which parameter indices to decode. `Fcs` gains a `columns: Arc<[OnceLock<Box<[f32]>>]>` field sized at `$PAR`, populated lazily on first access. `.events()` bypasses that cache entirely — it is a separate, uncached, single-pass materialization, because populating the cache during a full-frame extraction would defeat the point of having one (see spec's Stage-A rule).

**Tech Stack:** Rust, `polars` (DataFrame/Column), `rayon` (par_chunks_exact for the large-file path), `anyhow::Result`, existing `flow-fcs` crate conventions.

## Global Constraints

- This plan implements **Stage A only** — see `docs/superpowers/specs/2026-08-05-lazy-fcs-column-loading-design.md`. Stage B (removing the `data_frame` field, the `EventDataFrame` type split, and the 8-call-site + cross-repo migration) is out of scope for this plan and will be planned separately once Stage A has landed and been exercised against real files. This is an intentional scope boundary, not an oversight: Stage A and Stage B are mechanically coupled to different degrees of risk and touch different repositories (Stage B reaches into `tru-ols-cli` and `peacoqc-py`, which are standalone Cargo workspaces outside this repo's `[workspace] members`, plus `fast-flow` in a separate repo entirely).
- **Break nothing.** `Fcs::open`, `Fcs.data_frame`, and every existing public method must behave exactly as before. No existing test may change.
- **`.events()` must never populate the `columns` cache.** This is load-bearing for the design's memory argument (a QC'd file must not end up holding both its raw columns and its derived frame). A test enforces this directly.
- Keep the `.collect()`-based per-value decode loop, not `get_unchecked`/`set_len` tricks — a prior attempt at the latter regressed performance (`fcs/docs/PERF_AB.md`).
- Reuse `Fcs::parse_parameter_value_to_f32` for scalar decoding rather than reimplementing dtype dispatch — it already handles `I`/`F`/`D` correctly, including the widths that are valid for each.
- Bit-packed records (`$PnB` not a multiple of 8) are out of scope for the new lazy paths in this stage: `column()`/`columns()` return a clear `Err` for them, and `.events()` falls back to the existing `parse_bit_packed_data` + `extract_all_param_columns` pipeline unchanged. This is a real, permanent behavior for this format (bit-packing was deprecated in FCS 3.2), not a temporary gap.
- All new code lives in `fcs/src/columns.rs` plus additions to `fcs/src/file.rs` — **amended after Task 6's workspace verification**: `Fcs`'s other fields (`header`, `metadata`, `parameters`, `data_frame`, `file_access`) are all `pub`, and it turns out several other crates (`tru-ols`, `peacoqc-rs`, `gates`, plus `flow-fcs`'s own `compress`-feature tests) construct `Fcs` directly via struct-literal syntax in their own test fixtures, bypassing `Fcs::open()`. Adding the `pub(crate)` `columns` field broke every one of those literals — `cargo test --workspace` didn't even compile. Task 7 fixes this by adding a `#[cfg(any(test, feature = "test-util"))]` public constructor (`Fcs::for_testing`) and migrating every broken call site to it, which does touch those other crates. This was a genuine plan gap (the original scope boundary assumed all construction went through `Fcs::open()`/`Fcs::new()`), not scope creep — confirmed with the project owner before proceeding (see `flow-crates-3nt`).

---

### Task 1: Widen visibility of the three helpers the new module needs to reuse

**Files:**
- Modify: `fcs/src/file.rs:31` (`PARALLEL_THRESHOLD` constant)
- Modify: `fcs/src/file.rs:1049` (`parse_parameter_value_to_f32`)
- Modify: `fcs/src/file.rs:863` (`parse_bit_packed_data`)
- Modify: `fcs/src/file.rs:136` (`extract_all_param_columns` — already `pub`, no change needed, listed for reference)

**Interfaces:**
- Produces: `pub(crate) const PARALLEL_THRESHOLD: usize`, `pub(crate) fn Fcs::parse_parameter_value_to_f32(bytes: &[u8], bytes_per_param: usize, data_type: &FcsDataType, byte_order: &ByteOrder) -> Result<f32>`, `pub(crate) fn Fcs::parse_bit_packed_data(data_bytes: &[u8], bits_per_parameter: &[usize], data_types: &[FcsDataType], num_events: usize) -> Result<Vec<f32>>` — all three become visible to a sibling module in the same crate. No behavior change.

Rust privacy is per-module, not per-crate: an unmarked `fn` inside `impl Fcs` in `file.rs` is invisible to a new `columns.rs` module even though both are compiled into `flow-fcs`. `pub(crate)` is the minimal fix — these stay unreachable from outside the crate.

- [ ] **Step 1: Change the three visibility modifiers**

In `fcs/src/file.rs`, change:
```rust
const PARALLEL_THRESHOLD: usize = 400_000;
```
to:
```rust
pub(crate) const PARALLEL_THRESHOLD: usize = 400_000;
```

Change:
```rust
    #[cold]
    fn parse_parameter_value_to_f32(
```
to:
```rust
    #[cold]
    pub(crate) fn parse_parameter_value_to_f32(
```

Change:
```rust
    #[cold]
    fn parse_bit_packed_data(
```
to:
```rust
    #[cold]
    pub(crate) fn parse_bit_packed_data(
```

- [ ] **Step 2: Confirm the crate still builds with no warnings about newly-unused-pub items**

Run: `cargo check -p flow-fcs`
Expected: builds clean, no new warnings (these are still only used within the crate, so no "unused pub(crate)" lint fires).

- [ ] **Step 3: Commit**

```bash
git add fcs/src/file.rs
git commit -m "refactor(fcs): widen visibility of parse helpers to pub(crate)"
```

---

### Task 2: `ColumnLayout` — precompute per-parameter offsets, widths, types, and masks

**Files:**
- Create: `fcs/src/columns.rs`
- Modify: `fcs/src/lib.rs` (register the module)
- Test: inline `#[cfg(test)] mod tests` at the bottom of `fcs/src/columns.rs`

**Interfaces:**
- Consumes: `Metadata::get_number_of_parameters(&self) -> Result<&usize>`, `Metadata::get_number_of_events(&self) -> Result<&usize>`, `Metadata::get_bits_per_parameter(&self, parameter_number: usize) -> Result<usize>`, `Metadata::get_bytes_per_parameter(&self, parameter_number: usize) -> Result<usize>`, `Metadata::get_data_type_for_channel(&self, parameter_number: usize) -> Result<FcsDataType>`, `Metadata::get_byte_order(&self) -> Result<&ByteOrder>`, `Metadata::get_range_for_channel(&self, parameter_number: usize) -> Result<usize>` (all confirmed at `fcs/src/metadata.rs:265,272,346,367,298,409,394`).
- Produces: `pub(crate) struct ColumnLayout { num_events, bytes_per_event, bytes_per_parameter: Vec<usize>, param_offsets: Vec<usize>, data_types: Vec<FcsDataType>, byte_order: ByteOrder, range_masks: Vec<Option<u32>>, is_bit_packed: bool }` and `pub(crate) fn ColumnLayout::from_metadata(metadata: &Metadata) -> Result<Self>`. Task 3 consumes this type directly.

- [ ] **Step 1: Write the failing test**

Create `fcs/src/columns.rs` with just the test module first (no implementation yet), so it fails to compile — that's the "red" state for a type that doesn't exist yet:

```rust
//! Lazy, per-column extraction from row-major FCS DATA bytes.
//!
//! FCS event data is stored interleaved: `event0_p0, event0_p1, …, event1_p0, …`.
//! `ColumnLayout` precomputes the fixed per-parameter byte offsets and widths
//! from metadata once; `extract_columns` (added in the next task) walks the
//! bytes exactly once per call, decoding only the requested parameter indices.

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
```

- [ ] **Step 2: Register the module and run to verify it fails**

In `fcs/src/lib.rs`, add alongside the other `pub mod` lines (near `pub mod compress;`):
```rust
pub(crate) mod columns;
```

Run: `cargo test -p flow-fcs columns:: -- --nocapture`
Expected: FAIL to compile — `ColumnLayout` doesn't exist yet.

- [ ] **Step 3: Implement `ColumnLayout`**

Add above the `#[cfg(test)]` block in `fcs/src/columns.rs`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p flow-fcs columns:: -- --nocapture`
Expected: PASS — 3 tests (`layout_computes_offsets_and_stride_for_uniform_f32`, `layout_computes_range_mask_for_integer_params`, `layout_detects_bit_packed_records`).

- [ ] **Step 5: Commit**

```bash
git add fcs/src/columns.rs fcs/src/lib.rs
git commit -m "feat(fcs): add ColumnLayout, precomputed per-parameter byte layout"
```

---

### Task 3: `extract_columns` — the single row-major traversal primitive

**Files:**
- Modify: `fcs/src/columns.rs`

**Interfaces:**
- Consumes: `ColumnLayout` (Task 2), `Fcs::parse_parameter_value_to_f32` and `PARALLEL_THRESHOLD` (Task 1).
- Produces: `pub(crate) fn extract_columns(data_bytes: &[u8], layout: &ColumnLayout, wanted: &[usize]) -> Result<Vec<Box<[f32]>>>` — one `Box<[f32]>` per index in `wanted`, in the same order. Errors if `layout.is_bit_packed`. Task 4 (`Fcs::column`/`Fcs::columns`) and Task 5 (`Fcs::events`) both call this directly.

- [ ] **Step 1: Write the failing tests**

Append to the `#[cfg(test)] mod tests` block in `fcs/src/columns.rs` (before the closing `}`):

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p flow-fcs columns:: -- --nocapture`
Expected: FAIL to compile — `extract_columns` doesn't exist yet.

- [ ] **Step 3: Implement `extract_columns`**

Add to `fcs/src/columns.rs`, after `ColumnLayout`'s `impl` block:

```rust
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
                if let Some(mask) = layout.range_masks[idx] {
                    value = ((value as u32) & mask) as f32;
                }
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p flow-fcs columns:: -- --nocapture`
Expected: PASS — all 6 tests in the module now (3 from Task 2, 3 new).

- [ ] **Step 5: Commit**

```bash
git add fcs/src/columns.rs
git commit -m "feat(fcs): add extract_columns row-major traversal primitive"
```

---

### Task 4: Add the `columns` cache field to `Fcs`, and `.column()` / `.columns()`

**Files:**
- Modify: `fcs/src/file.rs` (struct definition at line 89, both construction sites: `parse_one_dataset` at line ~374 and the test helper in `fcs/src/tests.rs`)
- Modify: `fcs/src/tests.rs` (`create_test_fcs` helper, so it keeps compiling)
- Test: `fcs/src/file.rs`, new `#[cfg(test)]` block

**Interfaces:**
- Consumes: `extract_columns`, `ColumnLayout::from_metadata` (Task 2/3), `Fcs::find_parameter(&self, name: &str) -> Result<&Parameter>` (existing, `fcs/src/file.rs:1164`), `Parameter.parameter_number: usize` (existing field, 1-based).
- Produces: `Fcs.columns: Arc<[OnceLock<Box<[f32]>>]>` (new field), `pub fn Fcs::column(&self, channel_name: &str) -> Result<&[f32]>`, `pub fn Fcs::columns(&self, channel_names: &[&str]) -> Result<Vec<&[f32]>>`, a private `fn Fcs::validated_data_bytes(header: &Header, mmap: &Mmap, metadata: &Metadata) -> Result<&[u8]>` (the offset-validation logic extracted from `store_raw_data_as_dataframe`, now the single implementation both it and the new path call), and a private `fn Fcs::data_bytes(&self) -> Result<&[u8]>` thin wrapper over it. Task 5 (`events()`) and the oracle test in Task 6 both call `data_bytes()`.

- [ ] **Step 1: Extract the shared validation into a free function, called from both the old and new paths**

In `fcs/src/file.rs`, `store_raw_data_as_dataframe` (starting line 535) validates DATA offsets inline against `header`/`mmap`/`metadata` parameters, before `Self` exists. Rather than duplicating that block for the new `&self`-based path, pull it into a standalone associated function that takes the same three inputs explicitly, so both the construction-time caller and the new instance method can share it with no behavior change and no copy of the logic.

Add this new function to `impl Fcs` (near `find_parameter`, e.g. after line 1163):

```rust
    /// Returns the validated DATA segment byte slice for the given
    /// header/mmap/metadata triple. Shared by `store_raw_data_as_dataframe`
    /// (called during construction, before `Self` exists) and `data_bytes`
    /// (called on an already-constructed `Fcs`) so the two paths can't drift.
    ///
    /// # Errors
    /// Will return `Err` if the DATA offsets (from `$BEGINDATA`/`$ENDDATA` or
    /// the primary HEADER) fall outside the mapped file, or if start > end.
    fn validated_data_bytes<'a>(
        header: &Header,
        mmap: &'a Mmap,
        metadata: &Metadata,
    ) -> Result<&'a [u8]> {
        let mut data_start = *header.data_offset.start();
        let mut data_end = *header.data_offset.end();
        let mmap_len = mmap.len();

        if data_start == 0 {
            data_start = metadata
                .get_integer_keyword("$BEGINDATA")
                .map_err(|_| anyhow!("$BEGINDATA keyword not found. Unable to determine data start."))?
                .get_usize()
                .clone();
        }
        if data_end == 0 {
            data_end = metadata
                .get_integer_keyword("$ENDDATA")
                .map_err(|_| anyhow!("$ENDDATA keyword not found. Unable to determine data end."))?
                .get_usize()
                .clone();
        }

        if data_start >= mmap_len {
            return Err(anyhow!("Data start offset {} is beyond mmap length {}", data_start, mmap_len));
        }
        if data_end >= mmap_len {
            return Err(anyhow!("Data end offset {} is beyond mmap length {}", data_end, mmap_len));
        }
        if data_start > data_end {
            return Err(anyhow!("Data start offset {} is greater than end offset {}", data_start, data_end));
        }

        Ok(&mmap[data_start..=data_end])
    }

    /// Returns the validated DATA segment byte slice for this file. Thin
    /// `&self` wrapper over `validated_data_bytes` for callers that already
    /// have a constructed `Fcs`.
    ///
    /// # Errors
    /// Same conditions as `validated_data_bytes`.
    fn data_bytes(&self) -> Result<&[u8]> {
        Self::validated_data_bytes(&self.header, &self.file_access.mmap, &self.metadata)
    }
```

Then replace the inline validation block at the top of `store_raw_data_as_dataframe` (the code from `let mut data_start = *header.data_offset.start();` through `let data_bytes = &mmap[data_start..=data_end];`, lines 541-592) with a single call:

```rust
        let data_bytes = Self::validated_data_bytes(header, mmap, metadata)?;
```

This is a pure refactor — same validation, same error messages, same behavior — so the existing tests that exercise `store_raw_data_as_dataframe`'s error paths (offset-out-of-bounds, missing `$BEGINDATA`/`$ENDDATA`, start > end) must still pass unchanged after this change.

Run: `cargo check -p flow-fcs`
Expected: builds clean. The new `data_bytes` method is unused until Step 3 adds callers — expected, not a problem.

Run: `cargo test -p flow-fcs`
Expected: PASS, all 75 pre-existing tests — confirming the refactor didn't change `store_raw_data_as_dataframe`'s behavior.

- [ ] **Step 2: Add the `columns` field and wire up both construction sites**

In `fcs/src/file.rs`, change the struct (line 89):
```rust
#[derive(Debug, Clone)]
pub struct Fcs {
    pub header: Header,
    pub metadata: Metadata,
    pub parameters: ParameterMap,
    pub data_frame: EventDataFrame,
    pub file_access: AccessWrapper,

    /// Per-parameter lazy column cache, indexed by `parameter_number - 1`.
    /// `Arc<[..]>` (not `Vec<..>`) so `Fcs`'s derived `Clone` shares the
    /// warmed cache across clones instead of deep-copying every populated
    /// column — `OnceLock<T: Clone>` is itself `Clone` and would otherwise
    /// duplicate contents. Sized once at `$PAR` length and never resized, so
    /// element addresses are stable for the lifetime of the `Fcs`.
    columns: std::sync::Arc<[std::sync::OnceLock<Box<[f32]>>]>,
}
```

In `parse_one_dataset` (around line 374, where `let fcs = Self { ... }` is constructed), add the field:
```rust
        let n_params = *metadata.get_number_of_parameters().unwrap_or(&0);
        let columns = std::iter::repeat_with(std::sync::OnceLock::new)
            .take(n_params)
            .collect::<std::sync::Arc<[_]>>();

        let fcs = Self {
            parameters,
            data_frame,
            file_access,
            header,
            metadata,
            columns,
        };
```
Place the `n_params`/`columns` computation immediately before the existing `let fcs = Self { ... }` line — `metadata` is already in scope there.

In `fcs/src/tests.rs`'s `create_test_fcs()` helper (confirmed at `fcs/src/tests.rs:55-61`), the existing return is:
```rust
        Ok(Fcs {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
        })
```
Change it to add the new field, matching the 3 synthetic parameters (`FSC-A`, `SSC-A`, `FL1-A`) built earlier in the same function:
```rust
        Ok(Fcs {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
            columns: std::iter::repeat_with(std::sync::OnceLock::new).take(3).collect(),
        })
```

Run: `cargo check -p flow-fcs --all-targets`
Expected: builds clean. `cargo test -p flow-fcs` still passes (no behavior change yet — the field is populated but unused).

- [ ] **Step 3: Implement `.column()` and `.columns()`**

Add to `impl Fcs` in `fcs/src/file.rs`, near `get_parameter_events_slice` (line 1496):

```rust
    /// Returns the raw (never compensated/transformed) values for one
    /// parameter, computing and caching them on first access.
    ///
    /// Unlike `get_parameter_events_slice`, this never touches `data_frame`
    /// — it decodes directly from the mmap on first call, then serves the
    /// cached `Box<[f32]>` on every call after.
    ///
    /// # Errors
    /// Will return `Err` if `channel_name` isn't a known parameter, if the
    /// file is bit-packed (call `events()` instead), or if decoding fails.
    pub fn column(&self, channel_name: &str) -> Result<&[f32]> {
        let idx = self.find_parameter(channel_name)?.parameter_number - 1;
        if let Some(existing) = self.columns[idx].get() {
            return Ok(existing);
        }

        let layout = crate::columns::ColumnLayout::from_metadata(&self.metadata)?;
        let data_bytes = self.data_bytes()?;
        let mut decoded = crate::columns::extract_columns(data_bytes, &layout, &[idx])?;
        let boxed = decoded.pop().expect("extract_columns returns exactly one column for one requested index");

        // Another thread may have raced us to populate this slot; either
        // value is correct (both were decoded from the same immutable file),
        // so ignore a losing `set`.
        let _ = self.columns[idx].set(boxed);
        Ok(self.columns[idx].get().expect("just set or already set"))
    }

    /// Returns raw values for several parameters, decoding all uncached
    /// members in a single pass over the DATA segment rather than one pass
    /// per column. Prefer this over repeated `column()` calls when you know
    /// the full set you need up front.
    ///
    /// # Errors
    /// Will return `Err` under the same conditions as `column()`, for any of
    /// `channel_names`.
    pub fn columns(&self, channel_names: &[&str]) -> Result<Vec<&[f32]>> {
        let indices: Vec<usize> = channel_names
            .iter()
            .map(|name| Ok(self.find_parameter(name)?.parameter_number - 1))
            .collect::<Result<Vec<_>>>()?;

        let missing: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&idx| self.columns[idx].get().is_none())
            .collect();

        if !missing.is_empty() {
            let layout = crate::columns::ColumnLayout::from_metadata(&self.metadata)?;
            let data_bytes = self.data_bytes()?;
            let decoded = crate::columns::extract_columns(data_bytes, &layout, &missing)?;
            for (idx, boxed) in missing.into_iter().zip(decoded) {
                let _ = self.columns[idx].set(boxed);
            }
        }

        Ok(indices
            .into_iter()
            .map(|idx| self.columns[idx].get().expect("populated above").as_ref())
            .collect())
    }
```

- [ ] **Step 4: Write the failing tests**

Add a new `#[cfg(test)] mod lazy_column_tests` at the bottom of `fcs/src/file.rs` (before the existing `#[cfg(test)] mod tests` at line ~2398, or as a sibling module — either is fine, keep it separate from the existing compensation test module):

```rust
#[cfg(test)]
mod lazy_column_tests {
    use super::Fcs;

    const COMPLIANCE_FCS: &str =
        "/Users/kfls271/Rust/flow-crates/gates/Gating-ML.v1.5.081030.Compliance-tests.081030/List-mode Data Files/int-10000_events_random.fcs";

    #[test]
    fn column_matches_data_frame_oracle() {
        let fcs = Fcs::open(COMPLIANCE_FCS).expect("open compliance fixture");
        let channel = fcs.get_parameter_names_from_dataframe()[0].clone();

        let lazy = fcs.column(&channel).expect("lazy column").to_vec();
        let eager = fcs
            .get_parameter_events_slice(&channel)
            .expect("eager column")
            .to_vec();

        assert_eq!(lazy, eager, "lazy column() must match the eager data_frame for the same channel");
    }

    #[test]
    fn column_caches_after_first_access() {
        let fcs = Fcs::open(COMPLIANCE_FCS).expect("open compliance fixture");
        let channel = fcs.get_parameter_names_from_dataframe()[0].clone();

        let first = fcs.column(&channel).expect("first access").as_ptr();
        let second = fcs.column(&channel).expect("second access").as_ptr();
        assert_eq!(first, second, "second call must return the same cached allocation, not re-decode");
    }

    #[test]
    fn columns_batch_matches_individual_column_calls() {
        let fcs = Fcs::open(COMPLIANCE_FCS).expect("open compliance fixture");
        let names = fcs.get_parameter_names_from_dataframe();
        let (a, b) = (names[0].clone(), names[1].clone());

        let batch = fcs.columns(&[&a, &b]).expect("batch");
        let individual_a = fcs.column(&a).expect("a");
        let individual_b = fcs.column(&b).expect("b");

        assert_eq!(batch[0], individual_a);
        assert_eq!(batch[1], individual_b);
    }

    #[test]
    fn column_rejects_unknown_channel() {
        let fcs = Fcs::open(COMPLIANCE_FCS).expect("open compliance fixture");
        assert!(fcs.column("NOT-A-REAL-CHANNEL").is_err());
    }
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p flow-fcs lazy_column_tests:: -- --nocapture`
Expected: PASS — 4 tests. (These were "written failing" only in the sense that `column()`/`columns()` didn't exist before Step 3; since Step 3 already lands the implementation in this task, run this once after Step 3+4 together and confirm green.)

- [ ] **Step 6: Run the full existing suite to confirm nothing broke**

Run: `cargo test -p flow-fcs`
Expected: PASS — every pre-existing test, unchanged.

- [ ] **Step 7: Commit**

```bash
git add fcs/src/file.rs fcs/src/tests.rs
git commit -m "feat(fcs): add Fcs::column and Fcs::columns lazy cached accessors"
```

---

### Task 5: `.events()` — single-pass full materialization, cache untouched

**Files:**
- Modify: `fcs/src/file.rs`

**Interfaces:**
- Consumes: `ColumnLayout`, `extract_columns` (Task 2/3), `Fcs::parse_bit_packed_data`, `extract_all_param_columns` (existing, `fcs/src/file.rs:136`), `Fcs::data_bytes` (Task 4).
- Produces: `pub fn Fcs::events(&self) -> Result<EventDataFrame>`. Does not read or write `self.columns`.

- [ ] **Step 1: Write the failing tests**

Add to `lazy_column_tests` in `fcs/src/file.rs`:

```rust
    #[test]
    fn events_matches_data_frame_oracle() {
        let fcs = Fcs::open(COMPLIANCE_FCS).expect("open compliance fixture");
        let events_df = fcs.events().expect("events");

        assert_eq!(events_df.height(), fcs.data_frame.height());
        assert_eq!(events_df.width(), fcs.data_frame.width());
        for name in fcs.get_parameter_names_from_dataframe() {
            let from_events = events_df
                .column(&name)
                .unwrap()
                .f32()
                .unwrap()
                .cont_slice()
                .unwrap();
            let from_eager = fcs.get_parameter_events_slice(&name).unwrap();
            assert_eq!(from_events, from_eager, "column {name} mismatch between events() and data_frame");
        }
    }

    #[test]
    fn events_does_not_populate_the_column_cache() {
        let fcs = Fcs::open(COMPLIANCE_FCS).expect("open compliance fixture");
        let _ = fcs.events().expect("events");

        let channel = fcs.get_parameter_names_from_dataframe()[0].clone();
        let idx = fcs.find_parameter(&channel).unwrap().parameter_number - 1;
        assert!(
            fcs.columns[idx].get().is_none(),
            "events() must not populate the lazy column cache — a QC'd file would otherwise hold both the raw columns and the derived frame"
        );
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p flow-fcs lazy_column_tests:: -- --nocapture`
Expected: FAIL to compile — `Fcs::events` doesn't exist yet.

- [ ] **Step 3: Implement `.events()`**

Add to `impl Fcs`, after `.columns()`:

```rust
    /// Materializes every parameter into a single `DataFrame` in one pass
    /// over the DATA segment. Unlike `column()`/`columns()`, this is
    /// deliberately uncached: a transform pipeline that calls this once and
    /// drops the result when done should not leave every raw column resident
    /// afterward. Use `column()`/`columns()` instead when you only need a
    /// few channels — extracting all of them here costs the same traversal
    /// as extracting one.
    ///
    /// # Errors
    /// Will return `Err` if the DATA segment can't be validated, or if any
    /// value fails to decode for its declared data type/width.
    pub fn events(&self) -> Result<EventDataFrame> {
        let layout = crate::columns::ColumnLayout::from_metadata(&self.metadata)?;
        let data_bytes = self.data_bytes()?;
        let n_params = layout.bytes_per_parameter.len();

        let raw_columns: Vec<Box<[f32]>> = if layout.is_bit_packed {
            let bits_per_parameter: Vec<usize> = (1..=n_params)
                .map(|n| self.metadata.get_bits_per_parameter(n))
                .collect::<Result<Vec<_>>>()?;
            let f32_values = Self::parse_bit_packed_data(
                data_bytes,
                &bits_per_parameter,
                &layout.data_types,
                layout.num_events,
            )?;
            extract_all_param_columns(&f32_values, layout.num_events, n_params)
                .into_iter()
                .map(Vec::into_boxed_slice)
                .collect()
        } else {
            let all_indices: Vec<usize> = (0..n_params).collect();
            crate::columns::extract_columns(data_bytes, &layout, &all_indices)?
        };

        let mut df_columns: Vec<Column> = Vec::with_capacity(raw_columns.len());
        for (idx, boxed) in raw_columns.into_iter().enumerate() {
            let name = self.metadata.get_parameter_channel_name(idx + 1)?.to_string();
            df_columns.push(Column::new(name.as_str().into(), boxed.into_vec()));
        }

        let df = DataFrame::new(layout.num_events, df_columns)?;
        Ok(Arc::new(df))
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p flow-fcs lazy_column_tests:: -- --nocapture`
Expected: PASS — 6 tests total in the module.

- [ ] **Step 5: Run the full suite**

Run: `cargo test -p flow-fcs`
Expected: PASS, no regressions.

- [ ] **Step 6: Commit**

```bash
git add fcs/src/file.rs
git commit -m "feat(fcs): add Fcs::events single-pass materialization, cache-free"
```

---

### Task 6: Workspace-wide verification and a bit-packed end-to-end check

**Files:**
- Test: `fcs/src/file.rs` (`lazy_column_tests` module)

**Interfaces:**
- Consumes: everything from Tasks 1-5.
- Produces: nothing new — this task is verification only, confirming Stage A didn't regress anything outside `fcs` either.

- [ ] **Step 1: Write an end-to-end bit-packed test using the existing fixture-construction pattern**

The existing test `bit_packed_pnb10_record_uses_correct_stride_and_decodes` in `fcs/src/write.rs:978` already builds a real bit-packed `.fcs` file on disk. Add one assertion alongside it (in the same test function, after its existing assertions, right before `let _ = std::fs::remove_file(&tmp);`) to confirm the new lazy paths handle bit-packed data honestly rather than silently:

```rust
        // Stage A: column() must reject bit-packed files explicitly rather
        // than silently decoding them wrong (the byte-stride traversal can't
        // represent bit-packed records); events() must still work correctly,
        // unchanged, via the pre-existing parse_bit_packed_data path.
        assert!(
            fcs.column("P1").is_err(),
            "column() must reject bit-packed layouts, not attempt byte-stride decoding"
        );
        let events_df = fcs.events().expect("events() must still work for bit-packed data");
        assert_eq!(events_df.height(), fcs.data_frame.height());
```

- [ ] **Step 2: Run it**

Run: `cargo test -p flow-fcs bit_packed_pnb10_record_uses_correct_stride_and_decodes -- --nocapture`
Expected: PASS.

- [ ] **Step 3: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS. This is the real confirmation that Stage A's changes — all `pub(crate)`-only, plus one additive field and three new methods — are invisible to every other workspace crate (`peacoqc-rs`, `tru-ols`, `gates`, `plots`, etc.), none of which reference `Fcs.columns` or call the new methods yet.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -p flow-fcs --all-targets -- -D warnings`
Expected: clean. Pay particular attention to any `clippy::too_many_arguments` or `clippy::type_complexity` lint on `ColumnLayout` or `extract_columns` — if clippy flags the `Arc<[OnceLock<Box<[f32]>>]>` field type as complex, add a `type ColumnCache = std::sync::Arc<[std::sync::OnceLock<Box<[f32]>>]>;` alias in `columns.rs` and use it in the `Fcs` struct instead of the inline type.

- [ ] **Step 5: Commit** (only if Step 4 required a change; otherwise this task has no new commit beyond Task 5's)

```bash
git add fcs/src/file.rs fcs/src/write.rs fcs/src/columns.rs
git commit -m "test(fcs): verify bit-packed fallback and full-workspace compatibility"
```

---

### Task 7: Fix cross-crate `Fcs` test-fixture construction, restore workspace compatibility

**Why this task exists:** Task 6's `cargo test --workspace` run doesn't compile. `Fcs`'s other fields (`header`, `metadata`, `parameters`, `data_frame`, `file_access`) are all `pub`, so several crates historically built `Fcs` test fixtures via raw struct-literal syntax, bypassing `Fcs::open()` entirely. Adding the `pub(crate)` `columns` field in Task 4 broke every one of those literals — a `pub(crate)` field can't be named from outside the crate at all, so it's not a "missing field" a caller can just add. This was a real gap in the plan's original scope statement ("no other crate is touched"), confirmed with the project owner (see bead `flow-crates-3nt`) before writing this task. The fix: add a public, test-only constructor to `Fcs` and migrate every broken call site to it.

**Files:**
- Modify: `fcs/src/file.rs` (new `Fcs::for_testing` constructor)
- Modify: `fcs/Cargo.toml` (new `test-util` feature)
- Modify: `fcs/src/tests.rs`, `fcs/src/write.rs`, `fcs/src/compress.rs` (migrate 3 in-crate call sites)
- Modify: `tru-ols/Cargo.toml`, `tru-ols/src/fcs_integration.rs` (enable feature, migrate 3 call sites)
- Modify: `peacoqc-rs/Cargo.toml`, `peacoqc-rs/tests/synthetic_drift_peacoqc.rs` (enable feature, migrate 1 call site)
- Modify: `gates/Cargo.toml`, `gates/tests/test_helpers.rs`, `gates/examples/visualize_synthetic_data.rs` (enable feature, migrate 2 call sites)

**Interfaces:**
- Consumes: nothing new — every call site already has `Header`, `Metadata`, `ParameterMap`, `EventDataFrame` (`Arc<DataFrame>`), and `AccessWrapper` values in scope; they're just re-passed as arguments instead of struct-literal fields.
- Produces: `pub fn Fcs::for_testing(header: Header, metadata: Metadata, parameters: ParameterMap, data_frame: EventDataFrame, file_access: AccessWrapper) -> Self`, gated `#[cfg(any(test, feature = "test-util"))]`.

- [ ] **Step 1: Add the `test-util` feature and the constructor**

In `fcs/Cargo.toml`, add to the existing `[features]` table (near `compress=[...]`):
```toml
# Exposes Fcs::for_testing() for building fixtures outside this crate's own
# test/bench targets (where #[cfg(test)] alone would suffice). Purely
# additive — gates one constructor, no behavior change to anything else.
test-util = []
```

In `fcs/src/file.rs`, add this new method to `impl Fcs`, directly after `pub fn new() -> Result<Self> { ... }` (around line 223):

```rust
    /// Builds an `Fcs` directly from its parts, for test fixtures that don't
    /// go through `open()`. The `columns` cache always starts empty, sized to
    /// `parameters.len()` — the same invariant `open()`'s construction path
    /// maintains.
    ///
    /// Not part of the normal API: real code should always go through
    /// `open()`/`open_all()`, which parse a real file and guarantee `header`/
    /// `metadata`/`parameters`/`data_frame` are mutually consistent. This
    /// constructor makes no such guarantee — it exists so other crates' test
    /// fixtures (which build all of these by hand) can still construct an
    /// `Fcs` without reaching into `columns`, a cache-only field that isn't
    /// part of the public API.
    #[cfg(any(test, feature = "test-util"))]
    pub fn for_testing(
        header: Header,
        metadata: Metadata,
        parameters: ParameterMap,
        data_frame: EventDataFrame,
        file_access: AccessWrapper,
    ) -> Self {
        let n_params = parameters.len();
        Self {
            header,
            metadata,
            parameters,
            data_frame,
            file_access,
            columns: std::iter::repeat_with(std::sync::OnceLock::new)
                .take(n_params)
                .collect(),
        }
    }
```

Run: `cargo check -p flow-fcs --features test-util`
Expected: builds clean (the function is new and unused outside test/feature-gated contexts within this crate itself, which is fine — `cargo check -p flow-fcs` without `--features test-util` still builds too, since `cfg(test)` alone satisfies the gate for the crate's own test binary).

- [ ] **Step 2: Migrate the 3 in-crate call sites**

In `fcs/src/tests.rs`, replace the `Ok(Fcs { header: Header::new(), metadata: Metadata::new(), parameters: params, data_frame: Arc::new(df), file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))?, columns: std::iter::repeat_with(std::sync::OnceLock::new).take(3).collect(), })` (confirmed at `fcs/src/tests.rs:55-62`) with:
```rust
        Ok(Fcs::for_testing(
            Header::new(),
            Metadata::new(),
            params,
            Arc::new(df),
            AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
        ))
```

In `fcs/src/write.rs` (confirmed at `fcs/src/write.rs:813-820`), replace:
```rust
        let fcs = Fcs {
            header: Header::new(),
            metadata,
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(stub.to_str().unwrap()).expect("access"),
            columns: std::iter::repeat_with(std::sync::OnceLock::new).take(2).collect(),
        };
```
with:
```rust
        let fcs = Fcs::for_testing(
            Header::new(),
            metadata,
            params,
            Arc::new(df),
            AccessWrapper::new(stub.to_str().unwrap()).expect("access"),
        );
```

In `fcs/src/compress.rs` (confirmed at `fcs/src/compress.rs:382-388`), replace:
```rust
        let fcs = Fcs {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(placeholder.to_str().unwrap()).unwrap(),
        };
```
with:
```rust
        let fcs = Fcs::for_testing(
            Header::new(),
            Metadata::new(),
            params,
            Arc::new(df),
            AccessWrapper::new(placeholder.to_str().unwrap()).unwrap(),
        );
```
(Note: `compress.rs`'s site predates Task 4 and never got a `columns:` field added — it's been broken since Task 4 landed, just not caught until now because it's gated behind the `compress` feature, which is off by default. `Fcs::for_testing` fixes it too, no separate step needed.)

Run: `cargo test -p flow-fcs --features compress,test-util`
Expected: PASS, including `fcs/src/compress.rs`'s tests, which don't currently run under default features.

Run: `cargo test -p flow-fcs`
Expected: PASS — the default-feature test suite (94 tests as of Task 6) must be completely unaffected by this refactor.

- [ ] **Step 3: Enable the feature and migrate `tru-ols`**

In `tru-ols/Cargo.toml`, change (confirmed at line 32):
```toml
flow-fcs = { path = "../fcs", version = "^0.5.0", optional=true }
```
to:
```toml
flow-fcs = { path = "../fcs", version = "^0.5.0", optional=true, features=["test-util"] }
```
This enables `test-util` unconditionally whenever `tru-ols`'s own `flow-fcs` feature is active (on by default) — simpler than splitting a separate `[dev-dependencies]` entry, and harmless since the feature only unlocks one inert constructor. `tru-ols/Cargo.toml` has no existing `[dev-dependencies]` split for `flow-fcs` to extend, so this matches its existing single-declaration convention.

In `tru-ols/src/fcs_integration.rs`, there are 3 identical-shaped `Fcs { header: Header::new(), metadata: Metadata::new(), parameters: params, data_frame: Arc::new(df), file_access: AccessWrapper::new(...)... }` literals (confirmed at approximately lines 852, 1205, 1355 — search for `Fcs {` within `#[cfg(test)] mod tests` to find the exact current line numbers, since earlier commits may have shifted them slightly). Replace each with the `Fcs::for_testing(...)` equivalent, preserving each site's own local variable names and whatever wrapping (`Ok(...)`, direct assignment to `let stained_fcs = ...`, `let mut stained_fcs = ...`) it currently has — only the inner `Fcs { ... }` becomes `Fcs::for_testing(...)`, argument order `header, metadata, parameters, data_frame, file_access`. For example, the first site (confirmed shape):
```rust
        Ok(Fcs {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
        })
```
becomes:
```rust
        Ok(Fcs::for_testing(
            Header::new(),
            Metadata::new(),
            params,
            Arc::new(df),
            AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
        ))
```
Apply the same mechanical transform to the other two sites, adjusting only for each site's own wrapping/variable names — none of them have a `columns:` field to drop (they predate Task 4 and were never patched, which is *why* they're broken).

Run: `cargo test -p tru-ols --lib`
Expected: PASS — this specifically could not even compile before this task; a passing run here is the concrete proof this task fixes the regression.

- [ ] **Step 4: Enable the feature and migrate `peacoqc-rs`**

In `peacoqc-rs/Cargo.toml`, change (confirmed at line 28):
```toml
flow-fcs = { path = "../fcs", version = "^0.5.0", optional=true }
```
to:
```toml
flow-fcs = { path = "../fcs", version = "^0.5.0", optional=true, features=["test-util"] }
```

In `peacoqc-rs/tests/synthetic_drift_peacoqc.rs` (confirmed at line 43-49), replace:
```rust
    Fcs {
        header: Header::new(),
        metadata: Metadata::new(),
        parameters: params,
        data_frame: Arc::new(df),
        file_access: AccessWrapper::new(tmp.to_str().unwrap_or(".")).expect("access"),
    }
```
with:
```rust
    Fcs::for_testing(
        Header::new(),
        Metadata::new(),
        params,
        Arc::new(df),
        AccessWrapper::new(tmp.to_str().unwrap_or(".")).expect("access"),
    )
```

Run: `cargo test -p peacoqc-rs --all-features`
Expected: PASS.

- [ ] **Step 5: Enable the feature and migrate `gates`**

In `gates/Cargo.toml`, change (confirmed at line 19):
```toml
flow-fcs = { path = "../fcs", version = "^0.5.0" }
```
to:
```toml
flow-fcs = { path = "../fcs", version = "^0.5.0", features=["test-util"] }
```

In `gates/tests/test_helpers.rs` (confirmed at line 77-83), replace:
```rust
    Ok(Fcs {
        header: Header::new(),
        metadata: Metadata::new(),
        parameters: params,
        data_frame: Arc::new(df),
        file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
    })
```
with:
```rust
    Ok(Fcs::for_testing(
        Header::new(),
        Metadata::new(),
        params,
        Arc::new(df),
        AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
    ))
```

In `gates/examples/visualize_synthetic_data.rs` (confirmed at line 80-86), replace the identically-shaped literal with the same transform.

Run: `cargo test -p gates`
Expected: PASS.

Run: `cargo build -p gates --examples`
Expected: builds clean (examples aren't run by `cargo test`, only compiled — confirm `visualize_synthetic_data` itself still compiles).

- [ ] **Step 6: Full workspace verification — the actual goal of this task**

Run: `cargo test --workspace`
Expected: PASS. This is the command that failed before this task and is the reason this task exists — every crate in `[workspace] members` (`fcs`, `flow-fcs-compress`, `flow-fcs-bench`, `flow-linalg`, `flow-density`, `flow-clustering`, `flow-knn`, `flow-dimensional-reduction`, `flow-pacmap`, `plots`, `gates`, `peacoqc-rs`, `peacoqc-cli`, `tru-ols`, `flow-peak-detection`, `flow-control-detection`) must compile and pass.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: this may surface pre-existing lint debt unrelated to this task (Task 6 already found flow-fcs alone has some, tracked separately). Do not fix pre-existing warnings outside the files this task touches — only ensure the code this task added or changed (the `for_testing` constructor and the 9 migrated call sites) is clippy-clean. If pre-existing warnings block `-D warnings` from completing at the workspace level, note that in your report rather than fixing them (out of scope for this task) — but confirm via `cargo clippy -p flow-fcs -p tru-ols -p peacoqc-rs -p gates --all-targets -- -D warnings` (scoped to just the 4 crates this task touches) that this task's own changes are clean, which is what actually matters here.

- [ ] **Step 7: Commit**

```bash
git add fcs/Cargo.toml fcs/src/file.rs fcs/src/tests.rs fcs/src/write.rs fcs/src/compress.rs \
        tru-ols/Cargo.toml tru-ols/src/fcs_integration.rs \
        peacoqc-rs/Cargo.toml peacoqc-rs/tests/synthetic_drift_peacoqc.rs \
        gates/Cargo.toml gates/tests/test_helpers.rs gates/examples/visualize_synthetic_data.rs
git commit -m "fix(fcs): add Fcs::for_testing constructor, restore cross-crate test-fixture construction

Task 4's pub(crate) columns field broke every out-of-crate struct-literal
construction of Fcs, since a pub(crate) field can't be named externally at
all. Adds a public, feature-gated constructor and migrates every known
broken call site (tru-ols, peacoqc-rs, gates, plus flow-fcs's own
compress-feature tests) to use it instead."
```

**Known residual scope, not this task's job:** `tru-ols-cli` and `peacoqc-py` also construct `Fcs { .. }` literals directly and will need the same `Fcs::for_testing` migration, but neither is currently a `[workspace] members` entry, so neither blocks `cargo test --workspace` today. `tru-ols-cli`'s workspace-membership status is already tracked by `flow-crates-ihm`; note in your report that its `Fcs` construction sites will need this same fix whenever that bead is resolved, so the connection isn't lost.

---

### Task 8: Benchmark the new paths against the existing eager parse

**Files:**
- Create: `fcs/benches/lazy_column_access.rs`
- Modify: `fcs/Cargo.toml` (register the new `[[bench]]`)

**Interfaces:**
- Consumes: `Fcs::open`, `Fcs::column`, `Fcs::columns`, `Fcs::events` (all public by end of Task 5).
- Produces: a criterion report comparing (a) opening + reading 2 columns via `.columns()` vs. opening + reading via the eager `data_frame`, and (b) `.events()` vs. the eager path, on a real file. This is what Task-level "measure before declaring the memory design worth Stage B" evidence looks like — the spec's savings claims are about memory, but this stage must not silently regress CPU time in exchange.

- [ ] **Step 1: Write the benchmark**

Follow the existing convention in `fcs/benches/column_extract.rs` (criterion group/benchmark_group/throughput). Create `fcs/benches/lazy_column_access.rs`:

```rust
//! Criterion: lazy column/events access vs. the existing eager `data_frame`
//! parse, on a real compliance-corpus file. Stage A must not regress the
//! already-eager path's performance while adding the lazy one.

use criterion::{Criterion, criterion_group, criterion_main};
use flow_fcs::file::Fcs;
use std::hint::black_box;
use std::time::Duration;

const COMPLIANCE_FCS: &str =
    "/Users/kfls271/Rust/flow-crates/gates/Gating-ML.v1.5.081030.Compliance-tests.081030/List-mode Data Files/int-10000_events_random.fcs";

fn bench_two_column_access(c: &mut Criterion) {
    let fcs = Fcs::open(COMPLIANCE_FCS).expect("open compliance fixture");
    let names = fcs.get_parameter_names_from_dataframe();
    let (a, b) = (names[0].clone(), names[1].clone());

    let mut group = c.benchmark_group("two_column_access");
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("lazy_columns_uncached", |bencher| {
        bencher.iter_batched(
            || Fcs::open(COMPLIANCE_FCS).expect("reopen for cold cache"),
            |fresh| black_box(fresh.columns(&[&a, &b]).expect("columns")),
            criterion::BatchSize::LargeInput,
        );
    });

    group.bench_function("eager_data_frame_two_columns", |bencher| {
        bencher.iter(|| {
            let x = fcs.get_parameter_events_slice(&a).expect("a");
            let y = fcs.get_parameter_events_slice(&b).expect("b");
            black_box((x, y))
        });
    });

    group.finish();
}

fn bench_full_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_materialization");
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(20);

    group.bench_function("events_uncached", |bencher| {
        bencher.iter_batched(
            || Fcs::open(COMPLIANCE_FCS).expect("reopen for cold cache"),
            |fresh| black_box(fresh.events().expect("events")),
            criterion::BatchSize::LargeInput,
        );
    });

    group.bench_function("open_eager_baseline", |bencher| {
        bencher.iter(|| black_box(Fcs::open(COMPLIANCE_FCS).expect("open")));
    });

    group.finish();
}

criterion_group!(benches, bench_two_column_access, bench_full_materialization);
criterion_main!(benches);
```

In `fcs/Cargo.toml`, add alongside the existing `[[bench]]` entries (near line 102, after `column_extract`):
```toml
[[bench]]
name   ="lazy_column_access"
harness=false
```

- [ ] **Step 2: Run it**

Run: `cargo bench -p flow-fcs --bench lazy_column_access`
Expected: completes and prints a criterion report. There's no pass/fail assertion here — this is a measurement, not a test. Record the reported numbers in the task's completion notes (or the bead) so a future reader knows whether `two_column_access/lazy_columns_uncached` is in the same ballpark as `eager_data_frame_two_columns`, and whether `full_materialization/events_uncached` is roughly at parity with `open_eager_baseline` (it should be — same traversal, same decode logic, no `Arc<DataFrame>` double-parse).

- [ ] **Step 3: Commit**

```bash
git add fcs/benches/lazy_column_access.rs fcs/Cargo.toml
git commit -m "bench(fcs): compare lazy column/events access against the eager baseline"
```

---

## Self-Review Notes

- **Spec coverage:** Stage A's full bullet list (`Arc<[OnceLock<Box<[f32]>>]>`, extraction primitive, `column()`/`columns()`/`events()`, oracle testing against `data_frame`, "events() must not populate the cache", bit-packed fallback) is covered by Tasks 2-6. `open_metadata_only()` is correctly *excluded* per the spec's Stage-A/B split. The `Arc<[..]>`-not-`Vec<..>` Clone-sharing footgun from the spec is called out in Task 4's struct comment and constraint list.
- **Type consistency:** `ColumnLayout` (Task 2) is consumed identically in Tasks 3, 4, and 5. `extract_columns`'s signature (`data_bytes: &[u8], layout: &ColumnLayout, wanted: &[usize]) -> Result<Vec<Box<[f32]>>>`) is used the same way in `column()`, `columns()`, and `events()`. `parameter_number - 1` for cache indexing is applied consistently in `column()` and `columns()`.
- **No placeholders:** every step has literal code, not a description of code. The one deliberately-scoped-out item (`open_metadata_only`) is explained, not left vague.
