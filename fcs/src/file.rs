// Internal crate imports
use crate::{
    FcsDataType, TransformType, Transformable,
    byteorder::ByteOrder,
    crc::StoredCrc,
    header::Header,
    keyword::{IntegerableKeyword, StringableKeyword},
    metadata::Metadata,
    parameter::{EventDataFrame, EventDatum, Parameter, ParameterBuilder, ParameterMap},
};
// Standard library imports
use std::borrow::Cow;
use std::fs::File;
use std::ops::{Deref, RangeInclusive};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// External crate imports
use anyhow::{Context, Result, anyhow};
use byteorder::{BigEndian as BE, ByteOrder as BO, LittleEndian as LE};
use faer::Mat;
use itertools::{Itertools, MinMaxResult};
use memmap3::{Mmap, MmapOptions};
use polars::prelude::*;
use rayon::prelude::*;

/// Threshold for parallel processing: only use parallel for datasets larger than this
/// Below this threshold, parallel overhead exceeds benefits
/// Based on benchmarks: 400,000 values (50,000 events × 8 parameters)
/// - Float32: Always use sequential (benchmarks show sequential is 2-13x faster)
/// - Int16/Int32/Float64: Use parallel for datasets with ≥400k values
pub(crate) const PARALLEL_THRESHOLD: usize = 400_000;

/// A shareable wrapper around the file path and memory-map
///
/// Uses Arc<Mmap> to share the memory mapping across clones without creating
/// new file descriptors or memory mappings. This is more efficient than cloning
/// the underlying file descriptor and re-mapping.
#[derive(Debug, Clone)]
pub struct AccessWrapper {
    /// An owned, mutable path to the file on disk
    pub path: PathBuf,
    /// The memory-mapped file, shared via Arc for efficient cloning
    ///
    /// # Safety
    /// The Mmap is created from a File handle and remains valid as long as:
    /// 1. The file is not truncated while mapped
    /// 2. The file contents are not modified while mapped (we only read)
    /// 3. The Mmap is not accessed after the file is deleted
    ///
    /// Our usage satisfies these invariants because:
    /// - FCS files are read-only once opened (we never write back to them)
    /// - The file remains open (via File handle) for the lifetime of the Mmap
    /// - We only drop the Mmap when the FCS file is no longer needed
    pub mmap: Arc<Mmap>,
}

impl AccessWrapper {
    /// Creates a new `AccessWrapper` from a file path
    /// # Errors
    /// Will return `Err` if:
    /// - the file cannot be opened
    /// - the file cannot be memory-mapped
    pub fn new(path: &str) -> Result<Self> {
        let file = File::open(path)?;
        let path = PathBuf::from(path);

        // memmap3 provides better safety guarantees than memmap2, though OS-level
        // memory mapping still requires unsafe at creation time. The resulting Mmap
        // is safe to use and provides better guarantees than memmap2.
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        Ok(Self {
            path,
            mmap: Arc::new(mmap),
        })
    }
}

impl Deref for AccessWrapper {
    type Target = Mmap;

    fn deref(&self) -> &Self::Target {
        &self.mmap
    }
}

/// A struct representing an FCS file
#[derive(Debug, Clone)]
pub struct Fcs {
    /// The header segment of the fcs file, including the version, and byte offsets to the text, data, and analysis segments
    pub header: Header,
    /// The metadata segment of the fcs file, including the delimiter, and a hashmap of keyword/value pairs
    pub metadata: Metadata,
    /// A hashmap of the parameter names and their associated metadata
    pub parameters: ParameterMap,

    /// Event data stored in columnar format via Polars DataFrame (NEW)
    /// Each column represents one parameter (e.g., FSC-A, SSC-A, FL1-A)
    /// Polars provides:
    /// - Zero-copy column access
    /// - Built-in SIMD operations
    /// - Lazy evaluation for complex queries
    /// - Apache Arrow interop
    /// This is the primary data format going forward
    pub data_frame: EventDataFrame,

    /// A wrapper around the file, path, and memory-map
    pub file_access: AccessWrapper,

    /// Per-parameter lazy column cache, indexed by `parameter_number - 1`.
    /// `Arc<[..]>` (not `Vec<..>`) so `Fcs`'s derived `Clone` shares the
    /// warmed cache across clones instead of deep-copying every populated
    /// column — `OnceLock<T: Clone>` is itself `Clone` and would otherwise
    /// duplicate contents. Sized once at `$PAR` length and never resized, so
    /// element addresses are stable for the lifetime of the `Fcs`.
    ///
    /// `pub(crate)` rather than fully private: sibling modules (e.g.
    /// `write`'s tests) construct `Fcs` via struct-literal syntax and need
    /// to supply this field, but it is not part of the public API — external
    /// callers only ever reach it through `column()`/`columns()`.
    pub(crate) columns: std::sync::Arc<[std::sync::OnceLock<Box<[f32]>>]>,

    /// Byte offset of this data set's first byte within the file.
    ///
    /// Zero for the first (usually only) data set; for each data set reached via
    /// `$NEXTDATA` it is the previous data set's start plus that offset. Every
    /// offset in [`header`](Self::header) and in the `$BEGIN*`/`$END*` keywords
    /// is measured from here, not from byte zero (§2.4.3) - see
    /// [`resolve_segment_offsets`].
    pub dataset_start: usize,
}

/// Extract one parameter column from row-major flat `f32` event data.
///
/// FCS DATA is stored as `event0_p0, event0_p1, …, event1_p0, …`.
///
/// Note: a strided `get_unchecked` + `set_len` variant was A/B'd and regressed
/// vs this iterator path (see `fcs/docs/PERF_AB.md`); keep the collect form.
#[doc(hidden)]
#[inline]
pub fn extract_param_column(
    f32_values: &[f32],
    _n_events: usize,
    n_params: usize,
    param_idx: usize,
) -> Vec<f32> {
    f32_values
        .iter()
        .skip(param_idx)
        .step_by(n_params)
        .copied()
        .collect()
}

/// De-interleave all parameter columns from row-major flat `f32` event data.
#[doc(hidden)]
#[inline]
pub fn extract_all_param_columns(
    f32_values: &[f32],
    n_events: usize,
    n_params: usize,
) -> Vec<Vec<f32>> {
    (0..n_params)
        .map(|param_idx| extract_param_column(f32_values, n_events, n_params, param_idx))
        .collect()
}

/// Where a data set's segments actually live, after resolving the HEADER's
/// 8-digit offset fields against the TEXT keywords.
///
/// Offsets here are **file-absolute** - they can index the mmap directly. The
/// values on disk are not: §2.4.3 measures every HEADER field, and §3.3.3 every
/// `$BEGIN*`/`$END*` keyword, from the start of the *data set* that declares
/// them. [`resolve_segment_offsets`] is where the two coordinate systems meet.
/// Ranges are inclusive of their end byte, matching [`Header`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentOffsets {
    pub text: RangeInclusive<usize>,
    pub data: RangeInclusive<usize>,
    /// `None` when the data set has no ANALYSIS segment, which is the norm.
    pub analysis: Option<RangeInclusive<usize>>,
    /// Vendor-defined OTHER segments (§3.6), in HEADER order. Usually empty.
    pub other: Vec<RangeInclusive<usize>>,
}

impl SegmentOffsets {
    /// Offset of the last byte belonging to any segment of the data set.
    ///
    /// §3.7 defines the CRC's input as running through "the last byte of the
    /// final segment of the data set (which may be a TEXT, DATA, ANALYSIS or
    /// OTHER segment)" - all four, and in no promised order, so this takes a max
    /// rather than assuming DATA is last.
    ///
    /// Omitting OTHER here is not a harmless gap: it would make
    /// [`computed_crc`](Fcs::computed_crc) hash a range that stops short of the
    /// real end, so a *valid* file with a trailing OTHER segment would be
    /// reported as corrupt and refused by [`open_verified`](Fcs::open_verified).
    #[must_use]
    pub fn last_byte(&self) -> usize {
        let analysis_end = self.analysis.as_ref().map_or(0, |range| *range.end());
        let other_end = self.other.iter().map(|range| *range.end()).max().unwrap_or(0);
        (*self.text.end())
            .max(*self.data.end())
            .max(analysis_end)
            .max(other_end)
    }
}

/// Resolves one declared offset to a file-absolute position.
///
/// Every FCS offset is measured from the start of the *data set* that declares
/// it, not from the start of the file: HEADER fields per §2.4.3, `$BEGINDATA`
/// and `$BEGINANALYSIS` per §3.3.3, `$NEXTDATA` per §3.3.31. For the first data
/// set those are the same number, which is why the distinction stays invisible
/// until a file carries a *third* data set: a two-data-set chain only ever takes
/// one hop, and that hop starts at zero.
///
/// Vendors get this wrong often enough that a strict reading would fail to open
/// real files, so rather than assume, this disambiguates:
///
/// - `offset < dataset_start` can only be relative; read absolutely it would
///   place the segment before the data set that owns it.
/// - otherwise prefer the relative reading, unless it runs off the end of the
///   file - in which case the writer must have meant file-absolute.
///
/// The two readings are genuinely indistinguishable when `dataset_start <=
/// offset` and `dataset_start + offset` still fits in the file. The spec reading
/// wins there.
fn absolutize(offset: usize, dataset_start: usize, file_len: usize, what: &str) -> usize {
    if dataset_start == 0 {
        return offset;
    }
    let relative = dataset_start.saturating_add(offset);
    if offset >= dataset_start && relative >= file_len {
        tracing::warn!(
            field = what,
            offset,
            dataset_start,
            file_len,
            "FCS offset is not data-set-relative as §2.4.3 requires; reading it as \
             file-absolute, since the relative reading runs past the end of the file"
        );
        return offset;
    }
    relative
}

/// Resolves a data set's segment offsets to **file-absolute** ranges, falling
/// back to the TEXT keywords wherever the HEADER declares zero.
///
/// `dataset_start` is the file offset of this data set's first byte - zero for
/// the first data set, and `previous_start + $NEXTDATA` for each one after it.
/// See [`absolutize`] for how the on-disk relative offsets are rebased.
///
/// **Do not read the offsets off [`Header`] directly.** `Header::from_bytes`
/// reports the HEADER's 8-digit fields verbatim, and §2.2.4 requires those
/// fields to be `0` whenever a segment falls beyond the first 99,999,999 bytes,
/// with the real values carried in `$BEGINDATA`/`$ENDDATA` and
/// `$BEGINANALYSIS`/`$ENDANALYSIS`. A spec-conformant ~100 MB file therefore
/// reports `0..=0` for DATA, so any caller that trusts the header alone silently
/// reads the wrong byte range on exactly the large files this fallback exists
/// for - and on any data set past the first, it reads the wrong range full stop.
///
/// # Errors
/// Returns an error if DATA's offsets are zero in the HEADER and the
/// corresponding keywords are absent or unparseable - a data set whose DATA
/// segment cannot be located is not usable.
pub fn resolve_segment_offsets(
    header: &Header,
    metadata: &Metadata,
    dataset_start: usize,
    file_len: usize,
) -> Result<SegmentOffsets> {
    let keyword = |key: &str| {
        metadata
            .get_integer_keyword(key)
            .ok()
            .map(|value| *value.get_usize())
    };
    let rebase = |offset: usize, what: &str| absolutize(offset, dataset_start, file_len, what);

    // TEXT needs no fallback: §3.2.1 requires the primary TEXT segment to lie
    // entirely within the first 99,999,999 bytes of its data set, so its HEADER
    // offsets are always real.
    let text = text_range(header, dataset_start, file_len);

    let mut data_start = *header.data_offset.start();
    let mut data_end = *header.data_offset.end();
    if data_start == 0 {
        data_start = keyword("$BEGINDATA").ok_or_else(|| {
            anyhow!("$BEGINDATA keyword not found. Unable to determine data start.")
        })?;
    }
    if data_end == 0 {
        data_end = keyword("$ENDDATA")
            .ok_or_else(|| anyhow!("$ENDDATA keyword not found. Unable to determine data end."))?;
    }

    // ANALYSIS is optional, and an absent one is spelled `0 0` in the HEADER -
    // the same spelling as "too large to declare here". The keywords
    // disambiguate: present and non-zero means a real segment, anything else
    // means there isn't one. Unlike DATA, a missing ANALYSIS is not an error.
    let analysis_start = match *header.analysis_offset.start() {
        0 => keyword("$BEGINANALYSIS").unwrap_or(0),
        offset => offset,
    };
    let analysis_end = match *header.analysis_offset.end() {
        0 => keyword("$ENDANALYSIS").unwrap_or(0),
        offset => offset,
    };
    let analysis = (analysis_end > 0 && analysis_end >= analysis_start).then(|| {
        rebase(analysis_start, "$BEGINANALYSIS")..=rebase(analysis_end, "$ENDANALYSIS")
    });

    // OTHER segments have no keyword fallback - §3.6 confines them to the first
    // 99,999,999 bytes of the data set precisely so the HEADER can always
    // declare them - so the HEADER pairs are the whole story.
    let other = header
        .other_offsets
        .iter()
        .map(|range| {
            rebase(*range.start(), "OTHER begin")..=rebase(*range.end(), "OTHER end")
        })
        .collect();

    Ok(SegmentOffsets {
        text,
        data: rebase(data_start, "$BEGINDATA")..=rebase(data_end, "$ENDDATA"),
        analysis,
        other,
    })
}

/// The file-absolute byte range of a data set's TEXT segment.
///
/// Split out of [`resolve_segment_offsets`] because it is the one range that can
/// be computed without the metadata - and it has to be, since reading the
/// metadata means reading TEXT first.
fn text_range(header: &Header, dataset_start: usize, file_len: usize) -> RangeInclusive<usize> {
    absolutize(*header.text_offset.start(), dataset_start, file_len, "TEXT begin")
        ..=absolutize(*header.text_offset.end(), dataset_start, file_len, "TEXT end")
}

