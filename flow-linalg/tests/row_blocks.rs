use flow_linalg::{
    ChunkSize, ContractError, MatrixShape, RowBlock, RowReadError, RowSource, SliceRowSource,
    VecRowSink, read_row_block,
};
use std::convert::Infallible;
use std::ops::Range;

#[test]
fn chunk_size_rejects_zero() {
    assert_eq!(ChunkSize::new(0), Err(ContractError::ZeroChunkSize));
    assert_eq!(ChunkSize::new(3).map(ChunkSize::get), Ok(3));
}

#[test]
fn dimensions_reject_overflow_and_mismatched_data() {
    assert_eq!(
        MatrixShape::new(usize::MAX, 2),
        Err(ContractError::DimensionOverflow {
            rows: usize::MAX,
            columns: 2,
        })
    );

    assert_eq!(
        RowBlock::new(0, 2, 3, &[1_u8, 2, 3]),
        Err(ContractError::DataLengthMismatch {
            rows: 2,
            columns: 3,
            expected: 6,
            actual: 3,
        })
    );
}

#[test]
fn source_rejects_out_of_range_reads() {
    let data = [1_u8, 2, 3, 4, 5, 6];
    let mut source = SliceRowSource::new(&data, 3, 2).expect("valid source");
    let mut storage = Vec::new();

    assert_eq!(
        read_row_block(&mut source, 2..4, &mut storage),
        Err(RowReadError::Contract(ContractError::ReadOutOfRange {
            start: 2,
            end: 4,
            total_rows: 3,
        }))
    );
}

struct ShortReadSource {
    shape: MatrixShape,
}

impl RowSource<u8> for ShortReadSource {
    type Error = Infallible;

    fn shape(&self) -> MatrixShape {
        self.shape
    }

    fn read_rows(&mut self, rows: Range<usize>, storage: &mut Vec<u8>) -> Result<(), Self::Error> {
        storage.resize((rows.end - rows.start) * self.shape.columns() - 1, 0);
        Ok(())
    }
}

#[test]
fn source_length_mismatch_reports_requested_range() {
    let mut source = ShortReadSource {
        shape: MatrixShape::new(4, 2).expect("valid shape"),
    };
    let mut storage = Vec::new();

    assert_eq!(
        read_row_block(&mut source, 1..3, &mut storage),
        Err(RowReadError::Contract(
            ContractError::SourceLengthMismatch {
                start: 1,
                end: 3,
                columns: 2,
                expected: 4,
                actual: 3,
            }
        ))
    );
}

#[test]
fn vec_sink_enforces_order_and_finish_completeness() {
    let shape = MatrixShape::new(3, 2).expect("valid shape");
    let mut sink = VecRowSink::new(shape);
    let late = RowBlock::new(1, 1, 2, &[3_u8, 4]).expect("valid block");

    assert_eq!(
        sink.write(late),
        Err(ContractError::OutOfOrderBlock {
            expected_start: 0,
            actual_start: 1,
        })
    );

    let first = RowBlock::new(0, 2, 2, &[1_u8, 2, 3, 4]).expect("valid block");
    sink.write(first).expect("ordered block");
    assert_eq!(
        sink.finish(),
        Err(ContractError::IncompleteSink {
            expected_rows: 3,
            written_rows: 2,
        })
    );

    let mut complete_sink = VecRowSink::new(shape);
    complete_sink
        .write(RowBlock::new(0, 2, 2, &[1_u8, 2, 3, 4]).expect("valid first block"))
        .expect("ordered first block");
    complete_sink
        .write(RowBlock::new(2, 1, 2, &[5_u8, 6]).expect("valid final block"))
        .expect("ordered final block");
    assert_eq!(complete_sink.finish(), Ok(vec![1, 2, 3, 4, 5, 6]));
}

#[test]
fn chunk_ranges_include_ragged_final_chunk() {
    let chunk_size = ChunkSize::new(2).expect("non-zero");
    let ranges: Vec<_> = chunk_size.ranges(5).collect();
    assert_eq!(ranges, vec![0..2, 2..4, 4..5]);
}

#[test]
fn read_row_block_reuses_caller_storage() {
    let data = [1_u32, 2, 3, 4, 5, 6, 7, 8];
    let mut source = SliceRowSource::new(&data, 4, 2).expect("valid source");
    let mut storage = Vec::with_capacity(4);

    let first = read_row_block(&mut source, 0..2, &mut storage).expect("first read");
    let first_ptr = first.as_slice().as_ptr();
    drop(first);
    let second = read_row_block(&mut source, 2..4, &mut storage).expect("second read");

    assert_eq!(second.as_slice(), &[5, 6, 7, 8]);
    assert_eq!(second.as_slice().as_ptr(), first_ptr);
}

struct MaximumReadSource {
    shape: MatrixShape,
    maximum_rows: usize,
    largest_request: usize,
}

impl RowSource<u8> for MaximumReadSource {
    type Error = Infallible;

    fn shape(&self) -> MatrixShape {
        self.shape
    }

    fn read_rows(&mut self, rows: Range<usize>, storage: &mut Vec<u8>) -> Result<(), Self::Error> {
        let requested = rows.end - rows.start;
        self.largest_request = self.largest_request.max(requested);
        assert!(
            requested <= self.maximum_rows,
            "source read {requested} rows, configured maximum is {}",
            self.maximum_rows
        );
        storage.resize(requested * self.shape.columns(), 0);
        Ok(())
    }
}

#[test]
fn chunk_ranges_bound_every_source_read() {
    let shape = MatrixShape::new(7, 3).expect("valid shape");
    let chunk_size = ChunkSize::new(3).expect("non-zero");
    let mut source = MaximumReadSource {
        shape,
        maximum_rows: chunk_size.get(),
        largest_request: 0,
    };
    let mut storage = Vec::new();

    for range in chunk_size.ranges(shape.rows()) {
        let block = read_row_block(&mut source, range, &mut storage).expect("bounded read");
        assert!(block.rows() <= chunk_size.get());
    }

    assert_eq!(source.largest_request, 3);
}
