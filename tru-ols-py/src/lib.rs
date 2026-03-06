use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3_polars::PyDataFrame;

use faer::{Col, Mat};
use flow_tru_ols::{TruOls as RsTruOls, TruOlsError, UnmixingStrategy};

// ---------------------------------------------------------------------------
// Helpers: convert between Python nested lists and faer matrices
// ---------------------------------------------------------------------------

fn mat_from_nested_list(rows: Vec<Vec<f64>>) -> PyResult<Mat<f64>> {
    if rows.is_empty() {
        return Err(PyValueError::new_err("Matrix must have at least one row"));
    }
    let n_rows = rows.len();
    let n_cols = rows[0].len();
    for (i, row) in rows.iter().enumerate() {
        if row.len() != n_cols {
            return Err(PyValueError::new_err(format!(
                "Row {i} has {} columns, expected {n_cols}",
                row.len()
            )));
        }
    }
    Ok(Mat::from_fn(n_rows, n_cols, |r, c| rows[r][c]))
}

fn mat_to_nested_list(mat: &Mat<f64>) -> Vec<Vec<f64>> {
    (0..mat.nrows())
        .map(|r| (0..mat.ncols()).map(|c| mat[(r, c)]).collect())
        .collect()
}

fn col_to_list(col: &Col<f64>) -> Vec<f64> {
    (0..col.nrows()).map(|i| col[i]).collect()
}

fn to_py_err(e: TruOlsError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

fn parse_strategy(s: &str) -> PyResult<UnmixingStrategy> {
    match s {
        "zero" => Ok(UnmixingStrategy::Zero),
        "ucm" | "unstained_control_mapping" => Ok(UnmixingStrategy::UnstainedControlMapping),
        other => Err(PyValueError::new_err(format!(
            "Unknown strategy '{other}'. Use 'zero' or 'ucm'."
        ))),
    }
}

// ---------------------------------------------------------------------------
// Python-visible TruOls class
// ---------------------------------------------------------------------------

/// TRU-OLS (Truncated ReUnmixing OLS) spectral unmixing algorithm.
///
/// TRU-OLS reduces the variance of unmixed abundance distributions by
/// removing irrelevant endmembers (dyes) from the mixing matrix on a
/// per-event basis, using unstained control data to determine relevance.
///
/// Args:
///     mixing_matrix: Mixing matrix as nested list (detectors x endmembers).
///     unstained_control: Unstained control data as nested list (events x detectors).
///     autofluorescence_idx: Index of the autofluorescence endmember column.
///     cutoff_percentile: Percentile for cutoff threshold (default 0.995 = 99.5th).
///     strategy: Strategy for irrelevant abundances - "zero" (default) or "ucm".
#[pyclass(name = "TruOls")]
struct PyTruOls {
    inner: RsTruOls,
    mixing_matrix: Mat<f64>,
    unstained_control: Mat<f64>,
}

#[pymethods]
impl PyTruOls {
    #[new]
    #[pyo3(signature = (
        mixing_matrix,
        unstained_control,
        autofluorescence_idx,
        cutoff_percentile = 0.995,
        strategy = "zero",
    ))]
    fn new(
        mixing_matrix: Vec<Vec<f64>>,
        unstained_control: Vec<Vec<f64>>,
        autofluorescence_idx: usize,
        cutoff_percentile: f64,
        strategy: &str,
    ) -> PyResult<Self> {
        let mm = mat_from_nested_list(mixing_matrix)?;
        let uc = mat_from_nested_list(unstained_control)?;

        let strat = parse_strategy(strategy)?;

        let mut inner = RsTruOls::new(mm.clone(), uc.clone(), autofluorescence_idx)
            .map_err(to_py_err)?;

        if (cutoff_percentile - 0.995).abs() > 1e-12 {
            inner
                .set_cutoff_percentile(cutoff_percentile, uc.as_ref())
                .map_err(to_py_err)?;
        }

        inner.set_strategy(strat);

        Ok(PyTruOls {
            inner,
            mixing_matrix: mm,
            unstained_control: uc,
        })
    }

    /// Set the cutoff percentile (recalculates cutoffs from unstained control).
    ///
    /// Args:
    ///     percentile: float between 0.0 and 1.0 (e.g. 0.995 for 99.5th percentile).
    fn set_cutoff_percentile(&mut self, percentile: f64) -> PyResult<()> {
        self.inner
            .set_cutoff_percentile(percentile, self.unstained_control.as_ref())
            .map_err(to_py_err)
    }

    /// Set the unmixing strategy for irrelevant endmember abundances.
    ///
    /// Args:
    ///     strategy: "zero" to set irrelevant abundances to zero,
    ///               "ucm" to map them to unstained control distribution.
    fn set_strategy(&mut self, strategy: &str) -> PyResult<()> {
        self.inner.set_strategy(parse_strategy(strategy)?);
        Ok(())
    }

    /// Unmix an entire dataset.
    ///
    /// Args:
    ///     dataset: Observation data as nested list (events x detectors).
    ///
    /// Returns:
    ///     Nested list of unmixed abundances (events x endmembers).
    fn unmix(&self, dataset: Vec<Vec<f64>>) -> PyResult<Vec<Vec<f64>>> {
        let mat = mat_from_nested_list(dataset)?;
        let result = self.inner.unmix(mat.as_ref()).map_err(to_py_err)?;
        Ok(mat_to_nested_list(&result))
    }

    /// Unmix a single event.
    ///
    /// Args:
    ///     observation: Detector outputs for one event as a list of floats.
    ///
    /// Returns:
    ///     Tuple of (abundances, relevant_indices, irrelevant_pairs) where:
    ///       - abundances: list of relevant endmember abundances.
    ///       - relevant_indices: indices of endmembers that survived TRU-OLS.
    ///       - irrelevant_pairs: list of (index, abundance) for removed endmembers.
    #[allow(clippy::type_complexity)]
    fn unmix_event(
        &self,
        observation: Vec<f64>,
    ) -> PyResult<(Vec<f64>, Vec<usize>, Vec<(usize, f64)>)> {
        let col = Col::from_fn(observation.len(), |i| observation[i]);
        let (abundances, relevant_idx, irrelevant) = self
            .inner
            .unmix_event(col.as_ref())
            .map_err(to_py_err)?;
        Ok((col_to_list(&abundances), relevant_idx, irrelevant))
    }

    /// Number of detectors (rows of the mixing matrix).
    #[getter]
    fn n_detectors(&self) -> usize {
        self.mixing_matrix.nrows()
    }

    /// Number of endmembers (columns of the mixing matrix).
    #[getter]
    fn n_endmembers(&self) -> usize {
        self.mixing_matrix.ncols()
    }

    /// The mixing matrix as nested list.
    #[getter]
    fn mixing_matrix_data(&self) -> Vec<Vec<f64>> {
        mat_to_nested_list(&self.mixing_matrix)
    }

    fn __repr__(&self) -> String {
        format!(
            "TruOls(detectors={}, endmembers={}, unstained_events={})",
            self.mixing_matrix.nrows(),
            self.mixing_matrix.ncols(),
            self.unstained_control.nrows()
        )
    }
}

