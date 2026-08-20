//! Discover AF from synthetic unstained FCS, match stained events, then
//! `TruOls::from_preprocessed` on each AF group.
//!
//! Writes Gaussian-population FCS to a temp directory and re-opens them so the
//! column cache is file-backed (same path as a real panel).

use faer::Mat;
use flow_autospectral::{
    DiscoverConfig, DiscoveryBackend, MatchConfig, MixingMatrixAfOptions, discover_af_library,
    events_row_major_to_mat, group_events_by_af, match_events, tru_ols_from_selected_af,
};
use flow_fcs::Fcs;
use flow_fcs::synthetic::{
    GaussianComponent, GaussianPopulationsSpec, write_gaussian_populations_fcs,
};
use std::error::Error;

fn main() -> std::result::Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let unstained_path = dir.path().join("unstained.fcs");
    let stained_path = dir.path().join("stained.fcs");

    write_gaussian_populations_fcs(&unstained_path, &unstained_spec(800))?;
    write_gaussian_populations_fcs(&stained_path, &stained_spec(800))?;

    let unstained = Fcs::open(unstained_path.to_str().ok_or("utf8 path")?)?;
    let stained = Fcs::open(stained_path.to_str().ok_or("utf8 path")?)?;

    let fl_names = ["FL1-A", "FL2-A", "FL3-A"];
    let (u_events, n_u) = fluorescence_row_major(&unstained, &fl_names)?;
    let (s_events, n_s) = fluorescence_row_major(&stained, &fl_names)?;
    let detectors: Vec<String> = fl_names.iter().map(|s| (*s).to_string()).collect();
    let d = detectors.len();

    let cfg = DiscoverConfig {
        backend: DiscoveryBackend::KMeans,
        fixed_k: Some(2),
        seed: Some(7),
        ..DiscoverConfig::default()
    };
    let library = discover_af_library(&u_events, n_u, d, &detectors, &cfg)?;
    println!(
        "library: {} signatures ({})",
        library.n_signatures(),
        library.provenance
    );

    // Simple fluorophore column (peaks on FL1).
    let fluor_names = vec!["FITC".into()];
    let fluor = Mat::from_fn(d, 1, |i, _| if i == 0 { 1.0 } else { 0.05 });
    let matched = match_events(
        &s_events,
        n_s,
        &library,
        fluor.as_ref(),
        &MatchConfig {
            parallel_event_threshold: usize::MAX,
            ..MatchConfig::default()
        },
    )?;
    let groups = group_events_by_af(&matched);
    println!("AF groups: {}", groups.len());

    let unstained_mat = events_row_major_to_mat(&u_events, n_u, d)?;
    for (af_idx, events) in &groups {
        let selected = tru_ols_from_selected_af(
            MixingMatrixAfOptions {
                detector_names: &detectors,
                fluor_names: &fluor_names,
                fluor_matrix: fluor.as_ref(),
                library: &library,
                af_index: *af_idx,
                af_endmember_name: &library.names[*af_idx],
                af_correction: false,
            },
            unstained_mat.as_ref(),
            0.995,
        )?;
        let mut block = Vec::with_capacity(events.len() * d);
        for &ei in events {
            block.extend_from_slice(&s_events[ei * d..(ei + 1) * d]);
        }
        let obs = events_row_major_to_mat(&block, events.len(), d)?;
        let unmixed = selected.tru_ols.unmix(obs.as_ref())?;
        println!(
            "AF {} ({}): {} events, mixing {}×{}, unmixed {}×{}",
            af_idx,
            selected
                .mixing
                .endmember_names
                .last()
                .cloned()
                .unwrap_or_default(),
            events.len(),
            selected.mixing.matrix.nrows(),
            selected.mixing.matrix.ncols(),
            unmixed.nrows(),
            unmixed.ncols()
        );
    }
    Ok(())
}

fn fluorescence_row_major(
    fcs: &Fcs,
    names: &[&str],
) -> std::result::Result<(Vec<f64>, usize), Box<dyn Error>> {
    let cols = fcs.columns(names)?;
    let n = cols.first().map(|c| c.len()).unwrap_or(0);
    let d = cols.len();
    let mut out = vec![0.0; n * d];
    for e in 0..n {
        for c in 0..d {
            out[e * d + c] = f64::from(cols[c][e]);
        }
    }
    Ok((out, n))
}

fn unstained_spec(n_events: usize) -> GaussianPopulationsSpec {
    // Time, FSC-A, FSC-H, SSC-A, FL1, FL2, FL3 — two AF-like fluorescence clouds.
    let channel_names = flow_fcs::synthetic::cytometry_channel_names(3);
    let n = channel_names.len();
    let mut lo = vec![1.0_f32; n];
    let mut hi = vec![1.0_f32; n];
    lo[1] = 50_000.0;
    hi[1] = 52_000.0;
    lo[2] = 46_000.0;
    hi[2] = 48_000.0;
    lo[3] = 30_000.0;
    hi[3] = 32_000.0;
    lo[4] = 800.0;
    lo[5] = 200.0;
    lo[6] = 150.0;
    hi[4] = 200.0;
    hi[5] = 900.0;
    hi[6] = 180.0;
    GaussianPopulationsSpec {
        n_events,
        channel_names,
        components: vec![
            GaussianComponent {
                weight: 0.5,
                mean: lo,
                std_dev: vec![1.0, 4000.0, 3500.0, 3000.0, 80.0, 40.0, 30.0],
            },
            GaussianComponent {
                weight: 0.5,
                mean: hi,
                std_dev: vec![1.0, 4000.0, 3500.0, 3000.0, 40.0, 80.0, 30.0],
            },
        ],
        seed: 11,
    }
}

fn stained_spec(n_events: usize) -> GaussianPopulationsSpec {
    let mut spec = unstained_spec(n_events);
    spec.seed = 99;
    // Brighten FL1 on the second component (fluorophore-like).
    spec.components[1].mean[4] = 4_000.0;
    spec.components[1].std_dev[4] = 400.0;
    spec
}
