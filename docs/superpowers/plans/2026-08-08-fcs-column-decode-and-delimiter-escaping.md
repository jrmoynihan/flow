# FCS Column Decode and TEXT Delimiter Escaping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `extract_columns` decode FCS event data without per-event allocation or per-value type dispatch, and make the FCS writer escape the TEXT delimiter so a trustworthy benchmark fixture can be generated in the first place.

**Architecture:** Five sequenced phases. Two prerequisites first — portable test fixtures (so the equivalence safety net runs on any clone) and a correct TEXT writer (so the synthetic benchmark fixture is generatable). Then a benchmark harness and recorded baseline, then the decode rewrite, then re-measurement. The decode rewrite resolves a `Decoder` once per column instead of once per value, moving all fallibility out of the inner loop, and writes straight into pre-sized output buffers instead of building a `Vec<Vec<f32>>` intermediate.

**Tech Stack:** Rust 2024 edition, `anyhow`, `rayon`, `memchr`, `byteorder`, `polars`, `criterion`, `tempfile`. Crate under test is `flow-fcs` (directory `fcs/`).

**Source spec:** [`docs/superpowers/specs/2026-08-08-fcs-column-decode-and-delimiter-escaping-design.md`](../specs/2026-08-08-fcs-column-decode-and-delimiter-escaping-design.md)

**Beads:** `flow-crates-1o1` (Task 1), `flow-crates-1xb` (Tasks 2–6), `flow-crates-3si` (Tasks 7–11). `flow-crates-8px` is explicitly out of scope.

## Global Constraints

- **Task tracking uses `bd` only.** This repository's `CLAUDE.md` states: "Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists." Do not create tasks with any other tool. Do not use `bd edit` — it opens `$EDITOR` and blocks.
- **Git profile is conservative.** `CLAUDE.md`: "Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked." Every "Commit" step below is a **proposed** command: print it and wait for the user's go-ahead rather than running it, unless the user has granted commit authority for this session.
- **Escape version gate is `Version::V3_1` and later**, exactly: `V3_1 | V3_2 | V4_0` escape; `V1_0 | V2_0 | V3_0` do not. This value is not negotiable inside a task — it was chosen because FCS 3.1 is the first version that forbids empty keyword values, which is the precondition that makes a doubled delimiter decodable.
- **Delimiter validity range is ASCII `1..=126` inclusive**, checked at write time.
- **Empty keyword values are a hard write error under an escaping version**, naming the offending keyword. Not a warning, not a silent skip.
- **No new `unsafe`.** The parallel branch uses `split_at_mut`; the output buffers use `vec![0.0f32; n]`.
- **Bit-exact decode parity is mandatory.** Every value the rewritten `extract_columns` produces must equal what `Fcs::parse_parameter_value_to_f32` produces today, bit for bit. Reuse the `byteorder` crate readers (`LE::read_u16` etc.) rather than reimplementing, so parity holds by construction.
- **Corpus location:** ten git-tracked `.fcs` files in `gates/Gating-ML.v1.5.081030.Compliance-tests.081030/List-mode Data Files/`, which is one directory above the `fcs` crate manifest.
- Run the crate's tests with `cargo test -p flow-fcs`. Run one bench with `cargo bench -p flow-fcs --bench lazy_column_access`.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `fcs/src/corpus.rs` | **create** | Resolve the git-tracked compliance corpus relative to `CARGO_MANIFEST_DIR`. Used by unit tests, integration-style tests, and benches. |
| `fcs/src/text.rs` | **create** | One shared TEXT tokenizer (`Escaping`, `TextFields`) plus the write-side escape helper. The single unit both readers and the writer agree through. |
| `fcs/src/decode.rs` | **create** | `Decoder` — a `Copy` enum over the eight legal `(datatype, width, byteorder)` combinations, with a fallible `resolve` and an infallible `#[inline(always)] read`. |
| `fcs/src/lib.rs` | modify | Register the three new modules. |
| `fcs/src/metadata.rs` | modify | `from_text_segment` consumes `TextFields` instead of hand-rolling memchr alternation; gains a `Version` parameter. |
| `fcs/src/file.rs` | modify | `find_begindata_offset` consumes `TextFields`; call sites thread `Version`; `lazy_column_tests` uses `corpus`; oracle widened. |
| `fcs/src/write.rs` | modify | `serialize_metadata` and `resolve_layout` take `Version`; escaping, empty-value error, delimiter validation; ten form-feed workarounds and eight empty-`$PnS` sites removed. |
| `fcs/src/compress.rs` | modify | `resolve_layout` call site threads `Version`; hardcoded corpus path replaced. |
| `fcs/src/columns.rs` | modify | `ColumnPlan`, `fill_events`, rewritten `extract_columns`, `PARALLEL_BYTE_THRESHOLD`, decode tests. |
| `fcs/benches/lazy_column_access.rs` | modify | Portable corpus case plus a generated ~1M×20 `$DATATYPE F` synthetic case. |

---

# Phase 1 — Portable fixtures (`flow-crates-1o1`)

## Task 1: Resolve the compliance corpus from `CARGO_MANIFEST_DIR`

Today three sites hardcode `/Users/kfls271/Rust/flow-crates/...`. On any other clone `Fcs::open` returns `Err` and the `.expect()` **fails** — the tests break rather than skip. Phase 4 rewrites the decoder, and this is the equivalence net that has to catch a mistake, so it must run everywhere before anything else happens.

**Files:**
- Create: `fcs/src/corpus.rs`
- Modify: `fcs/src/lib.rs` (module list, after line 26 `pub mod crc;`)
- Modify: `fcs/src/file.rs:2989-2990` (the `COMPLIANCE_FCS` const in `mod lazy_column_tests`)
- Modify: `fcs/src/compress.rs:444` (the `path` local in `real_fcs_round_trip_int10000`)
- Modify: `fcs/benches/lazy_column_access.rs:10` (the `COMPLIANCE_FCS` const)
- Test: `fcs/src/corpus.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `flow_fcs::corpus::dir() -> std::path::PathBuf`
  - `flow_fcs::corpus::path(file_name: &str) -> std::path::PathBuf`
  - `flow_fcs::corpus::files() -> Vec<std::path::PathBuf>` — every `.fcs` in the corpus, sorted by file name for determinism
  - `flow_fcs::corpus::is_available() -> bool`

  The module is `#[doc(hidden)] pub` — unconditionally compiled (benches are separate crates and cannot see `#[cfg(test)]` items), but hidden from rustdoc because it is test/bench support, not API.

- [ ] **Step 1: Write the failing test**

Create `fcs/src/corpus.rs` with the test module only, so it fails to compile against the not-yet-written functions:

```rust
//! Locates the git-tracked Gating-ML compliance corpus relative to this
//! crate's manifest, so tests and benches work on any clone.
//!
//! The corpus lives one directory above the crate root (workspace-level
//! `gates/`), is checked into git, and is the only real-file fixture set the
//! FCS reader is validated against. Hardcoding absolute paths made the
//! equivalence tests machine-local: `Fcs::open` returned `Err` elsewhere and
//! the `.expect()` failed rather than skipped.

#[cfg(test)]
mod tests {
    #[test]
    fn corpus_dir_resolves_relative_to_manifest() {
        let dir = super::dir();
        assert!(
            dir.is_dir(),
            "corpus directory must exist relative to CARGO_MANIFEST_DIR, got {}",
            dir.display()
        );
    }

    #[test]
    fn corpus_contains_the_ten_tracked_files() {
        let files = super::files();
        assert_eq!(
            files.len(),
            10,
            "expected the 10 git-tracked corpus files, found {}: {:?}",
            files.len(),
            files
        );
    }

    #[test]
    fn corpus_files_are_sorted_for_determinism() {
        let files = super::files();
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted, "files() must return a deterministic order");
    }

    #[test]
    fn corpus_path_joins_a_named_file() {
        let path = super::path("int-10000_events_random.fcs");
        assert!(path.is_file(), "{} should be a file", path.display());
    }
}
```

Register the module in `fcs/src/lib.rs`, immediately after line 26 (`pub mod crc;`):

```rust
#[doc(hidden)]
pub mod corpus;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p flow-fcs corpus::`
Expected: FAIL — compile error, `cannot find function 'dir' in module 'super'` (and the same for `files` and `path`).

- [ ] **Step 3: Write the implementation**

Insert above the `#[cfg(test)] mod tests` block in `fcs/src/corpus.rs`:

```rust
use std::path::{Path, PathBuf};

/// Path of the Gating-ML compliance corpus directory.
///
/// `CARGO_MANIFEST_DIR` is `<workspace>/fcs`, and the corpus is a
/// workspace-level directory, hence the `..`.
#[must_use]
pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("gates")
        .join("Gating-ML.v1.5.081030.Compliance-tests.081030")
        .join("List-mode Data Files")
}

/// Path of one named corpus file. Does not check that it exists — callers
/// that need a skip-if-missing guard should use [`is_available`].
#[must_use]
pub fn path(file_name: &str) -> PathBuf {
    dir().join(file_name)
}

/// Every `.fcs` file in the corpus, sorted by path.
///
/// Read from the directory rather than hardcoded so the list cannot drift
/// from what is actually checked in. Returns empty if the directory is
/// missing, which lets callers skip rather than panic.
#[must_use]
pub fn files() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir()) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("fcs"))
        })
        .collect();
    paths.sort();
    paths
}

/// True if the corpus directory is present. Use to skip corpus-backed tests
/// on a checkout that does not have it.
#[must_use]
pub fn is_available() -> bool {
    dir().is_dir()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p flow-fcs corpus::`
Expected: PASS, 4 tests.

- [ ] **Step 5: Replace the three hardcoded paths**

In `fcs/src/file.rs`, inside `mod lazy_column_tests`, delete the `COMPLIANCE_FCS` const at lines 2989-2990 and replace it with a function:

```rust
    fn compliance_fcs() -> std::path::PathBuf {
        crate::corpus::path("int-10000_events_random.fcs")
    }
```

Then change every `Fcs::open(COMPLIANCE_FCS)` in that module to `Fcs::open(compliance_fcs().to_str().expect("utf-8 corpus path"))`. There are four such call sites, in `column_matches_data_frame_oracle`, `column_caches_after_first_access`, `columns_batch_matches_individual_column_calls`, and `columns_dedupes_repeated_channel_request`.

In `fcs/src/compress.rs`, replace line 444:

```rust
        let path = crate::corpus::path("int-10000_events_random.fcs");
```

and change `std::path::Path::new(path).exists()` on the following line to `path.exists()`, and `Fcs::open(path)` to `Fcs::open(path.to_str().expect("utf-8 corpus path"))`. Leave the `#[ignore]` attribute and the `exists()` guard in place — they are independently correct.

In `fcs/benches/lazy_column_access.rs`, delete line 10 and add near the top of `main`-visible scope:

```rust
fn compliance_fcs() -> String {
    flow_fcs::corpus::path("int-10000_events_random.fcs")
        .to_str()
        .expect("utf-8 corpus path")
        .to_string()
}
```

Replace every use of `COMPLIANCE_FCS` with a `&compliance_fcs()` binding hoisted out of the closure — inside `iter_batched` setup closures, bind `let fixture = compliance_fcs();` once before the closure and move a clone in.

- [ ] **Step 6: Verify nothing regressed**

Run: `cargo test -p flow-fcs`
Expected: PASS. The `lazy_column_tests` module in particular must pass — it was previously passing only because the corpus happened to be at the hardcoded absolute path.

Run: `cargo bench -p flow-fcs --bench lazy_column_access -- --test`
Expected: PASS (the `--test` flag runs each bench once for correctness rather than measuring).

Run: `rg -n "/Users/" fcs/src fcs/benches`
Expected: no output.

- [ ] **Step 7: Commit (propose; do not run without authority)**

```bash
git add fcs/src/corpus.rs fcs/src/lib.rs fcs/src/file.rs fcs/src/compress.rs fcs/benches/lazy_column_access.rs && git commit -m "test(fcs): resolve compliance corpus from CARGO_MANIFEST_DIR"
```

- [ ] **Step 8: Close the bead**

```bash
bd close flow-crates-1o1 --reason="Corpus resolved via fcs/src/corpus.rs from CARGO_MANIFEST_DIR. Three hardcoded absolute-path sites replaced (file.rs lazy_column_tests, compress.rs real_fcs_round_trip_int10000, benches/lazy_column_access.rs) -- the bead recorded two; compress.rs:444 was the third."
```

---

# Phase 2 — TEXT delimiter escaping (`flow-crates-1xb`)

## Task 2: Extract one shared TEXT tokenizer (no behaviour change)

There are two hand-rolled tokenizers today, and `find_begindata_offset`'s doc comment says it "mirrors `Metadata::from_mmap`'s delimiter-tokenization exactly". That is a comment, not a guarantee. Escaping is exactly the change that breaks the convention silently, and it would break it only on `$NEXTDATA` chains — the same multi-dataset blind spot that hid the `absolutize()` bug. Unify them **before** changing behaviour, so this task is provably a no-op and the next task has one place to edit.

