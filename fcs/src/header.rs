#[allow(clippy::module_name_repetitions)]
use super::version::Version;
use anyhow::{Result, anyhow};
use core::str;
// use image::EncodableLayout;
use memmap3::Mmap;
use serde::{Serialize, Serializer, ser::SerializeMap};
use std::ops::RangeInclusive;

/// Size in bytes of an FCS HEADER segment (§2.4.3): 6 version bytes, 4 spaces,
/// then six 8-byte ASCII offset fields.
pub const HEADER_SIZE: usize = 58;

/// Contains FCS version and byte offsets to text, data, and analysis segments
///
/// The header is the first segment of an FCS file (first 58 bytes) and contains:
/// - The FCS version string (e.g., "FCS3.1")
/// - Byte offsets to the TEXT segment (contains metadata/keywords)
/// - Byte offsets to the DATA segment (contains event data)
/// - Byte offsets to the ANALYSIS segment (optional, contains analysis results)
#[derive(Clone, Debug, Hash)]
pub struct Header {
    pub version: Version,
    pub text_offset: RangeInclusive<usize>,
    pub data_offset: RangeInclusive<usize>,
    pub analysis_offset: RangeInclusive<usize>,
    /// OTHER segments, in HEADER order (§3.6).
    ///
    /// The HEADER is not fixed at 58 bytes: §3.6 allows any number of extra
    /// 8-byte start/end offset pairs from byte 58 onward, holding vendor-defined
    /// OTHER segments. There is no count field - the pairs simply run up to
    /// wherever TEXT begins. Empty for the overwhelming majority of files.
    pub other_offsets: Vec<RangeInclusive<usize>>,
}
impl Serialize for Header {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_map(Some(5))?;
        state.serialize_entry("version", &self.version)?;
        state.serialize_entry("text_offset", &self.text_offset)?;
        state.serialize_entry("data_offset", &self.data_offset)?;
        state.serialize_entry("analysis_offset", &self.analysis_offset)?;
        state.serialize_entry("other_offsets", &self.other_offsets)?;
        state.end()
    }
}

