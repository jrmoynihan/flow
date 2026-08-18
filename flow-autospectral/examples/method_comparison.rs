//! Quality-first A/B of AF discovery backends and match strategies.
//!
//! Prints mean OLS residual (lower is better) for GMM / k-means / HNSW-medoid /
//! FlowSOM libraries, residual vs nearest-neighbour matching, and OLS vs TRU-OLS
//! unmixing on a synthetic two-AF panel. Not a throughput bench.

use faer::Mat;
use flow_autospectral::{
    DiscoverConfig, DiscoveryBackend, MatchConfig, MatchStrategy, MixingMatrixAfOptions,
    SomDiscoverConfig, discover_af_library, events_row_major_to_mat, match_events, ols_residual,
    swap_af_column, tru_ols_from_selected_af,
};
use std::error::Error;

fn main() -> std::result::Result<(), Box<dyn Error>> {
    let d = 4usize;
    let detectors: Vec<String> = (0..d).map(|i| format!("D{i}")).collect();
    let (unstained, n_u) = two_af_unstained(400, d);
    let (stained, n_s) = two_af_stained(400, d);
    let fluor_names = vec!["FluoA".into(), "FluoB".into()];
    // Two fluor columns + unused spillover; TRU-OLS can drop the weak one.
    let fluor = Mat::from_fn(d, 2, |i, j| if i == j + 1 { 1.0 } else { 0.04 });

    let backends = [
        ("gmm", DiscoveryBackend::Gmm, SomDiscoverConfig::default()),
        (
            "kmeans",
            DiscoveryBackend::KMeans,
            SomDiscoverConfig::default(),
        ),
        (
            "hnsw-medoid",
            DiscoveryBackend::HnswMedoid,
            SomDiscoverConfig::default(),
        ),
        (
            "flowsom",
            DiscoveryBackend::FlowSom,
            SomDiscoverConfig {
                width: 4,
                height: 4,
                n_epochs: 6,
                radius: Some(2.0),
            },
        ),
    ];

    println!("method\tmatch\tunmix\tmean_residual");
    for (name, backend, som) in backends {
        let cfg = DiscoverConfig {
            backend,
            fixed_k: Some(2),
            seed: Some(42),
            som,
            ..DiscoverConfig::default()
        };
        let lib = match discover_af_library(&unstained, n_u, d, &detectors, &cfg) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("{name}: discover failed: {e}");
                continue;
            }
        };

        for (match_name, strategy) in [
            ("residual", MatchStrategy::ResidualOls),
            ("nn", MatchStrategy::NearestNeighbor),
        ] {
            let matched = match_events(
                &stained,
                n_s,
                &lib,
                fluor.as_ref(),
                &MatchConfig {
                    strategy,
                    parallel_event_threshold: usize::MAX,
                    ..MatchConfig::default()
                },
            )?;
            let mut ols_sum = 0.0;
            for (ei, &af) in matched.af_indices.iter().enumerate() {
                let m = swap_af_column(fluor.as_ref(), &lib, af)?;
                let y = &stained[ei * d..(ei + 1) * d];
                ols_sum += ols_residual(m.as_ref(), y)?;
            }
            println!("{name}\t{match_name}\tols\t{:.6}", ols_sum / n_s as f64);

            if strategy == MatchStrategy::ResidualOls {
                let u = events_row_major_to_mat(&unstained, n_u, d)?;
                let mut engines = Vec::new();
                for af in 0..lib.n_signatures() {
                    engines.push(tru_ols_from_selected_af(
                        MixingMatrixAfOptions {
                            detector_names: &detectors,
                            fluor_names: &fluor_names,
                            fluor_matrix: fluor.as_ref(),
                            library: &lib,
                            af_index: af,
                            af_endmember_name: &lib.names[af],
                            af_correction: false,
                        },
                        u.as_ref(),
                        0.995,
                    )?);
                }
                let mut tru_sum = 0.0;
                let mut tru_n = 0usize;
                for (ei, &af) in matched.af_indices.iter().enumerate() {
                    let selected = &engines[af];
                    let y = &stained[ei * d..(ei + 1) * d];
                    let obs = events_row_major_to_mat(y, 1, d)?;
                    let unmixed = selected.tru_ols.unmix(obs.as_ref())?;
                    let pred: Vec<f64> = (0..d)
                        .map(|i| {
                            (0..unmixed.ncols())
                                .map(|j| selected.mixing.matrix[(i, j)] * unmixed[(0, j)])
                                .sum()
                        })
                        .collect();
                    tru_sum += pred
                        .iter()
                        .zip(y.iter())
                        .map(|(p, t)| (p - t) * (p - t))
                        .sum::<f64>();
                    tru_n += 1;
                }
                println!(
                    "{name}\t{match_name}\ttru-ols\t{:.6}",
                    tru_sum / tru_n.max(1) as f64
                );
            }
        }
    }
    Ok(())
}

fn two_af_unstained(n: usize, d: usize) -> (Vec<f64>, usize) {
    let mut events = Vec::with_capacity(n * d);
    for i in 0..n {
        if i < n / 2 {
            events.extend((0..d).map(|c| if c == 0 { 8.0 } else { 1.0 }));
        } else {
            events.extend((0..d).map(|c| if c == d - 1 { 8.0 } else { 1.0 }));
        }
    }
    (events, n)
}

fn two_af_stained(n: usize, d: usize) -> (Vec<f64>, usize) {
    let mut events = Vec::with_capacity(n * d);
    for i in 0..n {
        if i < n / 2 {
            events.extend((0..d).map(|c| if c == 0 { 9.0 } else { 1.2 }));
        } else {
            events.extend((0..d).map(|c| if c == d - 1 { 9.0 } else { 1.2 }));
        }
    }
    (events, n)
}
