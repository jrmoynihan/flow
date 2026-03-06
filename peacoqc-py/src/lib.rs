use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3_polars::PyDataFrame;
use std::collections::HashMap;

use peacoqc_rs::{
    DoubletConfig, DoubletResult as RsDoubletResult, FcsFilter, MarginConfig,
    MarginResult as RsMarginResult, PeacoQCConfig, PeacoQCData,
    PeacoQCResult as RsPeacoQCResult, QCMode,
};

// ---------------------------------------------------------------------------
// Internal wrapper: implements PeacoQCData over a polars DataFrame + metadata
// ---------------------------------------------------------------------------

struct DataFrameQCData {
    df: polars::frame::DataFrame,
    channel_ranges: HashMap<String, (f64, f64)>,
}

impl PeacoQCData for DataFrameQCData {
    fn n_events(&self) -> usize {
        self.df.height()
    }

    fn channel_names(&self) -> Vec<String> {
        self.df
            .get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)> {
        self.channel_ranges.get(channel).copied()
    }

    fn get_channel_f64(&self, channel: &str) -> peacoqc_rs::Result<Vec<f64>> {
        let series = self
            .df
            .column(channel)
            .map_err(|_| peacoqc_rs::PeacoQCError::ChannelNotFound(channel.to_string()))?;

        if let Ok(f64_vals) = series.f64() {
            Ok(f64_vals.into_iter().flatten().collect())
        } else if let Ok(f32_vals) = series.f32() {
            Ok(f32_vals
                .into_iter()
                .flatten()
                .map(|v| v as f64)
                .collect())
        } else {
            Err(peacoqc_rs::PeacoQCError::InvalidChannel(format!(
                "Channel {channel} is not numeric (dtype: {:?})",
                series.dtype()
            )))
        }
    }
}

