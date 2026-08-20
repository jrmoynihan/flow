// Inner joint unmix, monomorphized via `type S` in the parent modules.

use crate::config::{JointUnmixConfig, force_sequential, quantile_type7};
use crate::error::{AutospectralError, Result};
use crate::library::AfLibrary;
use crate::variants::SpectralVariants;
use faer::linalg::solvers::Llt;
use faer::prelude::Solve;
use faer::{Mat, MatRef, Side};
use rayon::prelude::*;
use std::cell::RefCell;

pub(super) struct InnerResult {
    pub abundances: Vec<S>,
    pub n_events: usize,
    pub n_fluor: usize,
    pub af_index: Vec<usize>,
    pub variant_index: Vec<Option<usize>>,
}

pub(super) fn unmix_autospectral_joint_s(
    events_row_major: &[S],
    n_events: usize,
    fluor_matrix: MatRef<'_, S>,
    fluor_names: &[String],
    af_library: &AfLibrary,
    variants: &SpectralVariants,
    config: &JointUnmixConfig,
) -> Result<InnerResult> {
    let pre = JointPrecomp::build(
        events_row_major,
        n_events,
        fluor_matrix,
        fluor_names,
        af_library,
        variants,
        config,
    )?;
    pre.run(events_row_major, n_events, config)
}

struct FluorPrecomp {
    master_idx: usize,
    n_variants: usize,
    v_mats: Mat<S>,
    r_lib: Mat<S>,
    r_lib_sq: Mat<S>,
    r_dots: Vec<S>,
    v_lib: Mat<S>,
    w_leakage: Vec<S>,
    other_indices: Vec<usize>,
}

struct JointPrecomp {
    n_det: usize,
    n_fluor: usize,
    n_af: usize,
    spectra_df: Mat<S>,
    spectra_fd: Mat<S>,
    p_fd: Mat<S>,
    sst: Mat<S>,
    af_dk: Mat<S>,
    v_lib_af: Mat<S>,
    r_lib_af: Mat<S>,
    r_lib_af_w2: Mat<S>,
    r_dots_af: Vec<S>,
    r_dots_af_raw: Vec<S>,
    w_af: Vec<S>,
    sqrt_w_global: Vec<S>,
    noise_floor: Vec<S>,
    thresholds: Vec<S>,
    fluors: Vec<FluorPrecomp>,
    is_collinear: Vec<bool>,
    af_only: bool,
    cell_weight: bool,
}

/// Reused per-event buffers (one sequential instance, or one per Rayon worker).
struct EventScratch {
    n_det: usize,
    n_fluor: usize,
    n_af: usize,
    max_var: usize,
    n_active: usize,
    init_f: Vec<S>,
    base_resid: Vec<S>,
    k_af_buf: Vec<S>,
    sqrt_w: Vec<S>,
    fluor_unmixed: Vec<S>,
    resid_raw: Vec<S>,
    resid: Vec<S>,
    y_vec: Vec<S>,
    b_base: Vec<S>,
    w_eff: Vec<S>,
    rsw: Vec<S>,
    cross_v: Vec<S>,
    drsq_v: Vec<S>,
    other_unmixed: Vec<S>,
    g_cur: Vec<S>,
    dr: Vec<S>,
    best_v: Vec<isize>,
    committed: Vec<bool>,
    committed_dr: Vec<S>,
    committed_norm: Vec<S>,
    committed_ai: Vec<usize>,
    n_committed: usize,
    commits: Vec<(usize, usize)>,
    queued: Vec<(usize, usize)>,
    candidates: Vec<(S, usize, usize)>,
    cell_s: Mat<S>,
    cell_s_f_w: Mat<S>,
    a_base: Mat<S>,
    a_trial: Mat<S>,
    rhs: Mat<S>,
    s_new: Vec<S>,
    prev_row: Vec<S>,
    col_update: Vec<S>,
    b_trial: Vec<S>,
    trial_unmixed: Vec<S>,
    trial_resid_raw: Vec<S>,
    trial_resid: Vec<S>,
    relu_f: Vec<S>,
    coeff: Vec<S>,
    y_w: Vec<S>,
    y_hat: Vec<S>,
    y2: Vec<S>,
    q_flat: Vec<S>,
    q_ready: Vec<u8>,
    cell_weight_s_w: Mat<S>,
    cell_s_copied: bool,
    cell_s_f_w_copied: bool,
    a_base_copied: bool,
}

impl EventScratch {
    fn empty() -> Self {
        Self::with_dims(0, 0, 0, 0, 0)
    }

    fn with_dims(
        n_det: usize,
        n_fluor: usize,
        n_af: usize,
        max_var: usize,
        n_active: usize,
    ) -> Self {
        Self {
            n_det,
            n_fluor,
            n_af,
            max_var,
            n_active,
            init_f: vec![0.0 as S; n_fluor],
            base_resid: vec![0.0 as S; n_det],
            k_af_buf: vec![0.0 as S; n_af],
            sqrt_w: vec![1.0 as S; n_det],
            fluor_unmixed: vec![0.0 as S; n_fluor],
            resid_raw: vec![0.0 as S; n_det],
            resid: vec![0.0 as S; n_det],
            y_vec: vec![0.0 as S; n_det],
            b_base: vec![0.0 as S; n_fluor],
            w_eff: vec![0.0 as S; n_det],
            rsw: vec![0.0 as S; n_det],
            cross_v: vec![0.0 as S; max_var],
            drsq_v: vec![0.0 as S; max_var],
            other_unmixed: vec![0.0 as S; n_fluor],
            g_cur: vec![0.0 as S; max_var],
            dr: vec![0.0 as S; n_det],
            best_v: vec![-1; n_active],
            committed: vec![false; n_active],
            committed_dr: vec![0.0 as S; n_active.saturating_mul(n_det)],
            committed_norm: vec![0.0 as S; n_active],
            committed_ai: vec![0; n_active],
            n_committed: 0,
            commits: Vec::new(),
            queued: Vec::new(),
            candidates: Vec::new(),
            cell_s: Mat::<S>::zeros(n_fluor, n_det),
            cell_s_f_w: Mat::<S>::zeros(n_fluor, n_det),
            a_base: Mat::<S>::zeros(n_fluor, n_fluor),
            a_trial: Mat::<S>::zeros(n_fluor, n_fluor),
            rhs: Mat::<S>::zeros(n_fluor, 1),
            s_new: vec![0.0 as S; n_det],
            prev_row: vec![0.0 as S; n_det],
            col_update: vec![0.0 as S; n_fluor],
            b_trial: vec![0.0 as S; n_fluor],
            trial_unmixed: vec![0.0 as S; n_fluor],
            trial_resid_raw: vec![0.0 as S; n_det],
            trial_resid: vec![0.0 as S; n_det],
            relu_f: vec![0.0 as S; n_fluor],
            coeff: vec![0.0 as S; n_fluor],
            y_w: vec![0.0 as S; n_det],
            y_hat: vec![0.0 as S; n_det],
            y2: vec![0.0 as S; n_det],
            q_flat: vec![0.0 as S; n_active.saturating_mul(max_var)],
            q_ready: vec![0; n_active],
            cell_weight_s_w: Mat::<S>::zeros(n_fluor, n_det),
            cell_s_copied: false,
            cell_s_f_w_copied: false,
            a_base_copied: false,
        }
    }

