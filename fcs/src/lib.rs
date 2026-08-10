use std::path::PathBuf;

pub use byteorder::ByteOrder;
pub use datatype::FcsDataType;
pub use file::Fcs;
pub use header::Header;
pub use keyword::Keyword;
pub use matrix::MatrixOps;
pub use metadata::Metadata;
pub use parameter::{ChannelName, EventDataFrame, EventDatum, LabelName, Parameter, ParameterMap};
pub use transform::{Formattable, TransformType, Transformable};
pub use version::Version;
pub use conformance::{Severity, Violation};
pub use crc::StoredCrc;
pub use write::{
    ConformancePolicy, CrcPolicy, WriteOptions, add_column, concatenate_events,
    duplicate_fcs_file, edit_metadata_and_save, filter_events, write_fcs_file,
    write_fcs_file_with,
};

mod byteorder;
pub(crate) mod columns;
#[cfg(feature = "compress")]
pub mod compress;
pub mod conformance;
pub mod crc;
pub mod datatype;
pub mod datetime;
pub mod file;
pub mod header;
pub mod keyword;
pub mod matrix;
pub mod metadata;
pub mod parameter;
#[cfg(feature = "synthetic")]
pub mod synthetic;
mod tests;
pub mod transform;
pub mod upgrade;
pub mod version;
pub mod write;

pub type GUID = String;
pub type FileKeyword = String;
pub type FilePath = PathBuf;
pub type EventCount = usize;
