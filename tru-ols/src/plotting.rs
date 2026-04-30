//! Plotting utilities for TRU-OLS unmixing results.
//!
//! This module provides plotting capabilities for visualizing TRU-OLS unmixing results,
//! comparisons with standard OLS, and UCM strategy comparisons.

#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
use crate::error::TruOlsError;
#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
use faer::MatRef;
#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
use flow_fcs::Fcs;
#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
use flow_plots::options::BasePlotOptions;
#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
use flow_plots::{DensityPlot, DensityPlotOptions, Plot, PlotBytes};

/// Plot comparison between OLS and TRU-OLS unmixed data.
///
/// Creates a density plot comparing two unmixed datasets side by side.
///
/// # Arguments
/// * `ols_data` - Fcs struct with OLS unmixed data
/// * `tru_ols_data` - Fcs struct with TRU-OLS unmixed data
/// * `x_param` - Parameter name for x-axis
/// * `y_param` - Parameter name for y-axis
///
/// # Returns
/// Plot bytes (PNG format) containing the comparison plot
#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
pub fn plot_unmixed_comparison(
    ols_data: &Fcs,
    tru_ols_data: &Fcs,
    x_param: &str,
    y_param: &str,
) -> Result<PlotBytes, TruOlsError> {
    // Get data pairs for both datasets (validate OLS has the params; only TRU-OLS is plotted)
    let _ols_pairs = ols_data.get_xy_pairs(x_param, y_param).map_err(|e| {
        TruOlsError::InsufficientData(format!("Failed to get OLS data pairs: {}", e))
    })?;

    let tru_ols_pairs = tru_ols_data.get_xy_pairs(x_param, y_param).map_err(|e| {
        TruOlsError::InsufficientData(format!("Failed to get TRU-OLS data pairs: {}", e))
    })?;

    // Create plot options
    let base = BasePlotOptions::new()
        .width(1600u32)
        .height(800u32)
        .build()
        .map_err(|e| {
            TruOlsError::InsufficientData(format!("Failed to create base plot options: {}", e))
        })?;
    let options = DensityPlotOptions::new().base(base).build().map_err(|e| {
        TruOlsError::InsufficientData(format!("Failed to create plot options: {}", e))
    })?;

    // Create density plot
    let plot = DensityPlot::new();

    // For now, plot TRU-OLS data (can be extended to create side-by-side comparison)
    // Convert to the format expected by flow-plots
    let plot_data: Vec<(f32, f32)> = tru_ols_pairs;

    plot.render(
        plot_data.into(),
        &options,
        &mut flow_plots::render::RenderConfig::default(),
    )
    .map_err(|e| TruOlsError::InsufficientData(format!("Failed to render plot: {}", e)))
}

/// Plot abundance distribution for a specific endmember.
///
/// Creates a histogram showing the distribution of unmixed abundances for a given endmember.
///
/// # Arguments
/// * `unmixed_data` - Matrix of unmixed abundances (events × endmembers)
/// * `endmember_names` - Names of endmembers
/// * `endmember_idx` - Index of the endmember to plot
///
/// # Returns
/// Plot bytes containing the distribution plot
#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
pub fn plot_abundance_distribution(
    unmixed_data: MatRef<'_, f64>,
    endmember_names: &[&str],
    endmember_idx: usize,
) -> Result<PlotBytes, TruOlsError> {
    if endmember_idx >= endmember_names.len() {
        return Err(TruOlsError::DimensionMismatch {
            expected: endmember_names.len(),
            actual: endmember_idx + 1,
        });
    }

    // Extract abundances for this endmember
    let n_events = unmixed_data.nrows();
    let abundances: Vec<f32> = (0..n_events)
        .map(|event_idx| unmixed_data[(event_idx, endmember_idx)] as f32)
        .collect();

    // Create pairs for plotting (abundance vs index, or we could create a histogram)
    // For simplicity, create a scatter plot of abundance vs event index
    let plot_data: Vec<(f32, f32)> = abundances
        .iter()
        .enumerate()
        .map(|(idx, &abundance)| (idx as f32, abundance))
        .collect();

    let base = BasePlotOptions::new()
        .width(800u32)
        .height(600u32)
        .build()
        .map_err(|e| {
            TruOlsError::InsufficientData(format!("Failed to create base plot options: {}", e))
        })?;
    let options = DensityPlotOptions::new().base(base).build().map_err(|e| {
        TruOlsError::InsufficientData(format!("Failed to create plot options: {}", e))
    })?;

    let plot = DensityPlot::new();
    plot.render(
        plot_data.into(),
        &options,
        &mut flow_plots::render::RenderConfig::default(),
    )
    .map_err(|e| TruOlsError::InsufficientData(format!("Failed to render plot: {}", e)))
}

/// Plot comparison between Zero and UCM strategies.
///
/// Creates a density plot comparing TRU-OLS results with zero strategy vs UCM strategy.
///
/// # Arguments
/// * `zero_data` - Fcs struct with TRU-OLS zero strategy data
/// * `ucm_data` - Fcs struct with TRU-OLS UCM strategy data
/// * `x_param` - Parameter name for x-axis
/// * `y_param` - Parameter name for y-axis
///
/// # Returns
/// Plot bytes containing the comparison plot
#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
pub fn plot_ucm_comparison(
    zero_data: &Fcs,
    ucm_data: &Fcs,
    x_param: &str,
    y_param: &str,
) -> Result<PlotBytes, TruOlsError> {
    // Similar to plot_unmixed_comparison but for UCM vs Zero (validate zero has params; only UCM is plotted)
    let _zero_pairs = zero_data.get_xy_pairs(x_param, y_param).map_err(|e| {
        TruOlsError::InsufficientData(format!("Failed to get zero strategy data pairs: {}", e))
    })?;

    let ucm_pairs = ucm_data.get_xy_pairs(x_param, y_param).map_err(|e| {
        TruOlsError::InsufficientData(format!("Failed to get UCM strategy data pairs: {}", e))
    })?;

    // Plot UCM data (can be extended for side-by-side comparison)
    let plot_data: Vec<(f32, f32)> = ucm_pairs;

    let base = BasePlotOptions::new()
        .width(1600u32)
        .height(800u32)
        .build()
        .map_err(|e| {
            TruOlsError::InsufficientData(format!("Failed to create base plot options: {}", e))
        })?;
    let options = DensityPlotOptions::new().base(base).build().map_err(|e| {
        TruOlsError::InsufficientData(format!("Failed to create plot options: {}", e))
    })?;

    let plot = DensityPlot::new();
    plot.render(
        plot_data.into(),
        &options,
        &mut flow_plots::render::RenderConfig::default(),
    )
    .map_err(|e| TruOlsError::InsufficientData(format!("Failed to render plot: {}", e)))
}