    fn ensure(&mut self, pre: &JointPrecomp) {
        let n_active = pre.fluors.len();
        let max_var = pre.fluors.iter().map(|f| f.n_variants).max().unwrap_or(0);
        if self.n_det != pre.n_det
            || self.n_fluor != pre.n_fluor
            || self.n_af != pre.n_af
            || self.max_var != max_var
            || self.n_active != n_active
        {
            *self = Self::with_dims(pre.n_det, pre.n_fluor, pre.n_af, max_var, n_active);
        }
    }

    fn begin_event(&mut self) {
        self.cell_s_copied = false;
        self.cell_s_f_w_copied = false;
        self.a_base_copied = false;
        self.n_committed = 0;
        self.q_ready.fill(0);
        self.best_v.fill(-1);
        self.commits.clear();
        self.queued.clear();
        self.candidates.clear();
    }

    fn ensure_cell_s(&mut self, spectra_fd: MatRef<'_, S>) {
        if !self.cell_s_copied {
            copy_mat_into(spectra_fd, &mut self.cell_s);
            self.cell_s_copied = true;
        }
    }

    fn ensure_cell_s_f_w(&mut self, spectra_fd: MatRef<'_, S>) {
        if !self.cell_s_f_w_copied {
            copy_mat_into(spectra_fd, &mut self.cell_s_f_w);
            self.cell_s_f_w_copied = true;
        }
    }

    fn ensure_a_base(&mut self, sst: MatRef<'_, S>) {
        if !self.a_base_copied {
            copy_mat_into(sst, &mut self.a_base);
            self.a_base_copied = true;
        }
    }
}

thread_local! {
    static EVENT_SCRATCH: RefCell<EventScratch> = RefCell::new(EventScratch::empty());
}

fn with_thread_scratch<R>(pre: &JointPrecomp, f: impl FnOnce(&mut EventScratch) -> R) -> R {
    EVENT_SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        scratch.ensure(pre);
        f(&mut scratch)
    })
}

impl JointPrecomp {
    fn build(
        events_row_major: &[S],
        n_events: usize,
        fluor_matrix: MatRef<'_, S>,
        fluor_names: &[String],
        af_library: &AfLibrary,
        variants: &SpectralVariants,
        config: &JointUnmixConfig,
    ) -> Result<Self> {
        if n_events == 0 {
            return Err(AutospectralError::EmptyEvents);
        }
        if config.n_af_passes < 1 {
            return Err(AutospectralError::InvalidConfig(
                "n_af_passes must be >= 1".into(),
            ));
        }
        if !(0.0..=1.0).contains(&config.refine_af_quantile) {
            return Err(AutospectralError::InvalidConfig(
                "refine_af_quantile must be between 0 and 1".into(),
            ));
        }
        let n_det = fluor_matrix.nrows();
        let n_fluor = fluor_matrix.ncols();
        if n_det == 0 || n_fluor == 0 {
            return Err(AutospectralError::InvalidConfig(
                "fluor_matrix must have at least one detector and one fluorophore".into(),
            ));
        }
        if events_row_major.len() != n_events * n_det {
            return Err(AutospectralError::DetectorMismatch {
                expected: n_det,
                got: events_row_major.len().checked_div(n_events.max(1)).unwrap_or(0),
            });
        }
        if fluor_names.len() != n_fluor {
            return Err(AutospectralError::InvalidConfig(format!(
                "fluor_names length {} != mixing columns {}",
                fluor_names.len(),
                n_fluor
            )));
        }
        if af_library.n_detectors() != n_det {
            return Err(AutospectralError::DetectorMismatch {
                expected: n_det,
                got: af_library.n_detectors(),
            });
        }
        if af_library.n_signatures() == 0 {
            return Err(AutospectralError::EmptyLibrary);
        }
        if !variants.thresholds.is_empty() && variants.thresholds.len() != n_fluor {
            return Err(AutospectralError::InvalidConfig(
                "variant thresholds length must match fluorophore count".into(),
            ));
        }

        let n_af = af_library.n_signatures();
        let noise_floor = resolve_noise_floor(n_det, config)?;
        let (w_global, sqrt_w_global) = global_weights(
            events_row_major,
            n_events,
            n_det,
            &noise_floor,
            config.cell_weight,
        );

        let spectra_df = fluor_matrix.to_owned();
        let spectra_fd = transpose_mat(spectra_df.as_ref());
        let mut spectra_w = Mat::<S>::zeros(n_fluor, n_det);
        for f in 0..n_fluor {
            for d in 0..n_det {
                spectra_w[(f, d)] = spectra_fd[(f, d)] * sqrt_w_global[d];
            }
        }
        let sst: Mat<S> = &spectra_w * spectra_w.transpose();
        let mut p_fd = solve_square(sst.as_ref(), spectra_w.as_ref())?;
        for f in 0..n_fluor {
            for d in 0..n_det {
                p_fd[(f, d)] *= sqrt_w_global[d];
            }
        }

        let af_dk = mat_cast(af_library.signatures.as_ref());
        let v_lib_af: Mat<S> = &p_fd * &af_dk;
        let r_lib_af: Mat<S> = &af_dk - &spectra_df * &v_lib_af;

        let mut r_dots_af = vec![0.0 as S; n_af];
        let mut r_dots_af_raw = vec![0.0 as S; n_af];
        let mut r_lib_af_w2 = Mat::<S>::zeros(n_det, n_af);
        for j in 0..n_af {
            let mut wr = 0.0 as S;
            let mut raw = 0.0 as S;
            for d in 0..n_det {
                let r = r_lib_af[(d, j)];
                raw += r * r;
                let rw = r * sqrt_w_global[d];
                wr += rw * rw;
                r_lib_af_w2[(d, j)] = r * w_global[d] * w_global[d];
            }
            r_dots_af[j] = wr.max(1e-10 as S);
            r_dots_af_raw[j] = raw;
        }

        let af_cov = sample_cov_columns(af_dk.as_ref());
        let af_cov_unmix: Mat<S> = &p_fd * &af_cov * p_fd.transpose();
        let mut w_af = vec![0.0 as S; n_fluor];
        for f in 0..n_fluor {
            w_af[f] = af_cov_unmix[(f, f)].abs().sqrt() + (1e-8 as S);
        }

        let af_only = variants.is_empty();
        let mut fluors = Vec::new();
        if !af_only {
            for (name, vmat) in &variants.variants {
                let Some(master_idx) = fluor_names.iter().position(|n| n == name) else {
                    continue;
                };
                if vmat.ncols() == 0 || vmat.nrows() != n_det {
                    continue;
                }
                let vmat_s = mat_cast(vmat.as_ref());
                let delta_obs = variants
                    .deltas
                    .get(name)
                    .map(|m| mat_cast(m.as_ref()))
                    .unwrap_or_else(|| {
                        let mut dlt = Mat::<S>::zeros(n_det, vmat_s.ncols());
                        for v in 0..vmat_s.ncols() {
                            for d in 0..n_det {
                                dlt[(d, v)] = vmat_s[(d, v)] - spectra_df[(d, master_idx)];
                            }
                        }
                        dlt
                    });
                fluors.push(build_fluor_precomp(
                    master_idx,
                    vmat_s.as_ref(),
                    delta_obs.as_ref(),
                    spectra_df.as_ref(),
                    spectra_fd.as_ref(),
                    p_fd.as_ref(),
                )?);
            }
        }

        let mut is_collinear = vec![false; n_fluor * n_fluor];
        for a in 0..fluors.len() {
            for b in (a + 1)..fluors.len() {
                let fa = fluors[a].master_idx;
                let fb = fluors[b].master_idx;
                let c = row_cosine_abs(p_fd.as_ref(), fa, fb);
                if c > (config.collinear_threshold as S) {
                    is_collinear[fa * n_fluor + fb] = true;
                    is_collinear[fb * n_fluor + fa] = true;
                }
            }
        }

        let thresholds = if variants.thresholds.len() == n_fluor {
            variants.thresholds.iter().map(|&x| x as S).collect()
        } else {
            vec![0.0 as S; n_fluor]
        };

        Ok(Self {
            n_det,
            n_fluor,
            n_af,
            spectra_df,
            spectra_fd,
            p_fd,
            sst,
            af_dk,
            v_lib_af,
            r_lib_af,
            r_lib_af_w2,
            r_dots_af,
            r_dots_af_raw,
            w_af,
            sqrt_w_global,
            noise_floor,
            thresholds,
            fluors,
            is_collinear,
            af_only,
            cell_weight: config.cell_weight,
        })
    }

