//! Synthetic data test utilities and tests for TRU-OLS

use faer::{Mat, MatRef};
use flow_tru_ols::{TruOls, UnmixingStrategy};
use rand::RngExt;

/// Generate a synthetic mixing matrix with specified dimensions
pub fn generate_mixing_matrix(n_detectors: usize, n_endmembers: usize) -> Mat<f64> {
    let mut rng = rand::rng();
    let mut matrix = Mat::zeros(n_detectors, n_endmembers);

    for i in 0..n_detectors {
        for j in 0..n_endmembers {
            if i == j {
                matrix[(i, j)] = 0.8 + rng.random_range(0.0..0.2);
            } else {
                matrix[(i, j)] = rng.random_range(0.0..0.1);
            }
        }
    }

    matrix
}

/// Generate synthetic observations from known abundances
pub fn generate_observations(
    mixing_matrix: MatRef<'_, f64>,
    abundances: MatRef<'_, f64>,
    noise_level: f64,
) -> Mat<f64> {
    let mut rng = rand::rng();
    let n_detectors = mixing_matrix.nrows();
    let n_events = abundances.nrows();

    let mut observations = Mat::zeros(n_detectors, n_events);
    faer::linalg::matmul::matmul(
        observations.as_mut(),
        faer::Accum::Replace,
        mixing_matrix,
        abundances.transpose(),
        1.0_f64,
        faer::Par::Seq,
    );

    if noise_level > 0.0 {
        for i in 0..n_detectors {
            for j in 0..n_events {
                observations[(i, j)] += rng.random_range(-noise_level..noise_level);
            }
        }
    }

    observations
}

/// Generate synthetic abundances matrix (events × endmembers)
pub fn generate_abundances(n_events: usize, n_endmembers: usize, sparsity: f64) -> Mat<f64> {
    let mut rng = rand::rng();
    let mut abundances = Mat::zeros(n_events, n_endmembers);

    for i in 0..n_events {
        for j in 0..n_endmembers {
            if rng.random::<f64>() > sparsity {
                abundances[(i, j)] = rng.random_range(0.0..100.0);
            }
        }
    }

    abundances
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::mat;

    #[test]
    fn test_generate_mixing_matrix() {
        let matrix = generate_mixing_matrix(4, 5);
        assert_eq!(matrix.nrows(), 4);
        assert_eq!(matrix.ncols(), 5);

        for i in 0..4.min(5) {
            assert!(matrix[(i, i)] > 0.5);
        }
    }

    #[test]
    fn test_generate_observations() {
        let mixing_matrix = mat![[1.0, 0.1], [0.1, 1.0]];
        let abundances = mat![[10.0, 5.0], [20.0, 15.0]];
        let observations = generate_observations(mixing_matrix.as_ref(), abundances.as_ref(), 0.1);

        assert_eq!(observations.nrows(), 2);
        assert_eq!(observations.ncols(), 2);
    }

    #[test]
    fn test_tru_ols_synthetic() {
        let mixing_matrix = mat![[1.0, 0.1, 0.05], [0.1, 1.0, 0.1], [0.05, 0.1, 1.0]];
        let n_detectors = 3;
        let n_endmembers = 3;

        let mut unstained = Mat::zeros(100, n_detectors);
        let mut rng = rand::rng();
        for i in 0..100 {
            for j in 0..n_detectors {
                unstained[(i, j)] = rng.random_range(-0.1..0.1);
            }
        }

        let tru_ols = TruOls::new(mixing_matrix.clone(), unstained, 0).unwrap();

        // test_abundances: 3 events × 3 endmembers
        let test_abundances = mat![[10.0, 0.0, 0.0], [0.0, 20.0, 0.0], [5.0, 5.0, 0.0]];
        // observations = mixing_matrix @ test_abundances^T -> 3×3 (detectors × events)
        let observations =
            generate_observations(mixing_matrix.as_ref(), test_abundances.as_ref(), 0.0);
        // TruOls::unmix expects events × detectors
        let observations_t = Mat::from_fn(3, 3, |i, j| observations[(j, i)]);
        let unmixed = tru_ols.unmix(observations_t.as_ref()).unwrap();

        assert_eq!(unmixed.nrows(), 3);
        assert_eq!(unmixed.ncols(), n_endmembers);
        assert!(unmixed[(0, 0)] > 5.0);
    }

    #[test]
    fn test_tru_ols_removes_irrelevant_endmembers() {
        let mixing_matrix = mat![[1.0, 0.1], [0.1, 1.0]];
        let mut unstained = Mat::zeros(100, 2);
        let mut rng = rand::rng();
        for i in 0..100 {
            for j in 0..2 {
                unstained[(i, j)] = rng.random_range(-0.1..0.1);
            }
        }

        let tru_ols = TruOls::new(mixing_matrix.clone(), unstained, 0).unwrap();

        let observation = mat![[10.0, 1.0]];
        let unmixed = tru_ols.unmix(observation.as_ref()).unwrap();

        assert!(unmixed[(0, 1)].abs() < 1.0);
    }
}
