//! Criterion: lazy column/events access vs. the existing eager `data_frame`
//! parse, on a real compliance-corpus file. Stage A must not regress the
//! already-eager path's performance while adding the lazy one.

use criterion::{Criterion, criterion_group, criterion_main};
use flow_fcs::file::Fcs;
use std::hint::black_box;
use std::time::Duration;

const COMPLIANCE_FCS: &str = "/Users/kfls271/Rust/flow-crates/gates/Gating-ML.v1.5.081030.Compliance-tests.081030/List-mode Data Files/int-10000_events_random.fcs";

fn bench_two_column_access(c: &mut Criterion) {
    let fcs = Fcs::open(COMPLIANCE_FCS).expect("open compliance fixture");
    let names = fcs.get_parameter_names_from_dataframe();
    let (a, b) = (names[0].clone(), names[1].clone());

    let mut group = c.benchmark_group("two_column_access");
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("lazy_columns_uncached", |bencher| {
        bencher.iter_batched(
            || Fcs::open(COMPLIANCE_FCS).expect("reopen for cold cache"),
            |fresh| {
                // `columns()` returns `Vec<&[f32]>` borrowing from `fresh`, so
                // the result can't be returned from this closure (it would
                // outlive the moved-in `fresh`). Force the same decode work
                // via `black_box` and let both drop together instead.
                let cols = fresh.columns(&[&a, &b]).expect("columns");
                black_box(&cols);
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.bench_function("eager_data_frame_two_columns", |bencher| {
        bencher.iter(|| {
            let x = fcs.get_parameter_events_slice(&a).expect("a");
            let y = fcs.get_parameter_events_slice(&b).expect("b");
            black_box((x, y))
        });
    });

    group.finish();
}

fn bench_full_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_materialization");
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(20);

    group.bench_function("events_uncached", |bencher| {
        bencher.iter_batched(
            || Fcs::open(COMPLIANCE_FCS).expect("reopen for cold cache"),
            |fresh| black_box(fresh.events().expect("events")),
            criterion::BatchSize::LargeInput,
        );
    });

    group.bench_function("open_eager_baseline", |bencher| {
        bencher.iter(|| black_box(Fcs::open(COMPLIANCE_FCS).expect("open")));
    });

    group.finish();
}

criterion_group!(benches, bench_two_column_access, bench_full_materialization);
criterion_main!(benches);
