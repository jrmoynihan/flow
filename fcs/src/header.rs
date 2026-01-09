#[allow(clippy::module_name_repetitions)]
use super::version::Version;
use anyhow::{Result, anyhow};
use core::str;
// use image::EncodableLayout;
use memmap3::Mmap;
use serde::{Serialize, Serializer, ser::SerializeMap};
use std::ops::RangeInclusive;

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
}
impl Serialize for Header {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_map(Some(2))?;
        state.serialize_entry("version", &self.version)?;
        state.serialize_entry("text_offset", &self.text_offset)?;
        state.serialize_entry("data_offset", &self.data_offset)?;
        state.serialize_entry("analysis_offset", &self.analysis_offset)?;
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
        }
    }
    /// Returns a new Header struct from a memory map of an FCS file
    /// # Errors
    /// Will return `Err` if:
    /// - the FCS version is not valid
    /// - the number of spaces in the header segment is not 4
    /// - the byte offsets for the TEXT, DATA, or ANALYSIS segments are not valid
    pub fn from_mmap(mmap: &Mmap) -> Result<Self> {
        // Check that bytes 6-9 are spaces:
        Self::check_header_spaces(&mmap[6..=9])?;
        // View the header segment and print the offsets to the console
        // Self::check_fcs_offsets(mmap);

        Ok(Self {
            version: Self::get_version(mmap)?,
            text_offset: Self::get_text_offsets(mmap)?,
            data_offset: Self::get_data_offsets(mmap)?,
            analysis_offset: Self::get_analysis_offsets(mmap)?,
        })
    }

    /// Returns the FCS version from the first 6 bytes of the file
    /// # Errors
    /// Will return `Err` if the version is not valid
    pub fn get_version(mmap: &Mmap) -> Result<Version> {
        let version = String::from_utf8(mmap[..6].to_vec())?;
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
    /// Parse an inclusive range of bytes from the memory map as an ASCII-encoded offset (in usize bytes)
    fn get_offset_from_header(mmap: &Mmap, start: usize, end: usize) -> Result<usize> {
        let offset_char = mmap[start..=end].as_ascii().expect("ascii not found");
        // println!("Offset bytes {:?}-{:?}: {:?}", &start, &end, &offset_char);
        // println!(
        //     "returned: {:?}",
        //     &offset_char.as_str().trim_ascii().parse::<usize>()?
        // );
        Ok(offset_char.as_str().trim_ascii().parse::<usize>()?)
    }
    /// Parse bytes 10-17 from the memory map as the ASCII-encoded offset (in usize bytes) to the first byte of the TEXT segment:
    fn get_text_offset_start(mmap: &Mmap) -> Result<usize> {
        Self::get_offset_from_header(mmap, 10, 17)
    }
    /// Parse bytes 18-25 as the ASCII-encoded offset (in usize bytes) to the last byte of the TEXT segment:
    fn get_text_offset_end(mmap: &Mmap) -> Result<usize> {
        Self::get_offset_from_header(mmap, 18, 25)
    }
    /// Parse bytes 26-33 as the ASCII-encoded offset to the first byte of the DATA segment:
    fn get_data_offset_start(mmap: &Mmap) -> Result<usize> {
        Self::get_offset_from_header(mmap, 26, 33)
    }
    /// Parse bytes 34-41 as the ASCII-encoded offset to the last byte of the DATA segment:
    fn get_data_offset_end(mmap: &Mmap) -> Result<usize> {
        Self::get_offset_from_header(mmap, 34, 41)
    }
    /// Parse bytes 42-49 as the ASCII-encoded offset to the first byte of the ANALYSIS segment:
    fn get_analysis_offset_start(mmap: &Mmap) -> Result<usize> {
        Self::get_offset_from_header(mmap, 42, 49)
    }
    /// Parse bytes 50-57 as the ASCII-encoded offset to the last byte of the ANALYSIS segment:
    fn get_analysis_offset_end(mmap: &Mmap) -> Result<usize> {
        Self::get_offset_from_header(mmap, 50, 57)
    }
    /// Returns the byte offsets for the TEXT segment
    fn get_text_offsets(mmap: &Mmap) -> Result<RangeInclusive<usize>> {
        let text_offset_start = Self::get_text_offset_start(mmap)?;
        let text_offset_end = Self::get_text_offset_end(mmap)?;
        Ok(text_offset_start..=text_offset_end)
    }
    /// Returns the byte offsets for the DATA segment
    fn get_data_offsets(mmap: &Mmap) -> Result<RangeInclusive<usize>> {
        let data_offset_start = Self::get_data_offset_start(mmap)?;
        let data_offset_end = Self::get_data_offset_end(mmap)?;
        Ok(data_offset_start..=data_offset_end)
    }
    /// Returns the byte offsets for the ANALYSIS segment
    fn get_analysis_offsets(mmap: &Mmap) -> Result<RangeInclusive<usize>> {
        let analysis_offset_start = Self::get_analysis_offset_start(mmap)?;
        let analysis_offset_end = Self::get_analysis_offset_end(mmap)?;
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
        println!("HEADER (first 58 bytes): {:?}", &mmap[0..58].as_ascii());
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
        println!("header range of TEXT: {:?}", &mmap[4700..=5216].as_ascii());
        Ok(())
    }
}
impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}