    fn run(
        &self,
        events_row_major: &[S],
        n_events: usize,
        config: &JointUnmixConfig,
    ) -> Result<InnerResult> {
        let parallel = !force_sequential()
            && n_events >= config.parallel_event_threshold
            && self.n_det > 0;
        let n_det = self.n_det;
        let n_fluor = self.n_fluor;

        let mut af_index = vec![0usize; n_events];
        let mut af_abund = vec![0.0 as S; n_events];
        let mut resid = vec![0.0 as S; n_events * n_det];

        if parallel {
            events_row_major
                .par_chunks(n_det)
                .zip(resid.par_chunks_mut(n_det))
                .zip(af_index.par_iter_mut())
                .zip(af_abund.par_iter_mut())
                .try_for_each(|(((y, rrow), af_j), af_k)| -> Result<()> {
                    with_thread_scratch(self, |scratch| {
                        let (j, k, _) = self.score_af(y, scratch)?;
                        *af_j = j;
                        *af_k = k;
                        axpy_col(self.af_dk.as_ref(), j, k, y, rrow);
                        Ok(())
                    })
                })?;
        } else {
            let mut scratch = EventScratch::empty();
            scratch.ensure(self);
            for i in 0..n_events {
                let y = &events_row_major[i * n_det..(i + 1) * n_det];
                let rrow = &mut resid[i * n_det..(i + 1) * n_det];
                let (j, k, _) = self.score_af(y, &mut scratch)?;
                af_index[i] = j;
                af_abund[i] = k;
                axpy_col(self.af_dk.as_ref(), j, k, y, rrow);
            }
        }

        if config.n_af_passes > 1 {
            #[allow(clippy::unnecessary_cast)]
            let af_abund_f64: Vec<f64> = af_abund.iter().map(|&x| x as f64).collect();
            #[allow(clippy::unnecessary_cast)]
            let cutoff = quantile_type7(&af_abund_f64, config.refine_af_quantile) as S;
            let mut still: Vec<u8> = af_abund
                .iter()
                .map(|a| u8::from(*a >= cutoff))
                .collect();
            for _ in 1..config.n_af_passes {
                if parallel {
                    resid
                        .par_chunks_mut(n_det)
                        .zip(still.par_iter_mut())
                        .zip(af_abund.par_iter_mut())
                        .try_for_each(|((rrow, keep), abund)| -> Result<()> {
                            if *keep == 0 {
                                return Ok(());
                            }
                            with_thread_scratch(self, |scratch| {
                                let (j_ref, k_ref, score) = self.score_af(rrow, scratch)?;
                                if score < (1.0 as S) {
                                    scale_sub_col(self.af_dk.as_ref(), j_ref, k_ref, rrow);
                                    *abund += k_ref;
                                    *keep = 1;
                                } else {
                                    *keep = 0;
                                }
                                Ok(())
                            })
                        })?;
                } else {
                    let mut scratch = EventScratch::empty();
                    scratch.ensure(self);
                    for i in 0..n_events {
                        if still[i] == 0 {
                            continue;
                        }
                        let rrow = &mut resid[i * n_det..(i + 1) * n_det];
                        let (j_ref, k_ref, score) = self.score_af(rrow, &mut scratch)?;
                        if score < (1.0 as S) {
                            scale_sub_col(self.af_dk.as_ref(), j_ref, k_ref, rrow);
                            af_abund[i] += k_ref;
                            still[i] = 1;
                        } else {
                            still[i] = 0;
                        }
                    }
                }
            }
        }

        let width = n_fluor + 1;
        let mut abundances = vec![0.0 as S; n_events * width];
        let mut variant_index = vec![None; n_events * n_fluor];

        if parallel {
            events_row_major
                .par_chunks(n_det)
                .zip(resid.par_chunks(n_det))
                .zip(abundances.par_chunks_mut(width))
                .zip(variant_index.par_chunks_mut(n_fluor))
                .zip(af_abund.par_iter())
                .try_for_each(|((((y, cell_resid), out_ab), var_idx), &k_af)| {
                    with_thread_scratch(self, |scratch| {
                        self.unmix_fluor_event(
                            y, cell_resid, k_af, config, scratch, out_ab, var_idx,
                        )
                    })
                })?;
        } else {
            let mut scratch = EventScratch::empty();
            scratch.ensure(self);
            for i in 0..n_events {
                let y = &events_row_major[i * n_det..(i + 1) * n_det];
                let cell_resid = &resid[i * n_det..(i + 1) * n_det];
                let out_ab = &mut abundances[i * width..(i + 1) * width];
                let var_idx = &mut variant_index[i * n_fluor..(i + 1) * n_fluor];
                self.unmix_fluor_event(
                    y,
                    cell_resid,
                    af_abund[i],
                    config,
                    &mut scratch,
                    out_ab,
                    var_idx,
                )?;
            }
        }

        Ok(InnerResult {
            abundances,
            n_events,
            n_fluor,
            af_index,
            variant_index,
        })
    }

