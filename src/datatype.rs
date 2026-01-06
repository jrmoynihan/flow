use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use strum_macros::Display;

/// The data type of the FCS file, which determines how the data is stored.
#[derive(Default, Display, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
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

    /// Returns the number of bytes per event for the data type as an unsigned integer
    #[must_use]
    pub const fn get_bytes_per_event(&self) -> usize {
        match self {
            Self::I | Self::F => 4,
            Self::D => 8,
            Self::A => 0,
        }
    }
}
