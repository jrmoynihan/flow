//! Criterion: lazy column/events access vs. the existing eager `data_frame`
//! parse, on a real compliance-corpus file. Stage A must not regress the
//! already-eager path's performance while adding the lazy one.
//!
//! Also carries a generated `$DATATYPE F`, 1,000,000 x 20 fixture (Stage B's
//! target shape) — see `synthetic_fcs` below for why it has to be generated
//! rather than committed.

use criterion::{Criterion, criterion_group, criterion_main};
use flow_fcs::file::Fcs;
use flow_fcs::keyword::{IntegerKeyword, Keyword, MixedKeyword};
use flow_fcs::metadata::Metadata;
use flow_fcs::version::Version;
use flow_fcs::write::write_fcs_file;
use polars::prelude::*;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// `$CYT` value written into the synthetic fixture. Contains three literal
/// spaces, which collide with `Metadata::new()`'s default space delimiter —
/// that collision is what makes writing this fixture at V3_1 exercise the
/// doubled-delimiter escaping path rather than passing vacuously. Shared as a
/// `const` so the generator and `assert_fixture_shape`'s read-back check
/// cannot drift apart.
const SYNTHETIC_CYT: &str = "flow-crates synthetic bench fixture";

fn compliance_fcs() -> String {
    flow_fcs::corpus::path("int-10000_events_random.fcs")
        .to_str()
        .expect("utf-8 corpus path")
        .to_string()
}

/// Generate a `$DATATYPE F`, little-endian FCS file of `n_events` x `n_params`
/// into `dir` and return its path.
///
/// The corpus has nothing above 50,000 x 8 int16, and a committed
/// multi-million-event file would be hundreds of megabytes, so this generates
/// one through `write.rs` instead. The seed corpus file is FCS3.0, and its
/// `header.version` is overridden to `V3_1` below: `$CYT`'s value contains
/// spaces and `Metadata::new()`'s default delimiter is a space, and pre-3.1
/// FCS has no delimiter-escape mechanism at all (`Escaping::for_version`
/// maps V1_0/V2_0/V3_0 to `Escaping::None`) — writing this fixture at V3_0
/// would silently corrupt TEXT rather than exercise anything. Forcing V3_1
/// puts the write on the `Escaping::Doubled` path that flow-crates-1xb made
/// correct, so the escaping the fixture depends on is real, not incidental.
fn synthetic_fcs(dir: &Path, n_events: usize, n_params: usize) -> PathBuf {
    let path = dir.join(format!("synthetic_{n_events}x{n_params}.fcs"));

    // Deterministic, non-degenerate values: a per-parameter offset keeps the
    // columns distinguishable so a transposition bug cannot pass unnoticed.
    let columns: Vec<Column> = (0..n_params)
        .map(|p| {
            let values: Vec<f32> = (0..n_events)
                .map(|e| (e as f32).mul_add(0.001, p as f32 * 1000.0))
                .collect();
            Column::new(format!("P{}", p + 1).into(), values)
        })
        .collect();
    let df = DataFrame::new_infer_height(columns).expect("df");

    // Seed from a real file so `file_access` and `header` are valid; the
    // writer reads neither `parameters` nor the `columns` cache, so replacing
    // `header.version`, `metadata`, and `data_frame` is sufficient.
    let seed = flow_fcs::corpus::path("int-10000_events_random.fcs");
    let mut fcs = Fcs::open(seed.to_str().expect("utf-8 corpus path")).expect("seed corpus file");
    fcs.header.version = Version::V3_1;

    let mut metadata = Metadata::new();
    metadata.insert_string_keyword("$BYTEORD".into(), "1,2,3,4".into());
    metadata.insert_string_keyword("$DATATYPE".into(), "F".into());
    metadata.insert_string_keyword("$MODE".into(), "L".into());
    metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
    metadata.insert_string_keyword("$CYT".into(), SYNTHETIC_CYT.into());
    for p in 1..=n_params {
        metadata.insert_string_keyword(format!("$P{p}N"), format!("P{p}"));
        metadata.keywords.insert(format!("$P{p}B"), Keyword::Int(IntegerKeyword::PnB(32)));
        metadata.keywords.insert(format!("$P{p}R"), Keyword::Int(IntegerKeyword::PnR(262_144)));
        metadata.keywords.insert(format!("$P{p}E"), Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)));
    }

    fcs.metadata = metadata;
    fcs.data_frame = Arc::new(df);

    write_fcs_file(fcs, &path).expect("write synthetic fixture");
    path
}

