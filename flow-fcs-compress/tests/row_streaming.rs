use std::error::Error;

use flow_fcs_compress::codec::{ChannelParams, CodecId};
use flow_fcs_compress::container::fcz::{
    FczReader, FczRowSink, FczRowSource, FczWriteOptions, FczWriter,
};
use flow_linalg::{ChunkSize, MatrixShape, RowBlock, RowSource, read_row_block};
use tempfile::TempDir;

fn params(name: &str) -> ChannelParams {
    ChannelParams {
        name: name.to_string(),
        stored_bits: 32,
        range: 262_144,
        log_decades: (0.0, 0.0),
        adc_bits: None,
        signed: true,
    }
}

#[test]
fn selected_rows_cross_chunks_and_preserve_requested_channel_order() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let path = temp.path().join("input.fcz");
    let mut writer = FczWriter::create(
        &path,
        FczWriteOptions {
            events_per_chunk: 3,
        },
    )?;
    writer.add_channel(params("A"), CodecId::LosslessF32BssZstd)?;
    writer.add_channel(params("B"), CodecId::LosslessF32BssZstd)?;
    for (chunk, range) in [(0, 0..3), (1, 3..6), (2, 6..8)] {
        let a: Vec<f32> = range.clone().map(|row| row as f32).collect();
        let b: Vec<f32> = range.map(|row| row as f32 + 100.0).collect();
        writer.write_chunk(0, chunk, &a)?;
        writer.write_chunk(1, chunk, &b)?;
    }
    writer.finish()?;

    let reader = FczReader::open(&path)?;
    let mut storage = Vec::new();
    reader.read_selected_rows_into(&[1, 0], 2..7, &mut storage)?;
    assert_eq!(
        storage,
        vec![102.0, 2.0, 103.0, 3.0, 104.0, 4.0, 105.0, 5.0, 106.0, 6.0]
    );
    Ok(())
}

#[test]
fn selected_rows_reject_invalid_requests() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let path = temp.path().join("input.fcz");
    let mut writer = FczWriter::create(
        &path,
        FczWriteOptions {
            events_per_chunk: 2,
        },
    )?;
    writer.add_channel(params("A"), CodecId::LosslessF32BssZstd)?;
    writer.write_chunk(0, 0, &[1.0, 2.0])?;
    writer.finish()?;
    let reader = FczReader::open(&path)?;
    let mut storage = Vec::new();

    assert!(
        reader
            .read_selected_rows_into(&[], 0..1, &mut storage)
            .is_err()
    );
    assert!(
        reader
            .read_selected_rows_into(&[0, 0], 0..1, &mut storage)
            .is_err()
    );
    assert!(
        reader
            .read_selected_rows_into(&[1], 0..1, &mut storage)
            .is_err()
    );
    assert!(
        reader
            .read_selected_rows_into(&[0], 1..3, &mut storage)
            .is_err()
    );
    Ok(())
}

#[test]
fn fcz_row_source_limits_reads_and_reuses_storage() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let path = temp.path().join("input.fcz");
    let mut writer = FczWriter::create(
        &path,
        FczWriteOptions {
            events_per_chunk: 3,
        },
    )?;
    writer.add_channel(params("A"), CodecId::LosslessF32BssZstd)?;
    for (chunk, values) in [(0, &[0.0, 1.0, 2.0][..]), (1, &[3.0, 4.0][..])] {
        writer.write_chunk(0, chunk, values)?;
    }
    writer.finish()?;
    let reader = FczReader::open(&path)?;

    for chunk_rows in [1, 2, 9] {
        let mut source = FczRowSource::new(&reader, &[0])?;
        let shape = source.shape();
        let mut storage = Vec::with_capacity(shape.elements());
        let pointer = storage.as_ptr();
        let mut collected = Vec::new();
        for range in ChunkSize::new(chunk_rows)?.ranges(shape.rows()) {
            let requested = range.end - range.start;
            let block = read_row_block(&mut source, range, &mut storage)?;
            assert_eq!(block.rows(), requested);
            assert!(block.rows() <= chunk_rows);
            collected.extend_from_slice(block.as_slice());
            assert_eq!(storage.as_ptr(), pointer);
        }
        assert_eq!(collected, [0.0, 1.0, 2.0, 3.0, 4.0]);
    }
    Ok(())
}

#[test]
fn fcz_row_sink_round_trips_ragged_final_chunk() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let path = temp.path().join("output.fcz");
    let channels = [
        (params("A"), CodecId::LosslessF32BssZstd),
        (params("B"), CodecId::LosslessF32BssZstd),
    ];
    let shape = MatrixShape::new(5, 2)?;
    let mut sink = FczRowSink::create(
        &path,
        shape,
        &channels,
        FczWriteOptions {
            events_per_chunk: 3,
        },
        b"metadata",
    )?;
    sink.write(RowBlock::new(0, 2, 2, &[0.0, 100.0, 1.0, 101.0])?)?;
    sink.write(RowBlock::new(
        2,
        3,
        2,
        &[2.0, 102.0, 3.0, 103.0, 4.0, 104.0],
    )?)?;
    let published = sink.finish()?;
    assert_eq!(published, path);

    let reader = FczReader::open(&path)?;
    let mut rows = Vec::new();
    reader.read_selected_rows_into(&[0, 1], 0..5, &mut rows)?;
    assert_eq!(
        rows,
        [0.0, 100.0, 1.0, 101.0, 2.0, 102.0, 3.0, 103.0, 4.0, 104.0]
    );
    Ok(())
}

#[test]
fn fcz_row_sink_rejects_order_shape_and_incomplete_finish_and_cleans_up()
-> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let path = temp.path().join("output.fcz");
    let channels = [(params("A"), CodecId::LosslessF32BssZstd)];
    let shape = MatrixShape::new(3, 1)?;

    let mut sink = FczRowSink::create(
        &path,
        shape,
        &channels,
        FczWriteOptions {
            events_per_chunk: 2,
        },
        b"",
    )?;
    assert!(sink.write(RowBlock::new(1, 1, 1, &[1.0])?).is_err());
    assert!(sink.write(RowBlock::new(0, 1, 2, &[1.0, 2.0])?).is_err());
    sink.write(RowBlock::new(0, 1, 1, &[1.0])?)?;
    assert!(sink.finish().is_err());
    assert!(!path.exists());
    assert!(temp.path().read_dir()?.all(|entry| {
        !entry
            .expect("directory entry")
            .file_name()
            .to_string_lossy()
            .contains("partial")
    }));

    let sink = FczRowSink::create(
        &path,
        shape,
        &channels,
        FczWriteOptions {
            events_per_chunk: 2,
        },
        b"",
    )?;
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