impl FcsFilter for DataFrameQCData {
    fn filter(&self, mask: &[bool]) -> peacoqc_rs::Result<Self> {
        use polars::prelude::*;
        let mask_series = Series::new("mask".into(), mask.to_vec());
        let mask_ca = mask_series.bool().map_err(|e| {
            peacoqc_rs::PeacoQCError::StatsError(format!(
                "Failed to create boolean mask: {e}"
            ))
        })?;
        let filtered = self.df.filter(mask_ca).map_err(peacoqc_rs::PeacoQCError::PolarsError)?;
        Ok(DataFrameQCData {
            df: filtered,
            channel_ranges: self.channel_ranges.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Helper: convert PeacoQCError → PyErr
// ---------------------------------------------------------------------------

fn to_py_err(e: peacoqc_rs::PeacoQCError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

fn anyhow_to_py_err(e: anyhow::Error) -> PyErr {
    PyValueError::new_err(e.to_string())
}

// ---------------------------------------------------------------------------
// Python-visible result types
// ---------------------------------------------------------------------------

/// Result of the PeacoQC quality control algorithm.
#[pyclass(name = "QCResult")]
#[derive(Clone)]
struct PyQCResult {
    inner: RsPeacoQCResult,
}

#[pymethods]
impl PyQCResult {
    /// Boolean list: True = keep, False = remove.
    #[getter]
    fn good_cells(&self) -> Vec<bool> {
        self.inner.good_cells.clone()
    }

    /// Percentage of events removed.
    #[getter]
    fn percentage_removed(&self) -> f64 {
        self.inner.percentage_removed
    }

    /// Isolation Tree percentage (if used).
    #[getter]
    fn it_percentage(&self) -> Option<f64> {
        self.inner.it_percentage
    }

    /// MAD percentage (if used).
    #[getter]
    fn mad_percentage(&self) -> Option<f64> {
        self.inner.mad_percentage
    }

    /// Consecutive filtering percentage.
    #[getter]
    fn consecutive_percentage(&self) -> f64 {
        self.inner.consecutive_percentage
    }

    /// Number of bins used.
    #[getter]
    fn n_bins(&self) -> usize {
        self.inner.n_bins
    }

    /// Events per bin.
    #[getter]
    fn events_per_bin(&self) -> usize {
        self.inner.events_per_bin
    }

    fn __repr__(&self) -> String {
        format!(
            "QCResult(removed={:.2}%, bins={}, events_per_bin={})",
            self.inner.percentage_removed, self.inner.n_bins, self.inner.events_per_bin
        )
    }
}

/// Result of margin removal.
#[pyclass(name = "MarginResult")]
#[derive(Clone)]
struct PyMarginResult {
    inner: RsMarginResult,
}

#[pymethods]
impl PyMarginResult {
    /// Boolean mask: True = keep, False = margin event.
    #[getter]
    fn mask(&self) -> Vec<bool> {
        self.inner.mask.clone()
    }

    /// Total percentage of events removed.
    #[getter]
    fn percentage_removed(&self) -> f64 {
        self.inner.percentage_removed
    }

    /// Per-channel removal counts: {channel: (min_removed, max_removed)}.
    #[getter]
    fn margin_matrix(&self) -> HashMap<String, (usize, usize)> {
        self.inner.margin_matrix.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "MarginResult(removed={:.2}%, channels={})",
            self.inner.percentage_removed,
            self.inner.margin_matrix.len()
        )
    }
}

/// Result of doublet removal.
#[pyclass(name = "DoubletResult")]
#[derive(Clone)]
struct PyDoubletResult {
    inner: RsDoubletResult,
}

#[pymethods]
impl PyDoubletResult {
    /// Boolean mask: True = keep, False = doublet.
    #[getter]
    fn mask(&self) -> Vec<bool> {
        self.inner.mask.clone()
    }

    /// Percentage of events removed.
    #[getter]
    fn percentage_removed(&self) -> f64 {
        self.inner.percentage_removed
    }

    /// Median ratio used.
    #[getter]
    fn median_ratio(&self) -> f64 {
        self.inner.median_ratio
    }

    /// MAD of ratios.
    #[getter]
    fn mad_ratio(&self) -> f64 {
        self.inner.mad_ratio
    }

    /// Threshold used for doublet detection.
    #[getter]
    fn threshold(&self) -> f64 {
        self.inner.threshold
    }

    fn __repr__(&self) -> String {
        format!(
            "DoubletResult(removed={:.2}%, threshold={:.4})",
            self.inner.percentage_removed, self.inner.threshold
        )
    }
}

/// Result of opening and optionally preprocessing an FCS file.
#[pyclass(name = "FcsFile")]
#[derive(Clone)]
struct PyFcsFile {
    inner: flow_fcs::file::Fcs,
}

#[pymethods]
impl PyFcsFile {
    /// Open an FCS file from disk.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let fcs = flow_fcs::file::Fcs::open(path).map_err(|e| {
            PyValueError::new_err(format!("Failed to open FCS file: {e}"))
        })?;
        Ok(PyFcsFile { inner: fcs })
    }

    /// Number of events in the file.
    #[getter]
    fn n_events(&self) -> usize {
        self.inner.n_events()
    }

    /// Channel names.
    #[getter]
    fn channel_names(&self) -> Vec<String> {
        self.inner.channel_names()
    }

    /// Fluorescence channel names (auto-detected, excluding FSC/SSC/Time).
    #[getter]
    fn fluorescence_channels(&self) -> Vec<String> {
        self.inner.get_fluorescence_channels()
    }

    /// Return the event data as a polars DataFrame.
    #[getter]
    fn dataframe(&self) -> PyResult<PyDataFrame> {
        let df: polars::frame::DataFrame = (*self.inner.data_frame).clone();
        Ok(PyDataFrame(df))
    }

    /// Whether compensation info ($SPILLOVER) exists.
    fn has_compensation(&self) -> bool {
        self.inner.has_compensation()
    }

    fn __repr__(&self) -> String {
        format!(
            "FcsFile(events={}, channels={})",
            self.inner.n_events(),
            self.inner.channel_names().len()
        )
    }
}

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

/// Run the PeacoQC quality control algorithm.
///
/// Args:
///     df: polars DataFrame with event data (columns = channels).
///     channels: list of channel names to analyze.
///     channel_ranges: dict mapping channel name to (min, max) range tuple.
///     mode: QC mode - "all" (default), "isolation_tree", "mad", or "none".
///     mad: MAD threshold multiplier (default 6.0).
///     it_limit: Isolation Tree gain limit (default 0.6).
///     consecutive_bins: minimum consecutive good bins (default 5).
///     min_cells: minimum events per bin (default 150).
///     max_bins: maximum number of bins (default 500).
///     events_per_bin: override auto-calculated events per bin.
///     remove_zeros: remove zeros before peak detection (default False).
///     peak_removal: peak removal fraction (default 1/3).
///
/// Returns:
///     QCResult with good_cells mask and statistics.
#[pyfunction]
#[pyo3(signature = (
    df,
    channels,
    channel_ranges,
    mode = "all",
    mad = 6.0,
    it_limit = 0.6,
    consecutive_bins = 5,
    min_cells = 150,
    max_bins = 500,
    events_per_bin = None,
    remove_zeros = false,
    peak_removal = None,
))]
#[allow(clippy::too_many_arguments)]
fn run_qc(
    df: PyDataFrame,
    channels: Vec<String>,
    channel_ranges: HashMap<String, (f64, f64)>,
    mode: &str,
    mad: f64,
    it_limit: f64,
    consecutive_bins: usize,
    min_cells: usize,
    max_bins: usize,
    events_per_bin: Option<usize>,
    remove_zeros: bool,
    peak_removal: Option<f64>,
) -> PyResult<PyQCResult> {
    let qc_mode = match mode {
        "all" => QCMode::All,
        "isolation_tree" => QCMode::IsolationTree,
        "mad" => QCMode::MAD,
        "none" => QCMode::None,
        other => {
            return Err(PyValueError::new_err(format!(
                "Unknown QC mode '{other}'. Use 'all', 'isolation_tree', 'mad', or 'none'."
            )));
        }
    };

    let data = DataFrameQCData {
        df: df.0,
        channel_ranges,
    };

    let config = PeacoQCConfig {
        channels,
        determine_good_cells: qc_mode,
        min_cells,
        max_bins,
        events_per_bin,
        mad,
        it_limit,
        consecutive_bins,
        remove_zeros,
        peak_removal: peak_removal.unwrap_or(1.0 / 3.0),
        ..Default::default()
    };

    let result = peacoqc_rs::peacoqc(&data, &config).map_err(to_py_err)?;
    Ok(PyQCResult { inner: result })
}

