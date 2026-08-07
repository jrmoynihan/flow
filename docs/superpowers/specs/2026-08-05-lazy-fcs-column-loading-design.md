# Lazy per-column loading for `Fcs`

**Date:** 2026-08-05
**Bead:** `flow-crates-dzw` (external: `gh-jrmoynihan/flow#21`)
**Status:** Design — approved in discussion, not yet planned

## Problem

`Fcs::open` eagerly de-interleaves the entire DATA segment into a fully
materialized `DataFrame`. For a 3M-event, 40-parameter file that is ~480 MB of
anonymous heap per file, with a ~960 MB transient peak during the build
(`fcs/src/file.rs:530-559`). Holding many large files open — increasingly normal
in flow cytometry analysis — scales that linearly.

The originating suggestion was to solve this with Polars `LazyFrame` +
`Engine::Streaming`. **That does not apply here.** Polars streaming pushes
projection and predicates into Polars' *own* scanners (Parquet, CSV, IPC). FCS
data is hand-parsed from an mmap; there is no `LazyFrame` source to push into.
`get_parameter_statistics` (`fcs/src/file.rs:1271`) already uses the streaming
engine, and it operates on an *already materialized* frame — it saves nothing at
load time. The laziness has to be built at the parse layer, not borrowed from
Polars.

## What this saves, and what it does not

Stated plainly so the spec does not over-claim:

**Does not reduce peak memory during QC.** `preprocess_fcs`
(`peacoqc-rs/src/lib.rs:366`) compensates and transforms the whole frame, and
`PeacoQCData::get_channel_f64` reads every fluorescence channel. In fast-flow,
QC runs almost immediately after open, so such a file materializes everything
within milliseconds. The transient peak is what it is today; this design moves
*when* the de-interleave happens, not *whether* it happens.

**Does save, in order of impact:**

1. **Steady state after QC — the largest win, and it applies to every file.**
   Today `fcs.data_frame` is overwritten with the transformed frame, so an
   `Arc<Fcs>` sitting in `compute-engine`'s `fcs_cache` retains ~480 MB
   indefinitely. Under this design the cached `Fcs` holds metadata only and the
   derived `EventDataFrame` is dropped when its consumer is done — and QC's
   retained outputs (the `good_cells` mask, summary statistics) are small. Peak
   is unchanged; the resident floor collapses. Short-lived peaks during an
   intensive algorithm are normal and acceptable; permanently resident copies of
   every file are not.
2. **Files opened but never analyzed.** Of 114 `.data_frame` occurrences across
   the workspace, ~35 are shape queries (27 `.height()`, 8 `.width()`, 1
   `get_column_names()`) that need no event data at all. A workspace with 100
   files where the user QCs five drops from ~48 GB to ~2.4 GB.
3. **The raw plotting path.** A two-channel scatter needs 24 MB, not 480 MB.
4. **Correctness.** Raw and derived data stop sharing a field. This is not a
   memory win but it is the defect that made the original cache proposal
   unworkable (see *Rejected alternatives*).

## Architecture

Split the one mutable `data_frame` field into two types with different
lifetimes and different guarantees.

```
Fcs                                        cheap handle, Arc-shareable
├─ header, metadata, parameters            parsed at open
├─ file_access: AccessWrapper              mmap, page-cache backed
└─ columns: Arc<[OnceLock<Box<[f32]>>]>    $PAR slots, empty at open

    .column(&self, name)      -> Result<&[f32]>          lazy, cached, RAW
    .columns(&self, names)    -> Result<Vec<&[f32]>>     one pass, cached, RAW
    .events(&self)            -> Result<EventDataFrame>  one pass, owned, uncached

EventDataFrame                             owned, droppable, possibly derived
└─ (private) Arc<DataFrame>
    .compensate() / .transform() / .filter() -> EventDataFrame
```

`Fcs` is the file. It is immutable, always raw, and safe to keep in the
`Arc<Fcs>` cache that `compute-engine/src/local_context.rs:14` already uses.

`EventDataFrame` is a value. It is what you get back from a transform, and it
goes away when you drop it. **Derived results are returned, never written back
into `Fcs`.** Eviction is `drop`, not an API.

The name `EventDataFrame` is reused: it is currently
`pub type EventDataFrame = Arc<DataFrame>` (`fcs/src/parameter.rs:17`) and
becomes a struct. Existing `Result<EventDataFrame>` signatures on
`apply_arcsinh_transform`, `apply_compensation`, etc. keep their spelling; the
eight field-assignment sites break loudly, which is what we want.

### Why `OnceLock`, and why address stability matters

Population must work through `&self`, because `Arc<Fcs>` is used pervasively and
there is no `&mut` to be had. That rules out plain `Vec`.

