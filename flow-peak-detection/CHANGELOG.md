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

 - <csr-id-4da5c1ad5fe8c7141777497c11ef40b8768ec537/> add flow-peak-detection and flow-control-detection crates
   KDE-based histogram peak finding and filename heuristics for classifying unstained/single-stain control files.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release over the course of 26 calendar days.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Add crates.io metadata for unpublished crates ([`9ee153e`](https://github.com/jrmoynihan/flow/commit/9ee153e04507d454f9509a4d2b2a2b3ffe9db17b))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Add flow-peak-detection and flow-control-detection crates ([`4da5c1a`](https://github.com/jrmoynihan/flow/commit/4da5c1ad5fe8c7141777497c11ef40b8768ec537))
</details>

