//! Per-working-directory persistence for tru-ols interactive mode.
//!
//! Stores the user's prior interactive choices in `./.tru-ols-state.json` so that
//! subsequent interactive runs can (a) re-run with the exact prior config, or
//! (b) edit a single setting while keeping the rest. This also doubles as an audit
//! log of the parameters used for a given unmix run.
//!
//! State is written on successful completion and read at the start of interactive mode.
//! Paths are stored as absolute so the state remains meaningful even if the user
//! re-invokes `tru-ols` from a different working directory but the `.tru-ols-state.json`
//! is still reachable (we always read from the current working directory).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Filename used for the per-working-directory state, anchored at `std::env::current_dir()`.
pub const STATE_FILE_NAME: &str = ".tru-ols-state.json";

/// A complete snapshot of the interactive choices for one unmix run. This is written at the end
/// of a successful interactive run and offered to the user at the start of the next one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedState {
    pub stained: PathBuf,
    /// Mixing-source index from [`prompt_mixing_source`]: 0=controls dir, 1=single-stain dir,
    /// 2=CSV mixing matrix, 3=SPILL.
    pub mixing_source: usize,
    pub controls: Option<PathBuf>,
    pub single_stain_controls: Option<PathBuf>,
    pub mixing_matrix: Option<PathBuf>,
    pub use_spill: bool,
    pub unstained: Option<PathBuf>,
    pub detectors: Vec<String>,
    pub endmembers: Vec<String>,
    pub cutoff_percentile: f64,
    pub strategy: String,
    pub autofluorescence: String,
    pub control_assignments: Option<Vec<(String, PathBuf)>>,
    pub output: Option<PathBuf>,
    pub auto_gate: bool,
    pub plot: bool,
    pub plot_format: String,
    pub plot_output_dir: Option<PathBuf>,
    pub compare_ols: bool,
    pub plot_both: bool,
    pub debug_control_plots: bool,
    pub peak_detection: bool,
    pub peak_threshold: f64,
    pub peak_bias: f64,
    pub peak_bias_negative: f64,
    pub use_negative_events: bool,
    pub autofluorescence_mode: String,
    pub af_weight: f64,
    pub min_negative_events: usize,
    pub export_mixing_matrix: Option<PathBuf>,
}

/// Path to the state file in the current working directory.
pub fn state_file_path() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("Failed to read current working directory")?;
    Ok(cwd.join(STATE_FILE_NAME))
}

/// Load prior state from the current working directory, if present. Returns `Ok(None)` when the
/// file is missing; surfaces parse errors so the user can decide whether to blow it away.
pub fn load() -> Result<Option<SavedState>> {
    let path = state_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read state file {}", path.display()))?;
    let state = serde_json::from_str::<SavedState>(&raw)
        .with_context(|| format!("Failed to parse state file {}", path.display()))?;
    Ok(Some(state))
}

/// Write state atomically to the current working directory. Writes to a `.tmp` then renames so a
/// crash mid-write does not corrupt the file.
pub fn save(state: &SavedState) -> Result<PathBuf> {
    let path = state_file_path()?;
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(state).context("Failed to serialize state")?;
    std::fs::write(&tmp, json)
        .with_context(|| format!("Failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("Failed to rename state file to {}", path.display()))?;
    Ok(path)
}

/// Short human-readable summary of a path or value for listing prior choices.
pub fn short_path(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}