/// A cursor for reading arbitrary-width unsigned integers from a byte buffer,
/// one bit at a time, MSB-first within each byte.
///
/// Used only for bit-packed (`$PnB` not a multiple of 8) FCS records, where
/// values aren't byte-aligned and the fast byte-stride paths don't apply.
struct BitReader<'a> {
    bytes: &'a [u8],
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bit_pos: 0 }
    }

    /// Read the next `bits` bits (MSB-first) as a `u32`.
    ///
    /// # Errors
    /// Will return `Err` if `bits` is more than 32, or if fewer than `bits`
    /// bits remain in the buffer.
    fn read_bits(&mut self, bits: usize) -> Result<u32> {
        if bits > 32 {
            return Err(anyhow!(
                "Bit-packed parameter width {bits} exceeds the 32-bit reader limit"
            ));
        }
        if self.bit_pos + bits > self.bytes.len() * 8 {
            return Err(anyhow!(
                "Bit-packed record ended mid-value: needed {bits} more bits at bit offset {}, only {} bits remain",
                self.bit_pos,
                self.bytes.len() * 8 - self.bit_pos
            ));
        }

        let mut value: u32 = 0;
        for _ in 0..bits {
            let byte = self.bytes[self.bit_pos / 8];
            let shift = 7 - (self.bit_pos % 8);
            let bit = (byte >> shift) & 1;
            value = (value << 1) | u32::from(bit);
            self.bit_pos += 1;
        }
        Ok(value)
    }
}