**Files:**
- Create: `fcs/src/text.rs`
- Modify: `fcs/src/lib.rs` (module list)
- Modify: `fcs/src/metadata.rs:77-203` (`from_text_segment` body)
- Modify: `fcs/src/file.rs:895-925` (`find_begindata_offset` body)
- Test: `fcs/src/text.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub(crate) enum Escaping { None, Doubled }`, `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`
  - `pub(crate) const fn Escaping::for_version(version: crate::version::Version) -> Escaping`
  - `pub(crate) struct TextFields<'a>`, with `pub(crate) fn new(body: &'a [u8], delimiter: u8, escaping: Escaping) -> Self`
  - `impl<'a> Iterator for TextFields<'a> { type Item = std::borrow::Cow<'a, str>; }`

  Fields alternate keyword, value, keyword, value… exactly as the two existing loops assume. `body` excludes the leading delimiter and includes the trailing one.

- [ ] **Step 1: Write the failing test**

Create `fcs/src/text.rs`:

```rust
//! One tokenizer for the FCS TEXT segment, shared by every reader.
//!
//! TEXT is `<delim>KEY<delim>VALUE<delim>KEY<delim>VALUE<delim>`. Two
//! independent hand-rolled walks of that structure used to exist — one in
//! `Metadata::from_text_segment` and one in `Fcs::find_begindata_offset` —
//! agreeing only by a doc comment that said so. They now share this unit,
//! because delimiter escaping is precisely the change that would desynchronize
//! them, and only on `$NEXTDATA` chains where nothing would notice.

#[cfg(test)]
mod tests {
    use super::{Escaping, TextFields};

    fn fields(body: &str, delimiter: u8, escaping: Escaping) -> Vec<String> {
        TextFields::new(body.as_bytes(), delimiter, escaping)
            .map(|field| field.into_owned())
            .collect()
    }

    #[test]
    fn splits_plain_key_value_pairs() {
        assert_eq!(
            fields("$PAR|2|$TOT|3|", b'|', Escaping::None),
            vec!["$PAR", "2", "$TOT", "3"]
        );
    }

    #[test]
    fn no_escaping_treats_a_doubled_delimiter_as_an_empty_field() {
        // FCS2.0 semantics: `real-8-parameters.data.fcs` really does contain
        // `\Comments\\Row\2\`, and `Comments` really does have an empty value.
        assert_eq!(
            fields("Comments||Row|2|", b'|', Escaping::None),
            vec!["Comments", "", "Row", "2"]
        );
    }

    #[test]
    fn doubled_escaping_folds_a_pair_into_one_literal_delimiter() {
        assert_eq!(
            fields("$COM|a||b|$PAR|2|", b'|', Escaping::Doubled),
            vec!["$COM", "a|b", "$PAR", "2"]
        );
    }

    #[test]
    fn doubled_escaping_handles_an_odd_run_as_literal_plus_separator() {
        // 3 delimiters = one literal + a field boundary.
        assert_eq!(
            fields("$COM|a|||$PAR|2|", b'|', Escaping::Doubled),
            vec!["$COM", "a|", "$PAR", "2"]
        );
    }

    #[test]
    fn doubled_escaping_handles_a_four_run_as_two_literals() {
        assert_eq!(
            fields("$COM|a||||b|", b'|', Escaping::Doubled),
            vec!["$COM", "a||b"]
        );
    }

    #[test]
    fn yields_a_trailing_field_with_no_terminating_delimiter() {
        // find_begindata_offset scans an unbounded tail; the last field there
        // is not delimiter-terminated.
        assert_eq!(
            fields("$PAR|2|$TOT|3", b'|', Escaping::None),
            vec!["$PAR", "2", "$TOT", "3"]
        );
    }

    #[test]
    fn empty_body_yields_nothing() {
        assert!(fields("", b'|', Escaping::None).is_empty());
        assert!(fields("", b'|', Escaping::Doubled).is_empty());
    }

    #[test]
    fn escaping_gate_starts_at_v3_1() {
        use crate::version::Version;
        assert_eq!(Escaping::for_version(Version::V1_0), Escaping::None);
        assert_eq!(Escaping::for_version(Version::V2_0), Escaping::None);
        assert_eq!(Escaping::for_version(Version::V3_0), Escaping::None);
        assert_eq!(Escaping::for_version(Version::V3_1), Escaping::Doubled);
        assert_eq!(Escaping::for_version(Version::V3_2), Escaping::Doubled);
        assert_eq!(Escaping::for_version(Version::V4_0), Escaping::Doubled);
    }
}
```

Register in `fcs/src/lib.rs`, after the `corpus` entry:

```rust
pub(crate) mod text;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p flow-fcs text::`
Expected: FAIL — compile error, `cannot find type 'Escaping' in module 'super'`.

- [ ] **Step 3: Write the implementation**

Insert above the test module in `fcs/src/text.rs`:

```rust
use crate::version::Version;
use std::borrow::Cow;

/// How a run of consecutive delimiters inside TEXT is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Escaping {
    /// Split on every delimiter; a run of N delimiters yields N-1 empty
    /// fields. FCS 3.0 and earlier permit empty keyword values, and corpus
    /// files use them, so `\\` there is a genuine empty value and no
    /// tokenizer can tell it apart from an escape.
    None,
    /// A run of N delimiters encodes N/2 literal delimiter characters, and
    /// terminates the field iff N is odd. Decodable only because FCS 3.1 and
    /// later forbid empty keyword values.
    Doubled,
}

impl Escaping {
    /// The gate sits at 3.1, not 3.0, deliberately. Being wrong at 3.1 means a
    /// rare FCS3.0 file with an escaped delimiter keeps mis-parsing — the
    /// status quo. Being wrong at 3.0 means a common FCS3.0 file with an empty
    /// value *newly* desynchronizes.
    pub(crate) const fn for_version(version: Version) -> Self {
        match version {
            Version::V1_0 | Version::V2_0 | Version::V3_0 => Self::None,
            Version::V3_1 | Version::V3_2 | Version::V4_0 => Self::Doubled,
        }
    }
}

/// Lazy iterator over the delimiter-separated fields of a TEXT segment body.
///
/// `body` must already exclude TEXT's leading delimiter. It may or may not be
/// delimiter-terminated: `Metadata::from_text_segment` passes a bounded slice
/// that is, and `Fcs::find_begindata_offset` passes the unbounded remainder of
/// the mmap that is not. Laziness is what lets the latter stop at
/// `$BEGINDATA` instead of walking into DATA.
pub(crate) struct TextFields<'a> {
    body: &'a [u8],
    delimiter: u8,
    escaping: Escaping,
    pos: usize,
}

impl<'a> TextFields<'a> {
    pub(crate) const fn new(body: &'a [u8], delimiter: u8, escaping: Escaping) -> Self {
        Self { body, delimiter, escaping, pos: 0 }
    }

    /// Length of the run of `self.delimiter` starting at `start`.
    fn run_length(&self, start: usize) -> usize {
        let mut end = start;
        while end < self.body.len() && self.body[end] == self.delimiter {
            end += 1;
        }
        end - start
    }
}

/// FCS requires TEXT to be ASCII. Invalid UTF-8 degrades to an empty field
/// rather than erroring, matching the pre-existing `unwrap_or_default()`
/// behaviour of both tokenizers this replaced.
fn as_str(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap_or_default()
}

impl<'a> Iterator for TextFields<'a> {
    type Item = Cow<'a, str>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.body.len() {
            return None;
        }

        match self.escaping {
            Escaping::None => {
                let start = self.pos;
                match memchr::memchr(self.delimiter, &self.body[start..]) {
                    Some(relative) => {
                        let end = start + relative;
                        self.pos = end + 1;
                        Some(Cow::Borrowed(as_str(&self.body[start..end])))
                    }
                    None => {
                        self.pos = self.body.len();
                        Some(Cow::Borrowed(as_str(&self.body[start..])))
                    }
                }
            }
            Escaping::Doubled => {
                let start = self.pos;
                let mut cursor = start;
                // Stays `None` while the field is one contiguous borrow; only
                // an actual un-doubling forces an owned String.
                let mut owned: Option<String> = None;

                loop {
                    let Some(relative) = memchr::memchr(self.delimiter, &self.body[cursor..])
                    else {
                        // Unterminated trailing field.
                        let tail = as_str(&self.body[cursor..]);
                        self.pos = self.body.len();
                        return Some(match owned {
                            Some(mut buffer) => {
                                buffer.push_str(tail);
                                Cow::Owned(buffer)
                            }
                            None => Cow::Borrowed(as_str(&self.body[start..])),
                        });
                    };

                    let run_start = cursor + relative;
                    let run = self.run_length(run_start);
                    let literals = run / 2;
                    let terminates = run % 2 == 1;

                    if literals == 0 && terminates {
                        // The common case: a lone delimiter ends the field.
                        self.pos = run_start + run;
                        return Some(match owned {
                            Some(mut buffer) => {
                                buffer.push_str(as_str(&self.body[cursor..run_start]));
                                Cow::Owned(buffer)
                            }
                            None => Cow::Borrowed(as_str(&self.body[start..run_start])),
                        });
                    }

                    let buffer = owned.get_or_insert_with(|| {
                        String::from(as_str(&self.body[start..cursor]))
                    });
                    buffer.push_str(as_str(&self.body[cursor..run_start]));
                    for _ in 0..literals {
                        buffer.push(self.delimiter as char);
                    }

                    if terminates {
                        self.pos = run_start + run;
                        return Some(Cow::Owned(owned.take().unwrap_or_default()));
                    }
                    cursor = run_start + run;
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p flow-fcs text::`
Expected: PASS, 8 tests.

- [ ] **Step 5: Rewire `Metadata::from_text_segment` onto the shared tokenizer**

In `fcs/src/metadata.rs`, replace the body of `from_text_segment` from the `let delimiter_positions` line (currently line 89) through the end of the trailing-segment block (currently line 197), leaving the `Self { keywords, delimiter: delimiter as char }` return in place. Keep the escaping fixed at `Escaping::None` in this task so behaviour is unchanged; Task 3 makes it version-dependent.

```rust
        let mut keywords: KeywordMap = FxHashMap::default();
        let mut fields = crate::text::TextFields::new(
            text_slice,
            delimiter,
            crate::text::Escaping::None,
        );

        while let Some(key) = fields.next() {
            let Some(value) = fields.next() else {
                // A keyword with no value at the end of TEXT. Invalid FCS, but
                // observed; drop it as the previous implementation did.
                tracing::debug!(
                    "Warning: Keyword '{}' at end of text segment ({:?}) has no value",
                    key, text_range
                );
                break;
            };
            if key.is_empty() {
                continue;
            }
            // Preserve key as-is: FCS spec reserves $ for standard keywords only.
            // User-defined keywords (e.g. "Tissue") must not gain a $ prefix.
            let normalized_key = key.to_string();
            match match_and_parse_keyword(&key, &value) {
                KeywordCreationResult::Int(k) => { keywords.insert(normalized_key, Keyword::Int(k)); }
                KeywordCreationResult::Float(k) => { keywords.insert(normalized_key, Keyword::Float(k)); }
                KeywordCreationResult::String(k) => { keywords.insert(normalized_key, Keyword::String(k)); }
                KeywordCreationResult::Byte(k) => { keywords.insert(normalized_key, Keyword::Byte(k)); }
                KeywordCreationResult::Mixed(k) => { keywords.insert(normalized_key, Keyword::Mixed(k)); }
                KeywordCreationResult::UnableToParse => {
                    tracing::debug!("Unable to parse keyword: {} with value: {}", key, value);
                }
            }
        }
```

This also removes the duplicated 30-line `match KeywordCreationResult` block that existed twice in the original — once for the main loop and once for the trailing segment.

- [ ] **Step 6: Rewire `find_begindata_offset` onto the shared tokenizer**

In `fcs/src/file.rs`, replace the body of `find_begindata_offset` (lines 896-924) between the `let rest = ...` line and the trailing `Err(anyhow!(...))`:

```rust
        let mut fields = crate::text::TextFields::new(
            rest,
            delimiter,
            crate::text::Escaping::None,
        );

        while let Some(key) = fields.next() {
            let Some(value) = fields.next() else { break };
            if key.eq_ignore_ascii_case("$BEGINDATA") {
                return value.trim().parse::<usize>().with_context(|| {
                    format!("Invalid $BEGINDATA value '{value}' while scanning for next dataset's TEXT boundary")
                });
            }
        }
```

Also update the doc comment at lines 883-889: replace *"Mirrors `Metadata::from_mmap`'s delimiter-tokenization exactly (same keyword/value alternation)"* with *"Shares `crate::text::TextFields` with `Metadata::from_text_segment`, so the two cannot drift, but stops at the first match instead of tokenizing the whole segment — the segment's end isn't known yet, which is the value this function exists to find."*

- [ ] **Step 7: Verify this task changed nothing observable**

Run: `cargo test -p flow-fcs`
Expected: PASS, with the same test count as before this task. Any failure here is a tokenizer transcription error, not a behaviour decision — the escaping policy is still `None` everywhere.

