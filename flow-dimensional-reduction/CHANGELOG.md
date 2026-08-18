# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### New Features

 - <csr-id-a803000a61436642b87ad70d6e1edd4deaeab54f/> add faer covariance-method PCA
   Rehomes the orphaned utils/src/pca as a workspace member. Uses the
   covariance method (d*d SVD) rather than data-matrix SVD, and accumulates
   means and covariance in f64 to avoid f32 accumulation error at large n.

### Bug Fixes

 - <csr-id-6b9125bd7a1001ac9e0f80691c3435f713de6dc8/> pin numerics in tests, harden validation
   Adds tests that fail if the U column-to-axis mapping is transposed or the
   transform centering is dropped — both previously survived mutation with a
   green suite. Pins the compile_fail doctest to E0599. Guards n*d against
   release-mode wrapping. Adds publish metadata and README.
 - <csr-id-5f96107800f27d9e5d64737ed8556118bb890b6c/> collapse double SVD, drop identity_op
   Review round 1: read U and S from a single Svd::<f64>::new(cov) call
   instead of pairing a standalone singular_values() with a separate Svd
   decomposition — the two calls had no guaranteed shared ordering. S()
   returns a DiagRef; go through column_vector().iter() to get the values.
   
   Also fix the brief's verbatim `n * 1` test assertion to `n`, since
   Task 2's lint gate (clippy --all-targets) lints test code and would
   otherwise fail on clippy::identity_op.

### New Features (BREAKING)

 - <csr-id-c8c164a0378fa1ee4c222a53ca50425cd0b88c2d/> gate transform behind fitted typestate

### Refactor (BREAKING)

 - <csr-id-796c04d84cfbda9591b6600519c771a3ff8fbbab/> flat components slice, scoped state types
   components() now returns &[f32] (k*d row-major) plus components_shape(),
   removing faer from the public API and the column-major/row-major mismatch.
   The basis is stored row-major rather than converted on read.
   
   State types renamed Unfitted/Fitted and moved to pca::state, resolving the
   collision with the PcaResult<T> alias. Root exports drop to four. Sealing
   is preserved via the still-private pca::sealed module.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release over the course of 15 calendar days.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Merge branch 'worktree-pr1-pca-typestate' ([`2cb86fb`](https://github.com/jrmoynihan/flow/commit/2cb86fbea24f6605f550e5af9830c77f8710f17b))
    - Flat components slice, scoped state types ([`796c04d`](https://github.com/jrmoynihan/flow/commit/796c04d84cfbda9591b6600519c771a3ff8fbbab))
    - Pin numerics in tests, harden validation ([`6b9125b`](https://github.com/jrmoynihan/flow/commit/6b9125bd7a1001ac9e0f80691c3435f713de6dc8))
    - Gate transform behind fitted typestate ([`c8c164a`](https://github.com/jrmoynihan/flow/commit/c8c164a0378fa1ee4c222a53ca50425cd0b88c2d))
    - Collapse double SVD, drop identity_op ([`5f96107`](https://github.com/jrmoynihan/flow/commit/5f96107800f27d9e5d64737ed8556118bb890b6c))
    - Add faer covariance-method PCA ([`a803000`](https://github.com/jrmoynihan/flow/commit/a803000a61436642b87ad70d6e1edd4deaeab54f))
</details>