impl Fcs {
    /// Creates a new Fcs file struct
    /// # Errors
    /// Will return `Err` if:
    /// - the file cannot be opened,
    /// - the file extension is not `fcs`,
    /// - the TEXT segment cannot be validated,
    /// - the raw data cannot be read,
    /// - the parameter names and labels cannot be generated
    pub fn new() -> Result<Self> {
        Ok(Self {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: ParameterMap::default(),
            data_frame: Arc::new(DataFrame::empty()),
            file_access: AccessWrapper::new("")?,
            columns: std::iter::repeat_with(std::sync::OnceLock::new).take(0).collect(),
            dataset_start: 0,
        })
    }

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
            dataset_start: 0,
        }
    }

    /// Opens and parses an FCS file from the given path
    ///
    /// This is the primary entry point for reading FCS files. It:
    /// - Validates the file extension (must be `.fcs`)
    /// - Memory-maps the file for efficient access
    /// - Parses the header segment to determine FCS version and segment offsets
    /// - Parses the text segment to extract metadata and keywords
    /// - Validates required keywords for the FCS version
    /// - Generates a GUID if one is not present
    /// - Loads event data into a Polars DataFrame for efficient columnar access
    ///
    /// # Arguments
    /// * `path` - Path to the FCS file (must have `.fcs` extension)
    ///
    /// # Errors
    /// Will return `Err` if:
    /// - the file cannot be opened or memory-mapped
    /// - the file extension is not `.fcs`
    /// - the FCS version is invalid or unsupported
    /// - required keywords are missing for the FCS version
    /// - the data segment cannot be read or parsed
    /// - parameter metadata cannot be generated
    ///
    /// # Example
    /// ```no_run
    /// use flow_fcs::Fcs;
    ///
    /// let fcs = Fcs::open("data/sample.fcs")?;
    /// println!("File has {} events", fcs.get_number_of_events()?);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn open(path: &str) -> Result<Self> {
        // Attempt to open the file path
        let file_access =
            AccessWrapper::new(path).with_context(|| format!("Failed to open file: {}", path))?;

        // Validate the file extension
        Self::validate_fcs_extension(&file_access.path).with_context(|| {
            format!("Invalid file extension for: {}", file_access.path.display())
        })?;

        // Create the header struct from a memory map of the file
        let header = Header::from_mmap(&file_access.mmap).with_context(|| {
            format!(
                "Failed to parse header from file: {}",
                file_access.path.display()
            )
        })?;

        let (fcs, _next_data_offset) = Self::parse_one_dataset(file_access, header, 0)?;
        fcs.warn_if_crc_conflicts();
        Ok(fcs)
    }

    /// As [`open`](Self::open), but refuses a file whose stored CRC contradicts
    /// its contents.
    ///
    /// `open` deliberately only warns: vendor files routinely carry an absent or
    /// simply wrong CRC, and hard-failing would make them unopenable for no
    /// safety gain. Use this where a corrupt file must not be processed - an
    /// ingest boundary, or anything that will overwrite the source.
    ///
    /// A file with no CRC (the `00000000` opt-out) is accepted: it asserts
    /// nothing, so there is nothing to contradict. Only a stored value that
    /// disagrees with the bytes is an error.
    ///
    /// # Errors
    /// As [`open`](Self::open), plus an error when the stored CRC conflicts with
    /// the computed one.
    pub fn open_verified(path: &str) -> Result<Self> {
        let fcs = Self::open(path)?;
        fcs.verify_crc()?;
        Ok(fcs)
    }

    /// The CRC field exactly as it appears on disk (§3.7).
    ///
    /// Reads the eight bytes immediately after this data set's final segment.
    /// Files written before this crate supported the CRC report
    /// [`StoredCrc::Missing`] - the bytes simply are not there.
    #[must_use]
    pub fn stored_crc(&self) -> StoredCrc {
        let Ok(segments) = self.segment_offsets() else {
            return StoredCrc::Missing;
        };
        crate::crc::parse_field(&self.file_access.mmap, segments.last_byte() + 1)
    }

    /// This data set's segments, as file-absolute byte ranges.
    ///
    /// # Errors
    /// As [`resolve_segment_offsets`].
    pub fn segment_offsets(&self) -> Result<SegmentOffsets> {
        resolve_segment_offsets(
            &self.header,
            &self.metadata,
            self.dataset_start,
            self.file_access.mmap.len(),
        )
    }

    /// CRC-16/KERMIT over this data set's bytes, as §3.7 defines the range:
    /// the first byte of the HEADER through the last byte of the final segment.
    ///
    /// §3.7 scopes the CRC to a data set, not a file, so a `$NEXTDATA`-chained
    /// data set hashes from [`dataset_start`](Self::dataset_start) rather than
    /// from byte zero.
    ///
    /// # Errors
    /// Returns an error if the segment offsets cannot be resolved, or if the
    /// declared range runs past the end of the file.
    pub fn computed_crc(&self) -> Result<u16> {
        let segments = self.segment_offsets()?;
        let start = self.dataset_start;
        let end = segments.last_byte();
        let mmap = &self.file_access.mmap;
        let bytes = mmap.get(start..=end).ok_or_else(|| {
            anyhow!(
                "data set at byte {start} declares its final segment ending at byte {end}, \
                 past the {} byte file",
                mmap.len()
            )
        })?;
        Ok(crate::crc::compute(bytes))
    }

    /// Whether this `Fcs` is the first data set in its file.
    #[must_use]
    pub const fn is_first_dataset(&self) -> bool {
        self.dataset_start == 0
    }

    /// Checks the stored CRC against the computed one.
    ///
    /// # Errors
    /// Returns an error only when a stored CRC *value* disagrees with the bytes.
    /// Absent, malformed, and missing fields pass: they make no claim, and
    /// rejecting them would fail every pre-CRC file in existence.
    pub fn verify_crc(&self) -> Result<()> {
        let stored = self.stored_crc();
        let StoredCrc::Value(claimed) = stored else {
            return Ok(());
        };
        let computed = self.computed_crc()?;
        if claimed == computed {
            return Ok(());
        }
        Err(anyhow!(
            "CRC mismatch in {}: file stores {claimed} but its bytes hash to {computed} \
             (the data set is corrupt, or was modified without updating the CRC)",
            self.file_access.path.display()
        ))
    }

    /// Logs a warning if the stored CRC contradicts the file's bytes.
    ///
    /// Called from [`open`](Self::open) so corruption is at least visible on the
    /// default path. Any failure to *evaluate* the CRC is itself only a debug
    /// note: a file we cannot check is not a file we should refuse.
    fn warn_if_crc_conflicts(&self) {
        let StoredCrc::Value(claimed) = self.stored_crc() else {
            return;
        };
        match self.computed_crc() {
            Ok(computed) if claimed != computed => tracing::warn!(
                file = %self.file_access.path.display(),
                stored = claimed,
                computed,
                "FCS CRC mismatch: the file may be corrupt or was modified without \
                 updating its CRC. Continuing anyway; use Fcs::open_verified to reject."
            ),
            Ok(_) => {}
            Err(error) => tracing::debug!(
                file = %self.file_access.path.display(),
                %error,
                "could not evaluate the stored FCS CRC"
            ),
        }
    }

    /// Opens and parses every dataset chained via `$NEXTDATA` in an FCS file
    ///
    /// Most FCS files contain exactly one dataset, in which case this costs the same
    /// as `open()` and returns a single-element vec. Multi-dataset files (e.g. Beckman
    /// `.lmd` exports) chain additional datasets via `$NEXTDATA`, which §3.3.31 defines
    /// as "the byte offset from the beginning of *a data set* to the first byte in the
    /// HEADER of the next data set" - **relative, not file-absolute**. Each hop is
    /// therefore `dataset_start + $NEXTDATA`, and the two only coincide for the first
    /// hop, which starts at zero. A two-data-set file takes exactly one hop and so
    /// cannot tell the two readings apart; it takes a third data set to see the
    /// difference, which is why `.lmd` files (always exactly two) never exposed this.
    ///
    /// §2.4.2 requires every data set to carry its own 58-byte HEADER, so that is what
    /// each hop looks for first. Vendors that omit it fall back to
    /// `header_for_dataset_at`, which treats the target as a bare TEXT segment and
    /// derives its end from the data set's own `$BEGINDATA`.
    ///
    /// `open()` deliberately stays a single-dataset call rather than being replaced by
    /// this: for the common single-dataset file it's the same cost, but for a genuine
    /// multi-dataset file it avoids parsing (and risking failure on) datasets a caller
    /// never asked for.
    ///
    /// # Errors
    /// Will return `Err` under the same conditions as `open()`, for any dataset in the
    /// chain, or if the `$NEXTDATA` chain points outside the file or loops back on
    /// itself.
    pub fn open_all(path: &str) -> Result<Vec<Self>> {
        let file_access =
            AccessWrapper::new(path).with_context(|| format!("Failed to open file: {}", path))?;

        Self::validate_fcs_extension(&file_access.path).with_context(|| {
            format!("Invalid file extension for: {}", file_access.path.display())
        })?;

        let mut header = Header::from_mmap(&file_access.mmap).with_context(|| {
            format!(
                "Failed to parse header from file: {}",
                file_access.path.display()
            )
        })?;
        let version = header.version;
        let mmap_len = file_access.mmap.len();

        let mut datasets = Vec::new();
        let mut dataset_start = 0usize;
        // Keyed on the data set's own base, not on its TEXT offset: under the
        // relative model every data set in the chain reports the same TEXT
        // offset (58, or 0 for a headerless one), so TEXT offsets would look
        // like a loop on the very first hop.
        let mut seen_starts = std::collections::HashSet::new();
        loop {
            if !seen_starts.insert(dataset_start) {
                return Err(anyhow!(
                    "$NEXTDATA chain looped back to an already-visited data set at byte \
                     {dataset_start}"
                ));
            }

            let (fcs, next_data_offset) =
                Self::parse_one_dataset(file_access.clone(), header, dataset_start)?;
            datasets.push(fcs);

            if next_data_offset == 0 {
                break;
            }
            // §3.3.31: relative to *this* data set's start, not the file's.
            let next_start = absolutize(next_data_offset, dataset_start, mmap_len, "$NEXTDATA");
            if next_start >= mmap_len {
                return Err(anyhow!(
                    "$NEXTDATA offset {next_data_offset} from the data set at byte \
                     {dataset_start} points to byte {next_start}, beyond the {mmap_len} byte file"
                ));
            }
            dataset_start = next_start;
            header = Self::header_for_dataset_at(&file_access.mmap, dataset_start, version)?;
        }

        Ok(datasets)
    }

    /// Parses a single dataset's TEXT/DATA segments, given a `Header` already located
    /// (either the file's primary 58-byte HEADER for dataset 1, or a synthetic header
    /// from `header_for_dataset_at` for dataset 2+).
    ///
    /// Returns the parsed `Fcs` plus the `$NEXTDATA` value (0 if absent/unparseable,
    /// meaning "no further dataset").
    ///
    /// # Errors
    /// Will return `Err` if the TEXT segment cannot be validated, the raw data cannot
    /// be read, or the parameter names and labels cannot be generated.
    fn parse_one_dataset(
        file_access: AccessWrapper,
        header: Header,
        dataset_start: usize,
    ) -> Result<(Self, usize)> {
        use tracing::debug;

        let mmap_len = file_access.mmap.len();
        let mut metadata = Metadata::from_text_segment(
            &file_access.mmap,
            &text_range(&header, dataset_start, mmap_len),
        );

        metadata
            .validate_text_segment_keywords(&header)
            .with_context(|| {
                format!(
                    "Failed to validate text segment keywords in file: {}",
                    file_access.path.display()
                )
            })?;
        // metadata.validate_number_of_parameters()?;
        metadata.validate_guid();

        // Log $TOT keyword value
        let tot_events = metadata.get_number_of_events().ok().copied();
        if let Some(tot) = tot_events {
            debug!("FCS file $TOT keyword: {} events", tot);
        }

        let parameters = Self::generate_parameter_map(&metadata).map_err(|e| {
            let diagnostic = Self::format_diagnostic_info(&header, &metadata, &file_access.path);
            anyhow!("Failed to generate parameter map: {}\n\n{}", e, diagnostic)
        })?;

        let data_frame =
            Self::store_raw_data_as_dataframe(&header, &file_access.mmap, &metadata, dataset_start)
                .map_err(|e| {
                    let diagnostic =
                        Self::format_diagnostic_info(&header, &metadata, &file_access.path);
                    anyhow!(
                        "Failed to store raw data as DataFrame: {}\n\n{}",
                        e,
                        diagnostic
                    )
                })?;

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
            dataset_start,
        };

        // Log DataFrame event count and compare to $TOT
        let df_events = fcs.get_event_count_from_dataframe();
        if let Some(tot) = tot_events {
            if df_events != tot {
                tracing::warn!(
                    "Event count mismatch: DataFrame has {} events but $TOT keyword says {} (difference: {})",
                    df_events,
                    tot,
                    tot as i64 - df_events as i64
                );
            } else {
                debug!("Event count matches $TOT keyword: {} events", df_events);
            }
        }

        // Log compensation status
        let has_compensation = fcs.has_compensation();
        debug!(
            "Compensation: {} (SPILLOVER keyword {})",
            if has_compensation {
                "available"
            } else {
                "not available"
            },
            if has_compensation {
                "present"
            } else {
                "missing"
            }
        );

        // Log parameter count
        let n_params = fcs.parameters.len();
        debug!(
            "FCS file loaded: {} parameters, {} events",
            n_params, df_events
        );

        // $NEXTDATA has no dedicated IntegerKeyword variant (see keyword/parsing.rs's
        // parse_fixed_keywords) — it always types as StringKeyword::Other, so it must be
        // read back out as a string and parsed manually.
        let next_data_offset = fcs
            .metadata
            .get_string_keyword("$NEXTDATA")
            .ok()
            .and_then(|kw| kw.get_str().trim().parse::<usize>().ok())
            .unwrap_or(0);

        Ok((fcs, next_data_offset))
    }

    /// Produces the `Header` for a data set reached via `$NEXTDATA`, whose first byte is
    /// at file offset `dataset_start`.
    ///
    /// §2.4.2 makes a HEADER mandatory for every data set, so the real 58-byte HEADER is
    /// tried first and used when it parses. Some vendors omit it and place a bare TEXT
    /// segment at the `$NEXTDATA` target instead; for those, a synthetic header is
    /// built. Either way the returned offsets are **data-set-relative**, matching what a
    /// real HEADER carries, so the rest of the pipeline rebases them uniformly.
    ///
    /// The synthetic case has no explicit "end of TEXT" value — it can only be derived
    /// as `$BEGINDATA - 1`, and `$BEGINDATA` is itself one of the keyword/value pairs
    /// inside the TEXT segment being bounded. `find_begindata_offset` performs a
    /// bounded, early-stopping scan for exactly that keyword, so it never reads into the
    /// DATA segment (which could otherwise contain byte values matching the delimiter).
    ///
    /// # Errors
    /// Will return `Err` if no HEADER is present and `$BEGINDATA` cannot be found or
    /// parsed before the end of the file, or if its value doesn't make sense as a TEXT
    /// end (at or before the start of the data set).
    fn header_for_dataset_at(
        mmap: &Mmap,
        dataset_start: usize,
        version: crate::version::Version,
    ) -> Result<Header> {
        if let Ok(header) = Header::from_bytes(&mmap[dataset_start..]) {
            return Ok(header);
        }

        let mmap_len = mmap.len();
        let begin_data = absolutize(
            Self::find_begindata_offset(mmap, dataset_start)?,
            dataset_start,
            mmap_len,
            "$BEGINDATA",
        );
        if begin_data <= dataset_start {
            return Err(anyhow!(
                "Invalid $BEGINDATA offset {begin_data} for headerless data set starting at \
                 {dataset_start}"
            ));
        }

        Ok(Header {
            version,
            // Relative: TEXT is the whole data set up to DATA, since there is no HEADER
            // occupying the first 58 bytes.
            text_offset: 0..=(begin_data - dataset_start - 1),
            data_offset: 0..=0,
            analysis_offset: 0..=0,
            // A data set with no HEADER cannot declare OTHER segments: §3.6 puts
            // their offsets in the HEADER and nowhere else.
            other_offsets: Vec::new(),
        })
    }

    /// Scans a TEXT segment starting at `text_start`, stopping as soon as `$BEGINDATA`'s
    /// value is found, and returns that value.
    ///
    /// Shares `crate::text::TextFields` with `Metadata::from_text_segment`, so the two
    /// cannot drift, but stops at the first match instead of tokenizing the whole
    /// segment — the segment's end isn't known yet, which is the value this function
    /// exists to find.
    ///
    /// # Errors
    /// Will return `Err` if `$BEGINDATA` is not found before the end of the mmap, or its
    /// value is not a valid unsigned integer.
    fn find_begindata_offset(mmap: &Mmap, text_start: usize) -> Result<usize> {
        let delimiter = mmap[text_start];
        let rest = &mmap[(text_start + 1)..];

        let mut fields = crate::text::TextFields::new(
            rest,
            delimiter,
            crate::text::Escaping::None,
        );

        while let Some(key) = fields.next() {
            let Some(value) = fields.next() else { break };
            if key.eq_ignore_ascii_case("$BEGINDATA") {
                return value.trim().parse::<usize>().with_context(|| {
                    format!("Invalid $BEGINDATA value '{value}' while scanning for next dataset's TEXT boundary")
                });
            }
        }

        Err(anyhow!(
            "Reached end of file while scanning for $BEGINDATA in dataset TEXT segment starting at offset {text_start}"
        ))
    }

    /// Validates that the file extension is `.fcs`
    /// # Errors
    /// Will return `Err` if the file extension is not `.fcs`
    fn validate_fcs_extension(path: &Path) -> Result<()> {
        let extension = path
            .extension()
            .ok_or_else(|| anyhow!("File has no extension"))?
            .to_str()
            .ok_or_else(|| anyhow!("File extension is not valid UTF-8"))?;

        if extension.to_lowercase() != "fcs" {
            return Err(anyhow!("Invalid file extension: {}", extension));
        }

        Ok(())
    }

    /// Reads raw data from FCS file and stores it as a Polars DataFrame
    /// Returns columnar data optimized for parameter-wise access patterns
    ///
    /// This function provides significant performance benefits over row-based array storage:
    /// - 2-5x faster data filtering and transformations
    /// - Native columnar storage (optimal for FCS parameter access patterns)
    /// - Zero-copy operations via Apache Arrow
    /// - Built-in SIMD acceleration
    ///
    /// # Errors
    /// Will return `Err` if:
    /// - The data cannot be read
    /// - The data cannot be converted to f32 values
    /// - The DataFrame cannot be constructed
    fn store_raw_data_as_dataframe(
        header: &Header,
        mmap: &Mmap,
        metadata: &Metadata,
        dataset_start: usize,
    ) -> Result<EventDataFrame> {
        // Validate data offset bounds before accessing mmap
        let mmap_len = mmap.len();
        let segments = resolve_segment_offsets(header, metadata, dataset_start, mmap_len)?;
        let data_start = *segments.data.start();
        let data_end = *segments.data.end();

        // Validate offsets
        if data_start >= mmap_len {
            return Err(anyhow!(
                "Data start offset {} is beyond mmap length {}",
                data_start,
                mmap_len
            ));
        }

        if data_end >= mmap_len {
            return Err(anyhow!(
                "Data end offset {} is beyond mmap length {}",
                data_end,
                mmap_len
            ));
        }

        if data_start > data_end {
            return Err(anyhow!(
                "Data start offset {} is greater than end offset {}",
                data_start,
                data_end
            ));
        }

        // Extract data bytes
        let data_bytes = &mmap[data_start..=data_end];

        let number_of_parameters = metadata
            .get_number_of_parameters()
            .context("Failed to retrieve the number of parameters from metadata")?;
        let number_of_events = metadata
            .get_number_of_events()
            .context("Failed to retrieve the number of events from metadata")?;

        // Calculate bytes per event by summing $PnB / 8 for each parameter
        // This is more accurate than using $DATATYPE which only provides a default
        let bytes_per_event = metadata
            .calculate_bytes_per_event()
            .context("Failed to calculate bytes per event")?;

        let byte_order = metadata
            .get_byte_order()
            .context("Failed to get the byte order from metadata")?;

        // Validate data size
        let expected_total_bytes = number_of_events * bytes_per_event;
        if data_bytes.len() < expected_total_bytes {
            return Err(anyhow!(
                "Insufficient data: expected {} bytes ({} events × {} bytes/event), but only have {} bytes",
                expected_total_bytes,
                number_of_events,
                bytes_per_event,
                data_bytes.len()
            ));
        }

        let data_types: Vec<FcsDataType> = (1..=*number_of_parameters)
            .map(|param_num| {
                metadata
                    .get_data_type_for_channel(param_num)
                    .with_context(|| format!("Failed to get data type for channel {}", param_num))
            })
            .collect::<Result<Vec<_>>>()?;

        // Raw (un-rounded) $PnB bit widths. Any parameter whose width isn't a
        // multiple of 8 means the whole record is bit-packed (values aren't
        // byte-aligned), which the byte-stride fast/variable-width paths below
        // can't represent — that requires a dedicated bit-level reader instead.
        let bits_per_parameter: Vec<usize> = (1..=*number_of_parameters)
            .map(|param_num| {
                metadata
                    .get_bits_per_parameter(param_num)
                    .with_context(|| {
                        format!("Failed to get bits per parameter for parameter {}", param_num)
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        let is_bit_packed = bits_per_parameter.iter().any(|&bits| bits % 8 != 0);

        let f32_values: Vec<f32> = if is_bit_packed {
            Self::parse_bit_packed_data(
                data_bytes,
                &bits_per_parameter,
                &data_types,
                *number_of_events,
            )?
        } else {
        // Collect bytes per parameter for each parameter
        let bytes_per_parameter: Vec<usize> = (1..=*number_of_parameters)
            .map(|param_num| {
                metadata
                    .get_bytes_per_parameter(param_num)
                    .with_context(|| {
                        format!(
                            "Failed to get bytes per parameter for parameter {}",
                            param_num
                        )
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        // Fast path: Check if all parameters are uniform (same bytes, same data type)
        // This allows us to use bytemuck zero-copy optimization
        let uniform_bytes = bytes_per_parameter.first().copied();
        let uniform_data_type = data_types.first().copied();
        let is_uniform = match (uniform_bytes, uniform_data_type) {
            (Some(bytes), Some(dt)) => {
                bytes_per_parameter.iter().all(|&b| b == bytes)
                    && data_types.iter().all(|&dt_val| dt_val == dt)
            }
            _ => false,
        };

        if is_uniform {
            // Fast path: All parameters have same size and type - use bytemuck for zero-copy
            let bytes_per_param = uniform_bytes.ok_or_else(|| anyhow!("No uniform bytes found"))?;
            let data_type =
                uniform_data_type.ok_or_else(|| anyhow!("No uniform data type found"))?;

            match (data_type, bytes_per_param) {
                (FcsDataType::F, 4) => {
                    // Fast path: float32 - use sequential (benchmarks show 2.57x faster than parallel)
                    let needs_swap = match (byte_order, cfg!(target_endian = "little")) {
                        (ByteOrder::LittleEndian, true) | (ByteOrder::BigEndian, false) => false,
                        _ => true,
                    };

                    match bytemuck::try_cast_slice::<u8, f32>(data_bytes) {
                        Ok(f32_slice) => {
                            tracing::debug!(
                                "✓ Fast path (bytemuck zero-copy, sequential): {} bytes, {} f32s",
                                data_bytes.len(),
                                f32_slice.len()
                            );

                            if needs_swap {
                                // Sequential byte swap - faster than parallel for float32
                                f32_slice
                                    .iter()
                                    .map(|&f| f32::from_bits(f.to_bits().swap_bytes()))
                                    .collect()
                            } else {
                                f32_slice.to_vec()
                            }
                        }
                        Err(_) => {
                            tracing::debug!(
                                "⚠ Fast path (bytemuck fallback, sequential): unaligned data ({} bytes)",
                                data_bytes.len()
                            );

                            // Fallback: parse in chunks sequentially (faster than parallel for float32)
                            data_bytes
                                .chunks_exact(4)
                                .map(|chunk| {
                                    let mut bytes = [0u8; 4];
                                    bytes.copy_from_slice(chunk);
                                    let bits = u32::from_ne_bytes(bytes);
                                    let bits = if needs_swap { bits.swap_bytes() } else { bits };
                                    f32::from_bits(bits)
                                })
                                .collect()
                        }
                    }
                }
                _ => {
                    // Uniform but not float32 - use optimized bulk parsing
                    Self::parse_uniform_data_bulk(
                        data_bytes,
                        bytes_per_param,
                        &data_type,
                        byte_order,
                        *number_of_events,
                        *number_of_parameters,
                    )?
                }
            }
        } else {
            // Slow path: Variable-width parameters - parse event-by-event
            Self::parse_variable_width_data(
                data_bytes,
                &bytes_per_parameter,
                &data_types,
                byte_order,
                *number_of_events,
                *number_of_parameters,
            )?
        }
        };

        // Create Polars Series for each parameter (column)
        // FCS data is stored row-wise (event1_param1, event1_param2, ..., event2_param1, ...)
        // We need to extract columns using stride access
        let mut columns: Vec<Column> = Vec::with_capacity(*number_of_parameters);

        for param_idx in 0..*number_of_parameters {
            let mut param_values = extract_param_column(
                &f32_values,
                *number_of_events,
                *number_of_parameters,
                param_idx,
            );

            // $PnR is the parameter's *true* ADC resolution, which can be narrower than
            // the storage width implied by $PnB. Some instruments (Beckman FC500/Gallios/
            // Navios, older BD) leave the unused high bits as noise rather than zeroing
            // them, so integer parameters must be masked down to their declared range.
            // Float/double parameters ($DATATYPE F/D) aren't bit-packed ADC values and
            // are exempt per spec.
            if data_types[param_idx] == FcsDataType::I {
                if let Ok(range) = metadata.get_range_for_channel(param_idx + 1) {
                    let mask = range.next_power_of_two().saturating_sub(1) as u32;
                    for value in &mut param_values {
                        *value = crate::columns::apply_range_mask(*value, Some(mask));
                    }
                }
            }

            // Verify we got the right number of events
            assert_eq!(
                param_values.len(),
                *number_of_events,
                "Parameter {} should have {} events, got {}",
                param_idx + 1,
                number_of_events,
                param_values.len()
            );

            // Get parameter name from metadata for column name
            let param_name = metadata
                .get_parameter_channel_name(param_idx + 1)
                .map(|s| s.to_string())
                .unwrap_or_else(|_| format!("P{}", param_idx + 1));

            // Create Series (Polars column) with name
            let series = Column::new(param_name.as_str().into(), param_values);
            columns.push(series);
        }

        // Create DataFrame from columns (height = number of events)
        let df = DataFrame::new(*number_of_events, columns).map_err(|e| {
            anyhow!(
                "Failed to create DataFrame from {} columns: {}",
                number_of_parameters,
                e
            )
        })?;

        // Verify DataFrame shape
        assert_eq!(
            df.height(),
            *number_of_events,
            "DataFrame height {} doesn't match expected events {}",
            df.height(),
            number_of_events
        );
        assert_eq!(
            df.width(),
            *number_of_parameters,
            "DataFrame width {} doesn't match expected parameters {}",
            df.width(),
            number_of_parameters
        );

        tracing::debug!(
            "✓ Created DataFrame: {} events × {} parameters",
            df.height(),
            df.width()
        );

        Ok(Arc::new(df))
    }

    /// Parse a bit-packed record (at least one `$PnB` not a multiple of 8)
    ///
    /// FCS bit-packing (deprecated in 3.2, but still found in older exports)
    /// stores parameters back-to-back at the bit level with no padding, so a
    /// record isn't a whole number of bytes per parameter — e.g. 8 channels of
    /// `$PnB=10` pack into a 10-byte (80-bit) record, not 16 bytes. This can't
    /// reuse the byte-stride fast/variable-width paths, so it reads sequentially
    /// with a bit cursor instead.
    ///
    /// Bit order: values are read MSB-first within the byte stream (the bit
    /// cursor consumes the most significant unread bit of the current byte
    /// first), matching the historical FCS packing convention. The FCS 3.2 spec
    /// deprecated this layout specifically because real-world vendors disagreed
    /// on bit order — this implementation is spec-derived and internally
    /// consistent (round-trips its own encoding), but hasn't been validated
    /// against a specific vendor's real bit-packed export.
    ///
    /// Only `$DATATYPE I` (integer) parameters are valid in a bit-packed record
    /// per spec; a non-integer parameter here is a metadata inconsistency.
    ///
    /// # Errors
    /// Will return `Err` if a parameter's data type isn't `I`, or if there
    /// isn't enough data for the declared number of events.
    pub(crate) fn parse_bit_packed_data(
        data_bytes: &[u8],
        bits_per_parameter: &[usize],
        data_types: &[FcsDataType],
        num_events: usize,
    ) -> Result<Vec<f32>> {
        if let Some(bad) = data_types.iter().find(|&&dt| dt != FcsDataType::I) {
            return Err(anyhow!(
                "Bit-packed ($PnB not a multiple of 8) records only support $DATATYPE I, found {:?}",
                bad
            ));
        }

        let mut reader = BitReader::new(data_bytes);
        let num_params = bits_per_parameter.len();
        let mut f32_values = Vec::with_capacity(num_events * num_params);

        for _ in 0..num_events {
            for &bits in bits_per_parameter {
                f32_values.push(reader.read_bits(bits)? as f32);
            }
        }

        Ok(f32_values)
    }

    /// Parse uniform data in bulk (all parameters have same size and type)
    ///
    /// This is faster than event-by-event parsing when all parameters are uniform.
    /// Uses conditional parallelization based on data type and size:
    /// - float32: always sequential (benchmarks show 2.57x faster)
    /// - int16/int32: parallel only above threshold (parallel is 1.84x faster for large datasets)
    /// - float64: parallel only above threshold
    ///
    /// # Arguments
    /// * `data_bytes` - Raw data bytes
    /// * `bytes_per_param` - Bytes per parameter (same for all)
    /// * `data_type` - Data type (same for all)
    /// * `byte_order` - Byte order
    /// * `num_events` - Number of events
    /// * `num_params` - Number of parameters
    ///
    /// # Errors
    /// Will return `Err` if parsing fails
    #[inline]
    fn parse_uniform_data_bulk(
        data_bytes: &[u8],
        bytes_per_param: usize,
        data_type: &FcsDataType,
        byte_order: &ByteOrder,
        num_events: usize,
        num_params: usize,
    ) -> Result<Vec<f32>> {
        let total_values = num_events * num_params;
        let use_parallel = total_values > PARALLEL_THRESHOLD;
        let mut f32_values = Vec::with_capacity(total_values);

        match (data_type, bytes_per_param) {
            (FcsDataType::I, 2) => {
                // int16 - parallel is 1.84x faster for large datasets
                if use_parallel {
                    data_bytes
                        .par_chunks_exact(2)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_u16(chunk),
                                ByteOrder::BigEndian => BE::read_u16(chunk),
                            };
                            value as f32
                        })
                        .collect_into_vec(&mut f32_values);
                } else {
                    // Sequential for small datasets
                    f32_values = data_bytes
                        .chunks_exact(2)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_u16(chunk),
                                ByteOrder::BigEndian => BE::read_u16(chunk),
                            };
                            value as f32
                        })
                        .collect();
                }
            }
            (FcsDataType::I, 4) => {
                // int32 - parallel only above threshold
                if use_parallel {
                    data_bytes
                        .par_chunks_exact(4)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_u32(chunk),
                                ByteOrder::BigEndian => BE::read_u32(chunk),
                            };
                            value as f32
                        })
                        .collect_into_vec(&mut f32_values);
                } else {
                    // Sequential for small datasets
                    f32_values = data_bytes
                        .chunks_exact(4)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_u32(chunk),
                                ByteOrder::BigEndian => BE::read_u32(chunk),
                            };
                            value as f32
                        })
                        .collect();
                }
            }
            (FcsDataType::F, 4) => {
                // float32 - always sequential (benchmarks show 2.57x faster than parallel)
                // This is a fallback path - normally handled by bytemuck in store_raw_data_as_dataframe
                let needs_swap = match (byte_order, cfg!(target_endian = "little")) {
                    (ByteOrder::LittleEndian, true) | (ByteOrder::BigEndian, false) => false,
                    _ => true,
                };
                f32_values = data_bytes
                    .chunks_exact(4)
                    .map(|chunk| {
                        let mut bytes = [0u8; 4];
                        bytes.copy_from_slice(chunk);
                        let bits = u32::from_ne_bytes(bytes);
                        let bits = if needs_swap { bits.swap_bytes() } else { bits };
                        f32::from_bits(bits)
                    })
                    .collect();
            }
            (FcsDataType::D, 8) => {
                // float64 - parallel only above threshold
                if use_parallel {
                    data_bytes
                        .par_chunks_exact(8)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_f64(chunk),
                                ByteOrder::BigEndian => BE::read_f64(chunk),
                            };
                            value as f32
                        })
                        .collect_into_vec(&mut f32_values);
                } else {
                    // Sequential for small datasets
                    f32_values = data_bytes
                        .chunks_exact(8)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_f64(chunk),
                                ByteOrder::BigEndian => BE::read_f64(chunk),
                            };
                            value as f32
                        })
                        .collect();
                }
            }
            _ => {
                return Err(anyhow!(
                    "Unsupported uniform data type: {:?} with {} bytes",
                    data_type,
                    bytes_per_param
                ));
            }
        }

        Ok(f32_values)
    }

    /// Parse a parameter value from bytes to f32 based on data type and bytes per parameter
    ///
    /// Handles different data types:
    /// - int16 (2 bytes) - unsigned integer
    /// - int32 (4 bytes) - unsigned integer
    /// - float32 (4 bytes) - single-precision floating point
    /// - float64 (8 bytes) - double-precision floating point
    ///
    /// # Arguments
    /// * `bytes` - Raw bytes for the parameter value
    /// * `bytes_per_param` - Number of bytes per parameter (from $PnB / 8)
    /// * `data_type` - Data type (I, F, or D)
    /// * `byte_order` - Byte order of the file
    ///
    /// # Errors
    /// Will return `Err` if the bytes cannot be parsed according to the data type
    #[cold]
    pub(crate) fn parse_parameter_value_to_f32(
        bytes: &[u8],
        bytes_per_param: usize,
        data_type: &FcsDataType,
        byte_order: &ByteOrder,
    ) -> Result<f32> {
        match (data_type, bytes_per_param) {
            (FcsDataType::I, 2) => {
                // int16 (unsigned 16-bit integer)
                let value = match byte_order {
                    ByteOrder::LittleEndian => LE::read_u16(bytes),
                    ByteOrder::BigEndian => BE::read_u16(bytes),
                };
                Ok(value as f32)
            }
            (FcsDataType::I, 4) => {
                // int32 (unsigned 32-bit integer)
                let value = match byte_order {
                    ByteOrder::LittleEndian => LE::read_u32(bytes),
                    ByteOrder::BigEndian => BE::read_u32(bytes),
                };
                Ok(value as f32)
            }
            (FcsDataType::F, 4) => {
                // float32 (single-precision floating point)
                Ok(byte_order.read_f32(bytes))
            }
            (FcsDataType::D, 8) => {
                // float64 (double-precision floating point) - convert to f32
                let value = match byte_order {
                    ByteOrder::LittleEndian => LE::read_f64(bytes),
                    ByteOrder::BigEndian => BE::read_f64(bytes),
                };
                Ok(value as f32)
            }
            (FcsDataType::I, _) => Err(anyhow!(
                "Unsupported integer size: {} bytes (expected 2 or 4)",
                bytes_per_param
            )),
            (FcsDataType::F, _) => Err(anyhow!(
                "Invalid float32 size: {} bytes (expected 4)",
                bytes_per_param
            )),
            (FcsDataType::D, _) => Err(anyhow!(
                "Invalid float64 size: {} bytes (expected 8)",
                bytes_per_param
            )),
            (FcsDataType::A, _) => Err(anyhow!("ASCII data type not supported")),
        }
    }

    /// Parse variable-width data event-by-event (cold path)
    ///
    /// This is the slower path used when parameters have different sizes/types.
    /// Marked as `#[cold]` to help the compiler optimize the hot path.
    ///
    /// # Arguments
    /// * `data_bytes` - Raw data bytes
    /// * `bytes_per_parameter` - Bytes per parameter for each parameter
    /// * `data_types` - Data type for each parameter
    /// * `byte_order` - Byte order
    /// * `num_events` - Number of events
    /// * `num_params` - Number of parameters
    ///
    /// # Errors
    /// Will return `Err` if parsing fails
    #[cold]
    fn parse_variable_width_data(
        data_bytes: &[u8],
        bytes_per_parameter: &[usize],
        data_types: &[FcsDataType],
        byte_order: &ByteOrder,
        num_events: usize,
        num_params: usize,
    ) -> Result<Vec<f32>> {
        let mut f32_values: Vec<f32> = Vec::with_capacity(num_events * num_params);
        let mut data_offset = 0;

        for event_idx in 0..num_events {
            for (param_idx, &bytes_per_param) in bytes_per_parameter.iter().enumerate() {
                let param_num = param_idx + 1;
                let data_type = &data_types[param_idx];

                // Extract bytes for this parameter value
                if data_offset + bytes_per_param > data_bytes.len() {
                    return Err(anyhow!(
                        "Insufficient data at event {}, parameter {}: need {} bytes but only have {} remaining",
                        event_idx + 1,
                        param_num,
                        bytes_per_param,
                        data_bytes.len() - data_offset
                    ));
                }

                let param_bytes = &data_bytes[data_offset..data_offset + bytes_per_param];
                let f32_value = Self::parse_parameter_value_to_f32(
                    param_bytes,
                    bytes_per_param,
                    data_type,
                    byte_order,
                )
                .map_err(|e| anyhow!("Failed to parse parameter {} value: {}", param_num, e))?;

                f32_values.push(f32_value);
                data_offset += bytes_per_param;
            }
        }

        Ok(f32_values)
    }

    /// Looks for the parameter name as a key in the `parameters` hashmap and returns a reference to it
    /// Performs case-insensitive lookup for parameter names
    /// # Errors
    /// Will return `Err` if the parameter name is not found in the `parameters` hashmap
    pub fn find_parameter(&self, parameter_name: &str) -> Result<&Parameter> {
        // Try exact match first (fast path)
        if let Some(param) = self.parameters.get(parameter_name) {
            return Ok(param);
        }

        // Case-insensitive fallback: search through parameter map
        for (key, param) in self.parameters.iter() {
            if key.eq_ignore_ascii_case(parameter_name) {
                return Ok(param);
            }
        }

        Err(anyhow!("Parameter not found: {parameter_name}"))
    }

    /// Returns the validated DATA segment byte slice for this file.
    ///
    /// Resolves offsets via [`resolve_segment_offsets`], which measures
    /// `$BEGINDATA`/`$ENDDATA` (or the primary HEADER's offsets) relative to
    /// this data set's own start (`self.dataset_start`), not byte zero of the
    /// file (§2.4.3) — the same dataset-relative mechanism
    /// `store_raw_data_as_dataframe` uses during construction, so the lazy
    /// column-loading path and the eager DataFrame path can't drift on
    /// multi-dataset (`$NEXTDATA`) files.
    ///
    /// # Errors
    /// Will return `Err` if the DATA offsets fall outside the mapped file, or
    /// if start > end.
    fn data_bytes(&self) -> Result<&[u8]> {
        let mmap = &self.file_access.mmap;
        let mmap_len = mmap.len();
        let segments =
            resolve_segment_offsets(&self.header, &self.metadata, self.dataset_start, mmap_len)?;
        let data_start = *segments.data.start();
        let data_end = *segments.data.end();

        if data_start >= mmap_len {
            return Err(anyhow!(
                "Data start offset {} is beyond mmap length {}",
                data_start,
                mmap_len
            ));
        }
        if data_end >= mmap_len {
            return Err(anyhow!(
                "Data end offset {} is beyond mmap length {}",
                data_end,
                mmap_len
            ));
        }
        if data_start > data_end {
            return Err(anyhow!(
                "Data start offset {} is greater than end offset {}",
                data_start,
                data_end
            ));
        }

        Ok(&mmap[data_start..=data_end])
    }

    /// Looks for the parameter name as a key in the `parameters` hashmap and returns a mutable reference to it
    /// Performs case-insensitive lookup for parameter names
    /// # Errors
    /// Will return `Err` if the parameter name is not found in the `parameters` hashmap
    pub fn find_mutable_parameter(&mut self, parameter_name: &str) -> Result<&mut Parameter> {
        // Try exact match first (fast path)
        // Note: We need to check if the key exists as Arc<str>, so we iterate to find exact match
        let exact_key = self
            .parameters
            .keys()
            .find(|k| k.as_ref() == parameter_name)
            .map(|k| k.clone());

        if let Some(key) = exact_key {
            return self
                .parameters
                .get_mut(&key)
                .ok_or_else(|| anyhow!("Parameter not found: {parameter_name}"));
        }

        // Case-insensitive fallback: find the key first (clone Arc to avoid borrow issues)
        let matching_key = self
            .parameters
            .keys()
            .find(|key| key.eq_ignore_ascii_case(parameter_name))
            .map(|k| k.clone());

        if let Some(key) = matching_key {
            return self
                .parameters
                .get_mut(&key)
                .ok_or_else(|| anyhow!("Parameter not found: {parameter_name}"));
        }

        Err(anyhow!("Parameter not found: {parameter_name}"))
    }

    /// Returns a zero-copy reference to a Polars Float32Chunked view of a column for the parameter
    ///
    /// This provides access to the underlying Polars chunked array, which is useful
    /// for operations that work directly with Polars types. For most use cases,
    /// `get_parameter_events_slice()` is preferred as it provides a simple `&[f32]` slice.
    ///
    /// # Arguments
    /// * `channel_name` - The channel name (e.g., "FSC-A", "FL1-A")
    ///
    /// # Errors
    /// Will return `Err` if:
    /// - the parameter name is not found in the parameters map
    /// - the column data type is not Float32
    pub fn get_parameter_events(&'_ self, channel_name: &str) -> Result<&Float32Chunked> {
        Ok(self
            .get_parameter_column(channel_name)?
            .f32()
            .map_err(|e| anyhow!("Parameter {} is not f32 type: {}", channel_name, e))?)
    }
    /// Get a reference to the Polars Column for a parameter by channel name
    ///
    /// This provides direct access to the underlying Polars column, which can be useful
    /// for advanced operations that require the full Polars API.
    ///
    /// # Arguments
    /// * `channel_name` - The channel name (e.g., "FSC-A", "FL1-A")
    ///
    /// # Errors
    /// Will return `Err` if the parameter name is not found in the DataFrame
    pub fn get_parameter_column(&'_ self, channel_name: &str) -> Result<&Column> {
        self.data_frame
            .column(channel_name)
            .map_err(|e| anyhow!("Parameter {} not found: {}", channel_name, e))
    }

    /// Looks for the parameter name as a key in the 'parameters' hashmap and returns a new Vec<f32> of the raw event data
    /// NOTE: This allocates a full copy of the events - prefer `get_parameter_events_slice` when possible
    /// # Errors
    /// Will return 'Err' if the parameter name is not found in the 'parameters hashmap or if the events are not found
    pub fn get_parameter_events_as_owned_vec(&self, channel_name: &str) -> Result<Vec<EventDatum>> {
        Ok(self.get_parameter_events_slice(channel_name)?.to_vec())
    }

    /// Returns the minimum and maximum values of the parameter
    /// # Errors
    /// Will return `Err` if the parameter name is not found in the 'parameters' hashmap or if the events are not found
    pub fn get_minmax_of_parameter(&self, channel_name: &str) -> Result<(EventDatum, EventDatum)> {
        let parameter = self.find_parameter(channel_name)?;
        let events = self.get_parameter_events(&parameter.channel_name)?;

        match events.iter().minmax() {
            MinMaxResult::NoElements => Err(anyhow!("No elements found")),
            MinMaxResult::OneElement(e) => Err(anyhow!("Only one element found: {:?}", e)),
            MinMaxResult::MinMax(min, max) => {
                let min_val = min.ok_or_else(|| anyhow!("Min value is None"))?;
                let max_val = max.ok_or_else(|| anyhow!("Max value is None"))?;
                Ok((min_val, max_val))
            }
        }
    }

    /// Creates a new `HashMap` of `Parameter`s
    /// using the `Fcs` file's metadata to find the channel and label names from the `PnN` and `PnS` keywords.
    /// Does NOT store events on the parameter.
    /// # Errors
    /// Will return `Err` if:
    /// - the number of parameters cannot be found in the metadata,
    /// - the parameter name cannot be found in the metadata,
    /// - the parameter cannot be built (using the Builder pattern)
    /// Format diagnostic information about FCS file state for error messages
    fn format_diagnostic_info(header: &Header, metadata: &Metadata, path: &Path) -> String {
        let mut lines = Vec::new();

        lines.push("=== FCS File Diagnostic Information ===".to_string());
        lines.push(format!("File path: {}", path.display()));
        lines.push(format!("FCS Version: {}", header.version));
        lines.push(format!(
            "Text segment: {}..={}",
            header.text_offset.start(),
            header.text_offset.end()
        ));
        lines.push(format!(
            "Data segment: {}..={}",
            header.data_offset.start(),
            header.data_offset.end()
        ));
        lines.push(format!(
            "Analysis segment: {}..={}",
            header.analysis_offset.start(),
            header.analysis_offset.end()
        ));
        lines.push(format!(
            "Delimiter: '{}' (0x{:02x})",
            metadata.delimiter, metadata.delimiter as u8
        ));

        // Get number of parameters
        let n_params = metadata
            .get_number_of_parameters()
            .map(|n| format!("{}", n))
            .unwrap_or_else(|e| format!("Error: {}", e));
        lines.push(format!("Number of parameters ($PAR): {}", n_params));

        // Get number of events
        let n_events = metadata
            .get_number_of_events()
            .map(|n| format!("{}", n))
            .unwrap_or_else(|e| format!("Error: {}", e));
        lines.push(format!("Number of events ($TOT): {}", n_events));

        // List all parameter keywords
        lines.push("\n=== Parameter Keywords ===".to_string());
        let number_of_parameters = metadata.get_number_of_parameters().ok().copied();
        if let Some(n_params) = number_of_parameters {
            for param_num in 1..=n_params {
                let pn_keywords = [
                    format!("$P{}N", param_num),
                    format!("$P{}S", param_num),
                    format!("$P{}B", param_num),
                    format!("$P{}E", param_num),
                    format!("$P{}R", param_num),
                ];

                let mut found_keywords = Vec::new();
                for key in &pn_keywords {
                    if let Some(kw) = metadata.keywords.get(key) {
                        let value = match kw {
                            crate::keyword::Keyword::String(sk) => sk.get_str().to_string(),
                            crate::keyword::Keyword::Int(ik) => ik.get_usize().to_string(),
                            crate::keyword::Keyword::Float(fk) => fk.to_string(),
                            crate::keyword::Keyword::Byte(bk) => bk.get_str().to_string(),
                            crate::keyword::Keyword::Mixed(mk) => mk.to_string(),
                        };
                        found_keywords.push(format!("  {} = {}", key, value));
                    } else {
                        found_keywords.push(format!("  {} = <MISSING>", key));
                    }
                }
                lines.push(format!("Parameter {}:", param_num));
                lines.extend(found_keywords);
            }
        } else {
            lines.push("  Could not determine number of parameters".to_string());
        }

        // List all keywords sorted
        lines.push("\n=== All Keywords (sorted) ===".to_string());
        let mut sorted_keys: Vec<_> = metadata.keywords.keys().collect();
        sorted_keys.sort();
        for key in sorted_keys {
            let kw = match metadata.keywords.get(key) {
                Some(kw) => kw,
                None => continue, // Skip missing keywords in diagnostic output
            };
            let value = match kw {
                crate::keyword::Keyword::String(sk) => sk.get_str().to_string(),
                crate::keyword::Keyword::Int(ik) => ik.get_usize().to_string(),
                crate::keyword::Keyword::Float(fk) => fk.to_string(),
                crate::keyword::Keyword::Byte(bk) => bk.get_str().to_string(),
                crate::keyword::Keyword::Mixed(mk) => mk.to_string(),
            };
            lines.push(format!("  {} = {}", key, value));
        }

        lines.join("\n")
    }

    pub fn generate_parameter_map(metadata: &Metadata) -> Result<ParameterMap> {
        let mut map = ParameterMap::default();
        let number_of_parameters = metadata.get_number_of_parameters()?;
        for parameter_number in 1..=*number_of_parameters {
            let channel_name = metadata.get_parameter_channel_name(parameter_number)
                .map_err(|e| anyhow!(
                    "Failed to get channel name for parameter {}: {}\n\nHint: Check that $P{}N keyword exists in metadata.",
                    parameter_number,
                    e,
                    parameter_number
                ))?;

            // Use label name or fallback to the parameter name
            let label_name = match metadata.get_parameter_label(parameter_number) {
                Ok(label) => label,
                Err(_) => channel_name,
            };

            let transform = if channel_name.contains("FSC")
                || channel_name.contains("SSC")
                || channel_name.contains("Time")
            {
                TransformType::Linear
            } else {
                TransformType::default()
            };

            // Get excitation wavelength from metadata if available
            let excitation_wavelength = metadata
                .get_parameter_excitation_wavelength(parameter_number)
                .ok()
                .flatten();

            let parameter = ParameterBuilder::default()
                // For the ParameterBuilder, ensure we're using the proper methods
                // that may be defined by the Builder derive macro
                .parameter_number(parameter_number)
                .channel_name(channel_name)
                .label_name(label_name)
                .transform(transform)
                .excitation_wavelength(excitation_wavelength)
                .build()?;

            // Add the parameter events to the hashmap keyed by the parameter name
            map.insert(channel_name.to_string().into(), parameter);
        }

        Ok(map)
    }

    /// Looks for a keyword among the metadata and returns its value as a `&str`
    /// # Errors
    /// Will return `Err` if the `Keyword` is not found in the `metadata` or if the `Keyword` cannot be converted to a `&str`
    pub fn get_keyword_string_value(&self, keyword: &str) -> Result<Cow<'_, str>> {
        // TODO: This should be a match statement
        if let Ok(keyword) = self.metadata.get_string_keyword(keyword) {
            Ok(keyword.get_str())
        } else if let Ok(keyword) = self.metadata.get_integer_keyword(keyword) {
            Ok(keyword.get_str())
        } else if let Ok(keyword) = self.metadata.get_float_keyword(keyword) {
            Ok(keyword.get_str())
        } else if let Ok(keyword) = self.metadata.get_byte_keyword(keyword) {
            Ok(keyword.get_str())
        } else if let Ok(keyword) = self.metadata.get_mixed_keyword(keyword) {
            Ok(keyword.get_str())
        } else {
            Err(anyhow!("Keyword not found: {}", keyword))
        }
    }
    /// A convenience function to return the `GUID` keyword from the `metadata` as a `&str`
    /// # Errors
    /// Will return `Err` if the `GUID` keyword is not found in the `metadata` or if the `GUID` keyword cannot be converted to a `&str`
    pub fn get_guid(&self) -> Result<Cow<'_, str>> {
        Ok(self.metadata.get_string_keyword("GUID")?.get_str())
    }

    /// Set or update the GUID keyword in the file's metadata
    pub fn set_guid(&mut self, guid: String) {
        self.metadata
            .insert_string_keyword("GUID".to_string(), guid);
    }

    /// A convenience function to return the `$FIL` keyword from the `metadata` as a `&str`
    /// # Errors
    /// Will return `Err` if the `$FIL` keyword is not found in the `metadata` or if the `$FIL` keyword cannot be converted to a `&str`
    pub fn get_fil_keyword(&self) -> Result<Cow<'_, str>> {
        Ok(self.metadata.get_string_keyword("$FIL")?.get_str())
    }

    /// A convenience function to return the `$TOT` keyword from the `metadata` as a `usize`
    /// # Errors
    /// Will return `Err` if the `$TOT` keyword is not found in the `metadata` or if the `$TOT` keyword cannot be converted to a `usize`
    pub fn get_number_of_events(&self) -> Result<&usize> {
        self.metadata.get_number_of_events()
    }

    /// A convenience function to return the `$PAR` keyword from the `metadata` as a `usize`
    /// # Errors
    /// Will return `Err` if the `$PAR` keyword is not found in the `metadata` or if the `$PAR` keyword cannot be converted to a `usize`
    pub fn get_number_of_parameters(&self) -> Result<&usize> {
        self.metadata.get_number_of_parameters()
    }

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
    ///
    /// # Warning
    /// Only meaningful on an `Fcs` obtained from `open()`/`open_all()`. A
    /// clone whose `data_frame` was replaced (e.g. by filtering or
    /// compensation elsewhere in this workspace) still shares this cache via
    /// `Arc`, and will decode against the *original* file's bytes — not
    /// whatever the replaced `data_frame` represents. See `flow-crates-rkq`.
    pub fn column(&self, channel_name: &str) -> Result<&[f32]> {
        Ok(self
            .columns(&[channel_name])?
            .pop()
            .expect("columns() returns exactly one slice for a single requested channel"))
    }

    /// Returns raw values for several parameters, decoding all uncached
    /// members in a single pass over the DATA segment rather than one pass
    /// per column. Prefer this over repeated `column()` calls when you know
    /// the full set you need up front.
    ///
    /// # Errors
    /// Will return `Err` under the same conditions as `column()`, for any of
    /// `channel_names`.
    ///
    /// # Warning
    /// Only meaningful on an `Fcs` obtained from `open()`/`open_all()`. A
    /// clone whose `data_frame` was replaced (e.g. by filtering or
    /// compensation elsewhere in this workspace) still shares this cache via
    /// `Arc`, and will decode against the *original* file's bytes — not
    /// whatever the replaced `data_frame` represents. See `flow-crates-rkq`.
    pub fn columns(&self, channel_names: &[&str]) -> Result<Vec<&[f32]>> {
        let indices: Vec<usize> = channel_names
            .iter()
            .map(|name| Ok(self.find_parameter(name)?.parameter_number - 1))
            .collect::<Result<Vec<_>>>()?;

        for &idx in &indices {
            if idx >= self.columns.len() {
                return Err(anyhow!(
                    "parameter index {} is outside this file's column cache ({} slots) — \
                     this Fcs was likely derived (e.g. via filtering or unmixing) rather than \
                     opened, and its column cache was never resized to match its current parameters",
                    idx,
                    self.columns.len()
                ));
            }
        }

        let mut missing: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&idx| self.columns[idx].get().is_none())
            .collect();
        missing.sort_unstable();
        missing.dedup();

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
    ///
    /// # Warning
    /// Only meaningful on an `Fcs` obtained from `open()`/`open_all()`. A
    /// clone whose `data_frame` was replaced (e.g. by filtering or
    /// compensation elsewhere in this workspace) still shares this cache via
    /// `Arc`, and will decode against the *original* file's bytes — not
    /// whatever the replaced `data_frame` represents. See `flow-crates-rkq`.
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
                .enumerate()
                .map(|(idx, mut column)| {
                    // `extract_columns` (the non-bit-packed path below) applies
                    // this masking internally; the bit-packed path decodes via
                    // `parse_bit_packed_data`/`extract_all_param_columns`
                    // instead, neither of which knows about `$PnR`, so it must
                    // be applied here to match the eager `data_frame` oracle.
                    for value in column.iter_mut() {
                        *value = crate::columns::apply_range_mask(*value, layout.range_masks[idx]);
                    }
                    column.into_boxed_slice()
                })
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

    // ==================== NEW POLARS-BASED ACCESSOR METHODS ====================

    /// Get events for a parameter as a slice of f32 values
    /// Polars gives us direct access to the underlying buffer (zero-copy)
    /// # Errors
    /// Will return `Err` if:
    /// - the parameter name is not found
    /// - the Series data type is not Float32
    /// - the data is chunked (rare for FCS files)
    pub fn get_parameter_events_slice(&self, channel_name: &str) -> Result<&[f32]> {
        self.get_parameter_events(channel_name)?
            .cont_slice()
            .map_err(|e| anyhow!("Parameter {} data is not contiguous: {}", channel_name, e))
    }

    /// Get two parameters as (x, y) pairs for plotting
    /// Optimized for scatter plot use case with zero allocations until the collect
    /// # Errors
    /// Will return `Err` if either parameter name is not found
    pub fn get_xy_pairs(&self, x_param: &str, y_param: &str) -> Result<Vec<(f32, f32)>> {
        let x_data = self.get_parameter_events_slice(x_param)?;
        let y_data = self.get_parameter_events_slice(y_param)?;

        // Verify both parameters have the same length
        if x_data.len() != y_data.len() {
            return Err(anyhow!(
                "Parameter length mismatch: {} has {} events, {} has {} events",
                x_param,
                x_data.len(),
                y_param,
                y_data.len()
            ));
        }

        // Zip is zero-cost abstraction - uses iterators efficiently
        Ok(x_data
            .iter()
            .zip(y_data.iter())
            .map(|(&x, &y)| (x, y))
            .collect())
    }

    /// Get DataFrame height (number of events)
    #[must_use]
    pub fn get_event_count_from_dataframe(&self) -> usize {
        self.data_frame.height()
    }

    /// Get DataFrame width (number of parameters)
    #[must_use]
    pub fn get_parameter_count_from_dataframe(&self) -> usize {
        self.data_frame.width()
    }

    /// Get DataFrame column names (parameter names)
    pub fn get_parameter_names_from_dataframe(&self) -> Vec<String> {
        self.data_frame
            .get_column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Aggregate statistics for a parameter using Polars' streaming API for low memory usage and minimal, chunked passes.
    ///
    /// When streaming is enabled, Polars creates a *Pipeline*:
    ///
    /// **Source**: It pulls a chunk of data from the disk (e.g., 50,000 rows).
    ///
    /// **Operators**: It passes that chunk through your expressions (calculating the running sum, count, min, and max for that specific chunk).
    ///
    /// **Sink**: It aggregates the results from all chunks into a final result.
    ///
    /// Because the statistics we are calculating (min, max, mean) are *associative* and *commutative*, Polars can calculate them partially on each chunk and then combine them at the very end.
    ///
    /// Returns (min, max, mean, std_dev)
    /// # Errors
    /// Will return `Err` if the parameter is not found or stats calculation fails
    pub fn get_parameter_statistics(&self, channel_name: &str) -> Result<(f32, f32, f32, f32)> {
        let stats = (*self.data_frame)
            .clone()
            .lazy()
            .select([
                col(channel_name).min().alias("min"),
                col(channel_name).max().alias("max"),
                col(channel_name).mean().alias("mean"),
                col(channel_name).std(1).alias("std"),
            ])
            .collect_with_engine(Engine::Streaming)?.unwrap_single();
        let min = stats
            .column("min")
            .map_err(|e| anyhow!("Column 'min' not found in statistics: {}", e))?
            .f32()
            .map_err(|e| anyhow!("Column 'min' is not f32 type: {}", e))?
            .get(0)
            .ok_or_else(|| anyhow!("No min value found"))?;
        let max = stats
            .column("max")
            .map_err(|e| anyhow!("Column 'max' not found in statistics: {}", e))?
            .f32()
            .map_err(|e| anyhow!("Column 'max' is not f32 type: {}", e))?
            .get(0)
            .ok_or_else(|| anyhow!("No max value found"))?;
        let mean = stats
            .column("mean")
            .map_err(|e| anyhow!("Column 'mean' not found in statistics: {}", e))?
            .f32()
            .map_err(|e| anyhow!("Column 'mean' is not f32 type: {}", e))?
            .get(0)
            .ok_or_else(|| anyhow!("No mean value found"))?;
        let std = stats
            .column("std")
            .map_err(|e| anyhow!("Column 'std' not found in statistics: {}", e))?
            .f32()
            .map_err(|e| anyhow!("Column 'std' is not f32 type: {}", e))?
            .get(0)
            .ok_or_else(|| anyhow!("No std deviation value found"))?;

        Ok((min, max, mean, std))
    }

    // ==================== TRANSFORMATION METHODS ====================

    /// Apply arcsinh transformation to a parameter using Polars
    /// This is the most common transformation for flow cytometry data
    /// Formula: arcsinh(x / cofactor)
    ///
    /// # Arguments
    /// * `parameter_name` - Name of the parameter to transform
    /// * `cofactor` - Scaling factor (typical: 150-200 for modern instruments)
    ///
    /// # Returns
    /// New DataFrame with the transformed parameter
    pub fn apply_arcsinh_transform(
        &self,
        parameter_name: &str,
        cofactor: f32,
    ) -> Result<EventDataFrame> {
        let df = (*self.data_frame).clone();

        // Get the column to transform
        let col = df
            .column(parameter_name)
            .map_err(|e| anyhow!("Parameter {} not found: {}", parameter_name, e))?;

        let series = col.as_materialized_series();
        let ca = series
            .f32()
            .map_err(|e| anyhow!("Parameter {} is not f32: {}", parameter_name, e))?;

        // Apply arcsinh transformation using TransformType implementation
        // The division by ln(10) was incorrectly converting to log10 scale,
        // which compressed the data ~2.3x and caused MAD to over-remove events
        use rayon::prelude::*;
        let transform = TransformType::Arcsinh { cofactor };
        let transformed: Vec<f32> = ca
            .cont_slice()
            .map_err(|e| anyhow!("Data not contiguous: {}", e))?
            .par_iter()
            .map(|&x| transform.transform(&x))
            .collect();

        // Create new column with transformed data
        let new_series = Series::new(parameter_name.into(), transformed);

        // Replace the column in DataFrame
        let mut new_df = df;
        new_df
            .replace(parameter_name, new_series.into())
            .map_err(|e| anyhow!("Failed to replace column: {}", e))?;

        Ok(Arc::new(new_df))
    }

    /// Apply arcsinh transformation to multiple parameters
    ///
    /// # Arguments
    /// * `parameters` - List of (parameter_name, cofactor) pairs
    ///
    /// # Returns
    /// New DataFrame with all specified parameters transformed
    pub fn apply_arcsinh_transforms(&self, parameters: &[(&str, f32)]) -> Result<EventDataFrame> {
        let mut df = (*self.data_frame).clone();

        use rayon::prelude::*;

        for &(param_name, cofactor) in parameters {
            let col = df
                .column(param_name)
                .map_err(|e| anyhow!("Parameter {} not found: {}", param_name, e))?;

            let series = col.as_materialized_series();
            let ca = series
                .f32()
                .map_err(|e| anyhow!("Parameter {} is not f32: {}", param_name, e))?;

            // Apply arcsinh transformation using TransformType implementation
            // Standard flow cytometry arcsinh - no division by ln(10)
            let transform = TransformType::Arcsinh { cofactor };
            let transformed: Vec<f32> = ca
                .cont_slice()
                .map_err(|e| anyhow!("Data not contiguous: {}", e))?
                .par_iter()
                .map(|&x| transform.transform(&x))
                .collect();

            let new_series = Series::new(param_name.into(), transformed);
            df.replace(param_name, new_series.into())
                .map_err(|e| anyhow!("Failed to replace column {}: {}", param_name, e))?;
        }

        Ok(Arc::new(df))
    }

    /// Apply default arcsinh transformation to all fluorescence parameters
    /// Automatically detects fluorescence parameters (excludes FSC, SSC, Time)
    /// Uses cofactor = 200 (good default for modern instruments)
    pub fn apply_default_arcsinh_transform(&self) -> Result<EventDataFrame> {
        let param_names = self.get_parameter_names_from_dataframe();

        // Filter to fluorescence parameters (exclude scatter and time)
        let fluor_params: Vec<(&str, f32)> = param_names
            .iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                !upper.contains("FSC") && !upper.contains("SSC") && !upper.contains("TIME")
            })
            .map(|name| (name.as_str(), 2000.0)) // Default cofactor = 2000
            .collect();

        self.apply_arcsinh_transforms(&fluor_params)
    }

    /// Apply biexponential (logicle) transformation matching FlowJo defaults
    /// Automatically detects fluorescence parameters (excludes FSC, SSC, Time)
    /// Uses FlowJo default parameters: top_of_scale=262144 (18-bit), positive_decades=4.5, negative_decades=0, width=0.5
    pub fn apply_default_biexponential_transform(&self) -> Result<EventDataFrame> {
        let param_names = self.get_parameter_names_from_dataframe();

        // Filter to fluorescence parameters (exclude scatter and time)
        let fluor_params: Vec<&str> = param_names
            .iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                !upper.contains("FSC") && !upper.contains("SSC") && !upper.contains("TIME")
            })
            .map(|name| name.as_str())
            .collect();

        let mut df = (*self.data_frame).clone();

        use rayon::prelude::*;

        // FlowJo default biexponential parameters
        let transform = TransformType::Biexponential {
            top_of_scale: 262144.0, // 18-bit data (2^18)
            positive_decades: 4.5,
            negative_decades: 0.0,
            width: 0.5,
        };

        for param_name in fluor_params {
            let col = df
                .column(param_name)
                .map_err(|e| anyhow!("Parameter {} not found: {}", param_name, e))?;

            let series = col.as_materialized_series();
            let ca = series
                .f32()
                .map_err(|e| anyhow!("Parameter {} is not f32: {}", param_name, e))?;

            // Apply biexponential transformation using TransformType implementation
            let transformed: Vec<f32> = ca
                .cont_slice()
                .map_err(|e| anyhow!("Data not contiguous: {}", e))?
                .par_iter()
                .map(|&x| transform.transform(&x))
                .collect();

            let new_series = Series::new(param_name.into(), transformed);
            df.replace(param_name, new_series.into())
                .map_err(|e| anyhow!("Failed to replace column {}: {}", param_name, e))?;
        }

        Ok(Arc::new(df))
    }

    // ==================== COMPENSATION METHODS ====================

    /// Extract compensation matrix from $SPILLOVER keyword
    /// Returns (matrix, channel_names) if spillover keyword exists
    /// Returns None if no spillover keyword is present in the file
    ///
    /// # Returns
    /// Some((compensation_matrix, channel_names)) if spillover exists, None otherwise
    ///
    /// # Errors
    /// Will return `Err` if spillover keyword is malformed
    pub fn get_spillover_matrix(&self) -> Result<Option<(Mat<f32>, Vec<String>)>> {
        use crate::keyword::{Keyword, MixedKeyword};

        // Try to get compensation matrix from $SPILLOVER (FCS 3.1+), $SPILL (unofficial/custom), or $COMP (FCS 3.0)
        // Check in order of preference: SPILLOVER (official) > SPILL (common) > COMP (legacy)
        let spillover_keyword = self
            .metadata
            .keywords
            .get("$SPILLOVER")
            .or_else(|| self.metadata.keywords.get("$SPILL"))
            .or_else(|| self.metadata.keywords.get("$COMP"));

        let spillover_keyword = match spillover_keyword {
            Some(Keyword::Mixed(MixedKeyword::SPILLOVER {
                n_parameters,
                parameter_names,
                matrix_values,
            })) => (
                *n_parameters,
                parameter_names.clone(),
                matrix_values.clone(),
            ),
            Some(_) => {
                // Keyword exists but has wrong type - might be stored as String(Other) if parsing failed
                // Try to parse it manually
                let keyword_name = if self.metadata.keywords.contains_key("$SPILLOVER") {
                    "$SPILLOVER"
                } else if self.metadata.keywords.contains_key("$SPILL") {
                    "$SPILL"
                } else if self.metadata.keywords.contains_key("$COMP") {
                    "$COMP"
                } else {
                    return Ok(None);
                };

                // Try to get the raw string value and parse it
                // This handles the case where $SPILL/$COMP was stored as String(Other) because
                // it wasn't recognized during initial parsing
                if let Some(Keyword::String(crate::keyword::StringKeyword::Other(value))) =
                    self.metadata.keywords.get(keyword_name)
                {
                    // Parse the string value as SPILLOVER using the same logic as parse_spillover
                    let parts: Vec<&str> = value.trim().split(',').collect();
                    if !parts.is_empty() {
                        if let Ok(n_parameters) = parts[0].trim().parse::<usize>() {
                            if parts.len() >= 1 + n_parameters {
                                let parameter_names: Vec<String> = parts[1..=n_parameters]
                                    .iter()
                                    .map(|s| s.trim().to_string())
                                    .collect();

                                let expected_matrix_size = n_parameters * n_parameters;
                                let matrix_start = 1 + n_parameters;

                                if parts.len() >= matrix_start + expected_matrix_size {
                                    // Parse matrix values (handle comma decimal separator)
                                    let mut matrix_values = Vec::new();
                                    for part in
                                        &parts[matrix_start..matrix_start + expected_matrix_size]
                                    {
                                        let cleaned = part.trim().replace(',', ".");
                                        if let Ok(val) = cleaned.parse::<f32>() {
                                            matrix_values.push(val);
                                        } else {
                                            break; // Failed to parse, give up
                                        }
                                    }

                                    if matrix_values.len() == expected_matrix_size {
                                        let matrix =
                                            Mat::from_fn(n_parameters, n_parameters, |i, j| {
                                                matrix_values[i * n_parameters + j]
                                            });
                                        return Ok(Some((matrix, parameter_names)));
                                    }
                                }
                            }
                        }
                    }
                }

                return Err(anyhow!(
                    "{} keyword exists but has wrong type or could not be parsed",
                    keyword_name
                ));
            }
            None => {
                // No spillover keyword - this is fine, not all files have it
                return Ok(None);
            }
        };

        let (n_params, param_names, matrix_values): (usize, Vec<String>, Vec<f32>) =
            spillover_keyword;

        // Validate matrix dimensions
        let expected_matrix_size = n_params * n_params;
        if matrix_values.len() != expected_matrix_size {
            return Err(anyhow!(
                "SPILLOVER matrix size mismatch: expected {} values for {}x{} matrix, got {}",
                expected_matrix_size,
                n_params,
                n_params,
                matrix_values.len()
            ));
        }

        // Create Mat from matrix values (FCS spillover is stored row-major)
        let matrix = Mat::from_fn(n_params, n_params, |i, j| matrix_values[i * n_params + j]);

        Ok(Some((matrix, param_names)))
    }

    /// Check if this file has compensation information
    #[must_use]
    pub fn has_compensation(&self) -> bool {
        self.get_spillover_matrix()
            .map(|opt| opt.is_some())
            .unwrap_or(false)
    }

    /// Resolve spillover channel labels to actual $PnN channel names.
    /// Some instruments (e.g. IntelliCyt iQue3) use parameter numbers ("1","2","3") in
    /// $SPILLOVER instead of channel names; these are mapped via metadata.get_parameter_channel_name.
    fn resolve_spillover_channel_names(&self, channel_names: &[String]) -> Result<Vec<String>> {
        let mut resolved = Vec::with_capacity(channel_names.len());
        for name in channel_names {
            let resolved_name = if let Ok(param_idx) = name.trim().parse::<usize>() {
                if param_idx >= 1 {
                    self.metadata
                        .get_parameter_channel_name(param_idx)
                        .map(|s| s.to_string())
                        .map_err(|e| {
                            anyhow!(
                                "Parameter {} not found: spillover references \"{}\" but {}",
                                param_idx,
                                name,
                                e
                            )
                        })?
                } else {
                    name.clone()
                }
            } else {
                name.clone()
            };
            if !self.parameters.contains_key(resolved_name.as_str()) {
                return Err(anyhow!(
                    "Spillover channel \"{}\" (resolved) not found in parameters",
                    resolved_name
                ));
            }
            resolved.push(resolved_name);
        }
        Ok(resolved)
    }

    /// Apply compensation from the file's $SPILLOVER keyword
    /// Convenience method that extracts spillover and applies it automatically
    ///
    /// Resolves spillover channel labels: some instruments (e.g. IntelliCyt iQue3) use
    /// parameter numbers ("1", "2", "3") in $SPILLOVER instead of $PnN names; these are
    /// mapped to actual channel names via metadata.
    ///
    /// # Returns
    /// New DataFrame with compensated data, or error if no spillover keyword exists
    pub fn apply_file_compensation(&self) -> Result<EventDataFrame> {
        let (comp_matrix, channel_names) = self
            .get_spillover_matrix()?
            .ok_or_else(|| anyhow!("No $SPILLOVER keyword found in FCS file"))?;

        let channel_refs: Vec<String> = self.resolve_spillover_channel_names(&channel_names)?;
        let channel_refs: Vec<&str> = channel_refs.iter().map(|s| s.as_str()).collect();

        self.apply_compensation(comp_matrix.as_ref(), &channel_refs)
    }

    /// Apply an externally supplied spillover matrix to this file's raw event data.
    ///
    /// Identical semantics to `get_compensated_parameters` (identity-check, parallel
    /// application via `flow_linalg`) but uses the caller-supplied matrix instead of
    /// reading `$SPILLOVER` from the file.
    ///
    /// # Arguments
    /// - `spillover`: NxN spillover matrix (NOT pre-inverted).
    /// - `matrix_channel_names`: channel names for matrix rows/cols as `&str`.
    ///   Callers with `Vec<String>` should map with `.iter().map(|s| s.as_str()).collect::<Vec<_>>()`.
    /// - `channels_needed`: which channels to return (subset of `matrix_channel_names`).
    ///
    /// Requires the `compensation` feature.
    #[cfg(feature = "compensation")]
    pub fn get_compensated_parameters_with_matrix(
        &self,
        spillover: faer::MatRef<'_, f32>,
        matrix_channel_names: &[&str],
        channels_needed: &[&str],
    ) -> anyhow::Result<std::collections::HashMap<String, Vec<f32>>> {
        use flow_linalg::compensation::compensate_channels;

        // Identity-check: if matrix is identity, bypass compensation entirely
        let n = spillover.nrows();
        let is_identity = (0..n).all(|i| {
            (0..n).all(|j| {
                let expected = if i == j { 1.0f32 } else { 0.0 };
                (spillover[(i, j)] - expected).abs() < 1e-6
            })
        });

        // Channels not in the spillover matrix are returned as-is (pass-through).
        // This keeps both code paths consistent and lets callers request FSC/SSC
        // alongside fluorescent channels without needing a separate call.
        let matrix_set: std::collections::HashSet<&str> =
            matrix_channel_names.iter().copied().collect();
        let (comp_channels, passthrough_channels): (Vec<&str>, Vec<&str>) = channels_needed
            .iter()
            .copied()
            .partition(|ch| matrix_set.contains(ch));

        if is_identity || comp_channels.is_empty() {
            let mut result = std::collections::HashMap::new();
            for &ch in channels_needed {
                let data = self.get_parameter_events_slice(ch)?;
                result.insert(ch.to_string(), data.to_vec());
            }
            return Ok(result);
        }

        // Build raw channel slice pairs — error if a matrix channel is absent from the file,
        // since missing channels silently corrupt the compensation math (treated as all-zero).
        let raw_pairs: Vec<(&str, Vec<f32>)> = matrix_channel_names
            .iter()
            .map(|&name| {
                let data = self
                    .get_parameter_events_slice(name)
                    .with_context(|| {
                        format!("channel '{name}' in spillover matrix not found in file")
                    })?;
                Ok((name, data.to_vec()))
            })
            .collect::<anyhow::Result<_>>()?;

        let raw_refs: Vec<(&str, &[f32])> = raw_pairs
            .iter()
            .map(|(name, data)| (*name, data.as_slice()))
            .collect();

        let mut result =
            compensate_channels(&raw_refs, spillover, matrix_channel_names, &comp_channels)
                .map_err(|e| anyhow::anyhow!("Compensation failed: {e}"))?;

        // Merge pass-through channels (not in spillover matrix) as raw values
        for &ch in &passthrough_channels {
            let data = self.get_parameter_events_slice(ch)?;
            result.insert(ch.to_string(), data.to_vec());
        }

        Ok(result)
    }

    /// OPTIMIZED: Get compensated data for specific parameters only (lazy/partial compensation)
    ///
    /// This is 15-30x faster than apply_file_compensation when you only need a few parameters
    /// because it:
    /// - Only compensates the requested channels (e.g., 2 vs 30)
    /// - Uses sparse matrix optimization for matrices with >80% zeros
    /// - Bypasses compensation entirely for identity matrices
    ///
    /// # Arguments
    /// * `channels_needed` - Only the channel names you need compensated (typically 2 for a plot)
    ///
    /// # Returns
    /// HashMap of channel_name -> compensated data (as Vec<f32>)
    ///
    /// # Performance
    /// - Dense matrix (2/30 channels): **15x faster** (150ms → 10ms)
    /// - Sparse matrix (90% sparse): **50x faster** (150ms → 3ms)
    /// - Identity matrix: **300x faster** (150ms → 0.5ms)
    pub fn get_compensated_parameters(
        &self,
        channels_needed: &[&str],
    ) -> Result<std::collections::HashMap<String, Vec<f32>>> {
        use std::collections::HashMap;

        // Get spillover matrix
        let (comp_matrix, matrix_channel_names) = self
            .get_spillover_matrix()?
            .ok_or_else(|| anyhow!("No $SPILLOVER keyword found in FCS file"))?;

        let n_events = self.get_event_count_from_dataframe();

        // OPTIMIZATION 1: Check if matrix is identity (no compensation needed)
        let is_identity = {
            let mut is_id = true;
            for i in 0..comp_matrix.nrows() {
                for j in 0..comp_matrix.ncols() {
                    let expected = if i == j { 1.0 } else { 0.0 };
                    if (comp_matrix[(i, j)] - expected).abs() > 1e-6 {
                        is_id = false;
                        break;
                    }
                }
                if !is_id {
                    break;
                }
            }
            is_id
        };

        if is_identity {
            tracing::debug!("🚀 Identity matrix detected - bypassing compensation");
            // Just return original data
            let mut result = HashMap::new();
            for &channel in channels_needed {
                let data = self.get_parameter_events_slice(channel)?;
                result.insert(channel.to_string(), data.to_vec());
            }
            return Ok(result);
        }

        // OPTIMIZATION 2: Analyze sparsity
        let total_elements = comp_matrix.nrows() * comp_matrix.ncols();
        let mut non_zero_count = 0;
        for i in 0..comp_matrix.nrows() {
            for j in 0..comp_matrix.ncols() {
                if comp_matrix[(i, j)].abs() > 1e-6 {
                    non_zero_count += 1;
                }
            }
        }
        let sparsity = 1.0 - (non_zero_count as f64 / total_elements as f64);
        let is_sparse = sparsity > 0.8;

        tracing::debug!(
            "📊 Compensation matrix: {:.1}% sparse, {} non-zero coefficients",
            sparsity * 100.0,
            non_zero_count
        );

        // Find indices of channels we need
        let channel_indices: HashMap<&str, usize> = matrix_channel_names
            .iter()
            .enumerate()
            .map(|(i, name)| (name.as_str(), i))
            .collect();

        let needed_indices: Vec<(String, usize)> = channels_needed
            .iter()
            .filter_map(|&ch| channel_indices.get(ch).map(|&idx| (ch.to_string(), idx)))
            .collect();

        if needed_indices.is_empty() {
            return Err(anyhow!(
                "None of the requested channels found in compensation matrix"
            ));
        }

        // Extract ONLY the channels involved in compensating our needed channels
        // For each needed channel, we need all channels that have non-zero spillover
        let mut involved_indices = std::collections::HashSet::new();
        for &(_, row_idx) in &needed_indices {
            // Add the channel itself
            involved_indices.insert(row_idx);

            // Add channels with non-zero spillover
            if is_sparse {
                for col_idx in 0..comp_matrix.ncols() {
                    if comp_matrix[(row_idx, col_idx)].abs() > 1e-6 {
                        involved_indices.insert(col_idx);
                    }
                }
            } else {
                // For dense matrix, we need all channels
                for i in 0..comp_matrix.ncols() {
                    involved_indices.insert(i);
                }
            }
        }

        let mut involved_vec: Vec<usize> = involved_indices.into_iter().collect();
        involved_vec.sort_unstable();

        tracing::debug!(
            "🎯 Lazy compensation: loading {} channels (vs {} total)",
            involved_vec.len(),
            matrix_channel_names.len()
        );

        // Extract data for involved channels only
        let mut channel_data: Vec<Vec<f32>> = Vec::with_capacity(involved_vec.len());
        for &idx in &involved_vec {
            let channel_name = &matrix_channel_names[idx];
            let data = self.get_parameter_events_slice(channel_name)?;
            channel_data.push(data.to_vec());
        }

        // Extract sub-matrix for involved channels
        let sub_matrix = Mat::from_fn(involved_vec.len(), involved_vec.len(), |i, j| {
            comp_matrix[(involved_vec[i], involved_vec[j])]
        });

        // Use CPU compensation (benchmarked: GPU was slower due to transfer overhead)
        // Invert sub-matrix using faer (pure Rust, no system BLAS)
        let comp_inv = crate::matrix::MatrixOps::invert_matrix(sub_matrix.as_ref())
            .map_err(|e| anyhow!("Failed to invert compensation matrix: {:?}", e))?;

        // Compensate ONLY the involved channels
        use rayon::prelude::*;
        let n_involved = involved_vec.len();
        let compensated_data: Vec<Vec<f32>> = (0..n_involved)
            .into_par_iter()
            .map(|i| {
                let mut result = vec![0.0; n_events];
                for event_idx in 0..n_events {
                    let mut sum = 0.0;
                    for j in 0..n_involved {
                        sum += comp_inv[(i, j)] * channel_data[j][event_idx];
                    }
                    result[event_idx] = sum;
                }
                result
            })
            .collect();

        // Build result HashMap for only the channels we need
        let mut result = HashMap::new();
        for (channel_name, orig_idx) in needed_indices {
            if let Some(local_idx) = involved_vec.iter().position(|&x| x == orig_idx) {
                result.insert(channel_name, compensated_data[local_idx].clone());
            }
        }

        tracing::debug!("🚀 Lazy compensation completed");
        Ok(result)
    }

    /// Apply compensation matrix to the data using Polars
    /// Compensation corrects for spectral overlap between fluorescence channels
    ///
    /// # Arguments
    /// * `compensation_matrix` - 2D matrix where element [i,j] represents spillover from channel j into channel i
    /// * `channel_names` - Names of channels in the order they appear in the matrix
    ///
    /// # Returns
    /// New DataFrame with compensated fluorescence values
    ///
    /// # Example
    /// ```ignore
    /// // Create a 3x3 compensation matrix
    /// use faer::mat;
    /// let comp_matrix = mat![
    ///     [1.0, 0.1, 0.05],  // FL1-A compensation
    ///     [0.2, 1.0, 0.1],   // FL2-A compensation
    ///     [0.1, 0.15, 1.0],  // FL3-A compensation
    /// ];
    /// let channels = vec!["FL1-A", "FL2-A", "FL3-A"];
    /// let compensated = fcs.apply_compensation(comp_matrix.as_ref(), &channels)?;
    /// ```
    pub fn apply_compensation(
        &self,
        compensation_matrix: faer::MatRef<'_, f32>,
        channel_names: &[&str],
    ) -> Result<EventDataFrame> {
        let comp = compensation_matrix;
        // Verify matrix dimensions match channel names
        let n_channels = channel_names.len();
        if comp.nrows() != n_channels || comp.ncols() != n_channels {
            return Err(anyhow!(
                "Compensation matrix dimensions ({}, {}) don't match number of channels ({})",
                comp.nrows(),
                comp.ncols(),
                n_channels
            ));
        }

        // Extract data for channels to compensate
        let mut channel_data: Vec<Vec<f32>> = Vec::with_capacity(n_channels);
        let n_events = self.get_event_count_from_dataframe();

        for &channel_name in channel_names {
            let data = self.get_parameter_events_slice(channel_name)?;
            channel_data.push(data.to_vec());
        }

        // Use CPU compensation (benchmarked: GPU was slower due to transfer overhead)
        // Apply compensation: compensated = original * inverse(compensation_matrix)
        // For efficiency, we pre-compute the inverse using faer (pure Rust, no system BLAS)
        let comp_inv = crate::matrix::MatrixOps::invert_matrix(comp)
            .map_err(|e| anyhow!("Failed to invert compensation matrix: {:?}", e))?;

        // Perform matrix multiplication for each event
        use rayon::prelude::*;
        let compensated_data: Vec<Vec<f32>> = (0..n_channels)
            .into_par_iter()
            .map(|i| {
                let mut result = vec![0.0; n_events];
                for event_idx in 0..n_events {
                    let mut sum = 0.0;
                    for j in 0..n_channels {
                        sum += comp_inv[(i, j)] * channel_data[j][event_idx];
                    }
                    result[event_idx] = sum;
                }
                result
            })
            .collect();

        // Create new DataFrame with compensated values
        let mut df = (*self.data_frame).clone();

        for (i, &channel_name) in channel_names.iter().enumerate() {
            let new_series = Series::new(channel_name.into(), compensated_data[i].clone());
            df.replace(channel_name, new_series.into())
                .map_err(|e| anyhow!("Failed to replace column {}: {}", channel_name, e))?;
        }

        Ok(Arc::new(df))
    }

    /// Apply spectral unmixing (matrix solve only, no compensation or transformation)
    ///
    /// This method performs unmixing by solving: observation = mixing_matrix × abundances
    /// Does NOT apply compensation or transformations - these should be done separately.
    ///
    /// For overdetermined systems (more detectors than endmembers), uses least squares.
    ///
    /// # Arguments
    /// * `unmixing_matrix` - Matrix describing spectral signatures of fluorophores (detectors × endmembers)
    /// * `detector_names` - Names of detector channels (must match matrix rows)
    /// * `endmember_names` - Optional names for endmembers (columns). If None, uses "Endmember1", "Endmember2", etc.
    ///
    /// # Returns
    /// New DataFrame with unmixed endmember abundances (columns named by endmember names or indices)
    ///
    /// # Errors
    /// Returns error if detector names don't match matrix dimensions or data cannot be extracted
    pub fn apply_spectral_unmixing(
        &self,
        unmixing_matrix: faer::MatRef<'_, f32>,
        detector_names: &[&str],
        endmember_names: Option<&[&str]>,
    ) -> Result<EventDataFrame> {
        use faer::linalg::solvers::{Qr, SolveLstsq};

        // Verify matrix dimensions match detector names
        let n_detectors = detector_names.len();
        if unmixing_matrix.nrows() != n_detectors {
            return Err(anyhow!(
                "Unmixing matrix rows ({}) don't match number of detectors ({})",
                unmixing_matrix.nrows(),
                n_detectors
            ));
        }

        let n_endmembers = unmixing_matrix.ncols();
        let n_events = self.get_event_count_from_dataframe();

        // Extract data for detectors
        let mut detector_data: Vec<Vec<f32>> = Vec::with_capacity(n_detectors);
        for &detector_name in detector_names {
            let data = self.get_parameter_events_slice(detector_name)?;
            detector_data.push(data.to_vec());
        }

        // Observations matrix: events × detectors
        let observations = Mat::from_fn(n_events, n_detectors, |event_idx, detector_idx| {
            detector_data[detector_idx][event_idx]
        });

        // Perform unmixing: for each event, solve: observation = unmixing_matrix × abundances
        // For overdetermined systems (n_detectors > n_endmembers), use QR least squares (faer)
        let qr = Qr::new(unmixing_matrix);

        let mut unmixed_data: Vec<Vec<f32>> = Vec::with_capacity(n_events);
        for event_idx in 0..n_events {
            let b_col = Mat::from_fn(n_detectors, 1, |i, _| observations[(event_idx, i)]);
            let x_faer = qr.solve_lstsq(b_col.as_ref());
            let abundances: Vec<f32> = (0..n_endmembers).map(|i| x_faer[(i, 0)]).collect();
            unmixed_data.push(abundances);
        }

        // Create DataFrame with endmember abundances
        let mut columns: Vec<Column> = Vec::with_capacity(n_endmembers);
        for endmember_idx in 0..n_endmembers {
            let values: Vec<f32> = unmixed_data
                .iter()
                .map(|abundances| abundances[endmember_idx])
                .collect();

            // Use provided endmember name if available, otherwise use generic "Endmember{i}" format
            let column_name = if let Some(names) = endmember_names {
                names[endmember_idx].to_string()
            } else {
                format!("Endmember{}", endmember_idx + 1)
            };

            let series = Column::new(column_name.into(), values);
            columns.push(series);
        }

        let df = DataFrame::new(n_events, columns)
            .map_err(|e| anyhow!("Failed to create DataFrame: {}", e))?;

        Ok(Arc::new(df))
    }
}

