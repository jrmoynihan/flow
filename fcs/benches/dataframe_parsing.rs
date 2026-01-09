use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::time::Duration;

/// Generate synthetic FCS data for benchmarking
fn generate_test_data(num_events: usize, num_params: usize, bytes_per_param: usize) -> Vec<u8> {
    let total_bytes = num_events * num_params * bytes_per_param;
    let mut data = Vec::with_capacity(total_bytes);

    // Generate simple pattern data (alternating values)
    for i in 0..total_bytes {
        data.push((i % 256) as u8);
    }

    data
}

/// Parse uniform float32 data with parallel iteration (WITH compiler hints)
#[inline]
fn parse_uniform_f32_parallel_inline(
    data_bytes: &[u8],
    _num_events: usize,
    _num_params: usize,
) -> Vec<f32> {
    use rayon::prelude::*;

    data_bytes
        .par_chunks_exact(4)
        .map(|chunk| {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(chunk);
            f32::from_ne_bytes(bytes)
        })
        .collect()
}

/// Parse uniform float32 data with parallel iteration (WITHOUT compiler hints)
fn parse_uniform_f32_parallel_no_hints(
    data_bytes: &[u8],
    _num_events: usize,
    _num_params: usize,
) -> Vec<f32> {
    use rayon::prelude::*;

    data_bytes
        .par_chunks_exact(4)
        .map(|chunk| {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(chunk);
            f32::from_ne_bytes(bytes)
        })
        .collect()
}

/// Parse uniform float32 data without parallel iteration (sequential WITH inline hint)
#[inline]
fn parse_uniform_f32_sequential_inline(
    data_bytes: &[u8],
    _num_events: usize,
    _num_params: usize,
) -> Vec<f32> {
    data_bytes
        .chunks_exact(4)
        .map(|chunk| {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(chunk);
            f32::from_ne_bytes(bytes)
        })
        .collect()
}

/// Parse uniform float32 data without parallel iteration (sequential WITHOUT hints)
fn parse_uniform_f32_sequential_no_hints(
    data_bytes: &[u8],
    _num_events: usize,
    _num_params: usize,
) -> Vec<f32> {
    data_bytes
        .chunks_exact(4)
        .map(|chunk| {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(chunk);
            f32::from_ne_bytes(bytes)
        })
        .collect()
}

/// Parse uniform int16 data with parallel iteration (WITH inline hint)
#[inline]
fn parse_uniform_i16_parallel_inline(
    data_bytes: &[u8],
    _num_events: usize,
    _num_params: usize,
) -> Vec<f32> {
    use byteorder::{ByteOrder as BO, LittleEndian as LE};
    use rayon::prelude::*;

    data_bytes
        .par_chunks_exact(2)
        .map(|chunk| LE::read_u16(chunk) as f32)
        .collect()
}

/// Parse uniform int16 data with parallel iteration (WITHOUT hints)
fn parse_uniform_i16_parallel_no_hints(
    data_bytes: &[u8],
    _num_events: usize,
    _num_params: usize,
) -> Vec<f32> {
    use byteorder::{ByteOrder as BO, LittleEndian as LE};
    use rayon::prelude::*;

    data_bytes
        .par_chunks_exact(2)
        .map(|chunk| LE::read_u16(chunk) as f32)
        .collect()
}

/// Parse uniform int16 data without parallel iteration (sequential WITH inline)
#[inline]
fn parse_uniform_i16_sequential_inline(
    data_bytes: &[u8],
    _num_events: usize,
    _num_params: usize,
) -> Vec<f32> {
    use byteorder::{ByteOrder as BO, LittleEndian as LE};

    data_bytes
        .chunks_exact(2)
        .map(|chunk| LE::read_u16(chunk) as f32)
        .collect()
}

/// Parse uniform int16 data without parallel iteration (sequential WITHOUT hints)
fn parse_uniform_i16_sequential_no_hints(
    data_bytes: &[u8],
    _num_events: usize,
    _num_params: usize,
) -> Vec<f32> {
    use byteorder::{ByteOrder as BO, LittleEndian as LE};

    data_bytes
        .chunks_exact(2)
        .map(|chunk| LE::read_u16(chunk) as f32)
        .collect()
}

