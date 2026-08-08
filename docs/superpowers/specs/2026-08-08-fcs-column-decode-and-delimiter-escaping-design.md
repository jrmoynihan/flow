# FCS column decode rewrite and TEXT delimiter escaping

**Date:** 2026-08-08
**Beads:** `flow-crates-3si` (primary), `flow-crates-1xb`, `flow-crates-1o1`
**Status:** Design — approved in discussion, not yet planned

## Problem

`flow-crates-3si` is the headline defect: `extract_columns`
(`fcs/src/columns.rs:126`) — the primitive Stage A introduced for lazy
per-column access, and the primitive Stage B's entire memory argument rests on —
decodes badly.

Three costs compound in its inner loop:

1. **One allocation per event.** `decode_row` returns `Vec<f32>`, collected into
   a `Vec<Vec<f32>>` and only then transposed into columns
   (`fcs/src/columns.rs:163-181`). For a 3M-event, 40-parameter file that
   transient intermediate is *larger than the final column output* for small
   requests, which undercuts the memory win Stage B exists to deliver.
2. **A `Result` per value.** `decode_row` is fallible, so every single decoded
   value carries a `Result` through a `collect::<Result<Vec<_>>>()` — even
   though whether a column *can* decode is fixed for the whole file by its
   `(datatype, width, byteorder)` triple.
3. **A `#[cold]` call per value.** `Fcs::parse_parameter_value_to_f32`
   (`fcs/src/file.rs:1452`) is annotated `#[cold]` because it was previously
   only a rare fallback in `store_raw_data_as_dataframe`. Calling it once per
   value in a primary decode loop is a pessimization by construction, and it
   re-runs the `match (data_type, bytes_per_param)` dispatch per value.

This is what actually produced Task 8's benchmark result (`events_uncached` ~8x
slower than a bare `Fcs::open()`). The explanation recorded during
implementation — "two separate traversals" — was wrong: criterion's
`iter_batched` excludes setup-closure time, so `open()`'s parse was never in
that measurement at all. **That mis-attribution is the reason this spec treats
measurement as a first-class deliverable rather than a closing formality.**

Two prerequisites turned out to block a trustworthy fix.

## Prerequisite 1: the safety net only exists on one machine

`column_matches_data_frame_oracle` (`fcs/src/file.rs:2990`) compares `column()`
against the eager `data_frame` — exactly the equivalence net a decode rewrite
needs. But it hardcodes an absolute path, as does
`fcs/benches/lazy_column_access.rs:10`:

```
/Users/kfls271/Rust/flow-crates/gates/Gating-ML.../int-10000_events_random.fcs
```

On any other clone `Fcs::open` returns `Err` and the `.expect()` **fails** — the
tests do not skip, they break. The oracle also covers only parameter `[0]` of
one file. So `flow-crates-1o1`, filed as a P3 cleanup, is really a prerequisite:
without it the rewrite's safety net is machine-local and one column wide.

## Prerequisite 2: `write.rs` cannot yet be trusted to build a fixture

The corpus tops out at 50,000 events x 8 parameters of big-endian `int16`.
Nothing in it resembles the modern `$DATATYPE F`, 20-40 parameter, multi-million
event file Stage B targets, and committing one would be hundreds of megabytes.
The fixture must therefore be generated — and generating it through `write.rs`
means `write.rs` must be correct first.

It is not. `flow-crates-1xb`: `serialize_metadata` (`fcs/src/write.rs:621`)
joins keys and values with `metadata.delimiter` and never doubles an occurrence
of that byte inside a value, as FCS 3.1/3.2 require. Any value containing the
delimiter silently truncates and desynchronizes the remainder of TEXT on reopen.
The default delimiter is a space (`fcs/src/metadata.rs:41`), which is why this
bites free-text keywords immediately.

The workaround is widespread, and larger than `flow-crates-1xb` records. Ten
sites in `write.rs` force `delimiter = '\u{000c}'` before writing (`1036, 1166,
1265, 1355, 1451, 1583, 1683, 1816, 1982, 2113` — the bead's list of seven is
stale). More significantly, a downstream crate has shaped its **output format**
around the bug: `tru-ols/src/provenance.rs` defines `SAFE_TEXT_DELIMITER` and
`ensure_delimiter_survives_provenance(fcs)`, called from both
`Provenance::write_to` and `stamp_onto`, which rewrites a file's delimiter away
from space before provenance is written; and `default_software_tag()` is
documented as *"deliberately with no space in it"* for the same reason.