/// Remove margin events from flow cytometry data.
///
/// Margin events occur at detector saturation (min/max range boundaries).
///
/// Args:
///     df: polars DataFrame with event data.
///     channels: list of channel names to check.
///     channel_ranges: dict mapping channel name to (min, max) range.
///     remove_min: optional list of channels for minimum margin removal.
///     remove_max: optional list of channels for maximum margin removal.
///
/// Returns:
///     MarginResult with boolean mask and statistics.
#[pyfunction]
#[pyo3(signature = (df, channels, channel_ranges, remove_min = None, remove_max = None))]
fn remove_margins(
    df: PyDataFrame,
    channels: Vec<String>,
    channel_ranges: HashMap<String, (f64, f64)>,
    remove_min: Option<Vec<String>>,
    remove_max: Option<Vec<String>>,
) -> PyResult<PyMarginResult> {
    let data = DataFrameQCData {
        df: df.0,
        channel_ranges: channel_ranges.clone(),
    };

    let config = MarginConfig {
        channels,
        channel_specifications: Some(channel_ranges),
        remove_min,
        remove_max,
    };

    let result = peacoqc_rs::remove_margins(&data, &config).map_err(to_py_err)?;
    Ok(PyMarginResult { inner: result })
}

/// Remove doublet events based on area/height scatter ratio.
///
/// Args:
///     df: polars DataFrame with event data.
///     channel_ranges: dict mapping channel name to (min, max) range.
///     channel1: first channel name (default "FSC-A").
///     channel2: second channel name (default "FSC-H").
///     nmad: number of MADs above median for threshold (default 4.0).
///     b: shift parameter (default 0.0).
///
/// Returns:
///     DoubletResult with boolean mask and statistics.
#[pyfunction]
#[pyo3(signature = (df, channel_ranges, channel1 = "FSC-A", channel2 = "FSC-H", nmad = 4.0, b = 0.0))]
fn remove_doublets(
    df: PyDataFrame,
    channel_ranges: HashMap<String, (f64, f64)>,
    channel1: &str,
    channel2: &str,
    nmad: f64,
    b: f64,
) -> PyResult<PyDoubletResult> {
    let data = DataFrameQCData {
        df: df.0,
        channel_ranges,
    };

    let config = DoubletConfig {
        channel1: channel1.to_string(),
        channel2: channel2.to_string(),
        nmad,
        b,
    };

    let result = peacoqc_rs::remove_doublets(&data, &config).map_err(to_py_err)?;
    Ok(PyDoubletResult { inner: result })
}

