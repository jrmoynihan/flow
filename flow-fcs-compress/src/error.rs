use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("codec id {0:#06x} is not registered")]
    UnknownCodec(u16),

    #[error("decoded buffer length mismatch: expected {expected}, got {actual}")]
    LengthMismatch { expected: usize, actual: usize },

    #[error("chunk checksum mismatch (xxh3): expected {expected:#018x}, got {actual:#018x}")]
    ChecksumMismatch { expected: u64, actual: u64 },

    #[error("payload truncated: needed {needed} bytes, have {have}")]
    Truncated { needed: usize, have: usize },

    #[error("invalid channel params: {0}")]
    InvalidParams(&'static str),

    #[error("zstd: {0}")]
    Zstd(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