    fn score_af(&self, y: &[S], scratch: &mut EventScratch) -> Result<(usize, S, S)> {
        let n_fluor = self.n_fluor;
        let n_af = self.n_af;
        gemv(self.p_fd.as_ref(), y, &mut scratch.init_f);
        gemv(
            self.spectra_df.as_ref(),
            &scratch.init_f[..n_fluor],
            &mut scratch.base_resid,
        );
        for (br, &yi) in scratch.base_resid.iter_mut().zip(y) {
            *br = yi - *br;
        }
        let base_resid_sq = dot(&scratch.base_resid, &scratch.base_resid).max(1e-16 as S);
        let base_resid_norm = base_resid_sq.sqrt();
        let mut base_fluor_l1 = 0.0 as S;
        for f in 0..n_fluor {
            base_fluor_l1 += self.w_af[f] * scratch.init_f[f].abs();
        }
        let base_fluor_l1 = base_fluor_l1.max(1e-8 as S);

        for j in 0..n_af {
            scratch.k_af_buf[j] =
                (col_dot(self.r_lib_af_w2.as_ref(), j, y) / self.r_dots_af[j]).max(0.0 as S);
        }

        let mut best_j = 0usize;
        let mut best_score = S::INFINITY;
        let mut best_k = 0.0 as S;
        for j in 0..n_af {
            let k = scratch.k_af_buf[j];
            let cross = col_dot(self.r_lib_af.as_ref(), j, &scratch.base_resid);
            let resid_sq = (base_resid_sq - (2.0 as S) * k * cross + k * k * self.r_dots_af_raw[j]).max(0.0 as S);
            let presid = resid_sq.sqrt() / base_resid_norm;
            let mut leak = 0.0 as S;
            for f in 0..n_fluor {
                let diff = self.v_lib_af[(f, j)] * k - scratch.init_f[f];
                leak += self.w_af[f] * diff.abs();
            }
            let pfluor = leak / base_fluor_l1;
            let score = presid * pfluor;
            if score < best_score {
                best_score = score;
                best_j = j;
                best_k = k;
            }
        }
        Ok((best_j, best_k, best_score))
    }

