//! Incremental row-major FCS output with atomic publication.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use flow_linalg::{ContractError, MatrixShape, OrderedRowSink, RowBlock};
use thiserror::Error;

use crate::crc::Crc16;
use crate::datatype::FcsDataType;
use crate::file::FcsWriteSource;
use crate::keyword::{ByteKeyword, IntegerKeyword, Keyword};
use crate::write::{CrcPolicy, HEADER_SIZE, WriteOptions};
use crate::{ByteOrder, Fcs};

static PARTIAL_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Failure while incrementally writing an FCS row stream.
#[derive(Debug, Error)]
pub enum FcsRowSinkError {
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error("FCS filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Format(String),
}

/// Incremental `$DATATYPE F` row sink with fixed schema and atomic publication.
pub struct FcsFloatRowSink {
    file: Option<BufWriter<File>>,
    final_path: PathBuf,
    partial_path: PathBuf,
    shape: MatrixShape,
    next_row: usize,
    byte_order: ByteOrder,
    crc: Option<Crc16>,
    published: bool,
}

impl FcsFloatRowSink {
    /// Create a float FCS sink from an existing metadata/schema source.
    pub fn create(
        source: &(impl FcsWriteSource + ?Sized),
        path: impl AsRef<Path>,
        shape: MatrixShape,
        options: WriteOptions,
    ) -> Result<Self, FcsRowSinkError> {
        let final_path = path.as_ref().to_path_buf();
        if final_path.extension().and_then(|value| value.to_str()) != Some("fcs") {
            return Err(FcsRowSinkError::Format(
                "FCS output path must have .fcs extension".to_string(),
            ));
        }
        if shape.rows() == 0 || shape.columns() == 0 {
            return Err(FcsRowSinkError::Format(
                "FCS output shape must contain rows and columns".to_string(),
            ));
        }
        let mut metadata = source.write_metadata().clone();
        let declared_columns = *metadata
            .get_number_of_parameters()
            .map_err(|error| FcsRowSinkError::Format(format!("read $PAR: {error}")))?;
        if declared_columns != shape.columns() {
            return Err(FcsRowSinkError::Format(format!(
                "metadata declares {declared_columns} parameters but sink shape has {} columns",
                shape.columns()
            )));
        }
        metadata.keywords.insert(
            "$DATATYPE".to_string(),
            Keyword::Byte(ByteKeyword::DATATYPE(FcsDataType::F)),
        );
        for parameter in 1..=shape.columns() {
            metadata.keywords.insert(
                format!("$P{parameter}B"),
                Keyword::Int(IntegerKeyword::PnB(32)),
            );
            metadata.keywords.remove(&format!("$P{parameter}DATATYPE"));
        }
        let byte_order = metadata
            .get_byte_order()
            .map_err(|error| FcsRowSinkError::Format(format!("read $BYTEORD: {error}")))?
            .clone();
        let data_len = shape
            .elements()
            .checked_mul(std::mem::size_of::<f32>())
            .ok_or_else(|| {
                FcsRowSinkError::Format("FCS DATA byte length overflows usize".to_string())
            })?;
        crate::write::enforce_conformance(
            &metadata,
            source.write_header().version,
            options.conformance,
            &final_path,
        )
        .map_err(|error| FcsRowSinkError::Format(format!("FCS conformance: {error}")))?;
        let layout = crate::write::resolve_layout(
            &metadata,
            HEADER_SIZE,
            shape.rows(),
            shape.columns(),
            data_len,
            source.write_header().version,
        )
        .map_err(|error| FcsRowSinkError::Format(format!("resolve FCS layout: {error}")))?;
        let header = crate::write::build_header(
            &source.write_header().version,
            layout.text_start,
            layout.text_end,
            layout.data_start,
            layout.data_end,
        )
        .map_err(|error| FcsRowSinkError::Format(format!("build FCS header: {error}")))?;
        let partial_path = sibling_partial_path(&final_path)?;
        let initialized = (|| {
            let mut file = BufWriter::new(File::create(&partial_path)?);
            file.write_all(&header)?;
            file.write_all(&layout.text_segment)?;
            let mut crc = (options.crc == CrcPolicy::Compute).then(Crc16::new);
            if let Some(crc) = &mut crc {
                crc.update(&header);
                crc.update(&layout.text_segment);
            }
            Ok::<_, FcsRowSinkError>((file, crc))
        })();
        let (file, crc) = match initialized {
            Ok(initialized) => initialized,
            Err(error) => {
                let _ = std::fs::remove_file(&partial_path);
                return Err(error);
            }
        };
        Ok(Self {
            file: Some(file),
            final_path,
            partial_path,
            shape,
            next_row: 0,
            byte_order,
            crc,
            published: false,
        })
    }

