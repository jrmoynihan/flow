//! Interactive step-by-step prompts for TRU-OLS unmix options.
//!
//! Path inputs support environment variable expansion (e.g. `$EXP/file.fcs` or `${EXP}/TSC samples`)
//! so that interactive prompts behave like the shell for path arguments. Export variables in the
//! same shell before running (e.g. `export EXP='/path/to/Plate_001'`) so they expand correctly.

use anyhow::{Context, Result};
use inquire::ui::{Attributes, Color, RenderConfig, StyleSheet};
use inquire::{Confirm, CustomType, Select, Text};
use std::path::{Path, PathBuf};

use crate::commands;

/// Render config that shows the prompt (question) in bold and a distinct color
/// so it stands out from the options and help text.
fn prompt_highlight_render_config() -> RenderConfig<'static> {
    RenderConfig {
        prompt: StyleSheet::default()
            .with_fg(Color::LightCyan)
            .with_attr(Attributes::BOLD),
        ..RenderConfig::default()
    }
}

/// Normalize path input: trim, strip surrounding quotes, remove embedded newlines.
/// Handles pasted paths that may include quotes or accidental line breaks.
fn normalize_path_input(s: &str) -> String {
    let t = s.trim();
    let without_quotes = match (t.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')),
                                t.strip_prefix('"').and_then(|s| s.strip_suffix('"'))) {
        (Some(inner), _) | (_, Some(inner)) => inner.trim(),
        _ => t,
    };
    without_quotes.replace('\r', "").replace('\n', "")
}

/// Expand `$VAR` and `${VAR}` in the string using the process environment.
/// Unset variables are left unchanged. Used so interactive path input matches shell behavior.
fn expand_path_input(s: &str) -> String {
    let normalized = normalize_path_input(s);
    shellexpand::env_with_context_no_errors(&normalized, |var_name| std::env::var(var_name).ok())
        .into_owned()
}

/// Mixing matrix source choice (order: default is Controls directory).
const MIXING_SOURCE_OPTIONS: [&str; 4] = [
    "Controls directory (single-stain + unstained in one folder)",
    "Single-stain controls only (separate directory)",
    "CSV file (precomputed mixing matrix)",
    "Use SPILL from FCS file",
];

fn prompt_stained_path() -> Result<PathBuf> {
    let path = Text::new("Path to stained sample FCS file or directory of FCS files")
        .with_help_message("File or directory path (supports $VAR and ${VAR}); quotes and line breaks are stripped")
        .with_validator(|s: &str| {
            let expanded = expand_path_input(s);
            let p = Path::new(&expanded);
            if p.exists() {
                Ok(inquire::validator::Validation::Valid)
            } else {
                Ok(inquire::validator::Validation::Invalid(
                    "Path does not exist".into(),
                ))
            }
        })
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
    Ok(PathBuf::from(expand_path_input(&path)))
}

fn prompt_mixing_source() -> Result<usize> {
    let options: Vec<&str> = MIXING_SOURCE_OPTIONS.iter().copied().collect();
    let ans = Select::new("Mixing matrix source", options)
        .with_starting_cursor(0)
        .with_help_message("Controls directory is the default (all control files in one folder)")
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
    let idx = MIXING_SOURCE_OPTIONS
        .iter()
        .position(|&s| s == ans)
        .unwrap_or(0);
    Ok(idx)
}

fn prompt_path(prompt: &str, required: bool) -> Result<Option<PathBuf>> {
    let msg = if required {
        format!("{} (required)", prompt)
    } else {
        format!("{} (leave empty to skip)", prompt)
    };
    let path = Text::new(&msg)
        .with_validator(move |s: &str| {
            let normalized = normalize_path_input(s);
            if normalized.is_empty() {
                if required {
                    Ok(inquire::validator::Validation::Invalid(
                        "This path is required".into(),
                    ))
                } else {
                    Ok(inquire::validator::Validation::Valid)
                }
            } else {
                let expanded = expand_path_input(s);
                let p = Path::new(&expanded);
                if p.exists() {
                    Ok(inquire::validator::Validation::Valid)
                } else {
                    Ok(inquire::validator::Validation::Invalid(
                        "Path does not exist".into(),
                    ))
                }
            }
        })
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
    let normalized = normalize_path_input(&path);
    if normalized.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(expand_path_input(&path))))
    }
}

