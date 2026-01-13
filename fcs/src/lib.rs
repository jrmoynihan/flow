#![feature(ascii_char)]

use std::path::PathBuf;

pub use byteorder::ByteOrder;
pub use datatype::FcsDataType;
pub use file::Fcs;
pub use header::Header;
pub use keyword::Keyword;
pub use metadata::Metadata;
pub use parameter::{ChannelName, Parameter};
pub use transform::{Formattable, TransformType, Transformable};
pub use version::Version;

mod byteorder;
pub mod datatype;
pub mod file;
pub mod header;
pub mod keyword;
pub mod metadata;
pub mod parameter;
mod tests;
pub mod transform;
pub mod version;

pub type GUID = String;
pub type FileKeyword = String;
pub type FilePath = PathBuf;
pub type EventCount = usize;