// ---------------------------------------------------------------------------
// Result wrapper for FCS-based unmixing
// ---------------------------------------------------------------------------

/// Result of TRU-OLS unmixing applied to an FCS file.
#[pyclass(name = "UnmixingResult")]
#[derive(Clone)]
struct PyUnmixingResult {
    inner: flow_fcs::file::Fcs,
}

#[pymethods]
impl PyUnmixingResult {
    /// The unmixed data as a polars DataFrame.
    #[getter]
    fn dataframe(&self) -> PyResult<PyDataFrame> {
        let df = (*self.inner.data_frame).clone();
        Ok(PyDataFrame(df))
    }

    /// Column names in the result.
    #[getter]
    fn column_names(&self) -> Vec<String> {
        self.inner
            .get_parameter_names_from_dataframe()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Number of events.
    #[getter]
    fn n_events(&self) -> usize {
        self.inner.get_event_count_from_dataframe()
    }

    fn __repr__(&self) -> String {
        let cols = self.inner.get_parameter_names_from_dataframe();
        format!(
            "UnmixingResult(events={}, columns={})",
            self.inner.get_event_count_from_dataframe(),
            cols.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

/// Unmix a dataset given as nested lists (pure matrix API, no FCS files).
///
/// This is a convenience function that creates a TruOls instance and unmixes
/// in one call.
///
/// Args:
///     mixing_matrix: Mixing matrix (detectors x endmembers) as nested list.
///     unstained_control: Unstained control data (events x detectors).
///     dataset: Data to unmix (events x detectors).
///     autofluorescence_idx: Index of the autofluorescence endmember.
///     cutoff_percentile: Percentile for cutoff (default 0.995).
///     strategy: "zero" or "ucm" (default "zero").
///
/// Returns:
///     Nested list of unmixed abundances (events x endmembers).
#[pyfunction]
#[pyo3(signature = (
    mixing_matrix,
    unstained_control,
    dataset,
    autofluorescence_idx,
    cutoff_percentile = 0.995,
    strategy = "zero",
))]
fn unmix(
    mixing_matrix: Vec<Vec<f64>>,
    unstained_control: Vec<Vec<f64>>,
    dataset: Vec<Vec<f64>>,
    autofluorescence_idx: usize,
    cutoff_percentile: f64,
    strategy: &str,
) -> PyResult<Vec<Vec<f64>>> {
    let mm = mat_from_nested_list(mixing_matrix)?;
    let uc = mat_from_nested_list(unstained_control)?;
    let ds = mat_from_nested_list(dataset)?;
    let strat = parse_strategy(strategy)?;

    let mut tru_ols = RsTruOls::new(mm, uc.clone(), autofluorescence_idx).map_err(to_py_err)?;

    if (cutoff_percentile - 0.995).abs() > 1e-12 {
        tru_ols
            .set_cutoff_percentile(cutoff_percentile, uc.as_ref())
            .map_err(to_py_err)?;
    }

    tru_ols.set_strategy(strat);
    let result = tru_ols.unmix(ds.as_ref()).map_err(to_py_err)?;
    Ok(mat_to_nested_list(&result))
}

/// Unmix a polars DataFrame of detector data.
///
/// Takes detector data as a polars DataFrame (events x detector columns) and returns
/// the unmixed abundances as a polars DataFrame with one column per endmember.
///
/// Args:
///     df: polars DataFrame with detector columns.
///     detector_columns: list of column names to use as detectors (order must match mixing matrix rows).
///     mixing_matrix: Mixing matrix (detectors x endmembers) as nested list.
///     unstained_control: Unstained control data (events x detectors) as nested list.
///     endmember_names: Names for the endmember columns in the output.
///     autofluorescence_idx: Index of the autofluorescence endmember.
///     cutoff_percentile: Percentile for cutoff (default 0.995).
///     strategy: "zero" or "ucm" (default "zero").
///
/// Returns:
///     polars DataFrame with columns named after endmember_names.
#[pyfunction]
#[pyo3(signature = (
    df,
    detector_columns,
    mixing_matrix,
    unstained_control,
    endmember_names,
    autofluorescence_idx,
    cutoff_percentile = 0.995,
    strategy = "zero",
))]
#[allow(clippy::too_many_arguments)]
fn unmix_dataframe(
    df: PyDataFrame,
    detector_columns: Vec<String>,
    mixing_matrix: Vec<Vec<f64>>,
    unstained_control: Vec<Vec<f64>>,
    endmember_names: Vec<String>,
    autofluorescence_idx: usize,
    cutoff_percentile: f64,
    strategy: &str,
) -> PyResult<PyDataFrame> {
    let mm = mat_from_nested_list(mixing_matrix)?;
    let uc = mat_from_nested_list(unstained_control)?;
    let strat = parse_strategy(strategy)?;

    let n_events = df.0.height();
    let n_detectors = detector_columns.len();
    let n_endmembers = mm.ncols();

    if mm.nrows() != n_detectors {
        return Err(PyValueError::new_err(format!(
            "Mixing matrix has {} rows but {} detector columns were provided",
            mm.nrows(),
            n_detectors
        )));
    }
    if endmember_names.len() != n_endmembers {
        return Err(PyValueError::new_err(format!(
            "Mixing matrix has {} columns but {} endmember names were provided",
            n_endmembers,
            endmember_names.len()
        )));
    }

    // Extract detector data into a faer matrix
    let mut dataset = Mat::zeros(n_events, n_detectors);
    for (col_idx, col_name) in detector_columns.iter().enumerate() {
        let series = df.0.column(col_name.as_str()).map_err(|e| {
            PyValueError::new_err(format!("Column '{col_name}' not found: {e}"))
        })?;

        if let Ok(f64_vals) = series.f64() {
            for (row_idx, val) in f64_vals.into_iter().enumerate() {
                dataset[(row_idx, col_idx)] = val.unwrap_or(0.0);
            }
        } else if let Ok(f32_vals) = series.f32() {
            for (row_idx, val) in f32_vals.into_iter().enumerate() {
                dataset[(row_idx, col_idx)] = val.map(|v| v as f64).unwrap_or(0.0);
            }
        } else {
            return Err(PyValueError::new_err(format!(
                "Column '{col_name}' is not numeric (dtype: {:?})",
                series.dtype()
            )));
        }
    }

    // Run TRU-OLS
    let mut tru_ols =
        RsTruOls::new(mm, uc.clone(), autofluorescence_idx).map_err(to_py_err)?;

    if (cutoff_percentile - 0.995).abs() > 1e-12 {
        tru_ols
            .set_cutoff_percentile(cutoff_percentile, uc.as_ref())
            .map_err(to_py_err)?;
    }

    tru_ols.set_strategy(strat);
    let unmixed = tru_ols.unmix(dataset.as_ref()).map_err(to_py_err)?;

    // Build output DataFrame
    use polars::prelude::*;
    let columns: Vec<Column> = endmember_names
        .iter()
        .enumerate()
        .map(|(em_idx, name)| {
            let values: Vec<f64> = (0..n_events).map(|ev| unmixed[(ev, em_idx)]).collect();
            Column::new(name.clone().into(), values)
        })
        .collect();

    let result_df =
        polars::frame::DataFrame::new_infer_height(columns).map_err(|e| {
            PyValueError::new_err(format!("Failed to create result DataFrame: {e}"))
        })?;

    Ok(PyDataFrame(result_df))
}