#[cfg(test)]
mod lazy_column_tests {
    use super::Fcs;
    use crate::{Header, Metadata, Parameter, TransformType, file::AccessWrapper, parameter::ParameterMap};
    use polars::{frame::DataFrame, prelude::Column};

    fn compliance_fcs() -> std::path::PathBuf {
        crate::corpus::path("int-10000_events_random.fcs")
    }

    #[test]
    fn columns_returns_err_not_panic_when_cache_is_smaller_than_parameter_number() {
        // `Fcs::for_testing` sizes the column cache to `parameters.len()`, so
        // giving it a single parameter yields a 1-slot cache. If that
        // parameter's `parameter_number` doesn't correspond to a slot within
        // that cache (as would happen on a derived `Fcs` whose `parameters`
        // were replaced without resizing `columns` — see the production
        // scenario in tru-ols's spectral-unmixing output), `columns()` must
        // return `Err` instead of panicking on an out-of-bounds index.
        let temp_path = std::env::temp_dir().join("test_fcs_oob_column_cache.tmp");
        std::fs::write(&temp_path, b"test").expect("write temp file");

        let mut params = ParameterMap::default();
        params.insert(
            "OutOfRange".into(),
            Parameter::new(&5, "OutOfRange", "OutOfRange", &TransformType::Linear),
        );

        let df = DataFrame::new(1, vec![Column::new("OutOfRange".into(), vec![1.0f32])])
            .expect("build test dataframe");

        let fcs = Fcs::for_testing(
            Header::new(),
            Metadata::new(),
            params,
            std::sync::Arc::new(df),
            AccessWrapper::new(temp_path.to_str().unwrap_or("")).expect("access wrapper"),
        );

        let result = fcs.column("OutOfRange");
        assert!(
            result.is_err(),
            "out-of-bounds cache index must return Err, not panic"
        );
        let message = result.unwrap_err().to_string();
        assert!(
            message.contains("column cache"),
            "error message should mention the column cache/index mismatch, got: {message}"
        );
    }