/// Parse variable-width data event-by-event (WITH cold hint)
#[cold]
fn parse_variable_width_sequential_cold(
    data_bytes: &[u8],
    bytes_per_param: &[usize],
    num_events: usize,
    num_params: usize,
) -> Vec<f32> {
    use byteorder::{ByteOrder as BO, LittleEndian as LE};

    let mut f32_values = Vec::with_capacity(num_events * num_params);
    let mut data_offset = 0;

    for _event_idx in 0..num_events {
        for &bytes_per_param in bytes_per_param {
            let param_bytes = &data_bytes[data_offset..data_offset + bytes_per_param];

            let value = match bytes_per_param {
                2 => LE::read_u16(param_bytes) as f32,
                4 => {
                    let mut bytes = [0u8; 4];
                    bytes.copy_from_slice(param_bytes);
                    f32::from_ne_bytes(bytes)
                }
                _ => 0.0,
            };

            f32_values.push(value);
            data_offset += bytes_per_param;
        }
    }

    f32_values
}

/// Parse variable-width data event-by-event (WITHOUT cold hint)
fn parse_variable_width_sequential_no_hints(
    data_bytes: &[u8],
    bytes_per_param: &[usize],
    num_events: usize,
    num_params: usize,
) -> Vec<f32> {
    use byteorder::{ByteOrder as BO, LittleEndian as LE};

    let mut f32_values = Vec::with_capacity(num_events * num_params);
    let mut data_offset = 0;

    for _event_idx in 0..num_events {
        for &bytes_per_param in bytes_per_param {
            let param_bytes = &data_bytes[data_offset..data_offset + bytes_per_param];

            let value = match bytes_per_param {
                2 => LE::read_u16(param_bytes) as f32,
                4 => {
                    let mut bytes = [0u8; 4];
                    bytes.copy_from_slice(param_bytes);
                    f32::from_ne_bytes(bytes)
                }
                _ => 0.0,
            };

            f32_values.push(value);
            data_offset += bytes_per_param;
        }
    }

    f32_values
}

