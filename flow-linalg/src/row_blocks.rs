//! Validated contracts for processing row-major matrices in bounded row blocks.
//!
//! A [`RowSource`] reports one fixed [`MatrixShape`] and fills storage owned by
//! its caller. The caller may clear and reuse that storage after the returned
//! [`RowBlock`] is no longer borrowed; a block never owns or outlives its data.
//! Sources must return exactly the requested rows in row-major order.
//!
//! An [`OrderedRowSink`] accepts blocks in increasing, gap-free global row
//! order. Every block must have the sink's column count, and [`OrderedRowSink::finish`]
//! succeeds only after exactly the declared number of rows have been written.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::ops::Range;

/// A validation failure in a row-block contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    /// Chunk sizes must contain at least one row.
    ZeroChunkSize,
    /// A row-major element count cannot be represented by `usize`.
    DimensionOverflow { rows: usize, columns: usize },
    /// A row block's global end offset cannot be represented by `usize`.
    RowEndOverflow { start: usize, rows: usize },
    /// A row-major buffer does not contain exactly `rows * columns` elements.
    DataLengthMismatch {
        rows: usize,
        columns: usize,
        expected: usize,
        actual: usize,
    },
    /// A source returned the wrong number of elements for a requested range.
    SourceLengthMismatch {
        start: usize,
        end: usize,
        columns: usize,
        expected: usize,
        actual: usize,
    },
    /// A source read is reversed or extends beyond the source's rows.
    ReadOutOfRange {
        start: usize,
        end: usize,
        total_rows: usize,
    },
    /// A sink received a block whose start does not follow the prior block.
    OutOfOrderBlock {
        expected_start: usize,
        actual_start: usize,
    },
    /// A sink received a block with an incompatible column count.
    ColumnMismatch { expected: usize, actual: usize },
    /// A sink block extends beyond the sink's declared row count.
    BlockExceedsShape {
        block_end: usize,
        expected_rows: usize,
    },
    /// A sink was finished before all declared rows were written.
    IncompleteSink {
        expected_rows: usize,
        written_rows: usize,
    },
}

impl Display for ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroChunkSize => formatter.write_str("chunk size must be non-zero"),
            Self::DimensionOverflow { rows, columns } => write!(
                formatter,
                "row-major dimensions {rows}x{columns} overflow usize"
            ),
            Self::RowEndOverflow { start, rows } => write!(
                formatter,
                "row block starting at {start} with {rows} rows overflows usize"
            ),
            Self::DataLengthMismatch {
                rows,
                columns,
                expected,
                actual,
            } => write!(
                formatter,
                "row-major {rows}x{columns} data requires {expected} elements, got {actual}"
            ),
            Self::SourceLengthMismatch {
                start,
                end,
                columns,
                expected,
                actual,
            } => write!(
                formatter,
                "source returned {actual} elements for rows {start}..{end} with {columns} columns; expected {expected}"
            ),
            Self::ReadOutOfRange {
                start,
                end,
                total_rows,
            } => write!(
                formatter,
                "requested row range {start}..{end} is outside 0..{total_rows}"
            ),
            Self::OutOfOrderBlock {
                expected_start,
                actual_start,
            } => write!(
                formatter,
                "sink expected next block at row {expected_start}, got row {actual_start}"
            ),
            Self::ColumnMismatch { expected, actual } => {
                write!(formatter, "sink expected {expected} columns, got {actual}")
            }
            Self::BlockExceedsShape {
                block_end,
                expected_rows,
            } => write!(
                formatter,
                "block ends at row {block_end}, beyond sink row count {expected_rows}"
            ),
            Self::IncompleteSink {
                expected_rows,
                written_rows,
            } => write!(
                formatter,
                "sink expected {expected_rows} rows, but received {written_rows}"
            ),
        }
    }
}

impl Error for ContractError {}

/// A validated row/column shape with a representable row-major element count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatrixShape {
    rows: usize,
    columns: usize,
    elements: usize,
}

impl MatrixShape {
    /// Validates `rows * columns` and constructs a shape.
    pub fn new(rows: usize, columns: usize) -> Result<Self, ContractError> {
        let elements = rows
            .checked_mul(columns)
            .ok_or(ContractError::DimensionOverflow { rows, columns })?;
        Ok(Self {
            rows,
            columns,
            elements,
        })
    }

    /// Number of rows.
    pub const fn rows(self) -> usize {
        self.rows
    }

    /// Number of columns.
    pub const fn columns(self) -> usize {
        self.columns
    }

    /// Checked row-major element count.
    pub const fn elements(self) -> usize {
        self.elements
    }
}

/// A validated non-zero maximum number of rows per chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkSize(usize);

impl ChunkSize {
    /// Constructs a chunk size, rejecting zero.
    pub const fn new(rows: usize) -> Result<Self, ContractError> {
        if rows == 0 {
            Err(ContractError::ZeroChunkSize)
        } else {
            Ok(Self(rows))
        }
    }