/// Open an FCS file and return its data as a polars DataFrame.
///
/// This is a convenience function that opens an FCS file and returns
/// the event data directly as a polars DataFrame.
///
/// Args:
///     path: path to the FCS file.
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

/// Open an FCS file, apply compensation and transformation, then run PeacoQC.
///
/// This is the highest-level convenience function that mirrors the typical
/// R PeacoQC workflow:
///   1. Open FCS file
///   2. Apply compensation from $SPILLOVER
///   3. Apply biexponential (or arcsinh) transformation
///   4. Run PeacoQC on fluorescence channels
///
/// Args:
///     path: path to the FCS file.
///     channels: optional list of channels (auto-detects fluorescence channels if None).
///     apply_compensation: apply compensation from $SPILLOVER (default True).
///     apply_transformation: apply biexponential/arcsinh transform (default True).
///     mode: QC mode - "all", "isolation_tree", "mad", or "none" (default "all").
///     mad: MAD threshold multiplier (default 6.0).
///     it_limit: Isolation Tree gain limit (default 0.6).
///     consecutive_bins: minimum consecutive good bins (default 5).
///
/// Returns:
///     Tuple of (QCResult, polars DataFrame of the preprocessed data).
#[pyfunction]
#[pyo3(signature = (
    path,
    channels = None,
    apply_compensation = true,
    apply_transformation = true,
    mode = "all",
    mad = 6.0,
    it_limit = 0.6,
    consecutive_bins = 5,
))]
#[allow(clippy::too_many_arguments)]
fn run_qc_on_fcs(
    path: &str,
    channels: Option<Vec<String>>,
    apply_compensation: bool,
    apply_transformation: bool,
    mode: &str,
    mad: f64,
    it_limit: f64,
    consecutive_bins: usize,
) -> PyResult<(PyQCResult, PyDataFrame)> {
    let fcs = flow_fcs::file::Fcs::open(path)
        .map_err(|e| PyValueError::new_err(format!("Failed to open FCS file: {e}")))?;

    let fcs = peacoqc_rs::preprocess_fcs(fcs, apply_compensation, apply_transformation, 2000.0)
        .map_err(anyhow_to_py_err)?;

    let channels = channels.unwrap_or_else(|| fcs.get_fluorescence_channels());

    let qc_mode = match mode {
        "all" => QCMode::All,
        "isolation_tree" => QCMode::IsolationTree,
        "mad" => QCMode::MAD,
        "none" => QCMode::None,
        other => {
            return Err(PyValueError::new_err(format!(
                "Unknown QC mode '{other}'. Use 'all', 'isolation_tree', 'mad', or 'none'."
            )));
        }
    };

    let config = PeacoQCConfig {
        channels,
        determine_good_cells: qc_mode,
        mad,
        it_limit,
        consecutive_bins,
        apply_compensation: false, // already preprocessed
        apply_transformation: false,
        ..Default::default()
    };

    let result = peacoqc_rs::peacoqc(&fcs, &config).map_err(to_py_err)?;
    let df = (*fcs.data_frame).clone();

    Ok((PyQCResult { inner: result }, PyDataFrame(df)))
}

/// Open an FCS file as an FcsFile object for inspection and QC.
///
/// Args:
///     path: path to the FCS file.
///
/// Returns:
///     FcsFile object.
#[pyfunction]
fn open_fcs(path: &str) -> PyResult<PyFcsFile> {
    PyFcsFile::open(path)
}