Run: `cargo test --workspace`
Expected: PASS. `tru-ols-cli` calls `Metadata::from_mmap`, whose signature is unchanged.

- [ ] **Step 8: Commit (propose)**

```bash
git add fcs/src/text.rs fcs/src/lib.rs fcs/src/metadata.rs fcs/src/file.rs && git commit -m "refactor(fcs): share one TEXT tokenizer between metadata and \$NEXTDATA scan"
```

---

## Task 3: Version-gate un-doubling on read

**Files:**
- Modify: `fcs/src/metadata.rs` (`from_mmap`, `from_text_segment` signatures and bodies)
- Modify: `fcs/src/file.rs` (`parse_one_dataset` call site ~line 724; `find_begindata_offset` signature and its call site ~line 859)
- Test: `fcs/src/metadata.rs` (inline test module) and `fcs/src/text.rs`

**Interfaces:**
- Consumes: `crate::text::{Escaping, TextFields}` from Task 2.
- Produces:
  - `Metadata::from_text_segment(mmap: &Mmap, text_range: &RangeInclusive<usize>, version: Version) -> Metadata` — **breaking signature change** to a `pub fn`. `Metadata::from_mmap(mmap, header)` is unchanged and now forwards `header.version`.
  - `Fcs::find_begindata_offset(mmap: &Mmap, text_start: usize, version: Version) -> Result<usize>`

- [ ] **Step 1: Write the failing test**

Add to `fcs/src/text.rs`'s test module:

```rust
    #[test]
    fn corpus_empty_values_survive_under_v2_0_policy() {
        // real-8-parameters.data.fcs is FCS2.0 and contains `\Comments\\Row\2\`.
        // Under Doubled this would read as Comments="|Row" and shift every
        // subsequent field by one, which is exactly the desynchronization the
        // writer bug causes. Version::V2_0 must therefore not un-double.
        use crate::version::Version;
        let escaping = Escaping::for_version(Version::V2_0);
        assert_eq!(
            fields("Comments||Row|2|", b'|', escaping),
            vec!["Comments", "", "Row", "2"]
        );
    }
```

Add a new inline test module at the end of `fcs/src/metadata.rs`:

```rust
#[cfg(test)]
mod delimiter_escaping_read_tests {
    use crate::file::Fcs;

    /// Every corpus file must parse to the same keyword count it did before
    /// escaping existed. Two of the ten (`fcs2_int16_13367ev_8par_GvHD.fcs`
    /// and `real-8-parameters.data.fcs`) carry FCS2.0 empty values and are the
    /// load-bearing cases: under an unconditional un-double they would lose
    /// keywords as fields shifted.
    #[test]
    fn every_corpus_file_parses_without_field_shift() {
        if !crate::corpus::is_available() {
            eprintln!("compliance corpus missing, skipping");
            return;
        }
        for path in crate::corpus::files() {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let fcs = Fcs::open(path.to_str().expect("utf-8 path"))
                .unwrap_or_else(|e| panic!("open {name}: {e}"));

            // A shifted tokenizer loses $PAR/$TOT or reads them as garbage,
            // so these two accessors are a sharp shift detector.
            let par = *fcs.metadata.get_number_of_parameters()
                .unwrap_or_else(|e| panic!("{name}: $PAR: {e}"));
            let tot = *fcs.metadata.get_number_of_events()
                .unwrap_or_else(|e| panic!("{name}: $TOT: {e}"));
            assert!(par > 0, "{name}: $PAR must be positive");
            assert!(tot > 0, "{name}: $TOT must be positive");
            assert_eq!(
                fcs.data_frame.width(), par,
                "{name}: decoded column count must match $PAR"
            );
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify the corpus one passes and the policy one compiles**

Run: `cargo test -p flow-fcs delimiter_escaping_read_tests text::corpus_empty`
Expected: PASS both. These are the regression guard; they must be green *before* the policy changes, so a later failure is unambiguous.

- [ ] **Step 3: Thread `Version` through the read path**

In `fcs/src/metadata.rs`, change the signature at line 77 and forward from `from_mmap`:

```rust
    #[must_use]
    pub fn from_mmap(mmap: &Mmap, header: &Header) -> Self {
        Self::from_text_segment(mmap, &header.text_offset, header.version)
    }

    /// As [`from_mmap`](Self::from_mmap), but takes the TEXT segment's
    /// **file-absolute** byte range directly, plus the version whose escaping
    /// rules apply.
    ///
    /// [`Header`] carries data-set-relative offsets (§2.4.3), so any data set
    /// past the first in a `$NEXTDATA` chain must resolve those against its own
    /// base before they can index the mmap. `from_mmap` is the `base == 0` case.
    ///
    /// `version` decides whether a doubled delimiter is one escaped literal
    /// (FCS 3.1+) or two boundaries around an empty value (3.0 and earlier).
    /// Data sets reached through `$NEXTDATA` have no HEADER of their own, so
    /// callers pass the file's primary version.
    #[must_use]
    pub fn from_text_segment(
        mmap: &Mmap,
        text_range: &std::ops::RangeInclusive<usize>,
        version: crate::version::Version,
    ) -> Self {
```

Replace the fixed `crate::text::Escaping::None` in the body with:

```rust
            crate::text::Escaping::for_version(version),
```

In `fcs/src/file.rs`, at the `parse_one_dataset` call site (currently line 724):

```rust
        let mut metadata = Metadata::from_text_segment(
            &file_access.mmap,
            &text_range(&header, dataset_start, mmap_len),
            header.version,
        );
```

Change `find_begindata_offset`'s signature to take `version: crate::version::Version` as a third parameter, replace its fixed `Escaping::None` with `crate::text::Escaping::for_version(version)`, and update the call site inside `header_for_dataset_at` (line 859) to pass its already-present `version` argument:

```rust
            Self::find_begindata_offset(mmap, dataset_start, version)?,
```

- [ ] **Step 4: Run the full suite**

Run: `cargo test -p flow-fcs`
Expected: PASS. The corpus test from Step 1 is the one that matters — all ten files are FCS2.0/3.0 era, so they take the `Escaping::None` branch and nothing about them changes.

Run: `cargo test --workspace`
Expected: PASS. `tru-ols-cli/src/interactive.rs:68` uses `from_mmap`, whose signature did not change. If anything else calls `from_text_segment` directly it will fail to compile — add the version argument at that site.

- [ ] **Step 5: Commit (propose)**

```bash
git add fcs/src/metadata.rs fcs/src/file.rs fcs/src/text.rs && git commit -m "feat(fcs): un-double escaped TEXT delimiters on read for FCS3.1+"
```

---

## Task 4: Escape on write, error on empty values, validate the delimiter

**Files:**
- Modify: `fcs/src/text.rs` (add the write-side helper)
- Modify: `fcs/src/write.rs:575-580` (`resolve_layout` signature), `:591`/`:596` (its two internal `serialize_metadata` calls), `:621-628` (`serialize_metadata` signature), `:629-812` (body), `:144` (production call site)
- Modify: `fcs/src/compress.rs:163` (call site)
- Test: `fcs/src/write.rs` (new inline `#[cfg(test)] mod delimiter_escaping_write_tests`)

**Interfaces:**
- Consumes: `crate::text::{Escaping, TextFields}` from Task 2.
- Produces:
  - `pub(crate) fn crate::text::escape_into(out: &mut Vec<u8>, text: &str, delimiter: u8, escaping: Escaping)`
  - `pub(crate) fn crate::text::validate_delimiter(delimiter: char) -> anyhow::Result<u8>`
  - `resolve_layout(metadata: &Metadata, text_start: usize, n_events: usize, n_params: usize, data_len: usize, version: Version) -> Result<FcsLayout>` — `version` appended as the **sixth** parameter.
  - `serialize_metadata(metadata: &Metadata, n_events: usize, n_params: usize, data_start: usize, data_end: usize, version: Version) -> Result<Vec<u8>>` — `version` appended as the **sixth** parameter.

- [ ] **Step 1: Write the failing test**

Add to `fcs/src/text.rs`'s test module:

```rust
    #[test]
    fn escape_into_doubles_the_delimiter_only_under_doubled() {
        let mut out = Vec::new();
        super::escape_into(&mut out, "a|b", b'|', Escaping::Doubled);
        assert_eq!(out, b"a||b");

        let mut out = Vec::new();
        super::escape_into(&mut out, "a|b", b'|', Escaping::None);
        assert_eq!(out, b"a|b");
    }

    #[test]
    fn escape_then_tokenize_round_trips() {
        let mut body = Vec::new();
        for (key, value) in [("$COM", "hello world"), ("$PAR", "2")] {
            super::escape_into(&mut body, key, b' ', Escaping::Doubled);
            body.push(b' ');
            super::escape_into(&mut body, value, b' ', Escaping::Doubled);
            body.push(b' ');
        }
        let got: Vec<String> = TextFields::new(&body, b' ', Escaping::Doubled)
            .map(std::borrow::Cow::into_owned)
            .collect();
        assert_eq!(got, vec!["$COM", "hello world", "$PAR", "2"]);
    }

    #[test]
    fn validate_delimiter_accepts_ascii_1_to_126() {
        assert!(super::validate_delimiter('\u{0001}').is_ok());
        assert!(super::validate_delimiter(' ').is_ok());
        assert!(super::validate_delimiter('\u{000c}').is_ok());
        assert!(super::validate_delimiter('~').is_ok());
    }

    #[test]
    fn validate_delimiter_rejects_nul_and_out_of_range() {
        for bad in ['\u{0000}', '\u{007f}', '\u{00e9}'] {
            let err = super::validate_delimiter(bad).unwrap_err().to_string();
            assert!(
                err.contains("delimiter"),
                "error should name the delimiter, got: {err}"
            );
        }
    }
```

Add a new inline test module at the end of `fcs/src/write.rs`. Note the `stub` file: `Fcs::for_testing` needs an `AccessWrapper`, which needs a real path.

```rust
#[cfg(test)]
mod delimiter_escaping_write_tests {
    use super::*;
    use crate::keyword::{IntegerKeyword, Keyword, MixedKeyword, StringableKeyword};
    use crate::version::Version;
    use crate::{Header, Metadata, Parameter, TransformType, file::AccessWrapper, parameter::ParameterMap};
    use polars::prelude::Column;
    use std::sync::Arc;

    /// One-event, one-parameter FCS whose `$CYT` carries `cyt_value`, written
    /// under `version` with `delimiter`.
    ///
    /// `$CYT` is the carrier rather than `$COM` because `$COM` is not a
    /// recognized `StringKeyword` variant — `parse_string_keywords` returns
    /// `None` for it and the reader drops it, so it could never round-trip
    /// regardless of escaping. `$CYT` is FCS 1.0+, required from 3.2 on, and
    /// real cytometer names ("BD LSRFortessa X-20") contain spaces, which is
    /// exactly this bug's blast radius.
    ///
    /// Mirrors the fixture idiom of `write_fcs_header_and_text_data_offsets_agree`,
    /// including `$PnE` — that test is the proof this keyword set satisfies
    /// `enforce_conformance` for V3_1, since `serialize_metadata` synthesizes
    /// `$PAR`/`$TOT`/`$BEGIN*`/`$END*` itself.
    fn fixture(version: Version, delimiter: char, cyt_value: &str) -> (Fcs, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let stub = tmp.path().join("src.tmp");
        std::fs::write(&stub, b"x").expect("stub");

        let df = DataFrame::new_infer_height(vec![Column::new("FSC-A".into(), vec![1.0f32])])
            .expect("df");
        let mut params = ParameterMap::default();
        params.insert(
            "FSC-A".into(),
            Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
        );

        let mut metadata = Metadata::new();
        metadata.delimiter = delimiter;
        metadata.insert_string_keyword("$BYTEORD".into(), "1,2,3,4".into());
        metadata.insert_string_keyword("$DATATYPE".into(), "F".into());
        metadata.insert_string_keyword("$MODE".into(), "L".into());
        metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
        metadata.insert_string_keyword("$P1N".into(), "FSC-A".into());
        metadata.insert_string_keyword("$CYT".into(), cyt_value.into());
        metadata.keywords.insert("$P1B".into(), Keyword::Int(IntegerKeyword::PnB(32)));
        metadata.keywords.insert("$P1R".into(), Keyword::Int(IntegerKeyword::PnR(262144)));
        metadata.keywords.insert("$P1E".into(), Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)));

        let mut header = Header::new();
        header.version = version;

        let fcs = Fcs::for_testing(
            header,
            metadata,
            params,
            Arc::new(df),
            AccessWrapper::new(stub.to_str().expect("utf-8")).expect("access"),
        );
        (fcs, tmp)
    }

    /// `get_string_keyword` is an exact hashmap lookup with no `$`
    /// normalization, and the reader stores keys verbatim — so the written
    /// `$CYT` reads back under the key `$CYT`, not `CYT`.
    fn read_cyt(fcs: &Fcs) -> String {
        fcs.metadata
            .get_string_keyword("$CYT")
            .expect("$CYT present")
            .get_str()
            .into_owned()
    }

    #[test]
    fn value_containing_the_space_delimiter_round_trips() {
        let (fcs, tmp) = fixture(Version::V3_1, ' ', "BD LSRFortessa X-20");
        let out = tmp.path().join("rt.fcs");
        write_fcs_file(fcs, &out).expect("write");

        let read_back = Fcs::open(out.to_str().expect("utf-8")).expect("reopen");
        assert_eq!(
            read_cyt(&read_back),
            "BD LSRFortessa X-20",
            "a value containing the active delimiter must survive the round trip intact"
        );
    }

    #[test]
    fn value_containing_the_comma_delimiter_round_trips() {
        let (fcs, tmp) = fixture(Version::V3_1, ',', "a,b,c");
        let out = tmp.path().join("rt.fcs");
        write_fcs_file(fcs, &out).expect("write");

        let read_back = Fcs::open(out.to_str().expect("utf-8")).expect("reopen");
        assert_eq!(read_cyt(&read_back), "a,b,c");
    }

    #[test]
    fn value_containing_the_form_feed_delimiter_round_trips() {
        let (fcs, tmp) = fixture(Version::V3_1, '\u{000c}', "a\u{000c}b");
        let out = tmp.path().join("rt.fcs");
        write_fcs_file(fcs, &out).expect("write");

        let read_back = Fcs::open(out.to_str().expect("utf-8")).expect("reopen");
        assert_eq!(read_cyt(&read_back), "a\u{000c}b");
    }

    #[test]
    fn keywords_after_an_escaped_value_are_not_shifted() {
        // The whole point: a truncated value used to desynchronize everything
        // after it. $CYT sorts before $DATATYPE, $MODE, $P1N and the rest, so
        // a shift here corrupts every remaining keyword.
        let (fcs, tmp) = fixture(Version::V3_1, ' ', "one two three four");
        let out = tmp.path().join("rt.fcs");
        write_fcs_file(fcs, &out).expect("write");

        let read_back = Fcs::open(out.to_str().expect("utf-8")).expect("reopen");
        assert_eq!(
            read_back.metadata.get_string_keyword("$P1N").expect("$P1N").get_str(),
            "FSC-A"
        );
        assert_eq!(*read_back.metadata.get_number_of_parameters().expect("$PAR"), 1);
        assert_eq!(*read_back.metadata.get_number_of_events().expect("$TOT"), 1);
    }

    #[test]
    fn empty_value_is_an_error_under_v3_1_and_names_the_keyword() {
        let (fcs, tmp) = fixture(Version::V3_1, ' ', "");
        let out = tmp.path().join("rt.fcs");
        let err = write_fcs_file(fcs, &out).unwrap_err().to_string();
        assert!(
            err.contains("$CYT"),
            "the error must name the offending keyword, got: {err}"
        );
        assert!(
            err.contains("empty"),
            "the error must say the value is empty, got: {err}"
        );
    }

    #[test]
    fn empty_value_is_allowed_under_v2_0() {
        let (fcs, tmp) = fixture(Version::V2_0, ' ', "");
        let out = tmp.path().join("rt.fcs");
        write_fcs_file(fcs, &out).expect("FCS2.0 permits empty keyword values");
    }

    #[test]
    fn out_of_range_delimiter_is_rejected() {
        let (fcs, tmp) = fixture(Version::V3_1, '\u{0000}', "x");
        let out = tmp.path().join("rt.fcs");
        let err = write_fcs_file(fcs, &out).unwrap_err().to_string();
        assert!(
            err.contains("delimiter"),
            "the error must name the delimiter, got: {err}"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p flow-fcs delimiter_escaping_write_tests`
Expected: FAIL. `value_containing_the_space_delimiter_round_trips` fails on the truncated `$COM` (this is `flow-crates-1xb` reproduced); the empty-value and delimiter-validation tests fail because no such error exists yet.

- [ ] **Step 3: Add the write-side helpers to `fcs/src/text.rs`**

```rust
/// Append `text` to `out`, doubling any occurrence of `delimiter` when the
/// version escapes. The key is escaped as well as the value: user-defined
/// keywords are free-form and can contain the delimiter too.
pub(crate) fn escape_into(out: &mut Vec<u8>, text: &str, delimiter: u8, escaping: Escaping) {
    match escaping {
        Escaping::None => out.extend_from_slice(text.as_bytes()),
        Escaping::Doubled => {
            for &byte in text.as_bytes() {
                out.push(byte);
                if byte == delimiter {
                    out.push(byte);
                }
            }
        }
    }
}

/// The TEXT delimiter must be a single byte in ASCII 1-126.
///
/// NUL is excluded because it cannot be distinguished from padding, and
/// anything at or above 127 is either DEL or the lead byte of a multi-byte
/// UTF-8 sequence — neither is a single-byte delimiter, and `memchr` on a
/// truncated lead byte would split mid-character.
///
/// # Errors
/// Returns `Err` naming the rejected delimiter if it falls outside that range.
pub(crate) fn validate_delimiter(delimiter: char) -> anyhow::Result<u8> {
    let code = delimiter as u32;
    if (1..=126).contains(&code) {
        Ok(code as u8)
    } else {
        Err(anyhow::anyhow!(
            "Invalid TEXT delimiter U+{code:04X}: must be a single ASCII byte in 1-126 \
             (NUL and anything at or above DEL are not representable as a delimiter)"
        ))
    }
}
```

- [ ] **Step 4: Thread `Version` into `resolve_layout` and `serialize_metadata`**

In `fcs/src/write.rs`, add `version: crate::version::Version` as the final parameter of both functions, and forward it from `resolve_layout`'s two internal calls (currently lines 591 and 596). Add to `resolve_layout`'s doc comment:

```
/// `version` decides TEXT's escaping policy, which changes TEXT's length and
/// therefore `$BEGINDATA` — the fixed-point loop absorbs that with no extra
/// work, since it re-serializes until the offsets settle.
```

Update the two production call sites:

- `fcs/src/write.rs:144` — add `fcs.header.version,` as the sixth argument.
- `fcs/src/compress.rs:163` — add `self.header.version,` as the sixth argument.

- [ ] **Step 5: Rewrite `serialize_metadata` to collect pairs, then validate and escape**

Replace the top of the function body (currently lines 629-637) so it collects pairs rather than writing bytes directly:

```rust
    let delimiter = crate::text::validate_delimiter(metadata.delimiter)?;
    let escaping = crate::text::Escaping::for_version(version);

    // Collect first, serialize second. Validation needs to see each value
    // before any bytes are committed, and the closure below is `FnMut`, so it
    // cannot both borrow `text_segment` mutably and be read from.
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut add_keyword = |key: &str, value: &str| {
        pairs.push((format!("${key}"), value.to_string()));
    };
```

Leave every `add_keyword(...)` call between there and the end of the sorted-keys loop exactly as it is. Then replace the tail of the function (currently lines 809-812, the `text_segment.push(delimiter); Ok(text_segment)` block) with:

```rust
    let mut text_segment = Vec::new();
    for (key, value) in &pairs {
        if escaping == crate::text::Escaping::Doubled && value.is_empty() {
            return Err(anyhow!(
                "FCS {version} forbids empty keyword values, but {key} has one. \
                 An empty value serializes to a doubled delimiter, which reads back \
                 as one literal delimiter — silently corrupting every field after it. \
                 Omit {key} instead of writing it with no value."
            ));
        }
        text_segment.push(delimiter);
        crate::text::escape_into(&mut text_segment, key, delimiter, escaping);
        text_segment.push(delimiter);
        crate::text::escape_into(&mut text_segment, value, delimiter, escaping);
    }

    // Trailing delimiter terminates the last value; the reader's tokenizer
    // expects it.
    text_segment.push(delimiter);

    Ok(text_segment)
```

Remove the now-unused `let delimiter = metadata.delimiter as u8;` line that the validation call replaced.

- [ ] **Step 6: Run the new tests to verify they pass**

Run: `cargo test -p flow-fcs delimiter_escaping_write_tests text::`
Expected: PASS, 7 write tests plus the text-module tests.

- [ ] **Step 7: Run the full crate suite and record what Phase 2's rules surface**

Run: `cargo test -p flow-fcs`
Expected: **FAIL**, and specifically at the eight sites that write `$PnS` as an empty string. `Header::new()` defaults to `Version::V3_1`, so these now hit the empty-value error. This is the rule working correctly — FCS 3.1 forbids empty values, so the right encoding is to omit `$PnS`, not write it blank. Task 5 fixes them. Do not weaken the rule to make them pass.

Note the failing test names for Task 5. The eight sites are `fcs/src/write.rs` lines 1050, 1367, 1467, 1598, 1606, 1695, 1828, 1993.

- [ ] **Step 8: Commit (propose)**

```bash
git add fcs/src/text.rs fcs/src/write.rs fcs/src/compress.rs && git commit -m "fix(fcs): escape the TEXT delimiter on write for FCS3.1+ (flow-crates-1xb)"
```

---

## Task 5: Remove the form-feed workaround and the empty-`$PnS` writes

The ten forced `metadata.delimiter = '\u{000c}'` assignments exist only because the writer could not handle a delimiter that appears in values. With Task 4 landed they are dead weight, and worse: they mean the default space delimiter has never been exercised by a single test.

**Files:**
- Modify: `fcs/src/write.rs` — delete lines 1036, 1166, 1265, 1355, 1451, 1583, 1683, 1816, 1982, 2113 (all `metadata.delimiter = '\u{000c}';`)
- Modify: `fcs/src/write.rs` — lines 1050, 1367, 1467, 1598, 1606, 1695, 1828, 1993 (empty `$PnS` values)
- Modify: `fcs/src/columns.rs:196` and `:243` — the two `metadata.delimiter = '\u{000c}';` lines in synthetic test metadata (harmless, but they hide the default too)

**Interfaces:**
- Consumes: the escaping writer from Task 4.
- Produces: nothing new.

- [ ] **Step 1: Delete the ten form-feed assignments**

Run: `rg -n "metadata.delimiter = '\\\\u\\{000c\\}';" fcs/src/write.rs`
Expected: ten hits at the lines listed above. Delete each line. Deleting them makes every affected test exercise `Metadata::new()`'s default space delimiter, which is the point.

- [ ] **Step 2: Fix the eight empty `$PnS` writes**

`$PnS` is the parameter's stain/label name. Under FCS 3.1 an unknown stain is expressed by omitting the keyword, not by writing it empty. At each of lines 1050, 1367, 1467, 1598, 1606, 1695, 1828, 1993, **delete** the `insert_string_keyword("$PnS", "")` call rather than substituting a placeholder value — omission is the conformant encoding, and a placeholder would make the test assert something untrue about the file.

Where a test asserts on `$P1S` afterwards, give that parameter a real stain name instead of deleting (check each site; in `write_fcs_header_and_text_data_offsets_agree` at line 1050, `$P2S` is already `"FITC"` and `$P1S` is only there for symmetry, so deletion is right).

- [ ] **Step 3: Remove the two form-feed lines from `columns.rs` synthetic metadata**

Delete `metadata.delimiter = '\u{000c}';` from `synthetic_metadata_2f32` (line 196) and `synthetic_metadata_varying_widths` (line 243). These metadata objects are never serialized, so this is cosmetic — but leaving them would imply the workaround is still needed.

- [ ] **Step 4: Run the full crate suite**

Run: `cargo test -p flow-fcs`
Expected: PASS.

If a test now fails for a reason *other* than an empty value — for instance a value that contains a space and previously survived only because the delimiter was a form feed — that is a pre-existing writer defect becoming visible for the first time. Per the spec's risk section: **file it as its own bead and do not absorb the fix here.**

```bash
bd create --title="fcs: <one-line description>" --description="Surfaced by flow-crates-1xb Task 5, when the forced form-feed delimiter was removed from write.rs tests and the default space delimiter was exercised for the first time. <details>" --type=bug --priority=2
```

- [ ] **Step 5: Run the workspace suite**

Run: `cargo test --workspace`
Expected: PASS. `tru-ols` still forces `SAFE_TEXT_DELIMITER` via `ensure_delimiter_survives_provenance`, which remains correct (just no longer necessary) — retiring it is `flow-crates-8px`, deliberately out of scope.

- [ ] **Step 6: Commit (propose)**

```bash
git add fcs/src/write.rs fcs/src/columns.rs && git commit -m "test(fcs): exercise the default space delimiter, drop form-feed workaround"
```

---

## Task 6: Prove the two tokenizers agree on a `$NEXTDATA` chain

`find_begindata_offset` runs only on data sets past the first in a `$NEXTDATA` chain, and only when that data set has no HEADER of its own. Escaping is exactly the change that could desynchronize it from `from_text_segment` — silently, and only on multi-dataset files. A two-data-set fixture is not enough on its own, but combined with an escaped delimiter in the first data set it pins the behaviour.

**Files:**
- Test: `fcs/src/file.rs` (new inline `#[cfg(test)] mod nextdata_escaping_tests`)

**Interfaces:**
- Consumes: `Metadata::from_text_segment(mmap, range, version)` and `Fcs::find_begindata_offset(mmap, start, version)` from Task 3; the escaping writer from Task 4.
- Produces: nothing new.

- [ ] **Step 1: Read the existing two-data-set fixture builder**

The precedent is `open_all_traverses_nextdata_chain_across_two_datasets` at `fcs/src/write.rs:1678` (filed as `flow-crates-1mg`), whose nested `build_dataset_metadata(nextdata: usize) -> Metadata` helper at `write.rs:1681` composes exactly the file this task needs: data set 1 has a HEADER whose `$NEXTDATA` points at data set 2's TEXT start, and data set 2 has **no** HEADER of its own — which is precisely the case that forces `find_begindata_offset` to run.

Read `write.rs:1678-1795` in full before writing anything. Note that this test is also one Task 5 touches (its form-feed assignment is at line 1683 and its empty `$PnS` at 1695), so it should already be running under the default space delimiter by the time you get here.

Adapt `build_dataset_metadata` into `write_two_dataset_fixture(path: &Path, version: Version, cyt: &str)` by giving it a `$CYT` keyword and threading the version onto `Header::new()`. Lift it to the module level of the new test module rather than duplicating the body.

- [ ] **Step 2: Write the failing test**

```rust
#[cfg(test)]
mod nextdata_escaping_tests {
    use crate::file::Fcs;
    use crate::version::Version;

    /// A two-data-set file whose *first* data set carries a `$CYT` containing
    /// the active delimiter. Reading data set 2 requires
    /// `find_begindata_offset` to walk data set 1's TEXT with the same
    /// escaping policy `from_text_segment` used to write it. If the two
    /// disagree, the scan lands on the wrong `$BEGINDATA` and data set 2
    /// either fails to open or decodes the wrong bytes.
    #[test]
    fn nextdata_chain_survives_an_escaped_delimiter_in_dataset_one() {
        use crate::keyword::StringableKeyword;

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("chain.fcs");

        // Build a two-data-set FCS3.1 file. Data set 1's $CYT contains the
        // space delimiter; data set 2 is plain.
        write_two_dataset_fixture(&path, Version::V3_1, "BD LSRFortessa X-20");

        let datasets = Fcs::open_all(path.to_str().expect("utf-8")).expect("open_all");
        assert_eq!(datasets.len(), 2, "both data sets must be reachable");

        assert_eq!(
            datasets[0].metadata.get_string_keyword("$CYT").expect("$CYT").get_str(),
            "BD LSRFortessa X-20",
            "data set 1's escaped value must round-trip"
        );
        assert_eq!(
            *datasets[1].metadata.get_number_of_events().expect("$TOT"),
            datasets[1].data_frame.height(),
            "data set 2's $TOT must match its decoded height — a desynchronized \
             find_begindata_offset would land on the wrong DATA bytes"
        );
    }
}
```

`write_two_dataset_fixture` is the helper identified in Step 1; adapt its signature to take a version and a `$COM` value. If the existing helpers are not reusable as-is, lift the nearest one into a shared `fn` in the same test module rather than duplicating it.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p flow-fcs nextdata_escaping_tests`
Expected: FAIL initially only if the tokenizers disagree. If it passes on the first run, that is a valid outcome — Task 2 unified them by construction, and this test is the regression lock that keeps them unified. Record which it was.

- [ ] **Step 4: Fix any disagreement, then verify**

Run: `cargo test -p flow-fcs`
Expected: PASS.

- [ ] **Step 5: Commit (propose)**

```bash
git add fcs/src/file.rs && git commit -m "test(fcs): pin tokenizer agreement across a \$NEXTDATA chain with escaping"
```

- [ ] **Step 6: Close the bead**

```bash
bd close flow-crates-1xb --reason="Escaping implemented in fcs/src/text.rs, gated at Version::V3_1 (3.1 is the first version forbidding empty values, which is what makes doubling decodable -- the corpus's 5 consecutive-delimiter runs are all FCS2.0 empty values). One shared tokenizer now serves both Metadata::from_text_segment and Fcs::find_begindata_offset. Writer errors on empty values under 3.1+ and rejects delimiters outside ASCII 1-126. Ten form-feed workarounds removed from write.rs tests; eight empty \$PnS writes removed. tru-ols workaround retirement is flow-crates-8px."
```

---

# Phase 3 — Harness and baseline

## Task 7: Portable corpus case plus a generated ~1M×20 synthetic case, then record the baseline

The corpus tops out at 50,000 events × 8 parameters of big-endian `int16` — nothing like the modern `$DATATYPE F`, 20-parameter, multi-million-event file Stage B targets, and committing one would be hundreds of megabytes. So it gets generated at bench setup, through `write.rs`, which Phase 2 just made trustworthy.

The recorded numbers here are the *only* evidence the Phase 4 rewrite worked. The original 8× finding was mis-attributed because `iter_batched` excludes setup-closure time; do not repeat that by measuring a closure that does more than the thing under test.

**Files:**
- Modify: `fcs/benches/lazy_column_access.rs` (whole file)

**Interfaces:**
- Consumes: `flow_fcs::corpus::path` from Task 1; the escaping writer from Task 4.
- Produces: a `synthetic_fcs(dir: &Path, n_events: usize, n_params: usize) -> PathBuf` helper local to the bench.

- [ ] **Step 1: Add the synthetic fixture generator**

The bench is a separate crate, so `Fcs::for_testing` (gated on `#[cfg(any(test, feature = "test-util"))]`) is out of reach without a self-referential dev-dependency, and `Fcs::new()` is not an option either — it calls `AccessWrapper::new("")`, i.e. `File::open("")`, which always returns `Err`.

So the generator seeds from a real corpus file, which `open()` guarantees is internally consistent, then replaces `metadata` and `data_frame` — both `pub`. Public API only, no new feature, no second compilation of the crate.

```rust
use flow_fcs::file::Fcs;
use flow_fcs::keyword::{IntegerKeyword, Keyword, MixedKeyword};
use flow_fcs::metadata::Metadata;
use flow_fcs::write::write_fcs_file;
use polars::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Generate a `$DATATYPE F`, little-endian FCS file of `n_events` x `n_params`
/// into `dir` and return its path.
///
/// The corpus has nothing above 50,000 x 8 int16, and a committed multi-million
/// event file would be hundreds of megabytes. Generating through `write.rs` is
/// only safe because flow-crates-1xb made the writer's delimiter handling
/// correct; before that, any free-text keyword silently truncated.
fn synthetic_fcs(dir: &Path, n_events: usize, n_params: usize) -> PathBuf {
    let path = dir.join(format!("synthetic_{n_events}x{n_params}.fcs"));
    if path.exists() {
        return path;
    }

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
    // just `metadata` and `data_frame` is sufficient.
    let seed = flow_fcs::corpus::path("int-10000_events_random.fcs");
    let mut fcs = Fcs::open(seed.to_str().expect("utf-8 corpus path")).expect("seed corpus file");

    let mut metadata = Metadata::new();
    metadata.insert_string_keyword("$BYTEORD".into(), "1,2,3,4".into());
    metadata.insert_string_keyword("$DATATYPE".into(), "F".into());
    metadata.insert_string_keyword("$MODE".into(), "L".into());
    metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
    metadata.insert_string_keyword("$CYT".into(), "flow-crates synthetic bench fixture".into());
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
```

Note the `$CYT` value contains spaces and the delimiter defaults to a space — so this generator only produces a valid file *because* Task 4 landed. That is deliberate: it makes the Phase 2 dependency load-bearing rather than notional.

- [ ] **Step 2: Verify the fixture is valid before benchmarking it**

Add this bench-local sanity check and call it once from each bench function's setup:

```rust
/// A benchmark against a malformed fixture measures nothing. Check the file
/// reopens with the shape we asked for before timing anything.
fn assert_fixture_shape(path: &Path, n_events: usize, n_params: usize) {
    let fcs = Fcs::open(path.to_str().expect("utf-8")).expect("reopen synthetic fixture");
    assert_eq!(fcs.data_frame.height(), n_events, "synthetic fixture event count");
    assert_eq!(fcs.data_frame.width(), n_params, "synthetic fixture parameter count");
}
```

Run: `cargo bench -p flow-fcs --bench lazy_column_access -- --test`
Expected: PASS. A failure here means the fixture generator is wrong, and every number after it would be meaningless.

- [ ] **Step 3: Rework the bench groups**

Keep the corpus case — `fcs2_int16_50000ev_8par_random.fcs` is worth measuring precisely because it is awkward: `$BYTEORD 4,3,2,1` with `$P1B 16 / $P1R 1024` forces both a byte swap and a range mask, so it exercises the general path rather than any zero-copy shortcut, and 50,000 × 8 = 400,000 sits exactly on the current threshold.

```rust
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

    // One column of twenty: the Stage B case. The Vec<Vec<f32>> intermediate
    // is 20x the size of the output here, which is the cost being removed.
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

    group.bench_function("all_twenty_columns", |bencher| {
        bencher.iter_batched(
            || Fcs::open(&path).expect("reopen for cold cache"),
            |fresh| {
                let names: Vec<String> = (1..=PARAMS).map(|p| format!("P{p}")).collect();
                let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                let cols = fresh.columns(&refs).expect("columns");
                black_box(cols.len());
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}
```

Register it: `criterion_group!(benches, bench_two_column_access, bench_full_materialization, bench_synthetic_column_access);`

Delete the stale doc comment at lines 47-52 of the current file — the "no uniform-width fast path / `#[cold]`" explanation is about to stop being true. Replace it with a pointer to the spec.

- [ ] **Step 4: Record the baseline**

Run: `cargo bench -p flow-fcs --bench lazy_column_access 2>&1 | tee /tmp/fcs-3si-baseline.txt`
Expected: completes; note the mean time for each of `two_column_access/lazy_columns_uncached`, `full_materialization/events_uncached`, `synthetic_1Mx20/one_column_of_twenty`, `synthetic_1Mx20/all_twenty_columns`.

Attach them to the bead. Substitute the real numbers:

```bash
bd update flow-crates-3si --notes="BASELINE (pre-rewrite), $(uname -m) $(uname -s), cargo bench --bench lazy_column_access:
  two_column_access/lazy_columns_uncached  = <X> ms
  full_materialization/events_uncached     = <X> ms
  synthetic_1Mx20/one_column_of_twenty     = <X> ms
  synthetic_1Mx20/all_twenty_columns       = <X> ms
Criterion's own before/after comparison is also on disk in target/criterion/."
```

- [ ] **Step 5: Commit (propose)**

```bash
git add fcs/benches/lazy_column_access.rs && git commit -m "bench(fcs): add generated 1Mx20 float fixture, portable corpus case"
```

---

# Phase 4 — The decode rewrite (`flow-crates-3si`)

## Task 8: `Decoder` — resolve the type dispatch once per column

Today `Fcs::parse_parameter_value_to_f32` is called once per *value* and re-runs its `match (data_type, bytes_per_param)` every time — and it is `#[cold]`, which was correct when it was a rare fallback and is a pessimization by construction in a primary decode loop. Whether a column *can* decode is fixed for the whole file by its `(datatype, width, byteorder)` triple, so resolve it once.

**Files:**
- Create: `fcs/src/decode.rs`
- Modify: `fcs/src/lib.rs` (module list)
- Test: `fcs/src/decode.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::datatype::FcsDataType`, `crate::byteorder::ByteOrder`.
- Produces:
  - `pub(crate) enum Decoder { U16Le, U16Be, U32Le, U32Be, F32Le, F32Be, F64Le, F64Be }`, `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`
  - `pub(crate) fn Decoder::resolve(data_type: FcsDataType, bytes_per_param: usize, byte_order: &ByteOrder) -> anyhow::Result<Decoder>`
  - `pub(crate) fn Decoder::read(self, bytes: &[u8]) -> f32` — `#[inline(always)]`, infallible, reads exactly `bytes[..width]`

- [ ] **Step 1: Write the failing test**

Create `fcs/src/decode.rs`:

```rust
//! Per-column value decoders for byte-aligned FCS DATA.
//!
//! `(datatype, width, byteorder)` is fixed for a whole column by its metadata,
//! so the dispatch belongs at column-resolution time, not per value. Resolving
//! once also moves all fallibility out of the inner loop: `resolve` runs
//! `wanted.len()` times and can fail; `read` runs `num_events * wanted.len()`
//! times and cannot.

#[cfg(test)]
mod tests {
    use super::Decoder;
    use crate::byteorder::ByteOrder;
    use crate::datatype::FcsDataType;

    #[test]
    fn resolves_the_eight_legal_combinations() {
        let cases = [
            (FcsDataType::I, 2, ByteOrder::LittleEndian, Decoder::U16Le),
            (FcsDataType::I, 2, ByteOrder::BigEndian, Decoder::U16Be),
            (FcsDataType::I, 4, ByteOrder::LittleEndian, Decoder::U32Le),
            (FcsDataType::I, 4, ByteOrder::BigEndian, Decoder::U32Be),
            (FcsDataType::F, 4, ByteOrder::LittleEndian, Decoder::F32Le),
            (FcsDataType::F, 4, ByteOrder::BigEndian, Decoder::F32Be),
            (FcsDataType::D, 8, ByteOrder::LittleEndian, Decoder::F64Le),
            (FcsDataType::D, 8, ByteOrder::BigEndian, Decoder::F64Be),
        ];
        for (data_type, width, order, expected) in cases {
            let got = Decoder::resolve(data_type, width, &order)
                .unwrap_or_else(|e| panic!("{data_type:?}/{width}/{order:?}: {e}"));
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn rejects_unsupported_combinations_before_touching_bytes() {
        let order = ByteOrder::LittleEndian;
        let cases = [
            (FcsDataType::I, 3usize, "Unsupported integer size"),
            (FcsDataType::F, 8, "Invalid float32 size"),
            (FcsDataType::D, 4, "Invalid float64 size"),
            (FcsDataType::A, 1, "ASCII data type not supported"),
        ];
        for (data_type, width, expected) in cases {
            let err = Decoder::resolve(data_type, width, &order)
                .expect_err("should reject")
                .to_string();
            assert!(
                err.contains(expected),
                "error for {data_type:?}/{width} should contain {expected:?}, got: {err}"
            );
        }
    }

    #[test]
    fn reads_little_and_big_endian_integers() {
        assert_eq!(Decoder::U16Le.read(&[0x34, 0x12]), 0x1234_u16 as f32);
        assert_eq!(Decoder::U16Be.read(&[0x12, 0x34]), 0x1234_u16 as f32);
        assert_eq!(Decoder::U32Le.read(&[0x78, 0x56, 0x34, 0x12]), 0x1234_5678_u32 as f32);
        assert_eq!(Decoder::U32Be.read(&[0x12, 0x34, 0x56, 0x78]), 0x1234_5678_u32 as f32);
    }

    #[test]
    fn reads_little_and_big_endian_floats() {
        let value = 1234.5678_f32;
        assert_eq!(Decoder::F32Le.read(&value.to_le_bytes()), value);
        assert_eq!(Decoder::F32Be.read(&value.to_be_bytes()), value);

        let wide = 1234.5678_f64;
        assert_eq!(Decoder::F64Le.read(&wide.to_le_bytes()), wide as f32);
        assert_eq!(Decoder::F64Be.read(&wide.to_be_bytes()), wide as f32);
    }

    /// The rewrite must be bit-exact against the function it replaces, or
    /// every downstream analysis silently shifts.
    #[test]
    fn matches_parse_parameter_value_to_f32_bit_for_bit() {
        use crate::file::Fcs;
        let bytes: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        let cases = [
            (FcsDataType::I, 2usize),
            (FcsDataType::I, 4),
            (FcsDataType::F, 4),
            (FcsDataType::D, 8),
        ];
        for order in [ByteOrder::LittleEndian, ByteOrder::BigEndian] {
            for (data_type, width) in cases {
                let expected =
                    Fcs::parse_parameter_value_to_f32(&bytes[..width], width, &data_type, &order)
                        .expect("reference decode");
                let got = Decoder::resolve(data_type, width, &order)
                    .expect("resolve")
                    .read(&bytes[..width]);
                assert_eq!(
                    got.to_bits(),
                    expected.to_bits(),
                    "{data_type:?}/{width}/{order:?} must match the reference bit-for-bit"
                );
            }
        }
    }
}
```

Register in `fcs/src/lib.rs`:

```rust
pub(crate) mod decode;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p flow-fcs decode::`
Expected: FAIL — compile error, `cannot find type 'Decoder' in module 'super'`.

- [ ] **Step 3: Write the implementation**

Insert above the test module in `fcs/src/decode.rs`. Use the `byteorder` crate readers, matching `file.rs`'s idiom — this is also what makes bit-exact parity hold by construction rather than by inspection:

```rust
use crate::byteorder::ByteOrder;
use crate::datatype::FcsDataType;
use anyhow::{Result, anyhow};
use byteorder::{BigEndian as BE, ByteOrder as BO, LittleEndian as LE};

/// One resolved `(datatype, width, byteorder)` combination.
///
/// `Copy` and one byte wide, so a `ColumnPlan` holding one costs nothing to
/// pass into a parallel closure. The eight variants are every combination FCS
/// permits for byte-aligned data; bit-packed records take a different path
/// entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decoder {
    U16Le,
    U16Be,
    U32Le,
    U32Be,
    F32Le,
    F32Be,
    F64Le,
    F64Be,
}

impl Decoder {
    /// Resolve a column's decoder from its metadata, once.
    ///
    /// Error messages are reproduced verbatim from
    /// `Fcs::parse_parameter_value_to_f32`, which this replaces in the column
    /// path — a caller matching on the old text keeps working.
    ///
    /// # Errors
    /// Returns `Err` for any `(data_type, bytes_per_param)` pair FCS does not
    /// define, including any use of `$DATATYPE A`.
    pub(crate) fn resolve(
        data_type: FcsDataType,
        bytes_per_param: usize,
        byte_order: &ByteOrder,
    ) -> Result<Self> {
        let little = matches!(byte_order, ByteOrder::LittleEndian);
        Ok(match (data_type, bytes_per_param) {
            (FcsDataType::I, 2) => if little { Self::U16Le } else { Self::U16Be },
            (FcsDataType::I, 4) => if little { Self::U32Le } else { Self::U32Be },
            (FcsDataType::F, 4) => if little { Self::F32Le } else { Self::F32Be },
            (FcsDataType::D, 8) => if little { Self::F64Le } else { Self::F64Be },
            (FcsDataType::I, _) => {
                return Err(anyhow!(
                    "Unsupported integer size: {} bytes (expected 2 or 4)",
                    bytes_per_param
                ));
            }
            (FcsDataType::F, _) => {
                return Err(anyhow!(
                    "Invalid float32 size: {} bytes (expected 4)",
                    bytes_per_param
                ));
            }
            (FcsDataType::D, _) => {
                return Err(anyhow!(
                    "Invalid float64 size: {} bytes (expected 8)",
                    bytes_per_param
                ));
            }
            (FcsDataType::A, _) => return Err(anyhow!("ASCII data type not supported")),
        })
    }

    /// Decode one value. Infallible by construction: [`resolve`](Self::resolve)
    /// already proved the combination is legal, and the caller slices exactly
    /// the declared width.
    ///
    /// `bytes` must be at least as long as this decoder's width; callers pass
    /// `&event[offset..offset + width]`.
    #[inline(always)]
    pub(crate) fn read(self, bytes: &[u8]) -> f32 {
        match self {
            Self::U16Le => LE::read_u16(bytes) as f32,
            Self::U16Be => BE::read_u16(bytes) as f32,
            Self::U32Le => LE::read_u32(bytes) as f32,
            Self::U32Be => BE::read_u32(bytes) as f32,
            Self::F32Le => LE::read_f32(bytes),
            Self::F32Be => BE::read_f32(bytes),
            Self::F64Le => LE::read_f64(bytes) as f32,
            Self::F64Be => BE::read_f64(bytes) as f32,
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p flow-fcs decode::`
Expected: PASS, 5 tests. `matches_parse_parameter_value_to_f32_bit_for_bit` is the one that matters.

- [ ] **Step 5: Commit (propose)**

```bash
git add fcs/src/decode.rs fcs/src/lib.rs && git commit -m "feat(fcs): add per-column Decoder resolved once per column"
```

---

## Task 9: `ColumnPlan` and `fill_events`, sequential path

**Files:**
- Modify: `fcs/src/columns.rs:110-182` (imports and `extract_columns`)
- Test: `fcs/src/columns.rs` (extend the inline `mod tests`)

**Interfaces:**
- Consumes: `crate::decode::Decoder` from Task 8; `ColumnLayout` and `apply_range_mask`, already in `columns.rs`.
- Produces:
  - `struct ColumnPlan { offset: usize, width: usize, decoder: Decoder, mask: Option<u32> }`, `#[derive(Debug, Clone, Copy)]`
  - `fn build_plans(layout: &ColumnLayout, wanted: &[usize]) -> Result<Vec<ColumnPlan>>`
  - `fn fill_events(event_bytes: &[u8], bytes_per_event: usize, plans: &[ColumnPlan], outs: &mut [&mut [f32]])`
  - `extract_columns` keeps its existing signature: `(data_bytes: &[u8], layout: &ColumnLayout, wanted: &[usize]) -> Result<Vec<Box<[f32]>>>`

- [ ] **Step 1: Write the failing test**

Add to `fcs/src/columns.rs`'s `mod tests`. The existing tests only cover little-endian uniform f32 — no big-endian, no mixed widths, and nothing that would catch a transposition.

```rust
    /// 2 events x 3 params, big-endian, `$DATATYPE I`, widths [8, 2, 4] bytes
    /// (`$PnB` 64, 16, 32). Distinct values per column so a transposition
    /// cannot pass. `$PnB 64` for an integer is not a legal decode width, so
    /// param 0 is only ever requested to prove `resolve` rejects it.
    fn synthetic_metadata_be_mixed_widths() -> Metadata {
        let mut metadata = synthetic_metadata_varying_widths();
        metadata.keywords.insert(
            "$BYTEORD".to_string(),
            Keyword::Byte(ByteKeyword::BYTEORD(ByteOrder::BigEndian)),
        );
        metadata.keywords.insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(2)));
        metadata
    }

    #[test]
    fn extract_columns_decodes_big_endian_mixed_widths() {
        let metadata = synthetic_metadata_be_mixed_widths();
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");
        assert_eq!(layout.bytes_per_parameter, vec![8, 2, 4]);

        // Event 0: p0 = 8 filler bytes, p1 = 0x0102 BE, p2 = 0x00030405 BE.
        // Event 1: p0 = 8 filler bytes, p1 = 0x0607 BE, p2 = 0x00080909 BE.
        let mut data_bytes = Vec::new();
        data_bytes.extend_from_slice(&[0u8; 8]);
        data_bytes.extend_from_slice(&0x0102u16.to_be_bytes());
        data_bytes.extend_from_slice(&0x0003_0405u32.to_be_bytes());
        data_bytes.extend_from_slice(&[0u8; 8]);
        data_bytes.extend_from_slice(&0x0607u16.to_be_bytes());
        data_bytes.extend_from_slice(&0x0008_0909u32.to_be_bytes());

        let columns = super::extract_columns(&data_bytes, &layout, &[1, 2]).expect("extract");
        assert_eq!(
            &*columns[0],
            &[0x0102u16 as f32, 0x0607u16 as f32],
            "param 1 must be read big-endian at running-sum offset 8"
        );
        assert_eq!(
            &*columns[1],
            &[0x0003_0405u32 as f32, 0x0008_0909u32 as f32],
            "param 2 must be read big-endian at running-sum offset 10"
        );
    }

    #[test]
    fn extract_columns_rejects_an_illegal_width_before_decoding() {
        let metadata = synthetic_metadata_be_mixed_widths();
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");

        // Param 0 is $PnB 64 with $DATATYPE I — 8-byte integers are not a
        // legal FCS decode width. Pass an empty slice: resolution must fail
        // before any byte is touched, so the short slice is never reached.
        let err = super::extract_columns(&[], &layout, &[0]).unwrap_err().to_string();
        assert!(
            err.contains("Unsupported integer size"),
            "resolution must reject the width before decoding, got: {err}"
        );
    }

    #[test]
    fn extract_columns_preserves_column_identity_across_many_events() {
        // A transposition bug produces plausible-looking output when every
        // column holds the same values. Give each column a distinct offset.
        let mut metadata = synthetic_metadata_2f32();
        metadata.keywords.insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(1000)));
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");

        let mut data_bytes = Vec::with_capacity(1000 * 8);
        for e in 0..1000u32 {
            data_bytes.extend_from_slice(&(e as f32).to_le_bytes());
            data_bytes.extend_from_slice(&(e as f32 + 10_000.0).to_le_bytes());
        }

        let columns = super::extract_columns(&data_bytes, &layout, &[0, 1]).expect("extract");
        assert_eq!(columns[0].len(), 1000);
        assert_eq!(columns[1].len(), 1000);
        for e in 0..1000 {
            assert_eq!(columns[0][e], e as f32, "column 0, event {e}");
            assert_eq!(columns[1][e], e as f32 + 10_000.0, "column 1, event {e}");
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p flow-fcs columns::tests`
Expected: FAIL. `extract_columns_rejects_an_illegal_width_before_decoding` fails with the length-guard error (`data segment (0 bytes) is shorter than…`) instead of the width error, because today's guard runs before any type resolution.

- [ ] **Step 3: Write the implementation**

Replace `fcs/src/columns.rs` lines 110-182 (from the `use crate::file::Fcs;` block through the end of `extract_columns`):

```rust
use crate::decode::Decoder;
use anyhow::anyhow;
use rayon::prelude::*;

/// Everything the inner decode loop needs for one requested column,
/// precomputed. `Copy` so a slice of these can cross into a rayon closure
/// without a clone.
#[derive(Debug, Clone, Copy)]
struct ColumnPlan {
    /// Byte offset of this parameter within one event record.
    offset: usize,
    /// Byte width of this parameter.
    width: usize,
    /// Resolved decoder — the `(datatype, width, byteorder)` dispatch, done once.
    decoder: Decoder,
    /// `$PnR`-derived integer range mask, or `None`.
    mask: Option<u32>,
}

/// Resolve one plan per requested parameter index.
///
/// This is where every error in the column path now lives: it runs
/// `wanted.len()` times, not `num_events * wanted.len()` times, so the decode
/// loop below carries no `Result` at all.
///
/// # Errors
/// Returns `Err` if an index is out of range for the layout, or if a
/// parameter's `(datatype, width)` pair is not a legal FCS decode combination.
fn build_plans(layout: &ColumnLayout, wanted: &[usize]) -> Result<Vec<ColumnPlan>> {
    wanted
        .iter()
        .map(|&idx| {
            let offset = *layout.param_offsets.get(idx).ok_or_else(|| {
                anyhow!(
                    "parameter index {idx} out of range for a layout with {} parameters",
                    layout.param_offsets.len()
                )
            })?;
            let width = layout.bytes_per_parameter[idx];
            let decoder = Decoder::resolve(layout.data_types[idx], width, &layout.byte_order)?;
            Ok(ColumnPlan { offset, width, decoder, mask: layout.range_masks[idx] })
        })
        .collect()
}

/// Decode `event_bytes` into `outs`, one value per plan per event.
///
/// Infallible: `build_plans` already proved every decoder legal, and the
/// caller sized `event_bytes` to a whole number of events. `outs[c].len()`
/// must equal the number of events in `event_bytes`.
fn fill_events(
    event_bytes: &[u8],
    bytes_per_event: usize,
    plans: &[ColumnPlan],
    outs: &mut [&mut [f32]],
) {
    for (event_index, event) in event_bytes.chunks_exact(bytes_per_event).enumerate() {
        for (plan, out) in plans.iter().zip(outs.iter_mut()) {
            let raw = plan.decoder.read(&event[plan.offset..plan.offset + plan.width]);
            out[event_index] = apply_range_mask(raw, plan.mask);
        }
    }
}

/// Decode the requested parameter indices from row-major FCS event bytes in
/// a single pass over `data_bytes`. Extracting 1 column and extracting all of
/// them cost the same per-event traversal — the DATA segment is interleaved,
/// so there's no strided-vs-sequential choice to make, only how many values
/// to keep per event. Callers should batch every column they need into one
/// call rather than calling this once per column.
///
/// # Errors
/// Returns `Err` if `layout.is_bit_packed` (bit-packed records aren't
/// byte-aligned, so this stride-based traversal can't represent them — use
/// the existing `parse_bit_packed_data` path instead), if a requested index is
/// out of range, if a parameter's declared type/width pair isn't decodable, or
/// if `data_bytes` is shorter than the layout requires.
pub(crate) fn extract_columns(
    data_bytes: &[u8],
    layout: &ColumnLayout,
    wanted: &[usize],
) -> Result<Vec<Box<[f32]>>> {
    if layout.is_bit_packed {
        return Err(anyhow!(
            "bit-packed FCS records don't support lazy single-column access; call events() instead"
        ));
    }

    // Resolve before the length guard: an illegal $PnB is a metadata error and
    // should be reported as one even when the DATA segment is also wrong.
    let plans = build_plans(layout, wanted)?;

    let total_event_bytes = layout.num_events * layout.bytes_per_event;
    let event_bytes = data_bytes.get(..total_event_bytes).ok_or_else(|| {
        anyhow!(
            "data segment ({} bytes) is shorter than {} events x {} bytes/event",
            data_bytes.len(),
            layout.num_events,
            layout.bytes_per_event
        )
    })?;

    // `vec![0.0f32; n]` lowers to `alloc_zeroed` via the `IsZero`
    // specialization — untouched zero pages, not a memset, so there is no
    // reason to reach for `MaybeUninit` and no unsafe.
    let mut columns: Vec<Vec<f32>> = plans.iter().map(|_| vec![0.0f32; layout.num_events]).collect();

    extract_columns_inner(
        event_bytes,
        layout.bytes_per_event,
        &plans,
        &mut columns,
        total_event_bytes >= PARALLEL_BYTE_THRESHOLD,
    );

    Ok(columns.into_iter().map(Vec::into_boxed_slice).collect())
}
```

Add the sequential half of `extract_columns_inner` for now; Task 10 adds the parallel half:

```rust
/// Split out so tests can pin either branch deterministically instead of
/// relying on a fixture large enough to cross the threshold.
fn extract_columns_inner(
    event_bytes: &[u8],
    bytes_per_event: usize,
    plans: &[ColumnPlan],
    columns: &mut [Vec<f32>],
    parallel: bool,
) {
    let _ = parallel; // Task 10 wires up the parallel branch.
    let mut outs: Vec<&mut [f32]> = columns.iter_mut().map(Vec::as_mut_slice).collect();
    fill_events(event_bytes, bytes_per_event, plans, &mut outs);
}
```

Add the threshold constant near the top of `columns.rs`, after the module doc comment:

```rust
/// Byte count above which `extract_columns` decodes in parallel.
///
/// Deliberately distinct from [`crate::file::PARALLEL_THRESHOLD`], which
/// counts *values* and was tuned for the eager `parse_uniform_data_bulk` loop.
/// This traversal walks the whole DATA segment regardless of how many columns
/// are kept — `wanted.len()` only affects how many values are *stored* per
/// event, not how many bytes are stepped over. One column of a 300,000-event x
/// 40-parameter file scores 300,000 under the value-count predicate, stays
/// sequential, and still walks 48 MB.
///
/// Initial value; confirmed against `benches/lazy_column_access.rs` in
/// `flow-crates-3si`.
const PARALLEL_BYTE_THRESHOLD: usize = 1 << 20; // 1 MiB
```

Delete the now-unused `use crate::file::Fcs;` import if nothing else in the module needs it.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p flow-fcs columns::`
Expected: PASS, including the three pre-existing decode tests and the three new ones.

- [ ] **Step 5: Verify `parse_parameter_value_to_f32`'s `#[cold]` is now accurate**

Run: `rg -n "parse_parameter_value_to_f32" --type rust`
Expected: exactly two hits outside its own definition and tests — the definition at `fcs/src/file.rs:1452` and the call inside `parse_variable_width_data` at `fcs/src/file.rs:1547`. Since `parse_variable_width_data` is itself `#[cold]`, the annotation is now correct by subtraction. **Do not remove it** and do not add a non-cold variant; both options the bead offered are now unnecessary.

- [ ] **Step 6: Run the full suite**

Run: `cargo test -p flow-fcs`
Expected: PASS. `lazy_column_tests::column_matches_data_frame_oracle` is the load-bearing one — it compares the lazy path against the eager `data_frame` on a real corpus file.

- [ ] **Step 7: Commit (propose)**

```bash
git add fcs/src/columns.rs && git commit -m "perf(fcs): decode columns via per-column plans, no per-event Vec"
```

---

## Task 10: Parallel branch via `split_at_mut`, driven by bytes walked

**Files:**
- Modify: `fcs/src/columns.rs` (`extract_columns_inner`)
- Test: `fcs/src/columns.rs` (extend `mod tests`)

**Interfaces:**
- Consumes: `ColumnPlan`, `fill_events`, `extract_columns_inner`, `PARALLEL_BYTE_THRESHOLD` from Task 9.
- Produces: no new public names. `extract_columns_inner`'s `parallel: bool` becomes live.

- [ ] **Step 1: Write the failing test**

The parallel branch has **zero** coverage today. Two tests are needed: one that pins the branch directly, and one that genuinely crosses the threshold so the predicate itself is covered.

```rust
    #[test]
    fn both_branches_produce_identical_output() {
        let mut metadata = synthetic_metadata_2f32();
        metadata.keywords.insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(5_000)));
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");

        let mut data_bytes = Vec::with_capacity(5_000 * 8);
        for e in 0..5_000u32 {
            data_bytes.extend_from_slice(&(e as f32).to_le_bytes());
            data_bytes.extend_from_slice(&(e as f32 * -2.0).to_le_bytes());
        }
        let plans = super::build_plans(&layout, &[0, 1]).expect("plans");

        let run = |parallel: bool| {
            let mut columns: Vec<Vec<f32>> = vec![vec![0.0f32; 5_000]; 2];
            super::extract_columns_inner(
                &data_bytes,
                layout.bytes_per_event,
                &plans,
                &mut columns,
                parallel,
            );
            columns
        };

        assert_eq!(
            run(false),
            run(true),
            "the parallel branch must produce byte-identical output to the sequential one"
        );
    }

    #[test]
    fn crossing_the_byte_threshold_selects_the_parallel_branch_and_stays_correct() {
        // PARALLEL_BYTE_THRESHOLD is 1 MiB; 2 params x 4 bytes = 8 bytes/event,
        // so 200_000 events is 1.6 MB — comfortably over, and small enough to
        // stay a fast test.
        const EVENTS: usize = 200_000;
        let mut metadata = synthetic_metadata_2f32();
        metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(EVENTS)));
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");
        assert!(
            EVENTS * layout.bytes_per_event >= super::PARALLEL_BYTE_THRESHOLD,
            "fixture must actually cross the threshold, or this test proves nothing"
        );

        let mut data_bytes = Vec::with_capacity(EVENTS * 8);
        for e in 0..EVENTS {
            data_bytes.extend_from_slice(&(e as f32).to_le_bytes());
            data_bytes.extend_from_slice(&(e as f32 + 0.5).to_le_bytes());
        }

        let columns = super::extract_columns(&data_bytes, &layout, &[0, 1]).expect("extract");
        assert_eq!(columns[0].len(), EVENTS);
        // Spot-check the two chunk boundaries most likely to be wrong: the
        // first event of the second chunk, and the last event overall.
        assert_eq!(columns[0][0], 0.0);
        assert_eq!(columns[1][0], 0.5);
        assert_eq!(columns[0][8_192], 8_192.0, "first event of the second chunk");
        assert_eq!(columns[1][8_192], 8_192.5, "first event of the second chunk");
        assert_eq!(columns[0][EVENTS - 1], (EVENTS - 1) as f32, "last event");
        assert_eq!(columns[1][EVENTS - 1], (EVENTS - 1) as f32 + 0.5, "last event");
    }

    #[test]
    fn a_ragged_final_chunk_is_still_decoded() {
        // EVENTS_PER_CHUNK is 8_192; 8_193 events leaves a 1-event tail, which
        // is the shape most likely to be dropped by an off-by-one in the
        // split_at_mut walk.
        const EVENTS: usize = 8_193;
        let mut metadata = synthetic_metadata_2f32();
        metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(EVENTS)));
        let layout = super::ColumnLayout::from_metadata(&metadata).expect("layout");

        let mut data_bytes = Vec::with_capacity(EVENTS * 8);
        for e in 0..EVENTS {
            data_bytes.extend_from_slice(&(e as f32).to_le_bytes());
            data_bytes.extend_from_slice(&(e as f32).to_le_bytes());
        }
        let plans = super::build_plans(&layout, &[0]).expect("plans");
        let mut columns: Vec<Vec<f32>> = vec![vec![f32::NAN; EVENTS]];
        super::extract_columns_inner(
            &data_bytes,
            layout.bytes_per_event,
            &plans,
            &mut columns,
            true,
        );

        assert_eq!(columns[0][EVENTS - 1], (EVENTS - 1) as f32, "ragged tail event");
        assert!(
            columns[0].iter().all(|v| !v.is_nan()),
            "every slot must have been written; a NaN means a chunk was skipped"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p flow-fcs columns::tests::both_branches columns::tests::crossing columns::tests::a_ragged`
Expected: `both_branches_produce_identical_output` and `crossing_...` PASS trivially (the `parallel` flag is currently ignored, so both calls run the same code) and `a_ragged_final_chunk_is_still_decoded` PASSes too. **This is expected and is why Step 4 exists** — these tests only become meaningful once the branch is real. Note that they pass now, so a failure after Step 3 is unambiguous.

- [ ] **Step 3: Write the implementation**

Replace `extract_columns_inner` in `fcs/src/columns.rs`:

```rust
/// Events per parallel task. Large enough that per-task overhead is
/// negligible against 8 KiB-plus of decoding, small enough that a 20-column
/// request still produces enough tasks for rayon to balance across cores.
const EVENTS_PER_CHUNK: usize = 8_192;

/// Split out so tests can pin either branch deterministically instead of
/// relying on a fixture large enough to cross the threshold.
fn extract_columns_inner(
    event_bytes: &[u8],
    bytes_per_event: usize,
    plans: &[ColumnPlan],
    columns: &mut [Vec<f32>],
    parallel: bool,
) {
    if !parallel {
        let mut outs: Vec<&mut [f32]> = columns.iter_mut().map(Vec::as_mut_slice).collect();
        fill_events(event_bytes, bytes_per_event, plans, &mut outs);
        return;
    }

    // Peel chunk-sized `&mut` sub-slices off every output column in lockstep
    // with the event bytes. `split_at_mut` is what makes this safe Rust: each
    // task owns a disjoint window of every column, proven by the borrow
    // checker rather than asserted. Allocation is O(chunks * columns)
    // references — nothing proportional to events, which is the whole point.
    let mut tasks: Vec<(&[u8], Vec<&mut [f32]>)> = Vec::new();
    let mut rest_bytes = event_bytes;
    let mut rest_outs: Vec<&mut [f32]> = columns.iter_mut().map(Vec::as_mut_slice).collect();

    while !rest_bytes.is_empty() {
        let take = EVENTS_PER_CHUNK.min(rest_bytes.len() / bytes_per_event);
        if take == 0 {
            break;
        }
        let (chunk_bytes, tail_bytes) = rest_bytes.split_at(take * bytes_per_event);

        let mut chunk_outs = Vec::with_capacity(rest_outs.len());
        let mut tail_outs = Vec::with_capacity(rest_outs.len());
        for out in rest_outs.drain(..) {
            let (head, tail) = out.split_at_mut(take);
            chunk_outs.push(head);
            tail_outs.push(tail);
        }

        tasks.push((chunk_bytes, chunk_outs));
        rest_bytes = tail_bytes;
        rest_outs = tail_outs;
    }

    tasks.into_par_iter().for_each(|(bytes, mut outs)| {
        fill_events(bytes, bytes_per_event, plans, &mut outs);
    });
}
```

- [ ] **Step 4: Run the tests to verify they still pass, now meaningfully**

Run: `cargo test -p flow-fcs columns::`
Expected: PASS. `both_branches_produce_identical_output` now genuinely compares two code paths, and `a_ragged_final_chunk_is_still_decoded` genuinely exercises the `split_at_mut` walk with a 1-event tail.

- [ ] **Step 5: Verify no `unsafe` was introduced**

Run: `rg -n "unsafe" fcs/src/columns.rs fcs/src/decode.rs`
Expected: no output.

- [ ] **Step 6: Run the full suite under the thread sanitizer's cheap substitute**

Run: `cargo test -p flow-fcs --release`
Expected: PASS. Release mode is where a data race or an aliasing mistake in the parallel branch is most likely to surface as wrong output rather than a panic.

- [ ] **Step 7: Commit (propose)**

```bash
git add fcs/src/columns.rs && git commit -m "perf(fcs): parallel column decode via split_at_mut, byte-driven threshold"
```

---

## Task 11: Widen the equivalence oracle to every parameter of every corpus file

`column_matches_data_frame_oracle` currently checks parameter `[0]` of one file. That is one column of one file guarding a rewrite of the entire byte-aligned decode path.

**Files:**
- Modify: `fcs/src/file.rs` (`mod lazy_column_tests`, the `column_matches_data_frame_oracle` test)

**Interfaces:**
- Consumes: `crate::corpus::{files, is_available}` from Task 1; the rewritten `extract_columns` from Tasks 9-10.
- Produces: nothing new.

- [ ] **Step 1: Write the widened test**

Replace `column_matches_data_frame_oracle` in `fcs/src/file.rs`:

```rust
    /// The lazy per-column path and the eager `data_frame` path decode the
    /// same bytes through different code. They must agree bit-for-bit on every
    /// parameter of every corpus file — this is the only equivalence net the
    /// column decode rewrite has, and it used to cover one column of one file.
    ///
    /// Bit comparison, not `==`: a NaN in the data would make `==` pass
    /// vacuously, and `-0.0 == 0.0` would hide a sign error.
    #[test]
    fn column_matches_data_frame_oracle() {
        if !crate::corpus::is_available() {
            eprintln!("compliance corpus missing, skipping");
            return;
        }

        let mut checked = 0usize;
        for path in crate::corpus::files() {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let Ok(fcs) = Fcs::open(path.to_str().expect("utf-8 path")) else {
                // Some corpus files exist to exercise reader errors.
                continue;
            };

            for channel in fcs.get_parameter_names_from_dataframe() {
                let Ok(eager) = fcs.get_parameter_events_slice(&channel) else { continue };
                let lazy = match fcs.column(&channel) {
                    Ok(lazy) => lazy,
                    Err(e) => {
                        // Bit-packed files legitimately refuse the lazy path.
                        assert!(
                            e.to_string().contains("bit-packed"),
                            "{name}/{channel}: column() failed for an unexpected reason: {e}"
                        );
                        continue;
                    }
                };

                assert_eq!(
                    lazy.len(), eager.len(),
                    "{name}/{channel}: lazy and eager lengths differ"
                );
                for (event, (l, e)) in lazy.iter().zip(eager.iter()).enumerate() {
                    assert_eq!(
                        l.to_bits(), e.to_bits(),
                        "{name}/{channel}/event {event}: lazy {l} != eager {e}"
                    );
                }
                checked += 1;
            }
        }

        assert!(
            checked >= 8,
            "the oracle must actually compare something; only {checked} channels were checked"
        );
    }
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p flow-fcs lazy_column_tests::column_matches_data_frame_oracle -- --nocapture`
Expected: PASS, with `checked` well above 8 (the corpus has 10 files of 6-8 parameters each).

If it FAILS, the rewrite has a real decode discrepancy. The `{name}/{channel}/event {n}` message names the exact file, column and event — start there, not with a guess.

- [ ] **Step 3: Run the full suite in both profiles**

Run: `cargo test -p flow-fcs && cargo test -p flow-fcs --release`
Expected: PASS.

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -p flow-fcs --all-targets -- -D warnings`
Expected: no warnings. If `build_plans`'s closure trips `clippy::needless_range_loop` or similar, fix the lint rather than allowing it.

- [ ] **Step 5: Commit (propose)**

```bash
git add fcs/src/file.rs && git commit -m "test(fcs): widen lazy/eager oracle to every parameter of every corpus file"
```

---

# Phase 5 — Re-measure and close

## Task 12: Confirm the threshold, record before/after, close the beads

**Files:**
- Modify: `fcs/src/columns.rs` (`PARALLEL_BYTE_THRESHOLD` value and its doc comment, if the sweep says so)
- Modify: `fcs/benches/lazy_column_access.rs` (doc comment recording the result)

**Interfaces:**
- Consumes: the harness from Task 7, the rewrite from Tasks 8-11.
- Produces: nothing.

- [ ] **Step 1: Re-run the harness**

Run: `cargo bench -p flow-fcs --bench lazy_column_access 2>&1 | tee /tmp/fcs-3si-after.txt`
Expected: completes. Criterion prints a change-vs-baseline line for each benchmark, since Task 7's run populated `target/criterion/`.

- [ ] **Step 2: Sweep the parallel threshold**

The spec requires the crossover be chosen from measurement, not asserted. Try three values by editing `PARALLEL_BYTE_THRESHOLD` in `fcs/src/columns.rs` and re-running only the synthetic group:

```bash
cargo bench -p flow-fcs --bench lazy_column_access -- synthetic_1Mx20
```

Values to try: `256 * 1024`, `1 << 20` (the Task 9 default), `4 * 1024 * 1024`. Also re-run the corpus group at each, since `fcs2_int16_50000ev_8par_random.fcs` is 400,000 values × 2 bytes = 800 KB and straddles the first two candidates:

```bash
cargo bench -p flow-fcs --bench lazy_column_access -- two_column_access
```

Keep the value that is fastest or tied-fastest on both groups; prefer the larger one on a tie, since a needless rayon fan-out costs latency on small files where it buys nothing. Update the constant's doc comment to record the measured crossover and drop the "initial value" caveat.

- [ ] **Step 3: Update the bench doc comment**

Replace the stale Task 8 explanation block in `fcs/benches/lazy_column_access.rs` with the measured outcome:

```rust
/// Measured for `flow-crates-3si`. The pre-rewrite `events_uncached` ran ~8x
/// slower than `open_eager_baseline`, and the explanation recorded at the time
/// — "it pays for open()'s parse too" — was wrong: criterion's `iter_batched`
/// excludes setup-closure time. The real causes were one `Vec` allocation per
/// event, a `Result` per value, and a `#[cold]` call per value. See
/// `docs/superpowers/specs/2026-08-08-fcs-column-decode-and-delimiter-escaping-design.md`.
///
/// After the rewrite: <fill in the measured numbers from Step 1>.
```

- [ ] **Step 4: Record before/after on the bead**

Substitute the real numbers:

```bash
bd update flow-crates-3si --notes="AFTER (post-rewrite), same machine as the baseline:
  two_column_access/lazy_columns_uncached  = <X> ms (was <Y> ms, <Z>x)
  full_materialization/events_uncached     = <X> ms (was <Y> ms, <Z>x)
  synthetic_1Mx20/one_column_of_twenty     = <X> ms (was <Y> ms, <Z>x)
  synthetic_1Mx20/all_twenty_columns       = <X> ms (was <Y> ms, <Z>x)
PARALLEL_BYTE_THRESHOLD set to <N> from a 256KiB/1MiB/4MiB sweep across both the
synthetic and corpus groups.
parse_parameter_value_to_f32 left untouched: its only remaining caller is
parse_variable_width_data (file.rs:1547), which is itself #[cold], so the
annotation is accurate by subtraction.
Uniform-decoder monomorphization still deferred -- <state whether the numbers
justify revisiting it>."
```

- [ ] **Step 5: Final gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS with no warnings.

Run: `git status`
Expected: report the changed files to the user.

- [ ] **Step 6: Commit (propose)**

```bash
git add fcs/src/columns.rs fcs/benches/lazy_column_access.rs && git commit -m "perf(fcs): tune PARALLEL_BYTE_THRESHOLD from measurement, record results"
```

- [ ] **Step 7: Close the bead and check what unblocked**

```bash
bd close flow-crates-3si --suggest-next --reason="extract_columns rewritten: per-column Decoder resolved once (fcs/src/decode.rs), infallible inner loop, pre-sized alloc_zeroed output buffers, no Vec<Vec<f32>> intermediate, parallel branch via split_at_mut driven by PARALLEL_BYTE_THRESHOLD (bytes walked, not values kept). Parallel branch now has coverage; oracle widened from 1 column of 1 file to every parameter of every corpus file. Before/after in this issue's notes."
```

Expected: `flow-crates-8px` is reported as newly unblocked (it depends on `1xb`). It is out of scope for this plan — leave it open.

- [ ] **Step 8: Record what was learned**

```bash
bd remember "FCS TEXT delimiter escaping is only decodable from FCS3.1 onward, because 3.1 is the first version that forbids empty keyword values. All 5 runs of consecutive delimiters in the 10-file Gating-ML corpus are FCS2.0 empty values, not escapes -- real-8-parameters.data.fcs has '\\\\Comments\\\\\\\\Row\\\\2\\\\' where Comments genuinely has no value. Un-doubling unconditionally would make Comments absorb Row and shift every field after it. The gate lives in fcs/src/text.rs Escaping::for_version."
```

```bash
bd remember "Criterion's iter_batched EXCLUDES the setup closure from the measurement. A benchmark written as iter_batched(|| Fcs::open(p), |fcs| fcs.events()) does NOT include open()'s parse time. flow-crates-3si's original 8x finding was mis-attributed to 'two traversals' because of this; the real causes were one Vec alloc per event, a Result per value, and a #[cold] call per value. When a benchmark result looks like it includes setup cost, check which criterion helper is in use before theorizing."
```

---

## Out of scope

Named explicitly so no task quietly absorbs them:

- **`flow-crates-8px`** — retiring tru-ols's `SAFE_TEXT_DELIMITER`, `ensure_delimiter_survives_provenance` and the no-space constraint on `default_software_tag()`. It becomes unblocked when `1xb` closes, but removing it changes the delimiter of every file tru-ols emits, which is user-visible and deserves its own review.
- **`flow-crates-rkq`** — `extract_columns` errors only when `data_bytes` is too *short*, never when it is a superset, so a filtered clone silently decodes a truncated prefix. Task 9 touches the exact guard involved; leave the semantics as they are.
- **`flow-crates-lfg`** — the `$BYTEORD` silent downgrade lives in `serialize_metadata`, the same function Task 4 rewrites. Independent defect, own bead.
- **`flow-crates-yg8`** — `Fcs::new()` always returns `Err`, because it builds `file_access` as `AccessWrapper::new("")`. Filed while writing this plan (Task 7's generator originally used it). Zero callers today, so nothing here depends on the fix; Task 7 routes around it via `Fcs::open()` on a corpus file.
- **`flow-crates-zmx`** — the `VersionSpec` trait. Task 3's escaping gate is a plain `match` on `Version`, sited so `zmx` can absorb it later. Do not pre-abstract it.
- **Uniform-decoder monomorphization** — deferred pending Task 12's numbers. Once every column resolves to the same `Decoder`, the enum dispatch is a perfectly-predicted branch; a `ValueDecoder`-generic specialization is additive if the numbers demand it.