fn prompt_path_required(prompt: &str) -> Result<PathBuf> {
    prompt_path(prompt, true).and_then(|o| {
        o.context("Required path was not provided")
    })
}

fn prompt_comma_list(prompt: &str, required: bool) -> Result<Vec<String>> {
    let msg = if required {
        format!("{} (comma-separated, required)", prompt)
    } else {
        format!("{} (comma-separated, leave empty for auto-detect)", prompt)
    };
    let s = Text::new(&msg)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        if required {
            anyhow::bail!("This field is required");
        }
        return Ok(Vec::new());
    }
    Ok(trimmed
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect())
}

fn prompt_confirm(prompt: &str, default: bool) -> Result<bool> {
    Confirm::new(prompt)
        .with_default(default)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))
}

fn prompt_f64_with_default(prompt: &str, default: f64) -> Result<f64> {
    CustomType::<f64>::new(prompt)
        .with_default(default)
        .with_error_message("Please enter a valid number")
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))
}

fn prompt_usize_with_default(prompt: &str, default: usize) -> Result<usize> {
    CustomType::<usize>::new(prompt)
        .with_default(default)
        .with_error_message("Please enter a valid positive integer")
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))
}

/// Run the interactive flow and then call run_unmix_command with collected args.
pub fn run_interactive() -> Result<()> {
    println!("Interactive mode: you will be prompted for each option.\n");

    inquire::set_global_render_config(prompt_highlight_render_config());

    // Step 1: Stained path
    let stained = prompt_stained_path()?;

    // Step 2: Mixing source (default: Controls directory = index 0)
    let mixing_source = prompt_mixing_source()?;

    let (controls, single_stain_controls, mixing_matrix, use_spill, unstained, detectors, endmembers) =
        match mixing_source {
            0 => {
                // Controls directory
                let controls_dir = prompt_path_required("Path to controls directory (single-stain + unstained)")?;
                let unstained_override = prompt_path("Unstained control path (override auto-detect)", false)?;
                (
                    Some(controls_dir),
                    None,
                    None,
                    false,
                    unstained_override,
                    Vec::new(),
                    Vec::new(),
                )
            }
            1 => {
                // Single-stain controls only
                let single_stain_dir = prompt_path_required("Path to single-stain controls directory")?;
                let unstained_path = prompt_path_required("Path to unstained control FCS file")?;
                let detectors = prompt_comma_list("Detector names", false)?;
                let endmembers = prompt_comma_list("Endmember names", false)?;
                (
                    None,
                    Some(single_stain_dir),
                    None,
                    false,
                    Some(unstained_path),
                    detectors,
                    endmembers,
                )
            }
            2 => {
                // CSV mixing matrix
                let matrix_path = prompt_path_required("Path to mixing matrix CSV file")?;
                let detectors = prompt_comma_list("Detector names", true)?;
                let endmembers = prompt_comma_list("Endmember names", true)?;
                let unstained_path = prompt_path_required("Path to unstained control FCS file")?;
                (
                    None,
                    None,
                    Some(matrix_path),
                    false,
                    Some(unstained_path),
                    detectors,
                    endmembers,
                )
            }
            3 => {
                // Use SPILL from FCS
                let unstained_path = prompt_path_required("Path to unstained control FCS file")?;
                (
                    None,
                    None,
                    None,
                    true,
                    Some(unstained_path),
                    Vec::new(),
                    Vec::new(),
                )
            }
            _ => {
                unreachable!("mixing source index out of range");
            }
        };

    // Step 3: Unmixing parameters
    let cutoff_percentile = prompt_f64_with_default("Cutoff percentile", 0.995)?;
    let strategy_options = vec!["ucm", "zero"];
    let strategy = Select::new("Unmixing strategy", strategy_options)
        .with_starting_cursor(0)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?
        .to_string();
    let autofluorescence = Text::new("Autofluorescence endmember name")
        .with_default("Autofluorescence")
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
    let autofluorescence = autofluorescence.trim().to_string();
    if autofluorescence.is_empty() {
        anyhow::bail!("Autofluorescence name cannot be empty");
    }

    // Step 4: Output and plotting
    let output = prompt_path("Output FCS file path", false)?;
    let plot = prompt_confirm("Generate comparison plots?", true)?;
    let plot_format = if plot {
        let options = vec!["png", "svg", "pdf"];
        Select::new("Plot format", options)
            .with_starting_cursor(0)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?
            .to_string()
    } else {
        "png".to_string()
    };
    let mut plot_output_dir = prompt_path("Directory for plot outputs", false)?;
    let compare_ols = prompt_confirm("Also run standard OLS and compare?", true)?;
    let plot_both = prompt_confirm("Generate plots for both OLS and TRU-OLS?", false)?;

    let debug_control_plots = if (mixing_source == 0 || mixing_source == 1)
        && prompt_confirm(
            "Generate debug control plots (FSC-A vs SSC-A at cleanup stages + per-endmember spectral)?",
            false,
        )?
    {
        if plot_output_dir.is_none() {
            let dir = prompt_path_required("Plot output directory (required for debug control plots)")?;
            plot_output_dir = Some(dir);
        }
        true
    } else {
        false
    };

    // Step 5: Advanced options
    let (peak_detection, peak_threshold, peak_bias, peak_bias_negative, use_negative_events,
         autofluorescence_mode, af_weight, min_negative_events, auto_gate, export_mixing_matrix) =
        if prompt_confirm("Configure advanced options (peak detection, negative events, auto-gate)?", false)? {
            let peak_detection = prompt_confirm("Enable peak-based median selection for single-stain controls?", true)?;
            let peak_threshold = prompt_f64_with_default("Peak detection threshold (fraction of max density)", 0.3)?;
            let peak_bias = prompt_f64_with_default("Peak bias fraction for positive peaks (0.5 = upper 50%)", 0.5)?;
            let peak_bias_negative = prompt_f64_with_default("Peak bias fraction for negative peaks", 0.5)?;
            let af_mode_options = vec!["universal", "negative-events", "hybrid"];
            let autofluorescence_mode = Select::new("Autofluorescence mode", af_mode_options)
                .with_help_message(
                    "universal: unstained AF only | negative-events: per-control negative-event AF | hybrid: blend both",
                )
                .with_starting_cursor(0)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?
                .to_string();
            // Avoid redundant prompts: mode implies whether we extract negative-event AF.
            let (use_negative_events, af_weight) = match autofluorescence_mode.as_str() {
                "negative-events" | "hybrid" => {
                    let w = if autofluorescence_mode == "hybrid" {
                        prompt_f64_with_default(
                            "Autofluorescence weight for hybrid mode (0.0 = all negative-event, 1.0 = all universal)",
                            0.7,
                        )?
                    } else {
                        0.7
                    };
                    (true, w)
                }
                _ => {
                    let extract_for_diagnostics = prompt_confirm(
                        "Extract negative events from controls for diagnostics only? (mixing matrix still uses universal AF)",
                        false,
                    )?;
                    (extract_for_diagnostics, 0.7)
                }
            };
            let min_negative_events = if use_negative_events {
                prompt_usize_with_default("Minimum number of negative events required", 100)?
            } else {
                100
            };
            let auto_gate = prompt_confirm("Enable automated scatter and doublet gating?", true)?;
            let export_mixing_matrix = prompt_path("Export mixing matrix to CSV path", false)?;
            (peak_detection, peak_threshold, peak_bias, peak_bias_negative, use_negative_events,
             autofluorescence_mode, af_weight, min_negative_events, auto_gate, export_mixing_matrix)
        } else {
            (true, 0.3, 0.5, 0.5, false, "universal".to_string(), 0.7, 100, true, None)
        };

    // Invoke unmix
    commands::run_unmix_command(
        &stained,
        unstained.as_ref(),
        controls.as_ref(),
        mixing_matrix.as_ref(),
        use_spill,
        single_stain_controls.as_ref(),
        &detectors,
        &endmembers,
        &autofluorescence,
        cutoff_percentile,
        &strategy,
        output.as_ref(),
        plot,
        &plot_format,
        plot_output_dir.as_ref(),
        compare_ols,
        plot_both,
        peak_detection,
        peak_threshold,
        peak_bias,
        peak_bias_negative,
        use_negative_events,
        &autofluorescence_mode,
        af_weight,
        min_negative_events,
        auto_gate,
        debug_control_plots,
        export_mixing_matrix.as_ref(),
    )
}

