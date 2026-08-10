# How flow-crates parses FCS files — a description for independent validation

**Purpose of this document.** Hand this to an agent or reviewer with no prior
context on the work. It describes what the FCS parser does *now*, on branch
`fcs-column-decode-and-delimiter-escaping` (`7228996..9fd2334`), what
edge-case decisions were made and why, and where the reasoning is load-bearing
enough to be worth attacking. It is a description to be *falsified*, not a
defence. Where a decision is a judgment call rather than a consequence of the
standard, it says so.

Everything below is verifiable against the working tree; file:line references
are given throughout.

---

## 1. Background: the parts of an FCS file

An FCS (Flow Cytometry Standard) file has three segments:

- **HEADER** — fixed 58 bytes; carries the version string and byte offsets to
  the other segments.
- **TEXT** — a delimited key/value store. Its **first byte is the delimiter**,
  chosen by the writer. The rest is `<d>KEY<d>VALUE<d>KEY<d>VALUE<d>…`.
  Standard keywords are `$`-prefixed and reserved (`$PAR`, `$TOT`, `$PnB`,
  `$PnR`, `$PnN`, `$PnE`, `$DATATYPE`, `$BYTEORD`, `$BEGINDATA`, `$ENDDATA`,
  `$NEXTDATA`); anything else is a user keyword.
- **DATA** — the events. **Row-major and interleaved**: one event record
  contains one value for every parameter, back to back, and records follow one
  another. There is no per-column contiguity anywhere in the file.

Versions in play: 1.0, 2.0, 3.0, 3.1, 3.2, and a 4.0 placeholder. `$NEXTDATA`
may chain several data sets in one file; chained data sets have **no HEADER of
their own** and inherit the primary version.

---

## 2. TEXT parsing

### 2.1 One tokenizer, two consumers

`crate::text::TextFields` (`fcs/src/text.rs`) is the only thing in the codebase
that splits a TEXT segment. It has exactly two consumers:

- `Metadata::from_text_segment` (`fcs/src/metadata.rs:83`) — parses the whole
  segment into the keyword map.
- `Fcs::find_begindata_offset` (`fcs/src/file.rs:917`, helper `scan_for_begindata`
  at `:964`) — scans for
  `$BEGINDATA` and **stops at the first match**, because the segment's end is
  not yet known; finding it is the point of the scan.

This unification is the single most important structural property to check.
Before this branch there were two independent splitting implementations, and
they disagreed — which is how the escaping bug survived. **A validator should
verify that no third splitter has crept in** (`rg 'TextFields' fcs/src` should
show two consumers) and that the two consumers cannot reach different
conclusions about the same bytes.

### 2.2 Escaping is version-gated

`Escaping::for_version` (`fcs/src/text.rs:32`):

| Version | Policy |
|---|---|
| 1.0, 2.0, 3.0 | `Escaping::None` — split on every delimiter byte |
| 3.1, 3.2, 4.0 | `Escaping::Doubled` — a doubled delimiter is one literal delimiter inside the field |

The match is **exhaustive with no wildcard arm**, so adding a `Version` variant
is a compile error rather than a silent default. This is deliberate.