`Mutex<HashMap<..>>` also fails, for a reason that is easy to miss: you cannot
return a `&[f32]` that outlives the guard. `get_raw_channel_data`
(`fast-flow/src-tauri/src/commands/legacy/mod.rs:3250`) builds a
`Vec<&[f32]>` holding several simultaneous live borrows and then does
`from_raw_parts` over each. Under a lock you would have to `clone()` every
column on every access — a full copy per request, on the hottest path in the
app.

`OnceLock` gives interior mutability with no lock on the read path, and the
`Arc<[OnceLock<..>]>` is allocated once at `$PAR` length and never grows, so
element addresses are permanently stable. That is what makes it sound to hand
out `&[f32]` tied to `&self`.

**Footgun:** do not `#[derive(Clone)]`. `OnceLock<T: Clone>` is itself `Clone`
and deep-copies its contents, so a derived clone would duplicate every warmed
column. Using `Arc<[..]>` makes clones *share* the warmed cache, which is both
cheaper and consistent with how `Fcs` is actually held. Note this in the type's
doc comment.

## The traversal primitive

There is only one, because the DATA segment is row-major interleaved: every read
walks it identically with `chunks_exact(bytes_per_event)`. Extracting 2 columns
and extracting all 40 are the same loop over the same cache lines, differing
only in how many values are stored per event. There is no strided-vs-sequential
choice, and therefore no crossover threshold to benchmark and no adaptive
promotion heuristic.

```rust
let param_offset: usize = bytes_per_parameter[..param_idx].iter().sum();
let width = bytes_per_parameter[param_idx];

let column: Vec<f32> = data_bytes
    .chunks_exact(bytes_per_event)
    .take(num_events)
    .map(|event| decode(&event[param_offset..param_offset + width], dtype, byte_order))
    .collect();
```

Use `par_chunks_exact` above the existing `PARALLEL_THRESHOLD` (400 000).

Two correctness notes carried over from the review of the original proposal:

- The offset must be a **running sum of `bytes_per_parameter`**, not
  `param_idx * 4`. Widths vary; `fcs/src/file.rs:418-429` collects them
  per-parameter precisely because they do.
- Decoding is dtype-dependent. `f32::from_bits` is correct for
  `FcsDataType::F` but wrong for `FcsDataType::I`, which needs `value as f32`.
  Reuse the existing `parse_parameter_value_to_f32` logic rather than
  reimplementing it.

Keep the `collect` form. A `get_unchecked` + `set_len` variant was tried and
regressed (`fcs/docs/PERF_AB.md`).

### The one access rule

**Batch what you need into a single call.** One traversal emitting N columns is
fine; N traversals emitting one column each is N× the work. `columns(&[..])` for
a known set, `events()` for whole-frame work. This is a code-review rule enforced
at the call site, not runtime machinery.

`events()` must **not** populate the `OnceLock` cache. If it did, a QC'd file
would hold 480 MB of raw columns *plus* its derived frame — strictly worse than
today. `events()` is a pure single-pass materialization that returns owned data
and leaves `Fcs` empty.

### Warm-on-activate

When a file becomes the active one in the UI, fast-flow may kick off a
background pass to warm the columns it expects to need, so the first plot does
not pay first-touch latency. This is a fast-flow policy decision, not part of the
`fcs` API, and is optional. IPC round-trip count and payload size are unchanged
either way — the only new cost is one traversal on first touch.

## Error timing

`Fcs::open` keeps its eager DATA-offset validation against the mmap length
(`fcs/src/file.rs:361-383`, including the `$BEGINDATA`/`$ENDDATA` fallback).
That is pure arithmetic over already-parsed metadata and costs nothing, so a
malformed file still fails at `open()` rather than surfacing an error later from
a plotting call.

What defers is only byte→`f32` conversion. Errors that can *only* arise during
conversion (an unsupported `$DATATYPE`, a bad `$PnB`) surface from `column()` /
`columns()` / `events()`. Callers already handle `Result` at these points.

## Staged migration

Each stage compiles and passes tests on its own. There are two stages, not
three — an earlier draft of this design split "reroute reads" from "split the
types" as independent stages, but they are not mechanically separable.
`data_frame` is a `pub` **field** today. Eight sites assign to it directly
(`fcs.data_frame = compensated_df`), several from other crates, which is only
possible because it's a field. Making it lazily computed requires making it a
**method**, and a method cannot be assigned to — so removing write access and
introducing laziness are the same commit, not two.

**Stage A — add, break nothing.** Introduce `Arc<[OnceLock<Box<[f32]>>]>`, the
extraction primitive, and `column()` / `columns()` / `events()`. `open()` still
eagerly builds `data_frame` as today, unchanged, so every existing `Fcs` also
gets a (redundant, for now) populated column cache. Existing API untouched; new
paths testable in isolation against the eager `data_frame` as an oracle —
`column("FSC-A")` must equal `data_frame.column("FSC-A")` on the same file.

