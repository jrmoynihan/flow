# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Chore

 - <csr-id-9ee153e04507d454f9509a4d2b2a2b3ffe9db17b/> add crates.io metadata for unpublished crates
   Pin flow-pacmap's flow-knn path dependency to a version so cargo publish can succeed.

### Documentation

 - <csr-id-92e31b03dc632230809d10422be0c1062e6e9e1b/> consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate
   Rewrites READMEs across the workspace (fcs, flow-clustering,
   flow-control-detection, flow-density, flow-fcs-compress, flow-knn,
   flow-linalg, flow-pacmap, flow-peak-detection, gates, peacoqc-cli,
   peacoqc-rs, tru-ols, tru-ols-cli) to lead with install/quick-start/perf
   for downstream consumers, and adds a new flow-fcs-bench README.
   
   Adds a concrete usage example to peacoqc-py/README.md mirroring the
   docstring in peacoqc/__init__.py.
   
   Removes the superseded utils/ crate (clustering, KDE, and PCA helpers
   now live in their dedicated crates) and syncs beads issue/interaction
   export state.

### New Features

 - <csr-id-adceb9d0655b46e1fadb4ecb9f0e4dc6e3d73742/> expose full pco ChunkConfig for LosslessF32Pco
   Wrap pco 1.x ChunkConfig so callers can set mode, delta, and paging instead
   of only compression level.