    #[allow(clippy::too_many_arguments)]
    fn unmix_fluor_event(
        &self,
        cell_raw: &[S],
        cell_resid: &[S],
        k_af: S,
        config: &JointUnmixConfig,
        scratch: &mut EventScratch,
        out_ab: &mut [S],
        var_idx: &mut [Option<usize>],
    ) -> Result<()> {
        let n_det = self.n_det;
        let n_fluor = self.n_fluor;
        scratch.begin_event();
        var_idx.fill(None);
        scratch.sqrt_w.fill(1.0 as S);

        if self.cell_weight {
            for d in 0..n_det {
                scratch.y_w[d] = cell_resid[d] * self.sqrt_w_global[d];
            }
            for d in 0..n_det {
                let sw = self.sqrt_w_global[d];
                if let (Some(src), Some(dst)) = (
                    col_slice(self.spectra_fd.as_ref(), d),
                    col_slice_mut(&mut scratch.cell_weight_s_w, d),
                ) {
                    for (o, &s) in dst.iter_mut().zip(src) {
                        *o = s * sw;
                    }
                } else {
                    for f in 0..n_fluor {
                        scratch.cell_weight_s_w[(f, d)] = self.spectra_fd[(f, d)] * sw;
                    }
                }
            }
            weighted_lstsq(
                scratch.cell_weight_s_w.as_ref(),
                &scratch.y_w[..n_det],
                &mut scratch.coeff[..n_fluor],
                &mut scratch.rhs,
            )?;
            gemv(
                self.spectra_df.as_ref(),
                &scratch.coeff[..n_fluor],
                &mut scratch.y_hat,
            );
            for d in 0..n_det {
                scratch.y_hat[d] += cell_raw[d] - cell_resid[d];
                scratch.sqrt_w[d] =
                    1.0 / scratch.y_hat[d].abs().max(self.noise_floor[d]).sqrt();
            }
            for d in 0..n_det {
                let sw = scratch.sqrt_w[d];
                if let (Some(src), Some(dst)) = (
                    col_slice(self.spectra_fd.as_ref(), d),
                    col_slice_mut(&mut scratch.cell_s_f_w, d),
                ) {
                    for (o, &s) in dst.iter_mut().zip(src) {
                        *o = s * sw;
                    }
                } else {
                    for f in 0..n_fluor {
                        scratch.cell_s_f_w[(f, d)] = self.spectra_fd[(f, d)] * sw;
                    }
                }
            }
            scratch.cell_s_f_w_copied = true;
            for d in 0..n_det {
                scratch.y2[d] = cell_resid[d] * scratch.sqrt_w[d];
            }
            weighted_lstsq(
                scratch.cell_s_f_w.as_ref(),
                &scratch.y2[..n_det],
                &mut scratch.fluor_unmixed[..n_fluor],
                &mut scratch.rhs,
            )?;
        } else {
            gemv(
                self.p_fd.as_ref(),
                cell_resid,
                &mut scratch.fluor_unmixed,
            );
        }

        for f in 0..n_fluor {
            scratch.relu_f[f] = scratch.fluor_unmixed[f].max(0.0 as S);
        }
        gemv(
            self.spectra_df.as_ref(),
            &scratch.relu_f[..n_fluor],
            &mut scratch.resid_raw,
        );
        for d in 0..n_det {
            scratch.resid_raw[d] = cell_resid[d] - scratch.resid_raw[d];
            scratch.resid[d] = scratch.resid_raw[d] * scratch.sqrt_w[d];
        }

        if self.af_only {
            out_ab[..n_fluor].copy_from_slice(&scratch.fluor_unmixed[..n_fluor]);
            out_ab[n_fluor] = k_af;
            return Ok(());
        }

        if self.cell_weight {
            scratch.a_base = &scratch.cell_s_f_w * scratch.cell_s_f_w.transpose();
            scratch.a_base_copied = true;
            for d in 0..n_det {
                scratch.y_vec[d] = cell_resid[d] * scratch.sqrt_w[d];
                scratch.w_eff[d] = scratch.sqrt_w[d] * scratch.sqrt_w[d];
            }
            gemv(
                scratch.cell_s_f_w.as_ref(),
                &scratch.y_vec[..n_det],
                &mut scratch.b_base,
            );
        } else {
            scratch.y_vec[..n_det].copy_from_slice(cell_resid);
            gemv(
                self.spectra_fd.as_ref(),
                &scratch.y_vec[..n_det],
                &mut scratch.b_base,
            );
        }

        let cell_resid_ss = dot(cell_resid, cell_resid);
        let n_active = self.fluors.len();

        for _pass in 0..config.n_passes {
            let rss_curr = dot(&scratch.resid[..n_det], &scratch.resid[..n_det]).max(1e-12 as S);
            let rss_curr_sqrt = rss_curr.sqrt();
            let ratio_thresh_sq = (1.1025 as S) * rss_curr;
            let mut rss_accepted = rss_curr;
            if self.cell_weight {
                for d in 0..n_det {
                    scratch.rsw[d] = scratch.resid[d] * scratch.sqrt_w[d];
                }
            } else {
                scratch.rsw[..n_det].copy_from_slice(&scratch.resid[..n_det]);
            }

            scratch.candidates.clear();
            for ai in 0..n_active {
                let pc = &self.fluors[ai];
                let abund = scratch.fluor_unmixed[pc.master_idx];
                if abund < self.thresholds[pc.master_idx] {
                    continue;
                }
                let n_other = pc.other_indices.len();
                for (o, &fi) in pc.other_indices.iter().enumerate() {
                    scratch.other_unmixed[o] = scratch.fluor_unmixed[fi];
                }
                let mut base_leakage = 0.0 as S;
                for o in 0..n_other {
                    base_leakage += pc.w_leakage[o] * scratch.other_unmixed[o].abs();
                }
                let base_leakage = base_leakage.max(1e-8 as S);
                let cur_v = scratch.best_v[ai];

                if self.cell_weight && scratch.q_ready[ai] == 0 {
                    let start = ai * scratch.max_var;
                    for v in 0..pc.n_variants {
                        scratch.q_flat[start + v] =
                            col_dot(pc.r_lib_sq.as_ref(), v, &scratch.w_eff[..n_det]);
                    }
                    scratch.q_ready[ai] = 1;
                }

                for v in 0..pc.n_variants {
                    scratch.cross_v[v] = col_dot(pc.r_lib.as_ref(), v, &scratch.rsw[..n_det]);
                }
                if cur_v < 0 {
                    if self.cell_weight {
                        let start = ai * scratch.max_var;
                        scratch.drsq_v[..pc.n_variants]
                            .copy_from_slice(&scratch.q_flat[start..start + pc.n_variants]);
                    } else {
                        scratch.drsq_v[..pc.n_variants].copy_from_slice(&pc.r_dots);
                    }
                } else {
                    let cv = cur_v as usize;
                    for v in 0..pc.n_variants {
                        scratch.g_cur[v] = if self.cell_weight {
                            col_col_dot_w(
                                pc.r_lib.as_ref(),
                                v,
                                cv,
                                &scratch.w_eff[..n_det],
                            )
                        } else {
                            col_col_dot(pc.r_lib.as_ref(), v, cv)
                        };
                    }
                    if self.cell_weight {
                        let start = ai * scratch.max_var;
                        for v in 0..pc.n_variants {
                            scratch.drsq_v[v] = scratch.q_flat[start + v]
                                + scratch.q_flat[start + cv]
                                - (2.0 as S) * scratch.g_cur[v];
                        }
                    } else {
                        for v in 0..pc.n_variants {
                            scratch.drsq_v[v] =
                                pc.r_dots[v] + pc.r_dots[cv] - (2.0 as S) * scratch.g_cur[v];
                        }
                    }
                    let cross_cur = scratch.cross_v[cv];
                    for v in 0..pc.n_variants {
                        scratch.cross_v[v] -= cross_cur;
                    }
                }

                let abund2 = abund * abund;
                for v in 0..pc.n_variants {
                    let new_rss =
                        rss_curr - (2.0 as S) * abund * scratch.cross_v[v] + abund2 * scratch.drsq_v[v];
                    if new_rss > ratio_thresh_sq {
                        continue;
                    }
                    let mut leak_num = 0.0 as S;
                    for o in 0..n_other {
                        let vl = pc.v_lib[(o, v)];
                        let delta_l = if cur_v < 0 {
                            abund * vl
                        } else {
                            abund * (vl - pc.v_lib[(o, cur_v as usize)])
                        };
                        leak_num += pc.w_leakage[o] * (scratch.other_unmixed[o] - delta_l).abs();
                    }
                    let leakage_ratio = leak_num / base_leakage;
                    let resid_ratio = new_rss.max(0.0 as S).sqrt() / rss_curr_sqrt;
                    let joint_score = resid_ratio.max(1e-8 as S).powf(config.alpha as S)
                        * leakage_ratio.max(1e-8 as S).powf((1.0 - config.alpha) as S);
                    if joint_score < (1.0 as S) {
                        scratch.candidates.push((joint_score, ai, v));
                    }
                }
            }

            if scratch.candidates.is_empty() {
                break;
            }
            scratch.candidates.sort_by(|a, b| a.0.total_cmp(&b.0));

            scratch.committed.fill(false);
            scratch.n_committed = 0;
            scratch.commits.clear();
            scratch.queued.clear();

            for &(_score, ai, v) in &scratch.candidates.clone() {
                if scratch.committed[ai] {
                    continue;
                }
                let pc = &self.fluors[ai];
                let abund = scratch.fluor_unmixed[pc.master_idx];
                let cur_v = scratch.best_v[ai];
                if cur_v < 0 {
                    scale_col_into(pc.r_lib.as_ref(), v, abund, &mut scratch.dr);
                } else {
                    let cv = cur_v as usize;
                    scale_col_diff_into(pc.r_lib.as_ref(), v, cv, abund, &mut scratch.dr);
                }
                let dr_norm = norm(&scratch.dr[..n_det]).max(1e-12 as S);
                let mut conflict = false;
                for c in 0..scratch.n_committed {
                    let start = c * n_det;
                    let cosine = dot(
                        &scratch.dr[..n_det],
                        &scratch.committed_dr[start..start + n_det],
                    )
                    .abs()
                        / (dr_norm * scratch.committed_norm[c]);
                    if cosine > (0.5 as S) {
                        conflict = true;
                        let winner = self.fluors[scratch.committed_ai[c]].master_idx;
                        if config.joint_pair_resolution
                            && self.is_collinear[pc.master_idx * n_fluor + winner]
                        {
                            scratch.queued.push((ai, v));
                        }
                        break;
                    }
                }
                if conflict {
                    continue;
                }
                scratch.committed[ai] = true;
                let c = scratch.n_committed;
                let start = c * n_det;
                scratch.committed_dr[start..start + n_det]
                    .copy_from_slice(&scratch.dr[..n_det]);
                scratch.committed_norm[c] = dr_norm;
                scratch.committed_ai[c] = ai;
                scratch.n_committed += 1;
                scratch.commits.push((ai, v));
            }

            if scratch.commits.is_empty() {
                break;
            }

            let commits = scratch.commits.clone();
            for &(ai, v) in &commits {
                self.try_commit_variant(ai, v, scratch, cell_resid, &mut rss_accepted)?;
            }
            if config.joint_pair_resolution {
                let queued = scratch.queued.clone();
                for &(ai, v) in &queued {
                    self.try_commit_variant(ai, v, scratch, cell_resid, &mut rss_accepted)?;
                }
            }
            if dot(&scratch.resid_raw[..n_det], &scratch.resid_raw[..n_det])
                < (1e-16 as S) * cell_resid_ss
            {
                break;
            }
        }

        out_ab[..n_fluor].copy_from_slice(&scratch.fluor_unmixed[..n_fluor]);
        out_ab[n_fluor] = k_af;
        for (ai, pc) in self.fluors.iter().enumerate() {
            if scratch.best_v[ai] >= 0 {
                var_idx[pc.master_idx] = Some(scratch.best_v[ai] as usize);
            }
        }
        Ok(())
    }