#[cfg(test)]
mod tests {
    //! Path expansion tests. Manual check with a shell variable (no path committed):
    //! `export EXP=/path/to/Plate_001` then `tru-ols interactive` and enter `$EXP` at the first prompt.

    use super::*;
    use std::env;

    /// Unique env vars for path expansion tests (one per test to avoid parallel clashes).
    const TEST_ENV_VAR: &str = "TRU_OLS_TEST_PATH_EXPAND";
    const TEST_ENV_VAR_2: &str = "TRU_OLS_TEST_PATH_EXPAND_2";
    const TEST_ENV_VAR_TRAILING: &str = "TRU_OLS_TEST_PATH_EXPAND_TRAILING";
    const TEST_ENV_VAR_EXP_TSC: &str = "TRU_OLS_TEST_EXP_TSC";

    #[test]
    fn test_normalize_path_input_trim_and_quotes() {
        assert_eq!(normalize_path_input("  /foo/bar  "), "/foo/bar");
        assert_eq!(normalize_path_input("'/foo/bar'"), "/foo/bar");
        assert_eq!(normalize_path_input(r#""/foo/bar""#), "/foo/bar");
        assert_eq!(normalize_path_input("  '/foo/bar'  "), "/foo/bar");
    }

    #[test]
    fn test_normalize_path_input_strips_newlines() {
        assert_eq!(normalize_path_input("/foo/bar\n"), "/foo/bar");
        assert_eq!(normalize_path_input("/foo\n/bar"), "/foo/bar");
    }

    #[test]
    fn test_expand_path_input_with_shell_var() {
        let temp_dir = std::env::temp_dir();
        let test_path = temp_dir.join("tru_ols_expand_test");
        let _ = std::fs::create_dir_all(&test_path);
        let path_str = test_path.to_string_lossy();

        // SAFETY: test only; we remove the var before test ends and use a unique name.
        unsafe { env::set_var(TEST_ENV_VAR, path_str.as_ref()) };
        let expanded = expand_path_input(&format!("${{{}}}", TEST_ENV_VAR));
        unsafe { env::remove_var(TEST_ENV_VAR) };

        assert_eq!(expanded, path_str.as_ref(), "expand shell var should match");
        assert!(Path::new(&expanded).exists());
    }

    #[test]
    fn test_expand_path_input_dollar_var_syntax() {
        let temp_dir = std::env::temp_dir();
        let test_path = temp_dir.join("tru_ols_expand_test2");
        let _ = std::fs::create_dir_all(&test_path);
        let path_str = test_path.to_string_lossy();

        unsafe { env::set_var(TEST_ENV_VAR_2, path_str.as_ref()) };
        let expanded = expand_path_input(&format!("${{{}}}", TEST_ENV_VAR_2));
        unsafe { env::remove_var(TEST_ENV_VAR_2) };

        assert_eq!(expanded, path_str.as_ref());
    }

    #[test]
    fn test_expand_path_input_var_with_trailing_path() {
        let temp_dir = std::env::temp_dir();
        let sub = "tru_ols_sub";
        let test_path = temp_dir.join(sub);
        let _ = std::fs::create_dir_all(&test_path);
        let parent_str = temp_dir.to_string_lossy();

        unsafe { env::set_var(TEST_ENV_VAR_TRAILING, parent_str.as_ref()) };
        let expanded = expand_path_input(&format!("${{{}}}/{}", TEST_ENV_VAR_TRAILING, sub));
        unsafe { env::remove_var(TEST_ENV_VAR_TRAILING) };

        let expected = format!("{}/{}", parent_str, sub);
        assert_eq!(expanded, expected, "VAR/sub should expand");
        assert!(Path::new(&expanded).exists());
    }

    /// Mimics user case: EXP='.../Plate_001' and input "$EXP/TSC samples".
    /// Uses a temp base dir with spaces and a "TSC samples" subdir (no real path committed).
    #[test]
    fn test_expand_path_input_var_with_space_and_subpath() {
        let temp_dir = std::env::temp_dir();
        let base_name = "Plate_001";
        let sub_name = "TSC samples";
        let base_path = temp_dir.join(base_name);
        let full_path = base_path.join(sub_name);
        let _ = std::fs::create_dir_all(&full_path);
        let base_str = base_path.to_string_lossy();

        unsafe { env::set_var(TEST_ENV_VAR_EXP_TSC, base_str.as_ref()) };
        let expanded = expand_path_input(&format!("${{{}}}/{}", TEST_ENV_VAR_EXP_TSC, sub_name));
        unsafe { env::remove_var(TEST_ENV_VAR_EXP_TSC) };

        let expected = format!("{}/{}", base_str, sub_name);
        assert_eq!(expanded, expected, "VAR/TSC samples should expand when VAR has spaces");
        assert!(Path::new(&expanded).exists());
    }
}
