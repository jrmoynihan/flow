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
pub use write::{
    add_column, concatenate_events, duplicate_fcs_file, edit_metadata_and_save, filter_events,
    write_fcs_file,
};

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
pub mod write;

pub type GUID = String;
pub type FileKeyword = String;
pub type FilePath = PathBuf;
pub type EventCount = usize;
