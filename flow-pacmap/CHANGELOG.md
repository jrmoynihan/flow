# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.1 (2026-07-22)

### New Features

 - <csr-id-5df4317832bb258a0adb6a0989a2d01d7ca04404/> add PaCMAP dimensionality reduction crate
   Large-n embedding with faer PCA and optional HNSW/k-d tree KNN for flow cytometry datasets.

### Chore

 - <csr-id-ba96d7fb2b887ab666a3ecdea9f9f49b0cbbf3f4/> prepare 0.1.1 release with Wang et al. attribution
   Bump flow-pacmap to 0.1.1, pin the README install line, and scrub changelog
   wording that celebrated neutralizing academic attribution.
 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> polish pacmap/linalg/gates for crates.io release
   Neutralize third-party publication attribution in flow-pacmap, add
   README and publish metadata, and refresh install/API notes for the
   upcoming flow-linalg and flow-gates releases.

### Documentation

 - <csr-id-2db29b5e2ba3f6483b4604c4c2d4cebe81585fca/> restore Wang et al. 2021 academic attribution
   Crate docs, `Cargo.toml` description, and README References now cite
   Wang, Huang, Rudin & Shaposhnik (2021), JMLR 22 (PaCMAP Algorithm 1).
 - <csr-id-1d5806048f15f590ebe7b2ba449501aa73868b95/> polish changelogs for pacmap, linalg, and gates releases

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Prepare 0.1.1 release with Wang et al. attribution ([`ba96d7f`](https://github.com/jrmoynihan/flow/commit/ba96d7fb2b887ab666a3ecdea9f9f49b0cbbf3f4))
    - Restore Wang et al. 2021 attribution ([`2db29b5`](https://github.com/jrmoynihan/flow/commit/2db29b5e2ba3f6483b4604c4c2d4cebe81585fca))
    - Release flow-pacmap v0.1.0, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`e29c820`](https://github.com/jrmoynihan/flow/commit/e29c820dd65493c3a41f437b0e8f850c3cef8102))
    - Polish changelogs for pacmap, linalg, and gates releases ([`1d58060`](https://github.com/jrmoynihan/flow/commit/1d5806048f15f590ebe7b2ba449501aa73868b95))
    - Polish pacmap/linalg/gates for crates.io release ([`c9b7448`](https://github.com/jrmoynihan/flow/commit/c9b7448fef935e2ba6f3ea568ce092f9c777b53b))
    - Add PaCMAP dimensionality reduction crate ([`5df4317`](https://github.com/jrmoynihan/flow/commit/5df4317832bb258a0adb6a0989a2d01d7ca04404))
</details>

## 0.1.0 (2026-07-22)

<csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/>

### Chore

 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> polish pacmap/linalg/gates for crates.io release
   Add README and publish metadata for flow-pacmap, and refresh install/API
   notes for the upcoming flow-linalg and flow-gates releases.

### New Features

 - <csr-id-5df4317832bb258a0adb6a0989a2d01d7ca04404/> add PaCMAP dimensionality reduction crate
   Large-n 2D embedding with three-phase pair-weighted optimization, faer PCA
   initialization, and optional HNSW / k-d tree nearest-neighbour search.

### Documentation

 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> add README and crates.io publish metadata
   Crate description, repository/readme fields, and a minimal README for the
   first publish.
 - <csr-id-1d5806048f15f590ebe7b2ba449501aa73868b95/> polish changelogs for pacmap, linalg, and gates releases