/// A benchmark against a malformed fixture measures nothing. Check the file
/// reopens with the shape we asked for before timing anything.
///
/// `height()`/`width()` come from `$PAR`/`$TOT`/`$PnB`, which say nothing
/// about whether TEXT itself round-tripped correctly — a corrupted `$CYT`
/// would leave those checks green. `$CYT` is read back and compared against
/// `SYNTHETIC_CYT` (the same value the generator wrote) specifically because
/// it is the keyword whose embedded spaces depend on the V3_1
/// doubled-delimiter escaping this fixture exists to exercise.
fn assert_fixture_shape(path: &Path, n_events: usize, n_params: usize) {
    let fcs = Fcs::open(path.to_str().expect("utf-8")).expect("reopen synthetic fixture");
    assert_eq!(fcs.data_frame.height(), n_events, "synthetic fixture event count");
    assert_eq!(fcs.data_frame.width(), n_params, "synthetic fixture parameter count");
    assert_eq!(
        fcs.get_keyword_string_value("$CYT").expect("$CYT round-trips"),
        SYNTHETIC_CYT,
        "synthetic fixture $CYT survived the escaping round trip"
    );
}

fn bench_two_column_access(c: &mut Criterion) {
    let fcs = Fcs::open(&compliance_fcs()).expect("open compliance fixture");
    let names = fcs.get_parameter_names_from_dataframe();
    let (a, b) = (names[0].clone(), names[1].clone());

    let mut group = c.benchmark_group("two_column_access");
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("lazy_columns_uncached", |bencher| {
        let fixture = compliance_fcs();
        bencher.iter_batched(
            || Fcs::open(&fixture).expect("reopen for cold cache"),
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

/// Baseline recorded (Task 7, pre-rewrite) in `flow-crates-3si`'s notes and in
/// `target/criterion/` on disk. `iter_batched` excludes setup-closure time
/// from the measurement, so `events_uncached` running slower than
/// `open_eager_baseline` reflects `extract_columns`'s own decode cost, not
/// `open()`'s parse. See the Phase 4 spec for the rewrite this baseline is
/// meant to be compared against.
fn bench_full_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_materialization");
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(20);

    group.bench_function("events_uncached", |bencher| {
        let fixture = compliance_fcs();
        bencher.iter_batched(
            || Fcs::open(&fixture).expect("reopen for cold cache"),
            |fresh| black_box(fresh.events().expect("events")),
            criterion::BatchSize::LargeInput,
        );
    });

    group.bench_function("open_eager_baseline", |bencher| {
        let fixture = compliance_fcs();
        bencher.iter(|| black_box(Fcs::open(&fixture).expect("open")));
    });

    group.finish();
}

/// The corpus tops out at 50,000 x 8 (`fcs2_int16_50000ev_8par_random.fcs`,
/// used below), nothing like the multi-million-event, 20-parameter,
/// `$DATATYPE F` file Stage B targets. `fcs2_int16_50000ev_8par_random.fcs`
/// stays in the mix precisely because it is awkward: `$BYTEORD 4,3,2,1` with
/// `$P1B 16` / `$P1R 1024` forces both a byte swap and a range mask, so it
/// exercises the general decode path rather than any zero-copy shortcut, and
/// 50,000 x 8 = 400,000 values sits exactly on the current fast-path
/// threshold.
fn bench_synthetic_column_access(c: &mut Criterion) {
    let dir = tempfile::TempDir::new().expect("tempdir");
    const EVENTS: usize = 1_000_000;
    const PARAMS: usize = 20;
    let path = synthetic_fcs(dir.path(), EVENTS, PARAMS);
    assert_fixture_shape(&path, EVENTS, PARAMS);
    let path = path.to_str().expect("utf-8").to_string();

    let mut group = c.benchmark_group("synthetic_1Mx20");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10);

    // One column of twenty: the Stage B case. `Fcs::columns()` already limits
    // decoding to the requested indices, so this does not decode-then-discard
    // the other nineteen — `wanted.len() == 1` here, and the per-row
    // intermediate is already 1-wide, not 20-wide. What Phase 4 targets is
    // the per-event allocation this still pays: `extract_columns` builds one
    // heap-allocated `Vec<f32>` per row (1,000,000 short-lived allocations
    // for one column) and then transposes row-major into column-major
    // (`columns.rs:176-180`). Decoding straight into pre-allocated column
    // buffers removes both the per-row `Vec` and the transpose.
    group.bench_function("one_column_of_twenty", |bencher| {
        bencher.iter_batched(
            || Fcs::open(&path).expect("reopen for cold cache"),
            |fresh| {
                let column = fresh.column("P1").expect("column");
                black_box(column.len());
            },
            criterion::BatchSize::LargeInput,
        );
    });

    // The requested column set is invariant across iterations, so it is built
    // once here rather than inside the timed closure — building it per
    // iteration would charge every sample for 20 `format!` allocations plus a
    // `Vec<&str>` build that has nothing to do with the decode path under
    // test.
    let names: Vec<String> = (1..=PARAMS).map(|p| format!("P{p}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    group.bench_function("all_twenty_columns", |bencher| {
        bencher.iter_batched(
            || Fcs::open(&path).expect("reopen for cold cache"),
            |fresh| {
                let cols = fresh.columns(&refs).expect("columns");
                black_box(cols.len());
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_two_column_access,
    bench_full_materialization,
    bench_synthetic_column_access
);
criterion_main!(benches);