    /// Returns the non-zero number of rows.
    pub const fn get(self) -> usize {
        self.0
    }

    /// Splits `total_rows` into ordered ranges no larger than this chunk size.
    ///
    /// The last range is shorter when `total_rows` is not evenly divisible.
    pub fn ranges(self, total_rows: usize) -> ChunkRanges {
        ChunkRanges {
            next: 0,
            total_rows,
            chunk_size: self.0,
        }
    }
}

/// Ordered row ranges produced by [`ChunkSize::ranges`].
#[derive(Debug, Clone)]
pub struct ChunkRanges {
    next: usize,
    total_rows: usize,
    chunk_size: usize,
}

impl Iterator for ChunkRanges {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.total_rows {
            return None;
        }
        let remaining = self.total_rows - self.next;
        let end = self.next + remaining.min(self.chunk_size);
        let range = self.next..end;
        self.next = end;
        Some(range)
    }
}

/// A borrowed, validated row-major block with a global row offset.
///
/// `data` contains exactly `rows * columns` elements. The slice is borrowed
/// from caller-owned storage and remains valid only for this value's lifetime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowBlock<'a, T> {
    row_start: usize,
    shape: MatrixShape,
    data: &'a [T],
}

impl<'a, T> RowBlock<'a, T> {
    /// Validates the shape, global end row, and exact data length.
    pub fn new(
        row_start: usize,
        rows: usize,
        columns: usize,
        data: &'a [T],
    ) -> Result<Self, ContractError> {
        let shape = MatrixShape::new(rows, columns)?;
        row_start
            .checked_add(rows)
            .ok_or(ContractError::RowEndOverflow {
                start: row_start,
                rows,
            })?;
        if data.len() != shape.elements {
            return Err(ContractError::DataLengthMismatch {
                rows,
                columns,
                expected: shape.elements,
                actual: data.len(),
            });
        }
        Ok(Self {
            row_start,
            shape,
            data,
        })
    }

    /// Global index of this block's first row.
    pub const fn row_start(&self) -> usize {
        self.row_start
    }

    /// Global exclusive end row.
    pub fn row_end(&self) -> usize {
        self.row_start + self.shape.rows
    }

    /// Number of rows in this block.
    pub const fn rows(&self) -> usize {
        self.shape.rows
    }

    /// Number of columns in this block.
    pub const fn columns(&self) -> usize {
        self.shape.columns
    }

    /// The validated row-major elements.
    pub const fn as_slice(&self) -> &'a [T] {
        self.data
    }
}

/// A matrix source that fills caller-owned, reusable row-major storage.
///
/// Implementations must keep [`Self::shape`] stable and replace `storage`
/// with exactly `(rows.end - rows.start) * shape.columns()` elements. They
/// must not retain a reference to `storage`. Callers should normally use
/// [`read_row_block`], which validates the range and returned length.
pub trait RowSource<T> {
    /// Backend-specific read failure.
    type Error;

    /// Fixed shape of the source matrix.
    fn shape(&self) -> MatrixShape;

    /// Replaces `storage` with the requested rows in row-major order.
    fn read_rows(&mut self, rows: Range<usize>, storage: &mut Vec<T>) -> Result<(), Self::Error>;
}

/// A source read failure with requested-range context.
#[derive(Debug, PartialEq, Eq)]
pub enum RowReadError<E> {
    /// A caller or implementation violated the shape/range contract.
    Contract(ContractError),
    /// The source backend failed while reading the given range.
    Source { start: usize, end: usize, source: E },
}

impl<E: Display> Display for RowReadError<E> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => Display::fmt(error, formatter),
            Self::Source { start, end, source } => {
                write!(
                    formatter,
                    "source failed while reading rows {start}..{end}: {source}"
                )
            }
        }
    }
}

impl<E: Error + 'static> Error for RowReadError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            Self::Source { source, .. } => Some(source),
        }
    }
}

/// Reads and validates one block using storage that the caller can reuse.
///
/// The returned block borrows `storage`; mutate or reuse the vector only after
/// that block is no longer used.
pub fn read_row_block<'a, T, S>(
    source: &mut S,
    rows: Range<usize>,
    storage: &'a mut Vec<T>,
) -> Result<RowBlock<'a, T>, RowReadError<S::Error>>
where
    S: RowSource<T>,
{
    let shape = source.shape();
    if rows.start > rows.end || rows.end > shape.rows {
        return Err(RowReadError::Contract(ContractError::ReadOutOfRange {
            start: rows.start,
            end: rows.end,
            total_rows: shape.rows,
        }));
    }

    storage.clear();
    source
        .read_rows(rows.clone(), storage)
        .map_err(|source| RowReadError::Source {
            start: rows.start,
            end: rows.end,
            source,
        })?;

    let block_rows = rows.end - rows.start;
    let expected = block_rows * shape.columns;
    if storage.len() != expected {
        return Err(RowReadError::Contract(
            ContractError::SourceLengthMismatch {
                start: rows.start,
                end: rows.end,
                columns: shape.columns,
                expected,
                actual: storage.len(),
            },
        ));
    }

    RowBlock::new(rows.start, block_rows, shape.columns, storage).map_err(RowReadError::Contract)
}

