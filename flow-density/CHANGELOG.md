# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.3 (2026-11-12)

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
 - <csr-id-e33bbe75d3777e0b8f7c8f5ebe0d71259ab9b4ef/> add performance-guided development map and host calibrator
   Give agents a napkin-math floor, this-machine snapshot, gap index, and
   retry shortlist so hot-path work starts from hardware cost rather than
   another 5% unsafe A/B.

### New Features

 - <csr-id-c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926/> add shared KNN crate and unify Burn/cubeCL workspace deps
   Introduce flow-knn for portable graphs and ANN backends, pin Burn 0.21 with
   cubeCL 0.10 across GPU consumers, and path-patch pastey/bit-vec for sandbox builds.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Merge pull request #22 from jrmoynihan/feature/flow-autospectral ([`35007ba`](https://github.com/jrmoynihan/flow/commit/35007ba7841c164cf8e4ffae220a782dcb87d7ce))
    - Add performance-guided development map and host calibrator ([`e33bbe7`](https://github.com/jrmoynihan/flow/commit/e33bbe75d3777e0b8f7c8f5ebe0d71259ab9b4ef))
</details>

## 0.1.2 (2026-08-18)

### Documentation

 - <csr-id-3b118cede174e11c18a64c25e913416654b8c26e/> polish release notes and README install versions
   Add first-publish summaries for unpublished crates and bump install examples to the versions cargo-smart-release will publish.
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

 - <csr-id-c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926/> add shared KNN crate and unify Burn/cubeCL workspace deps
   Introduce flow-knn for portable graphs and ANN backends, pin Burn 0.21 with
   cubeCL 0.10 across GPU consumers, and path-patch pastey/bit-vec for sandbox builds.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release over the course of 16 calendar days.
 - 99 days passed between releases.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs-compress v0.1.4, flow-linalg v0.1.3, flow-fcs v0.5.2, flow-knn v0.1.1, flow-dimensional-reduction v0.1.0, flow-pacmap v0.1.2, flow-density v0.1.2, flow-plots v0.3.3, flow-clustering v0.1.2, flow-gates v0.5.0, flow-peak-detection v0.1.0, flow-control-detection v0.1.0, flow-tru-ols v0.2.0, peacoqc-rs v0.3.3, peacoqc-cli v0.3.0 ([`63fb593`](https://github.com/jrmoynihan/flow/commit/63fb5937185fba6f342ae3ae5b960a4df4c2bc72))
    - Polish release notes and README install versions ([`3b118ce`](https://github.com/jrmoynihan/flow/commit/3b118cede174e11c18a64c25e913416654b8c26e))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Add shared KNN crate and unify Burn/cubeCL workspace deps ([`c9223d6`](https://github.com/jrmoynihan/flow/commit/c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926))
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

 - <csr-id-fe642f65aa7d03fac6d688f4598143d8c2955137/> split flow-utils into flow-density + flow-clustering crates
   flow-utils bundled unrelated concerns (KDE, clustering, PCA). Split into
   focused crates so consumers only pull what they need. flow-utils removed
   from workspace members; existing code updated to import from new crates.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-linalg v0.1.0, flow-density v0.1.0, flow-clustering v0.1.0, flow-fcs-compress v0.1.0, flow-fcs v0.4.0 ([`e8c908e`](https://github.com/jrmoynihan/flow/commit/e8c908ef92fb68b8e2d01d3c1e8d6a294c8c6bda))
    - Bump flow-fcs to 0.4.0, add publish metadata to new crates ([`74956f9`](https://github.com/jrmoynihan/flow/commit/74956f94c544d1fa83f6fffbb18e2d4f5e6072ff))
    - Split flow-utils into flow-density + flow-clustering crates ([`fe642f6`](https://github.com/jrmoynihan/flow/commit/fe642f65aa7d03fac6d688f4598143d8c2955137))
</details>

