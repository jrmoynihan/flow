use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use strum_macros::Display;

/// The data type of the FCS file, which determines how event data is stored
///
/// FCS files can store data in different numeric formats. The most common is
/// single-precision floating point (F), which is also the default.
#[derive(Default, Display, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FcsDataType {
    /// Unsigned binary integer
    I,
    /// Single-precision floating point (f32)
    #[default]
    F,
    /// Double-precision floating point (f64)
    D,
    /// ASCII-encoded string (not supported)
    A,
}
impl FcsDataType {
    /// Matches the string pattern and returns the corresponding data type
    /// # Errors
    /// Will return `Err` if `data_type` is not a valid data type (ASCII-encoded strings are not supported, but binary integers, single-precision floating point, and double-precision floating point are supported)
    pub fn from_keyword_str(data_type: &str) -> Result<Self> {
        match data_type {
            "I" => Ok(Self::I),
            "F" => Ok(Self::F),
            "D" => Ok(Self::D),
            "A" => Err(anyhow!("ASCII-encoded string data type not supported")),
            _ => Err(anyhow!("Invalid data type")),
        }
    }

    /// Returns the keyword string representation of the data type
    pub fn to_keyword_str(&self) -> &str {
        match self {
            Self::I => "I (unsigned binary integer)",
            Self::F => "F (single-precision floating point)",
            Self::D => "D (double-precision floating point)",
            Self::A => "A (ASCII-encoded string)",
        }
    }

    /// Returns the number of bytes for the data type based on the number of bits
    ///
    /// This is used in conjunction with `$PnB` to determine the actual bytes per parameter.
    /// For `I` (integer) type, the actual bytes depend on `$PnB` (e.g., 16 bits = 2 bytes, 32 bits = 4 bytes).
    /// For `F` (float32), always 4 bytes.
    /// For `D` (float64), always 8 bytes.
    ///
    /// # Arguments
    /// * `bits` - Number of bits from `$PnB` keyword
    ///
    /// # Returns
    /// Number of bytes for this data type with the given bit width
    #[must_use]
    pub fn get_bytes_for_bits(&self, bits: usize) -> usize {
        match self {
            Self::I => (bits + 7) / 8, // Convert bits to bytes, rounding up
            Self::F => 4,
            Self::D => 8,
            Self::A => 0,
        }
    }
}