The workaround only relocates the problem: `$UNSTAINEDINFO`, `$PROJ` and `$COM`
can contain a form feed too, and `$TRUOLS_MIXMAT` cannot hold a comma-delimited
value if the delimiter is a comma.

## The finding that shapes the escaping fix

Un-doubling on read cannot be unconditional. Scanning all ten corpus files for
runs of consecutive delimiters inside TEXT finds five, across two files — and
**every one is an empty value, not an escape**:

```
fcs2_int16_13367ev_8par_GvHD.fcs   \&5Data File Prefix Part #1\\&6Data File Prefix Part #2\\
                                   \&13Analysis Doc.\\          <- also terminates TEXT
real-8-parameters.data.fcs         \Comments\\Row\2\
```

Both files are FCS2.0. Under run-length un-doubling, `Comments` would absorb
`Row` as its value and every subsequent field would shift by one — introducing
on *read* precisely the desynchronization this bug causes on *write*.

The reason doubling is decodable in FCS 3.1+ is that those versions **forbid
empty keyword values**. FCS2.0 files in the wild use them, so `\\` is genuinely
ambiguous there and no tokenizer resolves it. The escape must be version-gated.

## The other thing that must not be missed

There are **two** hand-rolled TEXT tokenizers, and they must agree:

- `Metadata::from_text_segment` (`fcs/src/metadata.rs:98`)
- `Fcs::find_begindata_offset` (`fcs/src/file.rs:895`), whose doc comment reads
  *"Mirrors `Metadata::from_mmap`'s delimiter-tokenization exactly"*

They agree today only by convention. Escaping is exactly the change that breaks
that convention silently — and it would break it on the `$NEXTDATA` chain scan,
i.e. only on multi-dataset files. That is the same blind spot that hid the
`absolutize()` bug (see bd memory `fcs-offsets-are-dataset-relative`): a
two-data-set fixture passes under both readings.

## Design

Three beads, in dependency order. `1o1` and `1xb` are prerequisites for `3si`
being measurable and for its fixture being generatable, respectively.

### Phase 1 — `flow-crates-1o1`: portable fixtures

Resolve the tracked corpus from `CARGO_MANIFEST_DIR` in both hardcoding sites:
`lazy_column_tests` (`fcs/src/file.rs:2990`) and
`fcs/benches/lazy_column_access.rs:10`. The corpus is git-tracked (529 files),
so relative resolution works on any clone.

### Phase 2 — `flow-crates-1xb`: TEXT delimiter escaping

**Extract one shared tokenizer.** A single keyword/value walk, parameterized by
escaping policy, consumed by both `Metadata::from_text_segment` and
`Fcs::find_begindata_offset`. This replaces the "mirrors exactly" convention
with a shared unit — targeted improvement of the code being changed, not
unrelated refactoring.

**Policy, gated at `Version::V3_1`:**

| Version | Read | Write |
|---|---|---|
| `V1_0` / `V2_0` / `V3_0` | split on every delimiter; empty values allowed (current behaviour) | emit as today |
| `V3_1` / `V3_2` / `V4_0` | run-length un-double: a run of *N* delimiters yields *N/2* literal delimiters, plus a field separator iff *N* is odd | double the delimiter in key and value; **error** on an empty value, naming the keyword |

The gate sits at V3_1 rather than V3_0 deliberately. V3_1 is where the spec
unambiguously forbids empty values, which is the precondition that makes
doubling decodable. The cost of being wrong at V3_1 is that a rare FCS3.0 file
with an escaped delimiter keeps mis-parsing — the status quo. The cost of being
wrong at V3_0 is that a common FCS3.0 file with an empty value *newly*
desynchronizes, and the corpus has no FCS3.0 fixture to check against.

**Empty values under an escaping version are a write error, not a warning.** An
empty value serializes to a doubled delimiter that reads back as a literal —
silently wrong. Erroring surfaces the caller's bug at write time, consistent
with the position `flow-crates-lfg` already argues for `$BYTEORD` (error rather
than guess).