    fn try_commit_variant(
        &self,
        opt_i: usize,
        v: usize,
        scratch: &mut EventScratch,
        cell_resid: &[S],
        rss_accepted: &mut S,
    ) -> Result<bool> {
        let n_det = self.n_det;
        let n_fluor = self.n_fluor;
        scratch.ensure_cell_s(self.spectra_fd.as_ref());
        if !self.cell_weight {
            scratch.ensure_cell_s_f_w(self.spectra_fd.as_ref());
            scratch.ensure_a_base(self.sst.as_ref());
        }
        let pc = &self.fluors[opt_i];
        let idx = pc.master_idx;
        for d in 0..n_det {
            scratch.prev_row[d] = scratch.cell_s[(idx, d)];
            scratch.cell_s[(idx, d)] = pc.v_mats[(d, v)];
            scratch.s_new[d] = pc.v_mats[(d, v)] * scratch.sqrt_w[d];
        }
        scratch.col_update.fill(0.0 as S);
        for d in 0..n_det {
            let sd = scratch.s_new[d];
            if let Some(col) = col_slice(scratch.cell_s_f_w.as_ref(), d) {
                for (cu, &c) in scratch.col_update.iter_mut().zip(col) {
                    *cu += c * sd;
                }
            } else {
                for r in 0..n_fluor {
                    scratch.col_update[r] += scratch.cell_s_f_w[(r, d)] * sd;
                }
            }
        }
        copy_mat_into(scratch.a_base.as_ref(), &mut scratch.a_trial);
        for r in 0..n_fluor {
            scratch.a_trial[(r, idx)] = scratch.col_update[r];
            scratch.a_trial[(idx, r)] = scratch.col_update[r];
        }
        scratch.a_trial[(idx, idx)] = dot(&scratch.s_new[..n_det], &scratch.s_new[..n_det]);
        scratch.b_trial[..n_fluor].copy_from_slice(&scratch.b_base[..n_fluor]);
        scratch.b_trial[idx] = dot(&scratch.s_new[..n_det], &scratch.y_vec[..n_det]);

        if solve_square_vec(
            scratch.a_trial.as_ref(),
            &scratch.b_trial[..n_fluor],
            &mut scratch.trial_unmixed[..n_fluor],
            &mut scratch.rhs,
        )
        .is_err()
        {
            for d in 0..n_det {
                scratch.cell_s[(idx, d)] = scratch.prev_row[d];
            }
            return Ok(false);
        }
        for f in 0..n_fluor {
            scratch.relu_f[f] = scratch.trial_unmixed[f].max(0.0 as S);
        }
        for d in 0..n_det {
            let pred = if let Some(col) = col_slice(scratch.cell_s.as_ref(), d) {
                dot(col, &scratch.relu_f[..n_fluor])
            } else {
                (0..n_fluor)
                    .map(|f| scratch.cell_s[(f, d)] * scratch.relu_f[f])
                    .sum::<S>()
            };
            scratch.trial_resid_raw[d] = cell_resid[d] - pred;
            scratch.trial_resid[d] = scratch.trial_resid_raw[d] * scratch.sqrt_w[d];
        }
        let trial_rss = dot(
            &scratch.trial_resid[..n_det],
            &scratch.trial_resid[..n_det],
        );
        if trial_rss < *rss_accepted {
            scratch.best_v[opt_i] = v as isize;
            scratch.fluor_unmixed[..n_fluor]
                .copy_from_slice(&scratch.trial_unmixed[..n_fluor]);
            scratch.resid_raw[..n_det].copy_from_slice(&scratch.trial_resid_raw[..n_det]);
            scratch.resid[..n_det].copy_from_slice(&scratch.trial_resid[..n_det]);
            *rss_accepted = trial_rss;
            for d in 0..n_det {
                scratch.cell_s_f_w[(idx, d)] = scratch.s_new[d];
            }
            copy_mat_into(scratch.a_trial.as_ref(), &mut scratch.a_base);
            scratch.b_base[..n_fluor].copy_from_slice(&scratch.b_trial[..n_fluor]);
            Ok(true)
        } else {
            for d in 0..n_det {
                scratch.cell_s[(idx, d)] = scratch.prev_row[d];
            }
            Ok(false)
        }
    }
}

