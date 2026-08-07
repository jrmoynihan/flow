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
    /// The HEADER is not fixed at 58 bytes: §3.6 and Table 2 allow any number of
    /// extra 8-byte start/end offset pairs from byte 58 onward, holding
    /// vendor-defined OTHER segments. There is no count field - the pairs simply
    /// run up to whichever segment starts first, which is not necessarily TEXT
    /// (see [`header_end`](Self::header_end)). Empty for the overwhelming
    /// majority of files.
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
        let data_offset = Self::get_data_offsets(bytes)?;
        let analysis_offset = Self::get_analysis_offsets(bytes)?;
        Ok(Self {
            version: Self::get_version(bytes)?,
            other_offsets: Self::get_other_offsets(
                all,
                Self::header_end(&text_offset, &data_offset, &analysis_offset),
            ),
            text_offset,
            data_offset,
            analysis_offset,
        })
    }

    /// Byte offset at which the HEADER stops and the next segment begins.
    ///
    /// The HEADER carries no length field and no count of its OTHER offset
    /// pairs, so its end is implied by whichever segment starts first - FCS 3.1
    /// Table 1 spelled the bound as "beginning of next segment". That segment is
    /// **not** always TEXT. §3.1 Example 3 is a legal HEADER whose DATA segment
    /// precedes TEXT:
    ///
    /// ```text
    /// FCS3.2******202451**203140****1792**202450*******0*******0
    /// ```
    ///
    /// Bounding the scan at TEXT there would read all 200,659 bytes of the DATA
    /// segment as candidate offset pairs, and any 16 bytes of event data that
    /// happen to be ASCII digits would be mistaken for an OTHER segment.
    ///
    /// Zero starts are skipped: they mean the segment is absent, or that it sits
    /// past the 99,999,999-byte limit and is declared only in TEXT. TEXT itself
    /// is always a usable bound, since §3.2.3 requires it within that limit.
    fn header_end(
        text: &RangeInclusive<usize>,
        data: &RangeInclusive<usize>,
        analysis: &RangeInclusive<usize>,
    ) -> usize {
        [*text.start(), *data.start(), *analysis.start()]
            .into_iter()
            .filter(|&start| start >= HEADER_SIZE)
            .min()
            .unwrap_or(HEADER_SIZE)
    }

    /// Parses the OTHER segment offset pairs that follow the fixed 58 bytes (§3.6).
    ///
    /// Table 2 lists them as 8-byte start/end pairs from byte 58 onward, with
    /// "there may be 0 or any number of user-defined OTHER segments" and no count
    /// field - see [`header_end`](Self::header_end) for how the run is bounded.
    /// Files with no OTHER segments have nothing between byte 58 and the next
    /// segment, so this yields an empty vec without touching the buffer, which is
    /// the common case and stays free.
    ///
    /// Deliberately lenient rather than fallible: §3.8 fills unused HEADER space
    /// with ASCII spaces, so a run of blank "pairs" is padding, not corruption,
    /// and a file is not worth refusing over a segment nothing reads yet.
    /// Anything that isn't a well-formed ascending non-zero pair is skipped.
    fn get_other_offsets(bytes: &[u8], header_end: usize) -> Vec<RangeInclusive<usize>> {
        let Some(region) = bytes.get(HEADER_SIZE..header_end.min(bytes.len())) else {
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

#[cfg(test)]
mod other_segment_bounds_tests {
    use super::{HEADER_SIZE, Header};

    /// Assembles a 58-byte HEADER plus whatever trails it.
    fn header_bytes(
        text: (usize, usize),
        data: (usize, usize),
        analysis: (usize, usize),
        trailing: &[u8],
    ) -> Vec<u8> {
        let mut bytes = b"FCS3.2    ".to_vec();
        for (start, end) in [text, data, analysis] {
            bytes.extend_from_slice(format!("{start:>8}{end:>8}").as_bytes());
        }
        assert_eq!(bytes.len(), HEADER_SIZE);
        bytes.extend_from_slice(trailing);
        bytes
    }

    /// §3.6 Table 2: the pairs after byte 58 are OTHER segments.
    #[test]
    fn offset_pairs_after_byte_58_are_read_as_other_segments() {
        let bytes = header_bytes(
            (74, 500),
            (501, 999),
            (0, 0),
            format!("{:>8}{:>8}", 1000, 1200).as_bytes(),
        );

        let header = Header::from_bytes(&bytes).expect("header");
        assert_eq!(header.other_offsets, vec![1000..=1200]);
        assert_eq!(header.text_offset, 74..=500);
    }

    /// The HEADER ends where the *next* segment begins, and §3.1 Example 3 makes
    /// that DATA rather than TEXT:
    ///
    /// ```text
    /// FCS3.2******202451**203140****1792**202450*******0*******0
    /// ```
    ///
    /// Bounding the OTHER scan at TEXT would run it across the whole DATA
    /// segment. The DATA here is built so that failure is deterministic rather
    /// than luck: its first 16 bytes are ASCII, so a scan starting at byte 58
    /// reads them as a well-formed pair and invents an OTHER segment.
    #[test]
    fn a_data_segment_preceding_text_is_not_scanned_for_other_offsets() {
        const PHANTOM: &[u8; 16] = b"     100     200";

        let mut data = PHANTOM.to_vec();
        data.resize(942, 0);
        let data_end = HEADER_SIZE + data.len() - 1;

        let mut trailing = data;
        trailing.extend_from_slice(&[b' '; 201]);
        let bytes = header_bytes(
            (data_end + 1, data_end + 201),
            (HEADER_SIZE, data_end),
            (0, 0),
            &trailing,
        );

        let header = Header::from_bytes(&bytes).expect("header");
        assert!(
            header.other_offsets.is_empty(),
            "DATA precedes TEXT, so the HEADER ends at byte {HEADER_SIZE} and declares no \
             OTHER segments; got {:?} read out of the DATA segment",
            header.other_offsets
        );
    }

    /// A segment declared as 0 is absent, or lives past the 99,999,999-byte limit
    /// and is declared only in TEXT (§3.1 Example 2). Either way it cannot bound
    /// the HEADER, and treating its 0 as the bound would suppress every OTHER
    /// segment on exactly the large files that are hardest to test.
    #[test]
    fn a_zeroed_data_offset_does_not_bound_the_header() {
        let bytes = header_bytes(
            (74, 500),
            (0, 0),
            (0, 0),
            format!("{:>8}{:>8}", 1000, 1200).as_bytes(),
        );

        let header = Header::from_bytes(&bytes).expect("header");
        assert_eq!(header.other_offsets, vec![1000..=1200]);
    }
}