**Why the gate exists, and why 3.1 is the boundary.** Escaping a delimiter by
doubling it is only *invertible* if empty keyword values are impossible —
otherwise `<d>KEY<d><d>NEXT<d>` is ambiguous between "KEY has an empty value"
and "KEY contains a literal delimiter and continues". FCS 3.1 is the first
version that **forbids empty keyword values**, precisely to make the escape
decodable. Applying un-doubling to a 2.0 or 3.0 file would therefore corrupt
it: all five runs of consecutive delimiters in the ten-file Gating-ML
compliance corpus are FCS 2.0 empty values, not escapes. One concrete case:
`real-8-parameters.data.fcs` contains `\Comments\\Row\2\`, where `$Comments`
genuinely has no value. Un-doubling it unconditionally makes `Comments` absorb
`Row` and shifts every field after it.

The un-doubling walk is at `fcs/src/text.rs:112-164`. For a run of N
consecutive delimiter bytes it emits `N / 2` literal delimiters into the field
and terminates the field iff `N` is odd.

**Borrow behaviour worth checking:** `TextFields` yields `Cow<str>`, and
produces `Cow::Owned` under `Doubled` **iff** an un-doubling actually happened.
That is not just an allocation optimisation — the C1 guard below depends on it
as an O(1) discriminant test.

### 2.3 The write side

`escape_into` (`fcs/src/text.rs:226`) doubles the delimiter under `Doubled`,
and escapes the **key as well as the value** — user keyword names are free-form
and can contain the delimiter too.

`serialize_metadata` in `fcs/src/write.rs` **hard-errors** when asked to write
an empty value at V3_1+, naming the offending keyword. This is the write-side
enforcement of the same invariant the read side relies on.

---

## 3. The hardest edge case: a non-conformant FCS 3.1+ file with an empty value

This is the decision most worth independent scrutiny.

### 3.1 The problem

A 3.1+ writer that emits an empty keyword value anyway produces
`<d>$P1S<d><d>$P2S<d>value<d>`. Under `Doubled`, the run of two folds into the
ongoing field, yielding the single key `$P1S<d>$P2S` with `value` attributed to
it. `$P1S` is lost and `$P2S` is unreachable.

This is not hypothetical. **flow-crates itself wrote such files** — its
unmixed-export path stamped FCS 3.2 while writing empty `$PnS` — until commits
`2ea7957` and `4ebc3c3` on this branch fixed it. Users hold those files. They
opened correctly before this branch (the reader was version-blind) and would
have opened silently *wrong* after it. That regression is what forced the
decision.

### 3.2 The damage is local, not cascading

An initial analysis claimed the shift cascades to the end of the segment. It
does not, and the corrected model is pinned by
`an_empty_value_merges_two_keys_under_doubled`. Each empty value collapses
three raw slots (key, empty value, next key) into one field — **two fields
removed, so key/value parity is preserved** for everything after it. The
damage: the empty-valued key is lost, the following key is welded and
unreachable, and the following *value* is misfiled under the welded key.
Everything after that parses correctly.

A validator should re-derive this by hand from `fcs/src/text.rs:112-164`. The
cascading model *is* correct for `Escaping::None`, which is probably where the
original confusion came from.

### 3.3 There is no byte-level signature

The natural-seeming detector — "an even-length delimiter run at a field
boundary is illegal in conformant 3.1+" — **is not implementable**, and this
was initially specified and then rejected during implementation. `escape_into`
produces an even run for every legal escaped literal, and `a<d><d>b` inside a
value is byte-identical, in the same position, to `$A<d><d>$B` across an empty
value. Worse, "at a field boundary" is itself derived from run parity, so the
predicate is circular. Implementing it literally would have fired on every
legally escaped value.

**This is the deepest point in the whole design: the ambiguity is real and
information-theoretic, not an artefact of the implementation.** Any proposed
alternative detector must be checked against that fact first.

### 3.4 What was chosen: a semantic fingerprint, then warn and retry unescaped

`looks_like_merged_keywords` (`fcs/src/text.rs:215`) flags a key that

1. **begins with `$`**, *and*
2. contains a literal delimiter, *and*
3. has text after that delimiter which also **begins with `$`**.

`$` is reserved for standard keywords, so `$…<d>$…` inside a *single* key means
two standard keywords were welded.

On a trip, both consumers emit `tracing::warn!` naming the version and
re-tokenize under `Escaping::None`
(`fcs/src/metadata.rs:97-131`, `fcs/src/file.rs:940-957`).

**Why both halves must look standard.** Testing only the second half flags a
legal user keyword whose name contains `<delimiter>$` — `COST $USD` under the
default space delimiter — and one hit re-parses the **entire segment** under
the wrong policy, silently corrupting a valid file. That is the same failure
class the guard exists to prevent, so a false positive is strictly worse than a
false negative. Requiring the leading `$` excludes every legal user keyword
*by construction* (a conformant user keyword can never start with `$`) rather
than by heuristic, and no standard keyword contains a plausible delimiter.

**Known limit, accepted:** an empty value between two *user* keywords (neither
`$`-prefixed) is not detected. Widening the predicate to "any literal delimiter
in a key" would misfire on the legal `MY KEY` case. The regression being
guarded is flow-crates' own former output, which is all standard keywords.

**Alternatives considered and rejected.** (a) Hard-error on the fingerprint —
symmetric with the write side and safest, but it breaks files that opened fine
before this branch. (b) Parse under both policies and keep whichever validates
— more general, but the selection criterion is fuzzy and a file could validate
under both, choosing silently.

### 3.5 The accepted read/write asymmetry

`write.rs` still refuses to *write* an empty value at V3_1+ while the reader now
accepts one. **A read-modify-write pipeline can therefore load a file it cannot
save.** This was accepted knowingly, and is commented at both sites, each
pointing at the other, so it reads as intentional. The rationale: reading a real
file correctly is worth more than symmetry, and continuing to reject the shape
on write is what stops us producing more such files.

**This is a legitimate target for challenge.** If a validator concludes the
pipeline breakage outweighs the read fidelity, that is a real argument.

### 3.6 Why the `$BEGINDATA` scan needs the fallback too

Not symmetry for its own sake. An empty value immediately ahead of
`$BEGINDATA` welds the keys into `$SOMETHING<d>$BEGINDATA`, which
`eq_ignore_ascii_case("$BEGINDATA")` does not match — so the scan walks past
the offset it exists to find and fails on a file whose TEXT is perfectly
recoverable. Letting only one consumer fall back would also reintroduce
tokenizer drift by the back door.

**Known scope asymmetry, unresolved:** the scan early-stops at `$BEGINDATA`, so
a fingerprint occurring *after* it flips the metadata parse to `None` while the
scan keeps its `Doubled` result. Harmless for the motivating shape (`$BEGINDATA`
is serialized early, ahead of the sorted user keywords), but the claim "the two
consumers cannot disagree" is slightly stronger than the code. **A validator
should try to construct a file where this matters.**

---

## 4. DATA parsing

### 4.1 Shape

`ColumnLayout::from_metadata` derives, per parameter: byte width from `$PnB`,
type from `$DATATYPE`, endianness from `$BYTEORD`, and a `$PnR`-derived range
mask (`range.next_power_of_two() - 1`, integer types only). `param_offsets` is
the **prefix sum** of `bytes_per_parameter`.

`Decoder` (`fcs/src/decode.rs`) resolves `(datatype, width, byteorder)` into one
of eight variants — `U16Le/Be`, `U32Le/Be`, `F32Le/Be`, `F64Le/Be` — **once per
column**, not once per value. `resolve` is fallible and runs `wanted.len()`
times; `read` is infallible and runs `num_events × wanted.len()` times. Its
error strings are reproduced verbatim from the function it replaced, so callers
matching on text keep working.

`extract_columns` (`fcs/src/columns.rs`) walks the interleaved DATA **once**,
writing directly into pre-sized `vec![0.0f32; n]` buffers (which lower to
`alloc_zeroed` via the `IsZero` specialization). No `Vec<Vec<f32>>` intermediate,
no `Result` per value.

Because DATA is interleaved, **extracting one column and extracting all of them
cost the same traversal**. Callers should batch.

### 4.2 Parallelism

Above `PARALLEL_BYTE_THRESHOLD` (1 MiB) the decode splits into 8,192-event
chunks. Disjointness is proven by the **borrow checker**: `split_at` on the byte
slice and `split_at_mut` in lockstep on every output column. **No `unsafe`
anywhere** in `columns.rs`, `decode.rs`, or `text.rs`.

The threshold counts **bytes walked over the whole DATA segment**, not values
kept. This matters: one column of a 300,000-event × 40-parameter file scores
300,000 under a value-count predicate, would stay sequential, and still walks
48 MB.

**1 MiB is measured, in one direction only.** Forcing the parallel branch on a
781.25 KiB fixture regressed decode by +31.5% and +43.8% across two independent
cycles (p = 0.00 both), so 256 KiB is positively ruled out. Nothing distinguishes
1 MiB from 4 MiB — no fixture lands between them (largest corpus DATA is
781.25 KiB, the synthetic is 76.3 MiB). Tracked as `flow-crates-nhd`. **Any doc
comment implying the upper bound is measured would be a defect.**

### 4.3 Known DATA-side gaps

- **`$PAR 0`** makes `bytes_per_event == 0`, and `chunks_exact(0)` panics
  regardless of slice length. Reachability unconfirmed. Tracked as
  `flow-crates-67w`.
- **Bit-packed records** are refused by the column path
  (`fcs/src/columns.rs:537` covers the refusal) with an error naming
  "bit-packed"; they take the older `parse_bit_packed_data` route.
- `extract_columns` errors when `data_bytes` is too *short* but not when it is a
  superset. Deliberate — callers pass a bounded `$BEGINDATA..$ENDDATA` slice —
  but it means a caller passing the whole mmap would decode garbage silently.

---

## 5. Why the testing had to be this rigorous

Four distinct reasons, each learned from a specific failure in this work.

**1. The corpus cannot see the bug.** The ten-file Gating-ML compliance corpus
is five FCS 2.0 files and five FCS 3.0 files — **zero FCS 3.1+ files**. So the
`Escaping::Doubled` *read* path, which is the policy for every modern file the
library will ever encounter, has no real-file coverage at all. Every fixture
exercising it must be hand-assembled byte by byte. **This is the single most
important thing for a validator to internalise**, and getting a third-party 3.1+
file into the corpus would change the risk profile of the whole area.

**2. A round-trip test cannot falsify an assumption both halves share.** The
writer forbids empty values at 3.1+; the reader assumed they could not occur.
Each half is correct in isolation. The contradiction lives only in the join, and
no test that goes `write → read` can produce it, because the writer refuses to
emit exactly the input that breaks the reader. This is why the C1 fixtures are
hand-built bytes and **not** produced through `write_fcs_file` — and why the
tests assert that keywords *after* the empty value are recovered intact, rather
than merely that `Fcs::open` returns `Ok`. Succeeding-while-garbled is the bug.

**3. Bit-exact oracles, because "close enough" hides sign and NaN errors.** The
lazy per-column path and the eager `data_frame` path decode the same bytes
through different code, so they are compared with `.to_bits()` rather than `==`
— a NaN makes `==` pass vacuously and `-0.0 == 0.0` hides a sign error. The
oracle covers **all 54 channels of all 10 corpus files with zero skips**, through
**both** the sequential and the forced-parallel branch. Its skip paths were
converted to naming panics and its floor made an equality assertion
(`checked == 54`), because a test whose failure mode is "silently tests less"
guarding the headline correctness property is not acceptable.

**4. Measurement discipline, because the machine lies.** Running all "before"
samples then all "after" samples made an **untouched control** benchmark report
"Performance has regressed" at +8.8% then +21.7%, both p = 0.00. Each side is
internally consistent, so the p-value structurally cannot detect the drift.
Interleaving the two binaries collapsed it to ~+2% (p = 0.54). Every performance
figure in the tree comes from an interleaved paired run carrying a control.
Relatedly: a sweep whose candidate values all select the same branch on every
available fixture measures nothing, which is why the 1 MiB–4 MiB question is
recorded as open rather than answered.

---

## 6. Suggested attack surface for a validator

Ranked by where a defect would be most costly:

1. Construct a **conformant** FCS 3.1+ TEXT segment that trips
   `looks_like_merged_keywords` and is therefore silently re-parsed under
   `Escaping::None`. A false positive here corrupts a valid file.
2. Construct a file where `find_begindata_offset` and
   `Metadata::from_text_segment` reach different escaping conclusions (see the
   early-stop scope asymmetry in §3.6).
3. Attack the claim in §3.3 that no byte-level signature exists. If one does,
   the semantic fingerprint and its known limits become unnecessary.
4. Check the damage model in §3.2 by hand against `fcs/src/text.rs:112-164`.
5. Find a `$PnB`/`$DATATYPE`/`$BYTEORD`/`$PnR` combination the eight `Decoder`
   variants mis-handle, or where the lazy and eager paths could diverge without
   the oracle noticing.
6. Verify `plan.offset + plan.width <= bytes_per_event` for every layout
   `from_metadata` can build. The prefix-sum argument is what makes the inner
   decode loop panic-free; `$PAR 0` (§4.3) is the one known gap.