    pub fn write(&mut self, block: RowBlock<'_, f32>) -> Result<(), FcsRowSinkError> {
        <Self as OrderedRowSink<f32>>::write(self, block)
    }

    pub fn finish(mut self) -> Result<PathBuf, FcsRowSinkError> {
        if self.next_row != self.shape.rows() {
            return Err(ContractError::IncompleteSink {
                expected_rows: self.shape.rows(),
                written_rows: self.next_row,
            }
            .into());
        }
        let checksum = self.crc.take().map(Crc16::finish);
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| FcsRowSinkError::Format("FCS sink is already closed".to_string()))?;
        file.write_all(&crate::crc::format_field(checksum))?;
        file.flush()?;
        file.get_ref().sync_all()?;
        drop(self.file.take());

        let path_text = self.partial_path.to_str().ok_or_else(|| {
            FcsRowSinkError::Format("partial FCS path is not valid UTF-8".to_string())
        })?;
        let opened = Fcs::open_verified(path_text)
            .map_err(|error| FcsRowSinkError::Format(format!("validate final FCS: {error}")))?;
        if *opened
            .get_number_of_events()
            .map_err(|error| FcsRowSinkError::Format(format!("validate $TOT: {error}")))?
            != self.shape.rows()
            || *opened
                .get_number_of_parameters()
                .map_err(|error| FcsRowSinkError::Format(format!("validate $PAR: {error}")))?
                != self.shape.columns()
        {
            return Err(FcsRowSinkError::Format(
                "final FCS shape does not match declared sink shape".to_string(),
            ));
        }
        drop(opened);
        std::fs::rename(&self.partial_path, &self.final_path)?;
        sync_parent_directory(&self.final_path);
        self.published = true;
        Ok(self.final_path.clone())
    }
}

impl OrderedRowSink<f32> for FcsFloatRowSink {
    type Error = FcsRowSinkError;
    type Output = PathBuf;

    fn shape(&self) -> MatrixShape {
        self.shape
    }

    fn write(&mut self, block: RowBlock<'_, f32>) -> Result<(), FcsRowSinkError> {
        if block.row_start() != self.next_row {
            return Err(ContractError::OutOfOrderBlock {
                expected_start: self.next_row,
                actual_start: block.row_start(),
            }
            .into());
        }
        if block.columns() != self.shape.columns() {
            return Err(ContractError::ColumnMismatch {
                expected: self.shape.columns(),
                actual: block.columns(),
            }
            .into());
        }
        if block.row_end() > self.shape.rows() {
            return Err(ContractError::BlockExceedsShape {
                block_end: block.row_end(),
                expected_rows: self.shape.rows(),
            }
            .into());
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| FcsRowSinkError::Format("FCS sink is already closed".to_string()))?;
        for &value in block.as_slice() {
            let bytes = match self.byte_order {
                ByteOrder::LittleEndian => value.to_le_bytes(),
                ByteOrder::BigEndian => value.to_be_bytes(),
            };
            file.write_all(&bytes)?;
            if let Some(crc) = &mut self.crc {
                crc.update(&bytes);
            }
        }
        self.next_row = block.row_end();
        Ok(())
    }

    fn finish(self) -> Result<PathBuf, FcsRowSinkError> {
        FcsFloatRowSink::finish(self)
    }
}

impl Drop for FcsFloatRowSink {
    fn drop(&mut self) {
        if !self.published {
            drop(self.file.take());
            let _ = std::fs::remove_file(&self.partial_path);
        }
    }
}

fn sibling_partial_path(final_path: &Path) -> Result<PathBuf, FcsRowSinkError> {
    let parent = final_path.parent().unwrap_or_else(|| Path::new("."));
    let name = final_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| FcsRowSinkError::Format("FCS output filename is not UTF-8".to_string()))?;
    let sequence = PARTIAL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".{name}.{}.{}.partial.fcs",
        std::process::id(),
        sequence
    )))
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
    }
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) {}
