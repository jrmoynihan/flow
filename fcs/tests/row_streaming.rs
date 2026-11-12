use std::error::Error;

use flow_fcs::{Fcs, FcsFloatRowSink, FcsRowSource, WriteOptions, corpus};
use flow_linalg::{ChunkSize, MatrixShape, RowBlock, RowSource, read_row_block};
use tempfile::TempDir;

#[test]
fn selected_row_ranges_match_eager_integer_events_and_preserve_order() -> Result<(), Box<dyn Error>>
{
    let path = corpus::path("int-10_events_6_parameters.fcs");
    let fcs = Fcs::open(path.to_string_lossy().as_ref())?;
    let names = fcs.get_parameter_names_from_dataframe();
    let requested = [2, 0];
    let mut storage = Vec::new();

    fcs.read_selected_rows_into(&requested, 3..8, &mut storage)?;

    let expected: Vec<f32> = (3..8)
        .flat_map(|row| {
            requested.map(|channel| {
                fcs.column(&names[channel])
                    .expect("fixture channel")
                    .get(row)
                    .copied()
                    .expect("fixture row")
            })
        })
        .collect();
    assert_eq!(storage, expected);
    Ok(())
}

#[test]
fn selected_row_ranges_reject_invalid_channel_requests() -> Result<(), Box<dyn Error>> {
    let path = corpus::path("int-10_events_6_parameters.fcs");
    let fcs = Fcs::open(path.to_string_lossy().as_ref())?;
    let mut storage = vec![99.0];

    for channels in [&[][..], &[0, 0][..], &[usize::MAX][..]] {
        assert!(
            fcs.read_selected_rows_into(channels, 0..1, &mut storage)
                .is_err()
        );
    }
    assert!(
        fcs.read_selected_rows_into(&[0], 9..11, &mut storage)
            .is_err()
    );
    Ok(())
}

#[test]
fn row_source_honors_chunk_sizes_offsets_and_reuses_caller_buffer() -> Result<(), Box<dyn Error>> {
    let path = corpus::path("int-10_events_6_parameters.fcs");
    let fcs = Fcs::open(path.to_string_lossy().as_ref())?;
    let mut expected = Vec::new();
    fcs.read_selected_rows_into(&[1, 0], 0..10, &mut expected)?;

    for chunk_rows in [1, 4, 64] {
        let mut source = FcsRowSource::new(&fcs, &[1, 0])?;
        let shape = source.shape();
        let mut storage = Vec::with_capacity(shape.elements());
        let pointer = storage.as_ptr();
        let mut collected = Vec::new();
        for range in ChunkSize::new(chunk_rows)?.ranges(shape.rows()) {
            let expected_start = range.start;
            let requested_rows = range.end - range.start;
            let block = read_row_block(&mut source, range, &mut storage)?;
            assert_eq!(block.row_start(), expected_start);
            assert!(block.rows() <= chunk_rows);
            assert_eq!(block.rows(), requested_rows);
            collected.extend_from_slice(block.as_slice());
            assert_eq!(
                storage.as_ptr(),
                pointer,
                "caller allocation should be reused"
            );
        }
        assert_eq!(collected, expected);
    }
    Ok(())
}

#[test]
fn float_fcs_row_sink_round_trips_ragged_blocks() -> Result<(), Box<dyn Error>> {
    let input = corpus::path("int-10_events_6_parameters.fcs");
    let source = Fcs::open(input.to_string_lossy().as_ref())?;
    let template = source.write_metadata_template();
    let temp = TempDir::new()?;
    let path = temp.path().join("output.fcs");
    let shape = MatrixShape::new(5, 6)?;
    let values: Vec<f32> = (0..shape.elements())
        .map(|value| value as f32 + 0.25)
        .collect();
    let mut sink = FcsFloatRowSink::create(&template, &path, shape, WriteOptions::default())?;
    sink.write(RowBlock::new(0, 2, 6, &values[..12])?)?;
    sink.write(RowBlock::new(2, 3, 6, &values[12..])?)?;
    assert_eq!(sink.finish()?, path);

    let written = Fcs::open(path.to_string_lossy().as_ref())?;
    let mut decoded = Vec::new();
    written.read_selected_rows_into(&[0, 1, 2, 3, 4, 5], 0..5, &mut decoded)?;
    assert_eq!(decoded, values);
    Ok(())
}

#[test]
fn float_fcs_row_sink_rejects_invalid_blocks_and_never_publishes_failure()
-> Result<(), Box<dyn Error>> {
    let input = corpus::path("int-10_events_6_parameters.fcs");
    let source = Fcs::open(input.to_string_lossy().as_ref())?;
    let template = source.write_metadata_template();
    let temp = TempDir::new()?;
    let path = temp.path().join("output.fcs");
    let shape = MatrixShape::new(3, 6)?;
    let mut sink = FcsFloatRowSink::create(&template, &path, shape, WriteOptions::default())?;

    assert!(sink.write(RowBlock::new(1, 1, 6, &[0.0; 6])?).is_err());
    assert!(sink.write(RowBlock::new(0, 1, 5, &[0.0; 5])?).is_err());
    sink.write(RowBlock::new(0, 1, 6, &[0.0; 6])?)?;
    assert!(sink.finish().is_err());
    assert!(!path.exists());
    assert!(temp.path().read_dir()?.all(|entry| {
        !entry
            .expect("directory entry")
            .file_name()
            .to_string_lossy()
            .contains("partial")
    }));

    let sink = FcsFloatRowSink::create(&template, &path, shape, WriteOptions::default())?;
    drop(sink);
    assert!(!path.exists());
    assert!(temp.path().read_dir()?.all(|entry| {
        !entry
            .expect("directory entry")
            .file_name()
            .to_string_lossy()
            .contains("partial")
    }));
    Ok(())
}