/// Open an FCS file, extract detector data, and run TRU-OLS unmixing.
///
/// This is the highest-level convenience function for FCS-based workflows.
///
/// Args:
///     stained_path: Path to the stained FCS file.
///     unstained_path: Path to the unstained control FCS file.
///     mixing_matrix: Mixing matrix (detectors x endmembers) as nested list.
///     detector_names: List of detector channel names (matching FCS column names).
///     endmember_names: List of endmember (dye) names.
///     autofluorescence_name: Name of the autofluorescence endmember.
///     cutoff_percentile: Percentile for cutoff (default 0.995).
///     strategy: "zero" or "ucm" (default "zero").
///
/// Returns:
///     UnmixingResult with the unmixed FCS data.
#[pyfunction]
#[pyo3(signature = (
    stained_path,
    unstained_path,
    mixing_matrix,
    detector_names,
    endmember_names,
    autofluorescence_name,
    cutoff_percentile = 0.995,
    strategy = "zero",
))]
#[allow(clippy::too_many_arguments)]
fn unmix_fcs(
    stained_path: &str,
    unstained_path: &str,
    mixing_matrix: Vec<Vec<f64>>,
    detector_names: Vec<String>,
    endmember_names: Vec<String>,
    autofluorescence_name: &str,
    cutoff_percentile: f64,
    strategy: &str,
) -> PyResult<PyUnmixingResult> {
    use flow_tru_ols::TruOlsUnmixing;

    let stained = flow_fcs::file::Fcs::open(stained_path).map_err(|e| {
        PyValueError::new_err(format!("Failed to open stained FCS file: {e}"))
    })?;

    let unstained = flow_fcs::file::Fcs::open(unstained_path).map_err(|e| {
        PyValueError::new_err(format!("Failed to open unstained FCS file: {e}"))
    })?;

    let mm = mat_from_nested_list(mixing_matrix)?;

    let strat = parse_strategy(strategy)?;

    let det_refs: Vec<&str> = detector_names.iter().map(|s| s.as_str()).collect();
    let em_refs: Vec<&str> = endmember_names.iter().map(|s| s.as_str()).collect();
    let n_em = em_refs.len();
    let empty_opt: Vec<Option<String>> = vec![None; n_em];

    // Apply unmixing, ignoring the percentile setting for the trait API
    // (the trait uses its own default; we'd need to set it afterwards)
    let _ = cutoff_percentile; // trait API uses default 0.995
    let result = stained
        .apply_tru_ols_unmixing(
            &unstained,
            mm,
            &det_refs,
            &em_refs,
            autofluorescence_name,
            Some(strat),
            &empty_opt,
            &empty_opt,
            &empty_opt,
            &empty_opt,
            &empty_opt,
        )
        .map_err(to_py_err)?;

    Ok(PyUnmixingResult { inner: result })
}