**Delimiter validation.** Reject any delimiter outside ASCII 1-126 at write
time, which subsumes the bead's NUL-rejection requirement.

**Version threading.** `Version` passes into `resolve_layout` and
`serialize_metadata`, neither of which receives it today. Escaping changes TEXT's
length, so `$BEGINDATA` shifts — `resolve_layout`'s existing fixed-point loop
(`fcs/src/write.rs:575`) absorbs that with no additional work, since it
re-serializes until the offsets settle.

The gate is a plain `match` on `Version`, sited so `flow-crates-zmx`'s
`VersionSpec` trait can absorb it later. It is deliberately not pre-abstracted.

**Remove the workaround, within flow-fcs only.** Drop the forced
`delimiter = '\u{000c}'` from the ten `write.rs` sites, so the default space
delimiter is actually exercised. tru-ols's `ensure_delimiter_survives_provenance`
machinery stays for now — see out of scope.

**Tests.**
- Round-trip a value containing the active delimiter through write -> `Fcs::open`
  unchanged, for space, comma and form-feed delimiters.
- Writer errors on an empty value under V3_1+, naming the keyword.
- Writer rejects a delimiter outside ASCII 1-126.
- Regression: all ten corpus files parse to identical keyword maps before and
  after. `fcs2_int16_13367ev_8par_GvHD.fcs` and `real-8-parameters.data.fcs` are
  the load-bearing cases — they carry the FCS2.0 empty values.
- Multi-dataset: a `$NEXTDATA` chain whose first dataset contains an escaped
  delimiter, so `find_begindata_offset` and `from_text_segment` are proven to
  agree rather than assumed to.

### Phase 3 — harness and baseline

Rework `fcs/benches/lazy_column_access.rs`:

- Corpus cases resolved portably (Phase 1). The 50,000 x 8 file is worth keeping:
  `$BYTEORD 4,3,2,1` with `$P1B 16 / $P1R 1024` forces both a byte swap and a
  range mask, so it exercises the general path rather than any zero-copy
  shortcut, and 50,000 x 8 = 400,000 sits exactly on the current threshold.
- One synthetic ~1M event x 20 parameter `$DATATYPE F` little-endian file
  (~80 MB DATA), generated through `write.rs` into a temp dir at bench setup.
  Past the parallel threshold, large enough that the `Vec<Vec>` intermediate is
  visible against the column output, and still a few seconds per sample.

Record the baseline **before** Phase 4 and attach it to `flow-crates-3si`.

### Phase 4 — `flow-crates-3si`: the decode rewrite

Three units in `fcs/src/columns.rs`, each testable independently:

| Unit | Purpose | Depends on |
|---|---|---|
| `Decoder` | `Copy` enum over the 8 legal `(datatype, width, byteorder)` combinations. `resolve()` is fallible and runs once per column; `read(&[u8]) -> f32` is `#[inline(always)]` and infallible | `FcsDataType`, `ByteOrder` |
| `ColumnPlan` | `{ offset, decoder, mask }` — everything the inner loop needs, precomputed | `Decoder`, `ColumnLayout` |
| `fill_events` | Infallible: walk an event-byte chunk, write `plans[c]`'s value into `outs[c][e]` | `ColumnPlan` |

`extract_columns` becomes orchestration only: bit-packed guard, length guard,
build plans, allocate, choose branch, hand slices to `fill_events`.

**All fallibility moves into `Decoder::resolve`**, which runs `wanted.len()`
times instead of `num_events * wanted.len()` times. `resolve` reproduces
`parse_parameter_value_to_f32`'s error messages verbatim so no error text
regresses.

**`parse_parameter_value_to_f32` needs no edit.** Once `extract_columns` stops
calling it, its only caller is `parse_variable_width_data`
(`fcs/src/file.rs:1547`), which is itself `#[cold]`. The annotation becomes
accurate again by subtraction — strictly better than either option the bead
offered (drop it, or add a non-cold variant).

**Allocation.** Output columns are pre-sized with `vec![0.0f32; n]`, which lowers
to `alloc_zeroed` (f32's all-zero bit pattern qualifies for the `IsZero`
specialization). That is untouched zero pages, not a memset — so no
`MaybeUninit` and no unsafe.