    #[test]
    fn column_matches_data_frame_oracle() {
        let fcs = Fcs::open(compliance_fcs().to_str().expect("utf-8 corpus path")).expect("open compliance fixture");
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
        let fcs = Fcs::open(compliance_fcs().to_str().expect("utf-8 corpus path")).expect("open compliance fixture");
        let channel = fcs.get_parameter_names_from_dataframe()[0].clone();

        let first = fcs.column(&channel).expect("first access").as_ptr();
        let second = fcs.column(&channel).expect("second access").as_ptr();
        assert_eq!(first, second, "second call must return the same cached allocation, not re-decode");
    }

    #[test]
    fn columns_batch_matches_individual_column_calls() {
        let fcs = Fcs::open(compliance_fcs().to_str().expect("utf-8 corpus path")).expect("open compliance fixture");
        let names = fcs.get_parameter_names_from_dataframe();
        let (a, b) = (names[0].clone(), names[1].clone());

        let batch = fcs.columns(&[&a, &b]).expect("batch");
        let individual_a = fcs.column(&a).expect("a");
        let individual_b = fcs.column(&b).expect("b");

        assert_eq!(batch[0], individual_a);
        assert_eq!(batch[1], individual_b);
    }

    #[test]
    fn columns_dedupes_repeated_channel_request() {
        let fcs = Fcs::open(compliance_fcs().to_str().expect("utf-8 corpus path")).expect("open compliance fixture");
        let channel = fcs.get_parameter_names_from_dataframe()[0].clone();

        let batch = fcs.columns(&[&channel, &channel]).expect("batch with duplicate channel");
        let individual = fcs.column(&channel).expect("individual column");

        assert_eq!(batch[0], individual, "first slot of duplicate request must match column()");
        assert_eq!(batch[1], individual, "second slot of duplicate request must match column()");
        assert_eq!(
            batch[0].as_ptr(),
            batch[1].as_ptr(),
            "both requested slots must be the same cached allocation, proving the channel was decoded once"
        );
    }

