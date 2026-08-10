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

/// Measured for `flow-crates-3si`. The pre-rewrite `events_uncached` ran ~8x
/// slower than `open_eager_baseline`, and the explanation recorded at the time
/// — "it pays for open()'s parse too" — was wrong: criterion's `iter_batched`
/// excludes setup-closure time. The real causes were one `Vec` allocation per
/// event, a `Result` per value, and a `#[cold]` call per value. See
/// `docs/superpowers/specs/2026-08-08-fcs-column-decode-and-delimiter-escaping-design.md`.
///
/// After the rewrite, measured as a criterion paired A/B (`2c8f09f`
/// `--save-baseline pre` vs. HEAD `--baseline pre`; this file is byte-identical
/// at both commits, so only `extract_columns` differs). Apple M5 Max, 18 cores:
///
/// | benchmark                                | before    | after     | criterion change   |
/// |------------------------------------------|-----------|-----------|--------------------|
/// | `two_column_access/lazy_columns_uncached`| 251.9 µs  | 28.67 µs  | −88.24% (p = 0.00) |
/// | `full_materialization/events_uncached`   | 640.5 µs  | 77.64 µs  | −87.87% (p = 0.00) |
/// | `synthetic_1Mx20/one_column_of_twenty`   | 11.06 ms  | 1.121 ms  | −89.81% (p = 0.00) |
/// | `synthetic_1Mx20/all_twenty_columns`     | 110.7 ms  | 5.449 ms  | −95.17% (p = 0.00) |
///
/// `events_uncached` no longer costs 8x `open_eager_baseline`; the two are now
/// within noise of each other (77.6 µs vs. 83.8 µs). `open_eager_baseline` and
/// `eager_data_frame_two_columns` are untouched by the rewrite and act as
/// controls: both stayed within ±2.5% across the paired runs — **but only once
/// the two binaries were interleaved** (before, HEAD, before, HEAD, back to
/// back). Collecting all the "before" samples first and all the "after"
/// samples afterwards made `open_eager_baseline` — code this rewrite does not
/// touch — report "Performance has regressed" at +8.8% and then +21.7%, both
/// at p = 0.00. That is machine drift over a long session, and criterion
/// cannot detect it, because each side is internally consistent. Interleave.
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

/// The corpus tops out at 50,000 x 8, nothing like the multi-million-event,
/// 20-parameter, `$DATATYPE F` file Stage B targets, hence this generated
/// fixture.
///
/// Note for anyone re-tuning `columns::PARALLEL_BYTE_THRESHOLD`: this
/// fixture's DATA segment is 1,000,000 x 20 x 4 = 76.3 MiB and the corpus
/// fixture used by the other two groups is 234 KiB, so every candidate
/// threshold between those two sizes produces identical behaviour on the
/// benchmarks as committed. To exercise the crossover, temporarily point
/// `compliance_fcs()` at `fcs2_int16_50000ev_8par_random.fcs` (50,000 x 8 x 2
/// = 781.25 KiB of DATA, byte-aligned, already covered by the lazy/eager
/// oracle) and run the `two_column_access` group. That probe is what ruled out
/// a 256 KiB threshold — see `columns::PARALLEL_BYTE_THRESHOLD`. Keep the
/// change as scaffolding; the committed bench must open
/// `int-10000_events_random.fcs`.
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
    // Ten samples, each opening and decoding a 76.3 MiB fixture, resolves
    // effects of roughly 15% at best. That is entirely adequate for what this
    // group was built to measure (-89.8% and -95.2%), and a larger `n` costs
    // real wall-clock on every bench run — but it means this group must not be
    // used to chase small changes. Anything under ~15% measured here is noise;
    // raise `sample_size` (and expect the run to get much slower) or build a
    // smaller fixture before trusting such a number.
    group.sample_size(10);

    // One column of twenty: the Stage B case. `Fcs::columns()` limits
    // decoding to the requested indices, so this does not decode-then-discard
    // the other nineteen. Post-rewrite there is no per-row `Vec` and no
    // row-major-to-column-major transpose either: `extract_columns` decodes
    // straight into pre-allocated column buffers. This still walks all
    // 76.3 MiB of DATA — `wanted.len()` changes how many values are stored
    // per event, not how many bytes are stepped over — which is why it lands
    // at ~1.1 ms against ~5.4 ms for all twenty rather than one twentieth.
    //
    // `black_box(column.len())` looks like it might not force the decode. It
    // does: the decode's result is installed in the `OnceCell` cache inside
    // `Fcs::columns` (`fcs/src/file.rs`), a side effect the optimizer cannot
    // drop. The pre-rewrite binary running the identical line at 11.06 ms
    // confirms it empirically.
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
