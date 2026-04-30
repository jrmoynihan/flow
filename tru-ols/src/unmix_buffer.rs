//! Reusable scratch buffers for [`crate::unmixing::TruOls`] hot paths.

/// Workspace for a single event: avoids repeated heap allocations inside the truncation loop.
#[allow(dead_code)]
pub(crate) struct UnmixScratch {
    /// Staging for one event’s raw detector row (disjoint from [`Self::adjusted_observation`] for borrow splitting).
    pub(crate) row_obs: Vec<f64>,
    pub(crate) working_m: Vec<f64>,
    pub(crate) adjusted_observation: Vec<f64>,
    pub(crate) b_rhs: Vec<f64>,
    pub(crate) x_out: Vec<f64>,
    pub(crate) gram: Vec<f64>,
    pub(crate) rhs_gram: Vec<f64>,
    pub(crate) current_indices: Vec<usize>,
    pub(crate) to_remove: Vec<(usize, usize, f64)>,
}

impl UnmixScratch {
    pub(crate) fn new(n_detectors: usize, n_endmembers: usize) -> Self {
        let cap_m = n_detectors * n_endmembers;
        Self {
            row_obs: vec![0.0_f64; n_detectors],
            working_m: vec![0.0_f64; cap_m],
            adjusted_observation: vec![0.0_f64; n_detectors],
            b_rhs: vec![0.0_f64; n_detectors],
            x_out: vec![0.0_f64; n_endmembers],
            gram: vec![0.0_f64; n_endmembers * n_endmembers.max(1)],
            rhs_gram: vec![0.0_f64; n_endmembers],
            current_indices: Vec::with_capacity(n_endmembers),
            to_remove: Vec::with_capacity(n_endmembers),
        }
    }
}

/// Copy `source` mixing matrix (column-major) into scratch and reset column index list to `0..n_em`.
pub(crate) fn copy_mixing_into_scratch(
    scratch: &mut UnmixScratch,
    source: faer::MatRef<'_, f64>,
    n_detectors: usize,
    n_endmembers: usize,
) {
    debug_assert_eq!(source.nrows(), n_detectors);
    debug_assert_eq!(source.ncols(), n_endmembers);
    for j in 0..n_endmembers {
        for i in 0..n_detectors {
            scratch.working_m[j * n_detectors + i] = source[(i, j)];
        }
    }
    scratch.current_indices.clear();
    scratch.current_indices.extend(0..n_endmembers);
}

/// Swap two columns in a column-major `nrows × ncols` matrix stored in `buf`.
pub(crate) fn swap_columns_cm(buf: &mut [f64], nrows: usize, _ncols: usize, c0: usize, c1: usize) {
    if c0 == c1 {
        return;
    }
    for r in 0..nrows {
        let i = c0 * nrows + r;
        let j = c1 * nrows + r;
        buf.swap(i, j);
    }
}

/// Swap entries `current_indices[i]` and `current_indices[j]`.
pub(crate) fn swap_index_entries(current_indices: &mut [usize], i: usize, j: usize) {
    current_indices.swap(i, j);
}

/// Build a `u128` bitmask of active global endmember indices (first `active_cols` entries).
#[cfg(feature = "unmix-cache")]
pub(crate) fn active_global_mask(current_indices: &[usize], active_cols: usize) -> u128 {
    let mut m = 0u128;
    for i in 0..active_cols {
        let g = current_indices[i];
        debug_assert!(
            g < crate::MAX_ENDMEMBERS_DEFAULT,
            "global endmember index must be < 128 without large-panels"
        );
        m |= 1u128 << g;
    }
    m
}