fn build_fluor_precomp(
    master_idx: usize,
    v_mats: MatRef<'_, S>,
    delta_obs: MatRef<'_, S>,
    spectra_df: MatRef<'_, S>,
    spectra_fd: MatRef<'_, S>,
    p_fd: MatRef<'_, S>,
) -> Result<FluorPrecomp> {
    let n_det = spectra_df.nrows();
    let n_fluor = spectra_df.ncols();
    let n_v = v_mats.ncols();
    let mut delta = Mat::<S>::zeros(n_det, n_v);
    for v in 0..n_v {
        for d in 0..n_det {
            delta[(d, v)] = v_mats[(d, v)] - spectra_df[(d, master_idx)];
        }
    }
    let mut other_indices = Vec::with_capacity(n_fluor.saturating_sub(1));
    for r in 0..n_fluor {
        if r != master_idx {
            other_indices.push(r);
        }
    }
    let n_other = other_indices.len();
    let mut s_nof = Mat::<S>::zeros(n_other, n_det);
    for (o, &fi) in other_indices.iter().enumerate() {
        for d in 0..n_det {
            s_nof[(o, d)] = spectra_fd[(fi, d)];
        }
    }
    let gram_nof: Mat<S> = &s_nof * s_nof.transpose();
    let u_nof = solve_square(gram_nof.as_ref(), s_nof.as_ref())?;
    let v_lib: Mat<S> = &u_nof * &delta;

    let mut delta_cov = sample_cov_columns(delta_obs);
    for d in 0..delta_cov.nrows() {
        delta_cov[(d, d)] += 1e-4 as S;
    }
    let leakage_cov: Mat<S> = &u_nof * &delta_cov * u_nof.transpose();
    let mut w_leakage = vec![0.0 as S; n_other];
    for o in 0..n_other {
        w_leakage[o] = leakage_cov[(o, o)].abs().sqrt() + (1e-8 as S);
    }

    let coef: Mat<S> = p_fd * &delta;
    let r_lib: Mat<S> = &delta - spectra_df * &coef;
    let mut r_lib_sq = Mat::<S>::zeros(n_det, n_v);
    let mut r_dots = vec![0.0 as S; n_v];
    for v in 0..n_v {
        let mut s = 0.0 as S;
        for d in 0..n_det {
            let r = r_lib[(d, v)];
            r_lib_sq[(d, v)] = r * r;
            s += r * r;
        }
        r_dots[v] = s;
    }

    Ok(FluorPrecomp {
        master_idx,
        n_variants: n_v,
        v_mats: v_mats.to_owned(),
        r_lib,
        r_lib_sq,
        r_dots,
        v_lib,
        w_leakage,
        other_indices,
    })
}

fn resolve_noise_floor(n_det: usize, config: &JointUnmixConfig) -> Result<Vec<S>> {
    match &config.noise_floor_per_detector {
        None => Ok(vec![config.noise_floor as S; n_det]),
        Some(v) if v.len() == 1 => Ok(vec![v[0] as S; n_det]),
        Some(v) if v.len() == n_det => Ok(v.iter().map(|&x| x as S).collect()),
        Some(v) => Err(AutospectralError::InvalidConfig(format!(
            "noise_floor_per_detector length {} != detectors {n_det}",
            v.len()
        ))),
    }
}

fn global_weights(
    events: &[S],
    n: usize,
    d: usize,
    noise_floor: &[S],
    cell_weight: bool,
) -> (Vec<S>, Vec<S>) {
    if !cell_weight {
        return (vec![1.0 as S; d], vec![1.0 as S; d]);
    }
    let mut mean = vec![0.0 as S; d];
    for e in 0..n {
        for c in 0..d {
            mean[c] += events[e * d + c];
        }
    }
    let inv = (1.0 as S) / (n as S);
    let mut w = vec![0.0 as S; d];
    let mut sw = vec![0.0 as S; d];
    for c in 0..d {
        w[c] = 1.0 / (mean[c] * inv).max(noise_floor[c]);
        sw[c] = w[c].sqrt();
    }
    (w, sw)
}


fn mat_cast(m: MatRef<'_, f64>) -> Mat<S> {
    Mat::from_fn(m.nrows(), m.ncols(), |i, j| m[(i, j)] as S)
}

fn transpose_mat(m: MatRef<'_, S>) -> Mat<S> {
    Mat::from_fn(m.ncols(), m.nrows(), |i, j| m[(j, i)])
}

fn solve_square(a: MatRef<'_, S>, b: MatRef<'_, S>) -> Result<Mat<S>> {
    match Llt::new(a, Side::Lower) {
        Ok(llt) => Ok(llt.solve(b)),
        Err(_) => {
            let lu = a.partial_piv_lu();
            Ok(lu.solve(b))
        }
    }
}

fn solve_square_vec(
    a: MatRef<'_, S>,
    b: &[S],
    out: &mut [S],
    rhs: &mut Mat<S>,
) -> Result<()> {
    debug_assert_eq!(rhs.nrows(), b.len());
    debug_assert_eq!(rhs.ncols(), 1);
    for (i, &bi) in b.iter().enumerate() {
        rhs[(i, 0)] = bi;
    }
    let x = solve_square(a, rhs.as_ref())?;
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = x[(i, 0)];
    }
    Ok(())
}

fn weighted_lstsq(
    s_w: MatRef<'_, S>,
    y_w: &[S],
    out: &mut [S],
    rhs_mat: &mut Mat<S>,
) -> Result<()> {
    // S_w is F×D; solve S_w S_w^T α = S_w y  (same as Armadillo solve(S_w.t(), y)).
    let gram: Mat<S> = s_w * s_w.transpose();
    let mut rhs = vec![0.0 as S; s_w.nrows()];
    gemv(s_w, y_w, &mut rhs);
    solve_square_vec(gram.as_ref(), &rhs, out, rhs_mat)
}