/// Borrowed in-memory [`RowSource`] useful for tests and small matrices.
///
/// Reads copy only the requested rows into caller-owned storage.
#[derive(Debug, Clone)]
pub struct SliceRowSource<'a, T> {
    data: &'a [T],
    shape: MatrixShape,
}

impl<'a, T> SliceRowSource<'a, T> {
    /// Constructs a source after validating the complete row-major slice.
    pub fn new(data: &'a [T], rows: usize, columns: usize) -> Result<Self, ContractError> {
        let shape = MatrixShape::new(rows, columns)?;
        if data.len() != shape.elements {
            return Err(ContractError::DataLengthMismatch {
                rows,
                columns,
                expected: shape.elements,
                actual: data.len(),
            });
        }
        Ok(Self { data, shape })
    }
}

impl<T: Clone> RowSource<T> for SliceRowSource<'_, T> {
    type Error = ContractError;

    fn shape(&self) -> MatrixShape {
        self.shape
    }

    fn read_rows(&mut self, rows: Range<usize>, storage: &mut Vec<T>) -> Result<(), Self::Error> {
        if rows.start > rows.end || rows.end > self.shape.rows {
            return Err(ContractError::ReadOutOfRange {
                start: rows.start,
                end: rows.end,
                total_rows: self.shape.rows,
            });
        }
        let start = rows.start * self.shape.columns;
        let end = rows.end * self.shape.columns;
        storage.clear();
        storage.extend_from_slice(&self.data[start..end]);
        Ok(())
    }
}

/// A sink for gap-free row blocks with a fixed output shape.
///
/// Callers must submit blocks in increasing order starting at row zero. A sink
/// consumes each borrowed block during [`Self::write`] and must not retain the
/// block's data reference. [`Self::finish`] consumes the sink and returns its
/// output only when all declared rows have been accepted.
pub trait OrderedRowSink<T> {
    /// Sink-specific write or finalization failure.
    type Error;
    /// Completed output returned after shape validation.
    type Output;

    /// Fixed shape expected by the sink.
    fn shape(&self) -> MatrixShape;

    /// Consumes one block before its borrowed buffer may be reused.
    fn write(&mut self, block: RowBlock<'_, T>) -> Result<(), Self::Error>;

    /// Validates completeness and returns the completed output.
    fn finish(self) -> Result<Self::Output, Self::Error>;
}

/// Ordered in-memory sink useful for tests and collected outputs.
#[derive(Debug, Clone)]
pub struct VecRowSink<T> {
    shape: MatrixShape,
    next_row: usize,
    data: Vec<T>,
}

impl<T> VecRowSink<T> {
    /// Creates an empty sink for `shape`.
    pub fn new(shape: MatrixShape) -> Self {
        Self {
            shape,
            next_row: 0,
            data: Vec::new(),
        }
    }
}

impl<T: Clone> VecRowSink<T> {
    /// Writes an ordered block.
    pub fn write(&mut self, block: RowBlock<'_, T>) -> Result<(), ContractError> {
        <Self as OrderedRowSink<T>>::write(self, block)
    }

    /// Finishes and returns row-major output after checking all rows arrived.
    pub fn finish(self) -> Result<Vec<T>, ContractError> {
        <Self as OrderedRowSink<T>>::finish(self)
    }
}

impl<T: Clone> OrderedRowSink<T> for VecRowSink<T> {
    type Error = ContractError;
    type Output = Vec<T>;

    fn shape(&self) -> MatrixShape {
        self.shape
    }

    fn write(&mut self, block: RowBlock<'_, T>) -> Result<(), Self::Error> {
        if block.row_start != self.next_row {
            return Err(ContractError::OutOfOrderBlock {
                expected_start: self.next_row,
                actual_start: block.row_start,
            });
        }
        if block.shape.columns != self.shape.columns {
            return Err(ContractError::ColumnMismatch {
                expected: self.shape.columns,
                actual: block.shape.columns,
            });
        }
        let block_end = block.row_end();
        if block_end > self.shape.rows {
            return Err(ContractError::BlockExceedsShape {
                block_end,
                expected_rows: self.shape.rows,
            });
        }
        self.data.extend_from_slice(block.data);
        self.next_row = block_end;
        Ok(())
    }

    fn finish(self) -> Result<Self::Output, Self::Error> {
        if self.next_row != self.shape.rows {
            return Err(ContractError::IncompleteSink {
                expected_rows: self.shape.rows,
                written_rows: self.next_row,
            });
        }
        Ok(self.data)
    }
}
