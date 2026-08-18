# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

Condition-number metrics now cover rectangular mixing matrices; Mage hotspot / SIF helpers from similarity.

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
 - <csr-id-6d051ac5a34e63a997ca85c8819b790c0d161c8a/> scrub changelog neutralize wording after release rewrite
   smart-release reintroduced commit-message phrasing about neutralizing
   attribution; restore neutral release notes and drop the misplaced
   pacmap prep entry from linalg/gates changelogs.

### New Features

 - <csr-id-aaee839d28030214d7836b502d74fe82be0ea022/> compute condition metrics for rectangular matrices
   Allow κ₂ / complexity on m×n mixing matrices via SVD over nonzero singular
   values, not only square spillover matrices.
 - <csr-id-79599d58958183b1177afdf80770d087ee7f3242/> Mage hotspot matrix and SIFs from similarity
 - <csr-id-c10b51bf31f30b4798eca428b5989749da481f43/> add matrix condition number metrics
   Expose κ₂ / complexity index helpers via SVD for assessing mixing-matrix stability.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release over the course of 26 calendar days.
 - 26 days passed between releases.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Compute condition metrics for rectangular matrices ([`aaee839`](https://github.com/jrmoynihan/flow/commit/aaee839d28030214d7836b502d74fe82be0ea022))
    - Mage hotspot matrix and SIFs from similarity ([`79599d5`](https://github.com/jrmoynihan/flow/commit/79599d58958183b1177afdf80770d087ee7f3242))
    - Add matrix condition number metrics ([`c10b51b`](https://github.com/jrmoynihan/flow/commit/c10b51bf31f30b4798eca428b5989749da481f43))
    - Scrub changelog neutralize wording after release rewrite ([`6d051ac`](https://github.com/jrmoynihan/flow/commit/6d051ac5a34e63a997ca85c8819b790c0d161c8a))
</details>

## 0.1.2 (2026-07-22)

<csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/>

### Chore

 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> polish pacmap/linalg/gates for crates.io release
   Add README and publish metadata for flow-pacmap, and refresh install/API
   notes for the upcoming flow-linalg and flow-gates releases.

### Chore

 - <csr-id-ba96d7fb2b887ab666a3ecdea9f9f49b0cbbf3f4/> prepare 0.1.1 release with Wang et al. attribution
   Bump flow-pacmap to 0.1.1, pin the README install line, and scrub changelog
   wording that celebrated neutralizing academic attribution.

### New Features

 - <csr-id-130af0763c00f9e1ab6b16c0b33bf94253ffb340/> estimate spillover from single-stain controls
   Add `estimate_spillover`, `SingleStainControl`, and `median` under the
   `compensation` feature: diagonal-normalized spillover from positive/negative
   population medians, with recovery and compensate round-trip tests.

### Documentation

 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> document spillover estimation in README
   Installation pin and public API examples for the new estimation helpers.
 - <csr-id-1d5806048f15f590ebe7b2ba449501aa73868b95/> polish changelogs for pacmap, linalg, and gates releases

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release.
 - 72 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-pacmap v0.1.1, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`b44c4f5`](https://github.com/jrmoynihan/flow/commit/b44c4f5915916ab3ce73a5b6ce421601090f77a5))
    - Prepare 0.1.1 release with Wang et al. attribution ([`ba96d7f`](https://github.com/jrmoynihan/flow/commit/ba96d7fb2b887ab666a3ecdea9f9f49b0cbbf3f4))
    - Release flow-pacmap v0.1.0, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`e29c820`](https://github.com/jrmoynihan/flow/commit/e29c820dd65493c3a41f437b0e8f850c3cef8102))
    - Polish changelogs for pacmap, linalg, and gates releases ([`1d58060`](https://github.com/jrmoynihan/flow/commit/1d5806048f15f590ebe7b2ba449501aa73868b95))
    - Polish pacmap/linalg/gates for crates.io release ([`c9b7448`](https://github.com/jrmoynihan/flow/commit/c9b7448fef935e2ba6f3ea568ce092f9c777b53b))
    - Estimate spillover from single-stain controls ([`130af07`](https://github.com/jrmoynihan/flow/commit/130af0763c00f9e1ab6b16c0b33bf94253ffb340))
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

<csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/>

### Chore

 - <csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/> bump flow-fcs to 0.4.0, add publish metadata to new crates
   - flow-fcs 0.3.0 → 0.4.0 (new compensation feature + public API)
   - flow-linalg, flow-density, flow-clustering: add repository field
     and smart-release scripts for first publish
   - Update all workspace consumers to ^0.4.0

### New Features

 - <csr-id-ff281f690256e43c04bbbb98e808c85b8122db29/> new crate with faer-based compensation primitives

### Bug Fixes

 - <csr-id-ae7bedcc755cf2c41b233e4b7f12c1bef07e0a31/> singularity check, length validation, rayon as optional dep

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-linalg v0.1.0, flow-density v0.1.0, flow-clustering v0.1.0, flow-fcs-compress v0.1.0, flow-fcs v0.4.0 ([`e8c908e`](https://github.com/jrmoynihan/flow/commit/e8c908ef92fb68b8e2d01d3c1e8d6a294c8c6bda))
    - Bump flow-fcs to 0.4.0, add publish metadata to new crates ([`74956f9`](https://github.com/jrmoynihan/flow/commit/74956f94c544d1fa83f6fffbb18e2d4f5e6072ff))
    - Singularity check, length validation, rayon as optional dep ([`ae7bedc`](https://github.com/jrmoynihan/flow/commit/ae7bedcc755cf2c41b233e4b7f12c1bef07e0a31))
    - New crate with faer-based compensation primitives ([`ff281f6`](https://github.com/jrmoynihan/flow/commit/ff281f690256e43c04bbbb98e808c85b8122db29))
</details>