fn sample_cov_columns(x: MatRef<'_, S>) -> Mat<S> {
    let d = x.nrows();
    let n = x.ncols();
    let mut cov = Mat::<S>::zeros(d, d);
    if n < 2 {
        return cov;
    }
    let mut mean = vec![0.0 as S; d];
    for j in 0..n {
        for i in 0..d {
            mean[i] += x[(i, j)];
        }
    }
    let inv_n = (1.0 as S) / (n as S);
    for m in &mut mean {
        *m *= inv_n;
    }
    let inv = (1.0 as S) / ((n - 1) as S);
    for j in 0..n {
        for a in 0..d {
            let da = x[(a, j)] - mean[a];
            for b in 0..=a {
                let db = x[(b, j)] - mean[b];
                cov[(a, b)] += da * db * inv;
            }
        }
    }
    for a in 0..d {
        for b in 0..a {
            cov[(b, a)] = cov[(a, b)];
        }
    }
    cov
}

fn gemv(a: MatRef<'_, S>, x: &[S], out: &mut [S]) {
    // faer's matmul/LU kernels use pulp explicit SIMD. This loop does not: it is
    // our per-event GEMV. Column-major axpy over `col(j).as_slice()` is packed;
    // the previous row-wise `a[(i, j)]` walk was strided on faer `Mat`.
    debug_assert_eq!(out.len(), a.nrows());
    debug_assert_eq!(x.len(), a.ncols());
    out.fill(0.0 as S);
    for j in 0..a.ncols() {
        let xj = x[j];
        if let Some(col) = col_slice(a, j) {
            for (o, &aij) in out.iter_mut().zip(col) {
                *o += aij * xj;
            }
        } else {
            for i in 0..a.nrows() {
                out[i] += a[(i, j)] * xj;
            }
        }
    }
}

#[inline]
fn col_slice(m: MatRef<'_, S>, j: usize) -> Option<&[S]> {
    m.col(j).try_as_col_major().map(|c| c.as_slice())
}

#[inline]
fn col_slice_mut(m: &mut Mat<S>, j: usize) -> Option<&mut [S]> {
    m.col_mut(j).try_as_col_major_mut().map(|c| c.as_slice_mut())
}

#[inline]
fn col_dot(m: MatRef<'_, S>, j: usize, x: &[S]) -> S {
    if let Some(col) = col_slice(m, j) {
        dot(col, x)
    } else {
        (0..m.nrows()).map(|i| m[(i, j)] * x[i]).sum::<S>()
    }
}

#[inline]
fn col_col_dot(m: MatRef<'_, S>, j: usize, k: usize) -> S {
    match (col_slice(m, j), col_slice(m, k)) {
        (Some(a), Some(b)) => dot(a, b),
        _ => (0..m.nrows()).map(|i| m[(i, j)] * m[(i, k)]).sum::<S>(),
    }
}

#[inline]
fn col_col_dot_w(m: MatRef<'_, S>, j: usize, k: usize, w: &[S]) -> S {
    match (col_slice(m, j), col_slice(m, k)) {
        (Some(a), Some(b)) => a
            .iter()
            .zip(b)
            .zip(w)
            .map(|((x, y), wi)| x * y * wi)
            .sum::<S>(),
        _ => (0..m.nrows())
            .map(|i| m[(i, j)] * m[(i, k)] * w[i])
            .sum::<S>(),
    }
}

fn copy_mat_into(src: MatRef<'_, S>, dst: &mut Mat<S>) {
    debug_assert_eq!(src.nrows(), dst.nrows());
    debug_assert_eq!(src.ncols(), dst.ncols());
    for j in 0..src.ncols() {
        if let (Some(s), Some(d)) = (col_slice(src, j), col_slice_mut(dst, j)) {
            d.copy_from_slice(s);
        } else {
            for i in 0..src.nrows() {
                dst[(i, j)] = src[(i, j)];
            }
        }
    }
}

/// `out = y - scale * A[:, j]`
fn axpy_col(a: MatRef<'_, S>, j: usize, scale: S, y: &[S], out: &mut [S]) {
    debug_assert_eq!(out.len(), y.len());
    if let Some(col) = col_slice(a, j) {
        for ((o, &yi), &cj) in out.iter_mut().zip(y).zip(col) {
            *o = yi - scale * cj;
        }
    } else {
        for (d, (o, &yi)) in out.iter_mut().zip(y).enumerate() {
            *o = yi - scale * a[(d, j)];
        }
    }
}

/// `y -= scale * A[:, j]`
fn scale_sub_col(a: MatRef<'_, S>, j: usize, scale: S, y: &mut [S]) {
    if let Some(col) = col_slice(a, j) {
        for (slot, &cj) in y.iter_mut().zip(col) {
            *slot -= scale * cj;
        }
    } else {
        for (d, slot) in y.iter_mut().enumerate() {
            *slot -= scale * a[(d, j)];
        }
    }
}

fn scale_col_into(a: MatRef<'_, S>, j: usize, scale: S, out: &mut [S]) {
    if let Some(col) = col_slice(a, j) {
        for (o, &cj) in out.iter_mut().zip(col) {
            *o = cj * scale;
        }
    } else {
        for (d, o) in out.iter_mut().enumerate() {
            *o = a[(d, j)] * scale;
        }
    }
}

fn scale_col_diff_into(a: MatRef<'_, S>, j: usize, k: usize, scale: S, out: &mut [S]) {
    match (col_slice(a, j), col_slice(a, k)) {
        (Some(cj), Some(ck)) => {
            for ((o, &vj), &vk) in out.iter_mut().zip(cj).zip(ck) {
                *o = (vj - vk) * scale;
            }
        }
        _ => {
            for (d, o) in out.iter_mut().enumerate() {
                *o = (a[(d, j)] - a[(d, k)]) * scale;
            }
        }
    }
}

fn dot(a: &[S], b: &[S]) -> S {
    a.iter().zip(b).map(|(x, y)| x * y).sum::<S>()
}

fn norm(a: &[S]) -> S {
    dot(a, a).sqrt()
}

fn row_cosine_abs(p: MatRef<'_, S>, i: usize, j: usize) -> S {
    let d = p.ncols();
    let mut num = 0.0 as S;
    let mut ni = 0.0 as S;
    let mut nj = 0.0 as S;
    for c in 0..d {
        let a = p[(i, c)];
        let b = p[(j, c)];
        num += a * b;
        ni += a * a;
        nj += b * b;
    }
    num.abs() / (ni.sqrt() * nj.sqrt() + (1e-12 as S))
}

