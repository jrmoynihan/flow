# flow-autospectral performance A/B notes

Follow workspace protocol in `docs/dev/UNSAFE_MICROOPT_AB.md` when micro-optimizing.

For match throughput, interleave baseline/HEAD Criterion runs and keep an untouched control bench (see beads memory `benchmark-a-b-on-this-machine-apple-m5`). The `match_events_nn` / `match_nn_control` groups are the control counterpart to residual matching.

```bash
cargo bench -p flow-autospectral --bench discover_and_match
cargo bench -p flow-autospectral --bench match_matrix
cargo bench -p flow-autospectral --bench scatter_clean --features gpu
```

When the algorithm itself changes, judge quality first (OLS residual, population spread), not wall time alone. Use `--example method_comparison --features tru-ols` to compare discovery backends on those metrics.

## Terms

These notes mix cytometry data with dense linear algebra. The two “grids” are not the same object:

| Term | Meaning here |
|------|----------------|
| **Event** | One particle (one row of the FCS measurement). There are *n* events. |
| **Detector** | One fluorescence channel. There are *d* detectors. |
| **Emitter** | One column of the mixing matrix: a fluorophore, a spectral variant of a fluorophore, or one autofluorescence (AF) library spectrum. There are *F* fluorophores and *K* AF spectra. |
| **Mixing matrix \(M\)** | Size *d* × (emitters). Each column is one emitter’s spectrum across detectors. An *entry* of \(M\) is \(M_{i,j}\) (detector *i*, emitter *j*) — not an “event.” |
| **[Ordinary least squares (OLS)](https://en.wikipedia.org/wiki/Ordinary_least_squares)** | Find emitter amounts \(\hat\alpha\) so that \(M\hat\alpha\) is closest to the event’s detector vector \(y\). |
| **[Matrix factorization](https://en.wikipedia.org/wiki/Matrix_decomposition)** | Rewrite \(M\) or \(M^\top M\) into a product of simpler matrices ([QR](https://en.wikipedia.org/wiki/QR_decomposition) of \(M\), or [Cholesky](https://en.wikipedia.org/wiki/Cholesky_decomposition) of \(M^\top M\)) so each later event is a cheaper triangular solve instead of a full decomposition. |
| **[Gram matrix](https://en.wikipedia.org/wiki/Gram_matrix)** | \(M^\top M\) (size emitters × emitters). It does not depend on which event you are fitting. |
| **[Dot product](https://en.wikipedia.org/wiki/Dot_product)** | \(\sum_i a_i b_i\) for two vectors of the same length. |
| **[Matrix–vector product](https://en.wikipedia.org/wiki/Matrix%E2%80%93vector_multiplication)** | \(M\alpha\): scale each column of \(M\) by the corresponding abundance and add those columns. |
| **[Column-major layout](https://en.wikipedia.org/wiki/Row-_and_column-major_order)** | Consecutive memory addresses hold consecutive detectors of *one* emitter (one column of \(M\)). |
| **[Spatial locality](https://en.wikipedia.org/wiki/Locality_of_reference)** | The CPU cache is faster when a loop reads addresses that sit next to each other rather than jumping by a full column on every inner step. |
| **[SIMD](https://en.wikipedia.org/wiki/Single_instruction,_multiple_data)** | One instruction updates several adjacent floating-point values. Contiguous columns make this practical; strided access usually does not. |
| **Scalar loop** | A loop in the host language that updates one number per iteration, as opposed to one library call over a whole vector or matrix (for example R `for` over list elements versus `S %*% alpha`). |
| **Heap allocation** | Asking the runtime for a new block of memory ([dynamic allocation](https://en.wikipedia.org/wiki/Memory_management#DYNAMIC)). Doing this *n* times means *n* allocator round-trips plus later releases. |
| **[Thread-local storage](https://en.wikipedia.org/wiki/Thread-local_storage)** | One private copy of a workspace per worker thread so parallel workers do not overwrite each other. |
| **[BLAS](https://en.wikipedia.org/wiki/Basic_Linear_Algebra_Subprograms)** | Standard dense linear-algebra kernels. A multithreaded BLAS inside an already-parallel event loop *nests* two thread pools and often slows the run. |

Rust-versus-R timing tables: [`comparison-with-r.md`](comparison-with-r.md). Criterion n×d grids: [`PERF_MATRIX.md`](PERF_MATRIX.md).

## Measured keep / skip rows (2026-08-18–19, Apple M5 Max, rustc 1.95.0)

The table is the Criterion record. The sections below state the problem each row was solving. The smoke grid was `match_matrix` at 10,000 events, 8 detectors, \(K \in \{1,8,32\}\) AF spectra, plus `scatter_clean` at 10,000 and 50,000 events.

| What we changed | Status | Before | After | Delta | Size | Code |
|-----------------|--------|--------|-------|-------|------|------|
| Reuse one OLS factorization per AF spectrum instead of decomposing again for every event | kept | 26.19 ms | 6.17 ms | −76% | 10,000 events, 8 detectors, 32 AF spectra | `match_residual_reused` in [`match_af.rs`](../src/match_af.rs) |
| Process residual matching across events in parallel | kept | 31.95 ms (one thread) | 6.17 ms | −81% | 10,000 events, 8 detectors, 32 AF spectra | `map_event_matches` in [`match_af.rs`](../src/match_af.rs) |
| Reuse one factorization when many events share the same mixing matrix | kept | 1.096 ms | 0.505 ms | −54% | 10,000 events, 8 detectors | `OlsFactor` in [`unmix_ols.rs`](../src/unmix_ols.rs) |
| Process that shared-matrix unmix across events in parallel | kept | 0.892 ms (one thread) | 0.505 ms | −43% | 10,000 events, 8 detectors | `unmix_events_ols_with` in [`unmix_ols.rs`](../src/unmix_ols.rs) |
| GPU scatter-clean as the default at 10,000 events | skipped | 12.58 ms CPU | 12.36 ms GPU | −1.7% | 10,000 events, 2 scatter detectors | below the 5% keep rule |
| GPU scatter-clean at 50,000 events | kept optional | 300.0 ms CPU | 98.0 ms GPU | −67% | 50,000 events, 2 scatter detectors | `KnnMethod::GpuExact` |
| GPU nearest-neighbour index (`NnDescent`) | skipped | — | — | — | — | query API needs `&mut` |
| Reuse per-event working arrays in joint unmix, copy the mixing matrix only when a variant is chosen, write outputs into pre-sized tables, and multiply along contiguous columns | **kept** | 4.464 ms | 2.096 ms | −56% | 10,000 events, 20 detectors, 8 fluorophores, 8 AF spectra | `EventScratch`, `try_commit_variant`, `gemv` in [`joint_inner.rs`](../src/joint_inner.rs) |
| Run joint unmix in `f32` faer instead of `f64` | **kept optional** | 295.4 ms `f64` | 240.8 ms `f32` | −18% | 200,000 events, 64 detectors, 8 fluorophores, 8 AF spectra | `JointUnmixPrecision::F32` |

The slower “decompose on every event” paths remain for A/B: set `MatchConfig::reuse_af_factors` or `OlsUnmixConfig::reuse_factor` to `false`.

## What actually got faster

Single-thread joint unmix is about twice as fast as AutoSpectralRcpp on the panels in [`comparison-with-r.md`](comparison-with-r.md). Separately, the last table row cut joint-unmix wall time by 56% versus the previous Rust implementation. Both come from the same operational shift: work that depends only on the panel is done once; work that depends on an event reuses memory and reads it in storage order. That shift is not specific to Rust. A C++, R/Rcpp, or Julia port can make the same change.

Joint timings below are Criterion `joint_unmix` at 10,000 events, 20 detectors, 8 fluorophores, and 8 AF spectra (`joint-alloc-pre` versus HEAD). Residual-match and OLS numbers are the 2026-08-18 `match_matrix` smoke grid.

### Decompose the mixing matrix once per panel, not once per event

**Problem.** OLS for one event is: find \(\hat\alpha\) so \(M\hat\alpha\) matches that event’s detector vector \(y\). The slow step is the [factorization](https://en.wikipedia.org/wiki/Matrix_decomposition) of \(M\) or of the [Gram matrix](https://en.wikipedia.org/wiki/Gram_matrix) \(M^\top M\). That factorization is identical for every event that uses the same \(M\).

**Solution.** Factor each distinct \(M\) once, then apply the factor to every event that needs it.

**What changes in operation.**

- Before: `match_residual_naive` in [`match_af.rs`](../src/match_af.rs) built a new \(M\) (fluorophores plus one AF column) and ran QR for every pair *(event, AF candidate)*. At 10,000 events and 32 AF spectra that is 320,000 factorizations.
- After: `match_residual_reused` builds one [`OlsFactor`](../src/unmix_ols.rs) per AF spectrum (Cholesky of \(M^\top M\) when it is positive definite and \(d \ge F+1\), otherwise QR of \(M\)). Each event is a matrix–vector product plus a triangular solve against that stored factor.
- Difference: *K* factorizations instead of *n × K*; measured −76% median time at 10,000 events, 8 detectors, 32 AF spectra (26.19 ms → 6.17 ms).

When every event already shares one \(M\), `unmix_events_ols_with` with `reuse_factor = true` is the same pattern: one `OlsFactor`, then one apply per event (−54%: 1.096 ms → 0.505 ms versus QR per event).

Joint unmix performs the same class of work in `JointPrecomp::build` ([`joint.rs`](../src/joint.rs)): one projector from detectors onto fluorophores, residual columns for the AF library, residual columns for each fluorophore’s variants, and one Gram matrix `sst`. The per-event loop does not rebuild those.

In another language:

1. Key the cached factor by AF index (residual match) or by the current set of variant columns (joint).
2. Run QR or Cholesky only when that key’s \(M\) has not been seen.
3. Per event, multiply and triangular-solve; do not decompose \(M\) again.

### Replace per-candidate OLS with precomputed residual columns and dot products

**Problem.** Choosing which AF spectrum or fluorophore variant fits an event looks like a new least-squares problem per candidate. A factorization per candidate costs on the order of \(d \times F^2\) work.

**Solution.** After the shared projector \(P\) exists, unmix fluorophores once for the event, form the leftover detector residual, and test each library column with [dot products](https://en.wikipedia.org/wiki/Dot_product) against residual columns that were built in `JointPrecomp::build`. That is `JointPrecomp::score_af` and the candidate loop in `unmix_fluor_event` ([`joint.rs`](../src/joint.rs)).

**What changes in operation.**

1. Unmix fluorophores once: \(\texttt{init_f} = P y\).
2. Form \(\texttt{base_resid} = y - S \cdot \texttt{init_f}\).
3. For each AF (or variant) column \(j\), compute the scale from a weighted dot product and the new residual norm from the identity \(\|r\|^2 - 2k\langle r, r_j\rangle + k^2\|r_j\|^2\).
4. When a variant is accepted, `try_commit_variant` overwrites one *row of the mixing matrix* (one emitter’s spectrum) and patches the matching row and column of the Gram matrix, instead of factoring the full \(d \times F\) matrix again.

- Before: one QR (or equivalent) per candidate.
- After: two matrix–vector products per event plus one dot product per candidate.
- Difference: work linear in \(d F + K d\) instead of \(K \times d F^2\) factorizations. The AF-only joint path (empty `SpectralVariants`: assign AF, do not swap fluorophore variants) is already close to full joint at 10,000 events × 20 detectors (~1.33× the AF-only median). The same AF-only control sped up 49% when per-event heap allocations were removed, which shows that path was dominated by allocation, not by a different variant-search algorithm.

In another language:

1. Precompute residual columns \(R = A - S(PA)\) for the AF library and for each fluorophore’s variants.
2. Rank candidates with dot products against the current residual.
3. Keep an \(F \times F\) Gram matrix and rewrite one row and one column when a variant is accepted.

### Allocate working arrays once per worker, not once per event

**Problem.** Joint unmix needs many length-*d* / length-*F* / length-(variant count) arrays for every event (residuals, weights, trial abundances, candidate lists). Allocating them inside the event loop means one [heap](https://en.wikipedia.org/wiki/Memory_management#DYNAMIC) round-trip per event. The parallel path also *returned* a newly owned vector per event and concatenated them (`collect`), which is another *n* allocations.

**Solution.** A workspace is those arrays allocated once and overwritten. `EventScratch` in [`joint.rs`](../src/joint.rs) is that workspace. `JointPrecomp::run` owns one instance on a single thread. `with_thread_scratch` keeps one instance per Rayon worker via [thread-local storage](https://en.wikipedia.org/wiki/Thread-local_storage). Output tables (`resid`, `abundances`, `af_index`) are allocated to *n* rows up front; the loop writes event *i* into row *i*. Square solves reuse one \(F \times 1\) right-hand side in `solve_square_vec`.

**What changes in operation.**

- Before: *n* sets of short arrays created and freed (plus *n* output vectors in the parallel path).
- After: one workspace per worker thread; *n* overwrites of the same memory; one abundance table of size \(n \times (F+1)\).
- Difference: measured −56% median joint-unmix time at 10,000 events, 20 detectors, 8 fluorophores (4.464 ms → 2.096 ms). The AF-only control dropped 49% on the same change, so most of that 56% is fewer allocations, not a different variant algorithm. At 10,000 events, the cost of constructing and freeing those short vectors was larger than the floating-point work they wrapped.

On one thread, `ensure` grows the workspace if the panel shape changed; `begin_event` clears flags and lists without releasing the buffers.

In another language:

1. Give each OpenMP or thread-pool worker a struct of arrays sized to *d*, *F*, and the variant counts.
2. Allocate the \(n \times (F+1)\) abundance table once.
3. In the parallel loop, write event *i* into row *i*. Do not return a new dense vector from the per-event function.

### Copy the mixing matrix only when an event accepts a spectral variant

**Problem.** Most events keep the master fluorophore spectra. Copying the full mixing matrix (\(F \times d\) entries) and the Gram matrix (\(F \times F\)) for every event moves a large block of memory that the event will never write.

**Solution.** Start each event with flags `cell_s_copied`, `cell_s_f_w_copied`, and `a_base_copied` false (the `cell_` prefix means “this event’s copy of the mixing matrix,” not a matrix entry). `ensure_cell_s` / `ensure_cell_s_f_w` / `ensure_a_base` copy from the shared panel matrices only when `try_commit_variant` first needs a writable copy. Later accepted variants overwrite that copy in place.

**What changes in operation.**

- Before: *n* copies of \(S\) and \(S S^\top\).
- After: shared read-only \(S\) while scoring; one copy on the first accepted variant for that event; later variants patch the copy.
- Difference: AF-only events and events that never accept a variant never copy \(S\). Memory traffic scales with accepted variants, not with *n*.

In another language: keep a read-only view of the panel spectra while ranking candidates. Allocate an event-local mixing matrix on the first accepted variant, not at the start of the per-event function.

### Multiply along contiguous mixing-matrix columns

**Problem.** faer stores \(M\) in [column-major order](https://en.wikipedia.org/wiki/Row-_and_column-major_order): detector values for one emitter are adjacent. The old `gemv` walked `matrix[(row, column)]` with the row index in the inner loop, so each inner step jumped `nrows` entries. That is a strided load: poor [spatial locality](https://en.wikipedia.org/wiki/Locality_of_reference), and [SIMD](https://en.wikipedia.org/wiki/Single_instruction,_multiple_data) cannot apply cleanly. faer’s own LU/Cholesky kernels already use SIMD; this per-event product did not.

**Solution.** `gemv` and `col_slice` in [`joint.rs`](../src/joint.rs) take column *j* as a contiguous slice (`col(j).try_as_col_major().as_slice()`) and add \(\alpha_j\) times that column into the output (a column-wise [matrix–vector product](https://en.wikipedia.org/wiki/Matrix%E2%80%93vector_multiplication)). `col_dot` and `axpy_col` use the same layout. The crate stores both `spectra_df` (*d* × *F*) and `spectra_fd` (*F* × *d*) so each product can stream a contiguous axis. The algebra is unchanged.

**What changes in operation.**

- Before: inner loop strides through memory by a full column.
- After: inner loop reads adjacent detector samples of one emitter, then the next emitter.
- Difference: each inner iteration hits the same cache line (and can use SIMD). The CPU loads a block of detectors once instead of faulting a new line on every detector index.

In another language:

- Armadillo / Fortran: `S.col(j)` in the inner kernel.
- R: a numeric matrix of size *d* × *F* and `S %*% alpha`, not a scalar loop over a list of per-fluorophore vectors.
- NumPy: `S @ alpha` with `S` shaped `(d, F)`.

### Run independent events in parallel after the shared factors exist

**Problem.** Events only write into their own output rows. After `JointPrecomp` or `OlsFactor` exists, there is no data dependence between events, so one core leaves the others idle. Very small files are the exception: starting a thread pool can cost more than the arithmetic.

**Solution.** Above `parallel_event_threshold` (default 256 events), Rayon splits the event range. `FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL=1` disables that. Joint `run` zips chunks of input events with chunks of the pre-sized output tables. The R sidecar sets `OMP_NUM_THREADS` to the timed thread count and BLAS libraries to 1 so the event pool does not nest with a threaded matrix multiply.

**What changes in operation.**

- Before: one thread walks all *n* events after the shared precomputation.
- After: many threads each take a slice of events; each thread has its own `EventScratch`.
- Difference: residual matching 31.95 ms → 6.17 ms (−81%) at 10,000 events, 8 detectors, 32 AF spectra. Shared-matrix unmix 0.892 ms → 0.505 ms (−43%). Multi-thread *versus AutoSpectralRcpp* (for example 200,000 events, 8 fluorophores, about 3.5×) is a different measurement from Criterion’s many-thread occupancy at 10,000 events; do not quote those Criterion events/s as the single-thread versus-R figure.

In another language:

1. Build the shared factors on one thread (or a serial region).
2. Parallelize over events only after that.
3. Set BLAS / `OMP_NUM_THREADS` to 1 inside that region unless nested parallelism is intentional.
4. Do not also parallelize the *K* AF candidates inside one event unless *K* is very large; that inner loop is already a short sequence of dot products.

## What we did not keep

GPU scatter-clean is not the default at 10,000 events (1.7% median, overlapping ranges). It is worth using at 50,000 events (`KnnMethod::GpuExact`). There is no GPU joint-unmix kernel; versus-R claims stay on CPU. The column-wise matrix–vector loops stay in safe code; layout and reuse were enough without `unsafe` indexing. When `JointUnmixConfig::cell_weight` is on (off by default), `weighted_lstsq` still allocates a new \(F \times F\) Gram matrix on each call. The default joint path uses the shared projector instead.

Internal `f32` joint unmix is not the default. At 10,000 events × 20 detectors the paired `f32` median was **slower** than `f64` (+157%). At 200,000 events × 64 detectors it was **−18%** (keep optional). Versus-R stays on `double`.

### Use binary32 only when the event table is large

**Problem.** After workspace reuse, the joint loop is GEMV / dots / a small Cholesky. On NEON, `f64` occupies two 64-bit lanes where `f32` occupies four, and the event table is twice as many bytes. At 200,000 events and 64 detectors that table is 102 MiB in `f64` versus 51 MiB in `f32` — both in DRAM, but the arithmetic width still changes.

**Solution.** [`JointUnmixPrecision::F32`](../src/config.rs) casts the panel and the event table once, runs the same algorithm in `faer::Mat<f32>`, and promotes abundances back to `f64`. Default remains `F64` so QC-core versus AutoSpectralRcpp stays on `double`.

**What changes in operation.**

- Before: every GEMV and Gram solve used `f64`.
- After (opt-in): the same loops run on `f32` storage; callers still pass `&[f64]`.
- Difference (2026-08-20, Apple M5 Max, rustc 1.95.0, paired IDs in `joint_unmix`): 200,000 events × 64 detectors 295.4 ms → 240.8 ms (**−18%**). 10,000 events × 20 detectors **+157%** — the extra cast and the `f32` kernels lose when the file still fits in L2 and occupancy, not DRAM, is the limit. Tiny-panel test: AF and variant indices match; abundances within ~1e-3 relative.

Do not quote `F32` as the vs-R path. Set it for large stained files after checking residual agreement on that panel.

A port should therefore:

1. Build the projector, residual libraries, and Gram matrix once per file.
2. Rank AF and variant candidates with dot products against those residual columns.
3. Keep one workspace of arrays per worker and one preallocated abundance table.
4. Copy the mixing matrix only when an event accepts a variant.
5. Implement matrix–vector products as loops over contiguous emitter columns.
6. Parallelize over events with BLAS held to one thread under that pool.
7. Keep `double` as the default; offer `float` only after a quality check, and only for large event tables.