/// Run PeacoQC directly on an FcsFile object.
///
/// Args:
///     fcs: FcsFile object (from open_fcs or FcsFile.open).
///     channels: optional list of channels (auto-detects fluorescence channels if None).
///     mode: QC mode (default "all").
///     mad: MAD threshold multiplier (default 6.0).
///     it_limit: Isolation Tree gain limit (default 0.6).
///     consecutive_bins: minimum consecutive good bins (default 5).
///
/// Returns:
///     QCResult.
#[pyfunction]
#[pyo3(signature = (fcs, channels = None, mode = "all", mad = 6.0, it_limit = 0.6, consecutive_bins = 5))]
fn run_qc_on_fcs_obj(
    fcs: &PyFcsFile,
    channels: Option<Vec<String>>,
    mode: &str,
    mad: f64,
    it_limit: f64,
    consecutive_bins: usize,
) -> PyResult<PyQCResult> {
    let channels = channels.unwrap_or_else(|| fcs.inner.get_fluorescence_channels());

    let qc_mode = match mode {
        "all" => QCMode::All,
        "isolation_tree" => QCMode::IsolationTree,
        "mad" => QCMode::MAD,
        "none" => QCMode::None,
        other => {
            return Err(PyValueError::new_err(format!(
                "Unknown QC mode '{other}'. Use 'all', 'isolation_tree', 'mad', or 'none'."
            )));
        }
    };

    let config = PeacoQCConfig {
        channels,
        determine_good_cells: qc_mode,
        mad,
        it_limit,
        consecutive_bins,
        ..Default::default()
    };

    let result = peacoqc_rs::peacoqc(&fcs.inner, &config).map_err(to_py_err)?;
    Ok(PyQCResult { inner: result })
}

/// Preprocess an FCS file (compensation + transformation) without running QC.
///
/// Args:
///     fcs: FcsFile object.
///     apply_compensation: apply $SPILLOVER compensation (default True).
///     apply_transformation: apply biexponential/arcsinh transform (default True).
///
/// Returns:
///     New FcsFile with preprocessed data.
#[pyfunction]
#[pyo3(signature = (fcs, apply_compensation = true, apply_transformation = true))]
fn preprocess(
    fcs: &PyFcsFile,
    apply_compensation: bool,
    apply_transformation: bool,
) -> PyResult<PyFcsFile> {
    let preprocessed = peacoqc_rs::preprocess_fcs(
        fcs.inner.clone(),
        apply_compensation,
        apply_transformation,
        2000.0,
    )
    .map_err(anyhow_to_py_err)?;
    Ok(PyFcsFile { inner: preprocessed })
}

/// Filter an FcsFile by a boolean mask, returning a new FcsFile.
///
/// Args:
///     fcs: FcsFile object.
///     mask: list of booleans (True = keep).
///
/// Returns:
///     Filtered FcsFile.
#[pyfunction]
fn filter_fcs(fcs: &PyFcsFile, mask: Vec<bool>) -> PyResult<PyFcsFile> {
    let filtered = fcs.inner.filter(&mask).map_err(to_py_err)?;
    Ok(PyFcsFile { inner: filtered })
}

// ---------------------------------------------------------------------------
// Python module definition
// ---------------------------------------------------------------------------

#[pymodule]
fn peacoqc(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyQCResult>()?;
    m.add_class::<PyMarginResult>()?;
    m.add_class::<PyDoubletResult>()?;
    m.add_class::<PyFcsFile>()?;

    m.add_function(wrap_pyfunction!(run_qc, m)?)?;
    m.add_function(wrap_pyfunction!(remove_margins, m)?)?;
    m.add_function(wrap_pyfunction!(remove_doublets, m)?)?;
    m.add_function(wrap_pyfunction!(read_fcs, m)?)?;
    m.add_function(wrap_pyfunction!(run_qc_on_fcs, m)?)?;
    m.add_function(wrap_pyfunction!(open_fcs, m)?)?;
    m.add_function(wrap_pyfunction!(run_qc_on_fcs_obj, m)?)?;
    m.add_function(wrap_pyfunction!(preprocess, m)?)?;
    m.add_function(wrap_pyfunction!(filter_fcs, m)?)?;

    Ok(())
}
