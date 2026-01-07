use anyhow::{Result, anyhow};
use byteorder::{BigEndian as BE, ByteOrder as BO, LittleEndian as LE};
use serde::{Deserialize, Serialize};
use strum_macros::Display;

/// Byte order (endianness) for reading numeric data from FCS files
///
/// FCS files can be written on either little-endian or big-endian systems.
/// The `$BYTEORD` keyword specifies which format is used.
#[derive(Display, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}
impl ByteOrder {
    /// Matches the string pattern and returns the corresponding byte order
    /// # Errors
    /// Will return `Err` if `byte_order` is not a valid byte order
    pub fn from_keyword_str(byte_order: &str) -> Result<Self> {
        match byte_order {
            "1,2,3,4" => Ok(Self::LittleEndian),
            "4,3,2,1" => Ok(Self::BigEndian),
            _ => Err(anyhow!("Invalid byte order")),
        }
    }

    /// Returns the byte order as a string
    pub const fn to_keyword_str(&self) -> &str {
        match self {
            Self::LittleEndian => "1,2,3,4",
            Self::BigEndian => "4,3,2,1",
        }
    }

    /// Reads a single 32-bit floating point number from a byte slice
    pub fn read_f32(&self, bytes: &[u8]) -> f32 {
        match self {
            Self::LittleEndian => LE::read_f32(bytes),
            Self::BigEndian => BE::read_f32(bytes),
        }
    }
}