impl Header {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            version: Version::V3_1,
            text_offset: 0..=0,
            data_offset: 0..=0,
            analysis_offset: 0..=0,
            other_offsets: Vec::new(),
        }
    }
    /// Returns a new Header struct from a memory map of an FCS file
    /// # Errors
    /// Will return `Err` if:
    /// - the FCS version is not valid
    /// - the number of spaces in the header segment is not 4
    /// - the byte offsets for the TEXT, DATA, or ANALYSIS segments are not valid
    pub fn from_mmap(mmap: &Mmap) -> Result<Self> {
        Self::from_bytes(mmap)
    }

    /// Returns a new `Header` parsed from the 58 bytes starting at `bytes[0]`.
    ///
    /// The FCS HEADER is positional: every field is a fixed byte range measured
    /// from the start of the *data set*, not the start of the file (§2.4.3). A
    /// file containing a `$NEXTDATA` chain therefore has one HEADER per data
    /// set, each at its own base - so parsing takes a slice whose first byte is
    /// that base, and `from_mmap` is just the `base == 0` case.
    ///
    /// # Errors
    /// Will return `Err` if:
    /// - the slice is shorter than the 58-byte HEADER
    /// - the FCS version is not valid
    /// - the number of spaces in bytes 6-9 is not 4
    /// - the byte offsets for the TEXT, DATA, or ANALYSIS segments are not valid
    pub fn from_bytes(all: &[u8]) -> Result<Self> {
        let bytes = all
            .get(..HEADER_SIZE)
            .ok_or_else(|| anyhow!("Truncated FCS HEADER: {} bytes, need 58", all.len()))?;
        // Check that bytes 6-9 are spaces:
        Self::check_header_spaces(&bytes[6..=9])?;

        let text_offset = Self::get_text_offsets(bytes)?;
        Ok(Self {
            version: Self::get_version(bytes)?,
            other_offsets: Self::get_other_offsets(all, *text_offset.start()),
            text_offset,
            data_offset: Self::get_data_offsets(bytes)?,
            analysis_offset: Self::get_analysis_offsets(bytes)?,
        })
    }

    /// Parses the OTHER segment offset pairs that follow the fixed 58 bytes (§3.6).
    ///
    /// The HEADER carries no count for these, so the only available terminator
    /// is where TEXT begins - the HEADER's true length is implicitly
    /// `text_offset.start()`. Files with no OTHER segments have nothing between
    /// byte 58 and TEXT, so this yields an empty vec without touching the
    /// buffer, which is the common case and stays free.
    ///
    /// Deliberately lenient rather than fallible: §2.2.11 fills unused HEADER
    /// space with ASCII spaces, so a run of blank "pairs" is padding, not
    /// corruption, and a file is not worth refusing over a segment nothing reads
    /// yet. Anything that isn't a well-formed ascending non-zero pair is skipped.
    fn get_other_offsets(bytes: &[u8], text_start: usize) -> Vec<RangeInclusive<usize>> {
        let Some(region) = bytes.get(HEADER_SIZE..text_start.min(bytes.len())) else {
            return Vec::new();
        };
        region
            .chunks_exact(16)
            .filter_map(|pair| {
                let start = Self::get_offset_from_header(pair, 0, 7).ok()?;
                let end = Self::get_offset_from_header(pair, 8, 15).ok()?;
                (end >= start && end > 0).then_some(start..=end)
            })
            .collect()
    }

    /// Returns the FCS version from the first 6 bytes of the header
    /// # Errors
    /// Will return `Err` if the version is not valid
    pub fn get_version(bytes: &[u8]) -> Result<Version> {
        let version = String::from_utf8(
            bytes
                .get(..6)
                .ok_or_else(|| anyhow!("Truncated FCS HEADER: no version field"))?
                .to_vec(),
        )?;
        Self::check_fcs_version(&version)
    }

    /// Returns a valid FCS version enum after checking that the parsed string from the header is valid
    /// # Errors
    /// Will return `Err` if the version is not valid
    pub fn check_fcs_version(version: &str) -> Result<Version> {
        match version {
            "FCS1.0" => Ok(Version::V1_0),
            "FCS2.0" => Ok(Version::V2_0),
            "FCS3.0" => Ok(Version::V3_0),
            "FCS3.1" => Ok(Version::V3_1),
            "FCS3.2" => Ok(Version::V3_2),
            "FCS4.0" => Ok(Version::V4_0),
            _ => Err(anyhow!("Invalid FCS version: {}", version)),
        }
    }
    /// Check for valid number of spaces (4) in the HEADER segment
    /// # Errors
    /// Will return `Err` if the number of spaces is not 4
    pub fn check_header_spaces(buffer: &[u8]) -> Result<()> {
        if bytecount::count(buffer, b' ') != 4 {
            return Err(anyhow!(
                "Invalid number of spaces in header segment.  File may be corrupted."
            ));
        }
        Ok(())
    }
    /// Parse an inclusive range of HEADER bytes as an ASCII-encoded offset (in usize bytes)
    fn get_offset_from_header(bytes: &[u8], start: usize, end: usize) -> Result<usize> {
        let offset_str = std::str::from_utf8(&bytes[start..=end])
            .map_err(|_| anyhow!("Invalid UTF-8 in header segment"))?;
        Ok(offset_str.trim().parse::<usize>()?)
    }
    /// Parse bytes 10-17 as the ASCII-encoded offset (in usize bytes) to the first byte of the TEXT segment:
    fn get_text_offset_start(bytes: &[u8]) -> Result<usize> {
        Self::get_offset_from_header(bytes, 10, 17)
    }
    /// Parse bytes 18-25 as the ASCII-encoded offset (in usize bytes) to the last byte of the TEXT segment:
    fn get_text_offset_end(bytes: &[u8]) -> Result<usize> {
        Self::get_offset_from_header(bytes, 18, 25)
    }
    /// Parse bytes 26-33 as the ASCII-encoded offset to the first byte of the DATA segment:
    fn get_data_offset_start(bytes: &[u8]) -> Result<usize> {
        Self::get_offset_from_header(bytes, 26, 33)
    }
    /// Parse bytes 34-41 as the ASCII-encoded offset to the last byte of the DATA segment:
    fn get_data_offset_end(bytes: &[u8]) -> Result<usize> {
        Self::get_offset_from_header(bytes, 34, 41)
    }
    /// Parse bytes 42-49 as the ASCII-encoded offset to the first byte of the ANALYSIS segment:
    fn get_analysis_offset_start(bytes: &[u8]) -> Result<usize> {
        Self::get_offset_from_header(bytes, 42, 49)
    }
    /// Parse bytes 50-57 as the ASCII-encoded offset to the last byte of the ANALYSIS segment:
    fn get_analysis_offset_end(bytes: &[u8]) -> Result<usize> {
        Self::get_offset_from_header(bytes, 50, 57)
    }
    /// Returns the byte offsets for the TEXT segment
    fn get_text_offsets(bytes: &[u8]) -> Result<RangeInclusive<usize>> {
        let text_offset_start = Self::get_text_offset_start(bytes)?;
        let text_offset_end = Self::get_text_offset_end(bytes)?;
        Ok(text_offset_start..=text_offset_end)
    }
    /// Returns the byte offsets for the DATA segment
    fn get_data_offsets(bytes: &[u8]) -> Result<RangeInclusive<usize>> {
        let data_offset_start = Self::get_data_offset_start(bytes)?;
        let data_offset_end = Self::get_data_offset_end(bytes)?;
        Ok(data_offset_start..=data_offset_end)
    }
    /// Returns the byte offsets for the ANALYSIS segment
    fn get_analysis_offsets(bytes: &[u8]) -> Result<RangeInclusive<usize>> {
        let analysis_offset_start = Self::get_analysis_offset_start(bytes)?;
        let analysis_offset_end = Self::get_analysis_offset_end(bytes)?;
        Ok(analysis_offset_start..=analysis_offset_end)
    }
    /// Debug utility to print FCS file segment offsets
    ///
    /// This function prints detailed information about the header segment
    /// and the byte offsets for TEXT, DATA, and ANALYSIS segments.
    /// Useful for debugging file parsing issues.
    ///
    /// # Arguments
    /// * `mmap` - Memory-mapped view of the FCS file
    ///
    /// # Errors
    /// Will return `Err` if offsets cannot be read from the header
    pub fn check_fcs_offsets(mmap: &Mmap) -> Result<()> {
        println!(
            "HEADER (first 58 bytes): {:?}",
            std::str::from_utf8(&mmap[0..58]).unwrap_or("<invalid utf-8>")
        );
        println!(
            "TEXT segment start offset: {:?}",
            Self::get_text_offset_start(mmap)?
        );
        println!(
            "TEXT segment end offset: {:?}",
            Self::get_text_offset_end(mmap)?
        );
        println!(
            "DATA segment start offset: {:?}",
            Self::get_data_offset_start(mmap)?
        );
        println!(
            "DATA segment end offset: {:?}",
            Self::get_data_offset_end(mmap)?
        );
        println!(
            "ANALYSIS segment start offset (optional): {:?}",
            Self::get_analysis_offset_start(mmap)
        );
        println!(
            "ANALYSIS segment end offset (optional): {:?}",
            Self::get_analysis_offset_end(mmap)
        );
        // print from byte 4700 to 5210 (end of text, beginning of data)
        println!(
            "header range of TEXT: {:?}",
            std::str::from_utf8(&mmap[4700..=5216]).unwrap_or("<invalid utf-8>")
        );
        Ok(())
    }
}
impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}
