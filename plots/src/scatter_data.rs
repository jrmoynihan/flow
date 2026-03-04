//! Scatter plot data types supporting simple, gate-colored, and z-axis colored plots.

/// Scatter plot data with optional per-point metadata for overlay and continuous coloring.
///
/// Use [`From`] to convert from `Vec<(f32, f32)>` for simple plots.
///
/// # Examples
///
/// Simple scatter (solid or density):
/// ```
/// # use flow_plots::scatter_data::ScatterPlotData;
/// let data: ScatterPlotData = vec![(1.0, 2.0), (3.0, 4.0)].into();
/// ```
///
/// Scatter with discrete gate colors:
/// ```
/// # use flow_plots::scatter_data::ScatterPlotData;
/// let points = vec![(1.0, 2.0), (3.0, 4.0), (5.0, 6.0)];
/// let gate_ids = vec![0, 1, 0];  // gate index per point
/// let data = ScatterPlotData::with_gates(points, gate_ids);
/// ```
///
/// Scatter colored by continuous z-axis:
/// ```
/// # use flow_plots::scatter_data::ScatterPlotData;
/// let points = vec![(1.0, 2.0), (3.0, 4.0)];
/// let z_values = vec![0.5, 1.0];
/// let data = ScatterPlotData::with_z(points, z_values);
/// ```
#[derive(Clone, Debug)]
pub struct ScatterPlotData {
    /// (x, y) coordinate pairs
    pub points: Vec<(f32, f32)>,
    /// Gate index per point (for discrete overlay coloring). Indexes into `gate_colors` in options.
    pub gate_ids: Option<Vec<u32>>,
    /// Z-axis value per point (for continuous colormap coloring)
    pub z_values: Option<Vec<f32>>,
}

impl ScatterPlotData {
    /// Create simple scatter data (no gates, no z-axis).
    pub fn new(points: Vec<(f32, f32)>) -> Self {
        Self {
            points,
            gate_ids: None,
            z_values: None,
        }
    }

    /// Create scatter data with discrete gate IDs for overlay coloring.
    ///
    /// # Errors
    /// Returns `Err` if `gate_ids.len() != points.len()`.
    pub fn with_gates(
        points: Vec<(f32, f32)>,
        gate_ids: Vec<u32>,
    ) -> Result<Self, ScatterDataError> {
        if gate_ids.len() != points.len() {
            return Err(ScatterDataError {
                points_len: points.len(),
                metadata_len: gate_ids.len(),
                field: "gate_ids",
            });
        }
        Ok(Self {
            points,
            gate_ids: Some(gate_ids),
            z_values: None,
        })
    }

    /// Create scatter data with z-axis values for continuous coloring.
    ///
    /// # Errors
    /// Returns `Err` if `z_values.len() != points.len()`.
    pub fn with_z(points: Vec<(f32, f32)>, z_values: Vec<f32>) -> Result<Self, ScatterDataError> {
        if z_values.len() != points.len() {
            return Err(ScatterDataError {
                points_len: points.len(),
                metadata_len: z_values.len(),
                field: "z_values",
            });
        }
        Ok(Self {
            points,
            gate_ids: None,
            z_values: Some(z_values),
        })
    }

    /// Slice of (x, y) points.
    pub fn xy(&self) -> &[(f32, f32)] {
        &self.points
    }

    /// Whether this data has gate overlay info.
    pub fn has_gates(&self) -> bool {
        self.gate_ids.is_some()
    }

    /// Whether this data has z-axis values for continuous coloring.
    pub fn has_z(&self) -> bool {
        self.z_values.is_some()
    }
}

impl From<Vec<(f32, f32)>> for ScatterPlotData {
    fn from(points: Vec<(f32, f32)>) -> Self {
        Self::new(points)
    }
}

/// Error when constructing scatter plot data with invalid metadata lengths.
#[derive(Debug, Clone)]
pub struct ScatterDataError {
    points_len: usize,
    metadata_len: usize,
    field: &'static str,
}

impl std::fmt::Display for ScatterDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "length mismatch: points has {} elements but {} has {}",
            self.points_len, self.field, self.metadata_len
        )
    }
}

impl std::error::Error for ScatterDataError {}