### Performance

 - <csr-id-05d8cf13511a8b064b406fd3a385eec95948b8bd/> bulk-write KnnGraph and typed load buffers
   Complete Campaign 2 A/B: keep staged write_all (−99.9%) and LE typed
   read_exact (−6.7%) at 100k×60; document reverted pacmap/compress/peaks
   scratch experiments with Criterion benches retained.
 - <csr-id-2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37/> bulk-load KnnGraph IO; record unsafe micro-opt A/B
   Keep the ~100× faster graph load via read_exact + LE bytemuck cast.
   Add Criterion benches and PERF_AB docs for the six-item A/B campaign;
   revert opts that missed the ≥5% keep rule (BSS, FCS columns/write,
   TRU-OLS SyncPtr, exact KNN / PaCMAP unchecked).

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release over the course of 16 calendar days.
 - 26 days passed between releases.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Add crates.io metadata for unpublished crates ([`9ee153e`](https://github.com/jrmoynihan/flow/commit/9ee153e04507d454f9509a4d2b2a2b3ffe9db17b))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Bulk-write KnnGraph and typed load buffers ([`05d8cf1`](https://github.com/jrmoynihan/flow/commit/05d8cf13511a8b064b406fd3a385eec95948b8bd))
    - Bulk-load KnnGraph IO; record unsafe micro-opt A/B ([`2d3c6fc`](https://github.com/jrmoynihan/flow/commit/2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37))
    - Expose full pco ChunkConfig for LosslessF32Pco ([`adceb9d`](https://github.com/jrmoynihan/flow/commit/adceb9d0655b46e1fadb4ecb9f0e4dc6e3d73742))
</details>

## v0.1.3 (2026-07-22)

### Documentation

 - <csr-id-b0010d3e67e8531fd345f404f369ee52af5a9169/> add row vs column major layout diagram

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release over the course of 55 calendar days.
 - 72 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-pacmap v0.1.1, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`b44c4f5`](https://github.com/jrmoynihan/flow/commit/b44c4f5915916ab3ce73a5b6ce421601090f77a5))
    - Release flow-pacmap v0.1.0, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`e29c820`](https://github.com/jrmoynihan/flow/commit/e29c820dd65493c3a41f437b0e8f850c3cef8102))
    - Add row vs column major layout diagram ([`b0010d3`](https://github.com/jrmoynihan/flow/commit/b0010d3e67e8531fd345f404f369ee52af5a9169))
</details>

## v0.1.2 (2026-05-11)

### Documentation

 - <csr-id-c5bda8ca7e17807268121b5027cfdc7849a91dd4/> consolidate codec table with encode/decode throughput and fidelity columns

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs-compress v0.1.2 ([`0eb992c`](https://github.com/jrmoynihan/flow/commit/0eb992c3d8e97e305a0a957d0a8bbbecb6e56467))
    - Consolidate codec table with encode/decode throughput and fidelity columns ([`c5bda8c`](https://github.com/jrmoynihan/flow/commit/c5bda8ca7e17807268121b5027cfdc7849a91dd4))
</details>

## v0.1.1 (2026-05-11)

<csr-id-90e3ee26926e8df26e30fdf12ac25aae632d1dd9/>

### Chore

 - <csr-id-90e3ee26926e8df26e30fdf12ac25aae632d1dd9/> bump to 0.1.1 with readme field for crates.io display

### Documentation

 - <csr-id-e14b4799997458dba9110f4723e88dc50c8dce5b/> add README.md to flow-linalg, flow-density, flow-clustering, flow-fcs-compress
   Each README describes the crate's purpose, public API, algorithms,
   scope boundaries, features, tests, and benchmarks for prospective
   users and contributors.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-linalg v0.1.1, flow-density v0.1.1, flow-clustering v0.1.1, flow-fcs-compress v0.1.1 ([`966d22a`](https://github.com/jrmoynihan/flow/commit/966d22ae4fbdd6114dc3862d45648fce7ebf53cc))
    - Bump to 0.1.1 with readme field for crates.io display ([`90e3ee2`](https://github.com/jrmoynihan/flow/commit/90e3ee26926e8df26e30fdf12ac25aae632d1dd9))
    - Add README.md to flow-linalg, flow-density, flow-clustering, flow-fcs-compress ([`e14b479`](https://github.com/jrmoynihan/flow/commit/e14b4799997458dba9110f4723e88dc50c8dce5b))
    - Merge branch 'feat/flow-fcs-compress' ([`ef239b2`](https://github.com/jrmoynihan/flow/commit/ef239b24dbacfabc1e68dfa5f4dc8baa49f9704a))
    - Merge pull request #20 from jrmoynihan/feat/flow-fcs-compress ([`f953bc5`](https://github.com/jrmoynihan/flow/commit/f953bc5df8f6978e3fe511538cb2943730a35eff))
</details>

## v0.1.0 (2026-05-11)

### Documentation

 - <csr-id-9012270efea8a503c72c32bc9bda717e0dde0a48/> fix section heading in ISAC proposal

### New Features

 - <csr-id-53b4eba342397dffb258f37c1a80a430683955b7/> add FczReader::warm_cache for page fault elimination before timed reads
 - <csr-id-a4a5e18e06b55de252b74110118ac72aa2fc0891/> add compression crate, benchmarks, and ISAC proposal
   Introduce two new workspace crates and an ISAC FCS WG proposal targeting
   compression and column-major DATA layout for the FCS standard.
   
   flow-fcs-compress (new crate, codec library + container adapters):
   - ColumnCodec trait with chunked encode/decode and zero-copy semantics

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release over the course of 3 calendar days.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-linalg v0.1.0, flow-density v0.1.0, flow-clustering v0.1.0, flow-fcs-compress v0.1.0, flow-fcs v0.4.0 ([`e8c908e`](https://github.com/jrmoynihan/flow/commit/e8c908ef92fb68b8e2d01d3c1e8d6a294c8c6bda))
    - Fix section heading in ISAC proposal ([`9012270`](https://github.com/jrmoynihan/flow/commit/9012270efea8a503c72c32bc9bda717e0dde0a48))
    - Add FczReader::warm_cache for page fault elimination before timed reads ([`53b4eba`](https://github.com/jrmoynihan/flow/commit/53b4eba342397dffb258f37c1a80a430683955b7))
    - Add compression crate, benchmarks, and ISAC proposal ([`a4a5e18`](https://github.com/jrmoynihan/flow/commit/a4a5e18e06b55de252b74110118ac72aa2fc0891))
</details>