**Parallel branch.** Peel chunk-sized `&mut` sub-slices off each output column
in lockstep with `split_at_mut`, pair each set with its matching `&[u8]` event
chunk, and `into_par_iter().for_each(fill_events)`. Safe Rust throughout.
Allocation is `O(chunks * k)` slices-of-references — nothing proportional to
events, which is the `Vec<Vec>` cost being removed.

**Threshold.** The predicate changes from `num_events * wanted.len()` to the
work actually done: `num_events * bytes_per_event`. The traversal walks the
whole DATA segment regardless of how many columns are kept; `wanted.len()` only
affects how many values are stored per event. Requesting one column from a
300,000-event x 40-parameter file currently scores 300,000, stays sequential,
and still walks 48 MB. This gets its own named constant,
`PARALLEL_BYTE_THRESHOLD`, local to `columns.rs` and documented as deliberately
distinct from `file.rs`'s `PARALLEL_THRESHOLD`, which was tuned for a
value-count loop — a different quantity. The crossover is chosen from the Phase
3 benchmark, not asserted.

**Uniform-decoder monomorphization is deferred, not dropped.** Once every column
resolves to the same `Decoder`, the enum dispatch is a perfectly-predicted
branch, which should recover most of what full monomorphization would buy.
Layering a `ValueDecoder`-generic specialization on top is additive if the
numbers demand it.

**Tests.**
- A private `extract_columns_inner(.., parallel: bool)` so both branches are
  pinned deterministically, *plus* a size-driven test that genuinely crosses
  `PARALLEL_BYTE_THRESHOLD`, so the predicate itself is covered. The parallel
  branch has zero coverage today.
- Big-endian decode. `columns.rs` has none, and the corpus is `$BYTEORD 4,3,2,1`.
- Mixed-width `[8, 2, 4]` decode. That layout has *layout* coverage
  (`layout_computes_running_sum_offsets_for_varying_widths`) but no decode
  coverage.
- `Decoder::resolve` rejecting an unsupported `(datatype, width)` before any
  bytes are touched.
- `column_matches_data_frame_oracle` widened from parameter `[0]` of one file to
  every parameter of every corpus file.

### Phase 5 — re-measure and close

Re-run the Phase 3 harness, record before/after on `flow-crates-3si`, close
`1o1`, `1xb`, `3si`.

## Out of scope

- **`flow-crates-rkq`** — `extract_columns` errors only when `data_bytes` is too
  *short*, never when it is merely a superset, which is how a filtered clone
  silently decodes a truncated prefix. Adjacent to this work and tempting to fix
  in passing; it belongs to `rkq`, whose resolution may dissolve under Stage B
  anyway.
- **`flow-crates-lfg`** — the `$BYTEORD` silent downgrade lives in
  `serialize_metadata`, the same function Phase 2 edits. It is an independent
  defect and stays in its own bead.
- **`flow-crates-8px`** (filed by this design) — retiring tru-ols's
  `ensure_delimiter_survives_provenance`, `SAFE_TEXT_DELIMITER` and the
  no-space constraint on `default_software_tag()`, all of which exist solely to
  work around `1xb`. Deliberately separate: removing them changes the delimiter
  of every file tru-ols emits, which is user-visible and deserves its own
  review. It must not be attempted before `1xb` is closed and verified, because
  the mechanism is load-bearing until then.
- **Uniform-decoder monomorphization** — deferred pending Phase 5 numbers, as
  above.

## Risks

- **The V3_1 gate could be wrong for FCS3.0.** Accepted deliberately, with the
  failure mode chosen to be "status quo persists" rather than "working files
  break". Revisit if an FCS3.0 fixture with escaped delimiters appears.
- **Removing the form-feed workaround may surface further writer defects.** That
  is the point — those tests have never exercised the default delimiter. Any new
  failure is a pre-existing bug becoming visible, and gets its own bead rather
  than being absorbed here.
- **Scope.** This began as a one-bead perf fix and became three beads including a
  P1 data-corruption fix with a spec-interpretation gate. Phases 1-2 and Phases
  3-5 are separable into two reviewable changes if that is preferred.