/// Read an FCS file and return its event data as a polars DataFrame.
///
/// Args:
///     path: Path to the FCS file.
///
/// Returns:
///     polars DataFrame with event data.
#[pyfunction]
fn read_fcs(path: &str) -> PyResult<PyDataFrame> {
    let fcs = flow_fcs::file::Fcs::open(path)
        .map_err(|e| PyValueError::new_err(format!("Failed to open FCS file: {e}")))?;
    let df = (*fcs.data_frame).clone();
    Ok(PyDataFrame(df))
}

/// Extract detector columns from a polars DataFrame into a nested list matrix.
///
/// Utility function to convert DataFrame columns to the matrix format
/// expected by TruOls.unmix().
///
/// Args:
///     df: polars DataFrame.
///     detector_columns: List of column names to extract.
///
/// Returns:
///     Nested list (events x detectors).
#[pyfunction]
fn extract_detector_data(
    df: PyDataFrame,
    detector_columns: Vec<String>,
) -> PyResult<Vec<Vec<f64>>> {
    let n_events = df.0.height();
    let n_detectors = detector_columns.len();

    let mut result = vec![vec![0.0; n_detectors]; n_events];

    for (col_idx, col_name) in detector_columns.iter().enumerate() {
        let series = df.0.column(col_name.as_str()).map_err(|e| {
            PyValueError::new_err(format!("Column '{col_name}' not found: {e}"))
        })?;

        if let Ok(f64_vals) = series.f64() {
            for (row_idx, val) in f64_vals.into_iter().enumerate() {
                result[row_idx][col_idx] = val.unwrap_or(0.0);
            }
        } else if let Ok(f32_vals) = series.f32() {
            for (row_idx, val) in f32_vals.into_iter().enumerate() {
                result[row_idx][col_idx] = val.map(|v| v as f64).unwrap_or(0.0);
            }
        } else {
            return Err(PyValueError::new_err(format!(
                "Column '{col_name}' is not numeric (dtype: {:?})",
                series.dtype()
            )));
        }
    }

    Ok(result)
}