    #[test]
    fn column_rejects_unknown_channel() {
        let fcs = Fcs::open(compliance_fcs().to_str().expect("utf-8 corpus path")).expect("open compliance fixture");
        assert!(fcs.column("NOT-A-REAL-CHANNEL").is_err());
    }

    #[test]
    fn events_matches_data_frame_oracle() {
        let fcs = Fcs::open(compliance_fcs().to_str().expect("utf-8 corpus path")).expect("open compliance fixture");
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
        let fcs = Fcs::open(compliance_fcs().to_str().expect("utf-8 corpus path")).expect("open compliance fixture");
        let _ = fcs.events().expect("events");

        assert!(
            fcs.columns.iter().all(|slot| slot.get().is_none()),
            "events() must not populate the lazy column cache — a QC'd file would otherwise hold both the raw columns and the derived frame"
        );
    }
}

#[cfg(test)]
#[cfg(feature = "compensation")]
mod compensation_method_tests {
    use super::*;

    /// Verifies the method compiles and identity matrix returns raw data.
    /// Full fixture-based test is #[ignore]d — needs test_data/compensation_test.fcs.
    #[test]
    #[ignore = "requires real FCS fixture"]
    fn test_with_matrix_matches_get_compensated_parameters() {
        // This test verifies parity between the two methods.
        // Run manually after loading test fixture:
        // cargo test --features compensation -- --ignored test_with_matrix
        let fcs = Fcs::open("test_data/compensation_test.fcs")
            .expect("test FCS file not found");
        if !fcs.has_compensation() { return; }
        let channels = ["BV421-A", "PE-A"];
        let expected = fcs.get_compensated_parameters(&channels).unwrap();
        let (matrix, names) = fcs.get_spillover_matrix().unwrap().unwrap();
        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let actual = fcs.get_compensated_parameters_with_matrix(matrix.as_ref(), &name_refs, &channels).unwrap();
        for &ch in &channels {
            let e = expected.get(ch).unwrap();
            let a = actual.get(ch).unwrap();
            assert_eq!(e.len(), a.len());
            for (&ev, &av) in e.iter().zip(a.iter()) {
                assert!((ev - av).abs() < 1e-4, "{ch}: {ev} vs {av}");
            }
        }
    }
}