`open_metadata_only()` is deliberately **not** part of Stage A. `Fcs` still
carries the `data_frame: EventDataFrame` field unchanged in this stage, and
there is no honest value to put there before data is read: an empty frame
would make `.height()` silently wrong, and changing the field to `Option`
would break exactly the call sites this stage promises not to touch.
`open_metadata_only()` moves to Stage B, where the field no longer exists and
"no data read yet" is simply the column cache's natural empty state.

**Stage B — lazy open and the type split, together.** `open()` stops eagerly
materializing, and `open_metadata_only()` is added alongside it — now trivial,
since with the `data_frame` field gone there is nothing to eagerly populate;
"no data read yet" is just the column cache's empty starting state.
`EventDataFrame` becomes the owned struct
(`fcs/src/parameter.rs:17`). The `data_frame` field is removed from `Fcs` in the
same change that adds `.column()` / `.events()` as its replacements, because the
compiler must flag every one of the 17 files touching `.data_frame` at once —
there is no intermediate state where it is both assignable and lazy. Migrate
the eight assignment sites so transforms return `EventDataFrame` rather than
mutate `Fcs`:

```
peacoqc-rs/src/lib.rs:333, 382, 409
fcs/src/write.rs:233, 304, 381
tru-ols/src/fcs_integration.rs:445
tru-ols-cli/src/qc_pipeline.rs:250
```

Only *after* those eight sites no longer exist — meaning nothing can hand back
an `Fcs` whose row count reflects a filter — reroute the ~35 shape-query sites
(27 `.height()`, 8 `.width()`, 1 `get_column_names()`) from
`fcs.data_frame.height()` to metadata (`$TOT`, `$PAR`). Doing this before the
assignment sites are gone would risk reading a stale in-progress mutation;
doing it last is what actually delivers the steady-state memory win, since it's
what stops `.height()` call sites from forcing materialization at all.

Downstream crates (`tru-ols`, `peacoqc-py`, fast-flow) update in this stage.
`peacoqc-py`'s Python surface is structurally unaffected — `inner:
flow_fcs::file::Fcs` is opaque (`peacoqc-py/src/lib.rs:237`).

## Rejected alternatives

**`Arc<Mutex<HashMap<String, Vec<f32>>>>` column cache with a miss path that
re-parses the mmap.** Three blocking defects:

1. `data_frame` is *mutable working state*, not a cache of the file. Eight sites
   overwrite it with compensated, transformed, filtered, or unmixed results. A
   cache whose miss path re-reads the mmap would silently return RAW values
   where the caller expects derived — at the wrong height for filtered frames
   and the wrong schema for unmixed ones
   (`tru-ols/src/fcs_integration.rs:1532` asserts the schema divergence). Not
   fixable by cache tuning; it is a type error wearing a cache costume.
2. Cannot return `&[f32]` (see *Why `OnceLock`* above).
3. Stride arithmetic and dtype handling were both wrong (see *The traversal
   primitive*).

**Polars `LazyFrame` + streaming engine.** No scanner to push into; see
*Problem*.

## Follow-ups (filed)

- `flow-crates-xii` — write channel data straight from mmap into the Tauri IPC
  buffer, skipping the intermediate column allocation. Must not populate the
  `OnceLock` cache.
- `flow-crates-bif` — stream GPU upload column-by-column across files without
  materializing the joined frame. Targets `fcs/src/write.rs:296`, which
  currently clones every frame. Scoped to the upload path only; on-disk
  `concat_fcs_files` is out of scope.

Not yet filed, worth considering after Stage C: `preprocess_fcs` could
materialize only the fluorescence set instead of the whole frame, skipping
scatter and Time channels. Modest (roughly 40 → 32 channels) but it is the
dominant fast-flow path. Note it must use an *uncached* subset extraction, not
`columns()` — caching 32 raw columns while also holding the derived frame would
cost more than it saves. That means either an `events_subset(&[names])` entry
point or a caching flag; decide when the follow-up is scoped, not here.

## Risks

- **Stage B is one large mechanical change, not a small one.** The compiler
  *will* flag all 17 files touching `.data_frame`, so no site is missed
  entirely. The risk is subtler: the path-of-least-resistance fix at most sites
  is to call `.events()` immediately, which compiles, is correct, and
  materializes the whole frame anyway — capturing none of the benefit. Each
  site needs a judgment call (does it need one column, a few, or the whole
  frame?), not a mechanical find-and-replace.
- **Shape-from-metadata must be the last change within Stage B, not the
  first.** Rerouting `.height()` to `$TOT` is only sound once nothing can hand
  back an `Fcs` whose row count reflects a filter — i.e. after the eight
  assignment sites are gone. Doing it earlier risks reading a stale in-progress
  mutation.
- **`events()` cache-population regression.** If someone later "optimizes"
  `events()` to fill the `OnceLock` slots, every QC'd file doubles its memory.
  This needs a test asserting the columns remain unpopulated after `events()`.
- **First-touch latency on cold files.** One traversal, unchanged bytes over
  IPC. Mitigable by warm-on-activate; not a correctness concern.