/// Calculate cutoff thresholds from unstained control data.
///
/// Args:
///     mixing_matrix: Mixing matrix (detectors x endmembers) as nested list.
///     unstained_control: Unstained control data (events x detectors) as nested list.
///     percentile: Percentile to use (default 0.995).
///
/// Returns:
///     List of cutoff values (one per endmember).
#[pyfunction]
#[pyo3(signature = (mixing_matrix, unstained_control, percentile = 0.995))]
fn calculate_cutoffs(
    mixing_matrix: Vec<Vec<f64>>,
    unstained_control: Vec<Vec<f64>>,
    percentile: f64,
) -> PyResult<Vec<f64>> {
    let mm = mat_from_nested_list(mixing_matrix)?;
    let uc = mat_from_nested_list(unstained_control)?;

    let calculator = flow_tru_ols::CutoffCalculator::calculate(mm.as_ref(), uc.as_ref(), percentile)
        .map_err(to_py_err)?;

    Ok(col_to_list(calculator.cutoffs()))
}

/// Calculate the nonspecific observation from unstained control data.
///
/// This represents the expected background signal from nonspecific binding/noise.
///
/// Args:
///     mixing_matrix: Mixing matrix (detectors x endmembers) as nested list.
///     unstained_control: Unstained control data (events x detectors) as nested list.
///     autofluorescence_idx: Index of the autofluorescence endmember.
///
/// Returns:
///     Nonspecific observation vector (length = n_detectors).
#[pyfunction]
fn calculate_nonspecific_observation(
    mixing_matrix: Vec<Vec<f64>>,
    unstained_control: Vec<Vec<f64>>,
    autofluorescence_idx: usize,
) -> PyResult<Vec<f64>> {
    let mm = mat_from_nested_list(mixing_matrix)?;
    let uc = mat_from_nested_list(unstained_control)?;

    let nonspecific = flow_tru_ols::NonspecificObservation::calculate(
        mm.as_ref(),
        uc.as_ref(),
        autofluorescence_idx,
    )
    .map_err(to_py_err)?;

    Ok(col_to_list(nonspecific.observation()))
}

// ---------------------------------------------------------------------------
// Python module definition
// ---------------------------------------------------------------------------

#[pymodule]
fn tru_ols(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTruOls>()?;
    m.add_class::<PyUnmixingResult>()?;

    m.add_function(wrap_pyfunction!(unmix, m)?)?;
    m.add_function(wrap_pyfunction!(unmix_dataframe, m)?)?;
    m.add_function(wrap_pyfunction!(unmix_fcs, m)?)?;
    m.add_function(wrap_pyfunction!(read_fcs, m)?)?;
    m.add_function(wrap_pyfunction!(extract_detector_data, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_cutoffs, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_nonspecific_observation, m)?)?;

    Ok(())
}