fn bench_dataframe_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("dataframe_parsing");

    // Test different sizes to find parallelization threshold
    // Flow cytometry datasets typically range from 50k-1M events
    // Testing: 10k, 25k, 50k, 100k, 250k, 500k, 1M events (all with 8 params)
    let sizes = vec![
        (10_000, 8),    // 10k events, 8 params = 80k values
        (25_000, 8),    // 25k events, 8 params = 200k values
        (50_000, 8),    // 50k events, 8 params = 400k values
        (100_000, 8),   // 100k events, 8 params = 800k values
        (250_000, 8),   // 250k events, 8 params = 2M values
        (500_000, 8),   // 500k events, 8 params = 4M values
        (1_000_000, 8), // 1M events, 8 params = 8M values
    ];

    // Benchmark 1: Uniform float32 (4 bytes) - parallel vs sequential
    for (num_events, num_params) in sizes.iter().copied() {
        let bytes_per_param = 4;
        let data = generate_test_data(num_events, num_params, bytes_per_param);
        let total_values = num_events * num_params;

        group.throughput(Throughput::Elements(total_values as u64));

        // WITH compiler hints
        group.bench_with_input(
            BenchmarkId::new(
                "uniform_f32_parallel_inline",
                format!("{}_events", num_events),
            ),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(parse_uniform_f32_parallel_inline(
                        black_box(data),
                        num_events,
                        num_params,
                    ))
                })
            },
        );

        // WITHOUT compiler hints
        group.bench_with_input(
            BenchmarkId::new(
                "uniform_f32_parallel_no_hints",
                format!("{}_events", num_events),
            ),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(parse_uniform_f32_parallel_no_hints(
                        black_box(data),
                        num_events,
                        num_params,
                    ))
                })
            },
        );

        // WITH compiler hints (sequential)
        group.bench_with_input(
            BenchmarkId::new(
                "uniform_f32_sequential_inline",
                format!("{}_events", num_events),
            ),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(parse_uniform_f32_sequential_inline(
                        black_box(data),
                        num_events,
                        num_params,
                    ))
                })
            },
        );

        // WITHOUT compiler hints (sequential)
        group.bench_with_input(
            BenchmarkId::new(
                "uniform_f32_sequential_no_hints",
                format!("{}_events", num_events),
            ),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(parse_uniform_f32_sequential_no_hints(
                        black_box(data),
                        num_events,
                        num_params,
                    ))
                })
            },
        );
    }

    // Benchmark 2: Uniform int16 (2 bytes) - parallel vs sequential
    for (num_events, num_params) in sizes.iter().copied() {
        let bytes_per_param = 2;
        let data = generate_test_data(num_events, num_params, bytes_per_param);
        let total_values = num_events * num_params;

        group.throughput(Throughput::Elements(total_values as u64));

        // WITH compiler hints
        group.bench_with_input(
            BenchmarkId::new(
                "uniform_i16_parallel_inline",
                format!("{}_events", num_events),
            ),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(parse_uniform_i16_parallel_inline(
                        black_box(data),
                        num_events,
                        num_params,
                    ))
                })
            },
        );

        // WITHOUT compiler hints
        group.bench_with_input(
            BenchmarkId::new(
                "uniform_i16_parallel_no_hints",
                format!("{}_events", num_events),
            ),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(parse_uniform_i16_parallel_no_hints(
                        black_box(data),
                        num_events,
                        num_params,
                    ))
                })
            },
        );

        // WITH compiler hints (sequential)
        group.bench_with_input(
            BenchmarkId::new(
                "uniform_i16_sequential_inline",
                format!("{}_events", num_events),
            ),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(parse_uniform_i16_sequential_inline(
                        black_box(data),
                        num_events,
                        num_params,
                    ))
                })
            },
        );

        // WITHOUT compiler hints (sequential)
        group.bench_with_input(
            BenchmarkId::new(
                "uniform_i16_sequential_no_hints",
                format!("{}_events", num_events),
            ),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(parse_uniform_i16_sequential_no_hints(
                        black_box(data),
                        num_events,
                        num_params,
                    ))
                })
            },
        );
    }

    // Benchmark 3: Variable-width (mixed sizes) - sequential only
    for (num_events, num_params) in sizes.iter().copied() {
        // Mix of 2-byte and 4-byte parameters
        let bytes_per_param: Vec<usize> = (0..num_params)
            .map(|i| if i % 2 == 0 { 2 } else { 4 })
            .collect();

        let total_bytes: usize = bytes_per_param.iter().sum::<usize>() * num_events;
        let data = generate_test_data(num_events, 1, total_bytes / num_events);
        let total_values = num_events * num_params;

        group.throughput(Throughput::Elements(total_values as u64));

        // WITH cold hint
        group.bench_with_input(
            BenchmarkId::new(
                "variable_width_sequential_cold",
                format!("{}_events", num_events),
            ),
            &(data.clone(), bytes_per_param.clone()),
            |b, (data, bytes_per_param)| {
                b.iter(|| {
                    black_box(parse_variable_width_sequential_cold(
                        black_box(data),
                        black_box(bytes_per_param),
                        num_events,
                        num_params,
                    ))
                })
            },
        );

        // WITHOUT cold hint
        group.bench_with_input(
            BenchmarkId::new(
                "variable_width_sequential_no_hints",
                format!("{}_events", num_events),
            ),
            &(data, bytes_per_param.clone()),
            |b, (data, bytes_per_param)| {
                b.iter(|| {
                    black_box(parse_variable_width_sequential_no_hints(
                        black_box(data),
                        black_box(bytes_per_param),
                        num_events,
                        num_params,
                    ))
                })
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(50)  // Reduced for faster runs with more test cases
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));  // Reduced for faster runs
    targets = bench_dataframe_parsing
}
criterion_main!(benches);
