//! Interactive step-by-step prompts for TRU-OLS unmix options.
//!
//! Control ↔ endmember pairing is inferred from matching file stems, then shown for confirmation
//! or row-wise edits. Plots, OLS comparison, and debug control exports are under optional advanced prompts.
//!
//! Path inputs support environment variable expansion (e.g. `$EXP/file.fcs` or `${EXP}/TSC samples`)
//! so that interactive prompts behave like the shell for path arguments. Export variables in the
//! same shell before running (e.g. `export EXP='/path/to/Plate_001'`) so they expand correctly.

use anyhow::{Context, Result};
use flow_fcs::file::AccessWrapper;
use flow_fcs::keyword::StringableKeyword;
use flow_fcs::{Fcs, Header, Metadata};
use inquire::ui::{Attributes, Color, RenderConfig, StyleSheet, Styled};
use inquire::{Confirm, CustomType, Select, Text};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::commands;
use crate::state::{self, SavedState};

/// Terminal styling: questions (prompts) vs answers (options, submitted values, help).
/// Respects `NO_COLOR` by keeping only weight contrast, no ANSI colors.
fn prompt_highlight_render_config() -> RenderConfig<'static> {
    let mut cfg = RenderConfig::default();
    if std::env::var_os("NO_COLOR").is_some() {
        cfg.prompt = StyleSheet::empty().with_attr(Attributes::BOLD);
        cfg.selected_option = Some(StyleSheet::empty().with_attr(Attributes::BOLD));
        return cfg;
    }

    cfg.prompt = StyleSheet::empty()
        .with_fg(Color::LightCyan)
        .with_attr(Attributes::BOLD);
    cfg.prompt_prefix = Styled::new("?").with_fg(Color::LightCyan);
    cfg.answered_prompt_prefix = Styled::new(">").with_fg(Color::LightGreen);
    cfg.help_message = StyleSheet::empty().with_fg(Color::DarkGrey);
    cfg.option = StyleSheet::empty().with_fg(Color::Grey);
    cfg.selected_option = Some(
        StyleSheet::empty()
            .with_fg(Color::LightYellow)
            .with_attr(Attributes::BOLD),
    );
    cfg.answer = StyleSheet::empty().with_fg(Color::LightGreen);
    cfg.text_input = StyleSheet::empty().with_fg(Color::AnsiValue(252));
    cfg.highlighted_option_prefix = Styled::new(">").with_fg(Color::LightYellow);
    cfg.unhighlighted_option_prefix = Styled::new(" ").with_fg(Color::DarkGrey);
    cfg
}

/// Single-line label for a control FCS: `$FIL` when present, otherwise the file name only.
fn control_fcs_choice_label(path: &Path) -> String {
    let Some(path_str) = path.to_str() else {
        return path.display().to_string();
    };
    match read_fil_keyword_trimmed(path_str) {
        Ok(Some(fil)) if !fil.is_empty() => fil,
        _ => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
    }
}

fn read_fil_keyword_trimmed(path: &str) -> Result<Option<String>> {
    let access = AccessWrapper::new(path)?;
    let header = Header::from_mmap(&access)?;
    let metadata = Metadata::from_mmap(&access, &header);
    Ok(metadata
        .get_string_keyword("$FIL")
        .ok()
        .map(|k| k.get_str().trim().to_string()))
}

/// First entry when stepping backward through pairing prompts (must not collide with `$FIL`).
const PAIRING_GO_BACK: &str = "Go back — re-select previous endmember";

fn unique_control_choice_labels(paths: &[PathBuf]) -> Vec<String> {
    let raw: Vec<String> = paths
        .iter()
        .map(|p| control_fcs_choice_label(p.as_path()))
        .collect();
    let mut out = Vec::with_capacity(raw.len());
    for (i, lab) in raw.iter().enumerate() {
        let n_same = raw.iter().filter(|x| x.as_str() == lab.as_str()).count();
        if n_same > 1 {
            let fname = paths[i]
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| paths[i].display().to_string());
            out.push(format!("{lab} (file: {fname})"));
        } else {
            out.push(lab.clone());
        }
    }
    out
}

/// Pad a cell to a fixed display width (`char` count) for monospace column alignment.
fn pad_table_cell(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - n))
    }
}

/// Print a 3-column pipe table with aligned delimiters; separator dash lengths match cell widths.
fn print_three_column_pipe_table(headers: [&str; 3], rows: &[[String; 3]]) {
    let mut widths = [
        headers[0].chars().count(),
        headers[1].chars().count(),
        headers[2].chars().count(),
    ];
    for r in rows {
        widths[0] = widths[0].max(r[0].chars().count());
        widths[1] = widths[1].max(r[1].chars().count());
        widths[2] = widths[2].max(r[2].chars().count());
    }
    println!(
        "| {} | {} | {} |",
        pad_table_cell(headers[0], widths[0]),
        pad_table_cell(headers[1], widths[1]),
        pad_table_cell(headers[2], widths[2]),
    );
    println!(
        "|{}|{}|{}|",
        "-".repeat(2 + widths[0]),
        "-".repeat(2 + widths[1]),
        "-".repeat(2 + widths[2]),
    );
    for r in rows {
        println!(
            "| {} | {} | {} |",
            pad_table_cell(&r[0], widths[0]),
            pad_table_cell(&r[1], widths[1]),
            pad_table_cell(&r[2], widths[2]),
        );
    }
}

fn print_control_pairing_table(assigns: &[(String, PathBuf)], current_display: &str) {
    let mut rows: Vec<[String; 3]> = Vec::with_capacity(assigns.len() + 1);
    for (idx, (em, pb)) in assigns.iter().enumerate() {
        let n = (idx + 1).to_string();
        let em_l = commands::endmember_display_label(em);
        let file = pb
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| pb.display().to_string());
        rows.push([n, em_l, file]);
    }
    let next = assigns.len() + 1;
    rows.push([
        next.to_string(),
        current_display.to_string(),
        "(pending)".to_string(),
    ]);
    print_three_column_pipe_table(["#", "endmember", "selected file"], &rows);
    println!();
}

fn print_complete_pairing_table(assigns: &[(String, PathBuf)]) {
    let mut rows: Vec<[String; 3]> = Vec::with_capacity(assigns.len());
    for (idx, (em, pb)) in assigns.iter().enumerate() {
        let n = (idx + 1).to_string();
        let em_l = commands::endmember_display_label(em);
        let file = pb
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| pb.display().to_string());
        rows.push([n, em_l, file]);
    }
    println!();
    print_three_column_pipe_table(["#", "endmember", "control file"], &rows);
    println!();
}

/// Pair each fluor endmember to the control file whose name stem matches (case-insensitive).
fn auto_pair_fluors_to_control_paths(
    fluors: &[String],
    candidates: &[PathBuf],
) -> Result<Vec<(String, PathBuf)>> {
    let mut stem_lower_to_path: HashMap<String, PathBuf> = HashMap::new();
    for p in candidates {
        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .with_context(|| format!("control path not valid UTF-8: {}", p.display()))?;
        let key = stem.to_lowercase();
        if stem_lower_to_path.insert(key, p.clone()).is_some() {
            anyhow::bail!(
                "More than one control file maps to the same stem (ignoring case): {}",
                stem
            );
        }
    }
    let mut out = Vec::with_capacity(fluors.len());
    for f in fluors {
        let key = f.to_lowercase();
        let path = stem_lower_to_path.get(&key).with_context(|| {
            format!(
                "No single-stain control file for endmember {:?} (need a file whose stem matches, e.g. {:?}.fcs)",
                f, f
            )
        })?;
        out.push((f.clone(), path.clone()));
    }
    Ok(out)
}

const REVIEW_PAIRINGS_ACCEPT: &str = "Accept pairings and continue";
const REVIEW_PAIRINGS_EDIT: &str = "Change one pairing...";

fn available_paths_for_row_reassign(
    full_candidates: &[PathBuf],
    assigns: &[(String, PathBuf)],
    row: usize,
) -> Vec<PathBuf> {
    let current = &assigns[row].1;
    let used_other: HashSet<PathBuf> = assigns
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != row)
        .map(|(_, (_, p))| p.clone())
        .collect();
    full_candidates
        .iter()
        .filter(|p| {
            let used_elsewhere = used_other
                .iter()
                .any(|x| x.as_path() == p.as_path());
            !used_elsewhere || p.as_path() == current.as_path()
        })
        .cloned()
        .collect()
}

/// Show auto pairings (by matching file stem to endmember id); loop until accepted or user edits.
fn prompt_pairings_confirm_or_edit(
    mut assigns: Vec<(String, PathBuf)>,
    full_candidates: &[PathBuf],
) -> Result<Vec<(String, PathBuf)>> {
    loop {
        println!(
            "Each endmember is paired with the single-stain control file that has the same name stem (text before .fcs), compared case-insensitively."
        );
        print_complete_pairing_table(&assigns);
        let menu = vec![REVIEW_PAIRINGS_ACCEPT, REVIEW_PAIRINGS_EDIT];
        let choice = Select::new("Review control ↔ endmember pairings", menu)
            .with_starting_cursor(0)
            .with_help_message("Pick an endmember row to assign a different control file; files already used elsewhere are hidden.")
            .prompt()
            .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
        if choice == REVIEW_PAIRINGS_ACCEPT {
            return Ok(assigns);
        }

        let row_labels: Vec<String> = assigns
            .iter()
            .enumerate()
            .map(|(i, (em, pb))| {
                let display = commands::endmember_display_label(em);
                let file = pb
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| pb.display().to_string());
                format!("{}  {display} → {file}", i + 1)
            })
            .collect();
        let row_pick = Select::new("Which endmember to re-assign?", row_labels.clone())
            .with_help_message("Choose the row to edit; then pick a replacement file.")
            .prompt()
            .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
        let row_idx = row_labels
            .iter()
            .position(|s| s == &row_pick)
            .context("internal: row label not found")?;

        let avail = available_paths_for_row_reassign(full_candidates, &assigns, row_idx);
        if avail.is_empty() {
            anyhow::bail!("No replacement control files available for this row");
        }
        let labels = unique_control_choice_labels(&avail);
        let display = commands::endmember_display_label(&assigns[row_idx].0);
        let file_prompt = format!("{display} — replacement control file:");
        let file_pick = Select::new(&file_prompt, labels.clone())
            .with_starting_cursor(0)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
        let file_idx = labels
            .iter()
            .position(|l| l == &file_pick)
            .with_context(|| format!("internal: file label not found for {:?}", file_pick))?;
        assigns[row_idx].1 = avail[file_idx].clone();
    }
}

/// Pair each fluor endmember to a distinct control file; removes chosen files from `available`.
fn prompt_fluor_control_assignments(
    fluors: &[String],
    available: &mut Vec<PathBuf>,
) -> Result<Vec<(String, PathBuf)>> {
    let mut assigns: Vec<(String, PathBuf)> = Vec::new();
    let mut i = 0usize;
    while i < fluors.len() {
        let fluor = &fluors[i];
        let display = commands::endmember_display_label(fluor);
        print_control_pairing_table(&assigns, &display);
        let help = if i > 0 {
            format!(
                "Choose which file defines {display}'s spectrum. The first list entry goes back to re-select the previous endmember."
            )
        } else {
            format!("Choose which file defines {display}'s spectrum")
        };

        let labels = unique_control_choice_labels(available);
        let mut options: Vec<String> = Vec::new();
        if i > 0 {
            options.push(PAIRING_GO_BACK.to_string());
        }
        options.extend(labels.iter().cloned());

        let start_cursor = if i > 0 && options.len() > 1 { 1 } else { 0 };
        let select_prompt = format!("{display} control single-stain FCS file:");
        let picked = Select::new(&select_prompt, options)
            .with_starting_cursor(start_cursor)
            .with_help_message(&help)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;

        if picked == PAIRING_GO_BACK {
            let Some((_prev_em, prev_path)) = assigns.pop() else {
                anyhow::bail!("internal: go back with empty assigns");
            };
            if i == 0 {
                anyhow::bail!("internal: go back at first endmember");
            }
            available.push(prev_path);
            available.sort();
            i -= 1;
            continue;
        }

        let file_idx = labels
            .iter()
            .position(|l| l == &picked)
            .with_context(|| format!("internal: label not found for selection {:?}", picked))?;
        let path = available[file_idx].clone();
        available.remove(file_idx);
        assigns.push((fluor.clone(), path));
        i += 1;
    }
    Ok(assigns)
}

/// Normalize path input: trim, strip surrounding quotes, remove embedded newlines.
/// Handles pasted paths that may include quotes or accidental line breaks.
fn normalize_path_input(s: &str) -> String {
    let t = s.trim();
    let without_quotes = match (
        t.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')),
        t.strip_prefix('"').and_then(|s| s.strip_suffix('"')),
    ) {
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
    prompt_path(prompt, true).and_then(|o| o.context("Required path was not provided"))
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

fn first_fcs_path_in_dir(dir: &Path) -> Result<PathBuf> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read stained directory: {}", dir.display()))?
    {
        let p = entry?.path();
        if p.extension().and_then(|s| s.to_str()) == Some("fcs") {
            return Ok(p);
        }
    }
    anyhow::bail!("No .fcs files found in stained directory {}", dir.display())
}

/// If an FCS with `unstained` in the filename exists in `folder`, skip prompting; otherwise prompt.
/// Returns `None` when auto-detection succeeds (`run_unmix_command` resolves unstained from the directory).
fn prompt_unstained_only_if_missing_in_folder(
    folder: &PathBuf,
    folder_label: &str,
) -> Result<Option<PathBuf>> {
    match commands::find_unstained_control(folder) {
        Ok(path) => {
            println!(
                "Unstained control found in {}: {}",
                folder_label,
                path.display()
            );
            Ok(None)
        }
        Err(e) => {
            println!(
                "{:#}\nProvide the path to your unstained control FCS file.",
                e
            );
            Ok(Some(prompt_path_required(
                "Path to unstained control FCS file",
            )?))
        }
    }
}

/// Run the interactive flow and then call run_unmix_command with collected args.
/// Entry point for `tru-ols interactive`. On first run (no `.tru-ols-state.json` in the cwd)
/// this walks through every prompt. On subsequent runs it offers a three-way launch menu:
/// reuse all prior choices, edit a single setting, or start fresh.
///
/// State is written to the cwd as JSON on successful completion, so the file doubles as an audit
/// log of parameters used for the most recent run.
pub fn run_interactive() -> Result<()> {
    inquire::set_global_render_config(prompt_highlight_render_config());

    // If a prior state file exists in the cwd, let the user reuse or edit it instead of stepping
    // through every prompt again.
    if let Some(prior) = state::load()? {
        match prompt_launch_mode(&prior)? {
            LaunchMode::ReuseAll => {
                println!("Re-running with prior configuration from {}.", state::STATE_FILE_NAME);
                return run_from_state(prior);
            }
            LaunchMode::EditOne => {
                let mut state = prior;
                edit_single_field(&mut state)?;
                return run_from_state(state);
            }
            LaunchMode::Fresh => {
                // fall through to the fresh flow
            }
        }
    }

    println!("Interactive mode: you will be prompted for each option.\n");

    let state = collect_fresh_state()?;
    run_from_state(state)
}

/// What the user picked from the launch menu on interactive re-invocation.
enum LaunchMode {
    ReuseAll,
    EditOne,
    Fresh,
}

fn prompt_launch_mode(prior: &SavedState) -> Result<LaunchMode> {
    println!("Found prior run configuration: {}", prior_summary_line(prior));
    let options = vec![
        "Re-run with all previous choices",
        "Edit a single setting, keep the rest",
        "Start fresh (ignore prior configuration)",
    ];
    let ans = Select::new("What would you like to do?", options.clone())
        .with_starting_cursor(0)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
    Ok(match options.iter().position(|&o| o == ans).unwrap_or(0) {
        0 => LaunchMode::ReuseAll,
        1 => LaunchMode::EditOne,
        _ => LaunchMode::Fresh,
    })
}

/// One-line summary for the launch menu so the user can recognise the prior run at a glance.
fn prior_summary_line(s: &SavedState) -> String {
    let stained = state::short_path(&s.stained);
    let src = MIXING_SOURCE_OPTIONS.get(s.mixing_source).copied().unwrap_or("?");
    format!(
        "stained={} | source={} | strategy={} | cutoff={:.3}",
        stained, src, s.strategy, s.cutoff_percentile
    )
}

/// Present the list of editable fields, prompt for just that field, and mutate `state` in place.
fn edit_single_field(state: &mut SavedState) -> Result<()> {
    let fields = EDITABLE_FIELDS;
    let labels: Vec<String> = fields
        .iter()
        .map(|f| format!("{} (current: {})", f.label, (f.show)(state)))
        .collect();
    let ans = Select::new("Which setting do you want to change?", labels.clone())
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
    let idx = labels.iter().position(|l| l == &ans).unwrap_or(0);
    (fields[idx].edit)(state)?;
    Ok(())
}

/// An editable field in the saved state: its human label, a renderer that shows the current value,
/// and a prompter that re-runs the single question and writes the new value into `state`.
struct EditableField {
    label: &'static str,
    show: fn(&SavedState) -> String,
    edit: fn(&mut SavedState) -> Result<()>,
}

const EDITABLE_FIELDS: &[EditableField] = &[
    EditableField {
        label: "Stained sample path",
        show: |s| s.stained.display().to_string(),
        edit: |s| {
            s.stained = prompt_stained_path()?;
            Ok(())
        },
    },
    EditableField {
        label: "Output FCS path",
        show: |s| match &s.output {
            Some(p) => p.display().to_string(),
            None => "(none)".to_string(),
        },
        edit: |s| {
            s.output = prompt_path("Output FCS file path", false)?;
            Ok(())
        },
    },
    EditableField {
        label: "Cutoff percentile",
        show: |s| format!("{:.3}", s.cutoff_percentile),
        edit: |s| {
            s.cutoff_percentile = prompt_f64_with_default("Cutoff percentile", s.cutoff_percentile)?;
            Ok(())
        },
    },
    EditableField {
        label: "Unmixing strategy",
        show: |s| s.strategy.clone(),
        edit: |s| {
            let options = vec!["ucm", "zero"];
            let start = options.iter().position(|&o| o == s.strategy).unwrap_or(0);
            s.strategy = Select::new("Unmixing strategy", options)
                .with_starting_cursor(start)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?
                .to_string();
            Ok(())
        },
    },
    EditableField {
        label: "Autofluorescence endmember name",
        show: |s| s.autofluorescence.clone(),
        edit: |s| {
            let name = Text::new("Autofluorescence endmember name")
                .with_default(&s.autofluorescence)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?;
            let name = name.trim().to_string();
            if name.is_empty() {
                anyhow::bail!("Autofluorescence name cannot be empty");
            }
            s.autofluorescence = name;
            Ok(())
        },
    },
    EditableField {
        label: "Auto-gate controls",
        show: |s| s.auto_gate.to_string(),
        edit: |s| {
            s.auto_gate =
                prompt_confirm("Enable automated scatter and doublet gating for controls?", s.auto_gate)?;
            Ok(())
        },
    },
    EditableField {
        label: "Mixing matrix export path",
        show: |s| match &s.export_mixing_matrix {
            Some(p) => p.display().to_string(),
            None => "(default: next to output)".to_string(),
        },
        edit: |s| {
            s.export_mixing_matrix = prompt_path("Export mixing matrix to CSV path", false)?;
            Ok(())
        },
    },
    EditableField {
        label: "Plot output directory",
        show: |s| match &s.plot_output_dir {
            Some(p) => p.display().to_string(),
            None => "(none)".to_string(),
        },
        edit: |s| {
            s.plot_output_dir = prompt_path("Directory for plot outputs", false)?;
            Ok(())
        },
    },
    EditableField {
        label: "Debug control plots",
        show: |s| s.debug_control_plots.to_string(),
        edit: |s| {
            s.debug_control_plots = prompt_confirm(
                "Generate debug control plots (FSC-A vs SSC-A at cleanup stages + per-endmember spectral)?",
                s.debug_control_plots,
            )?;
            if s.debug_control_plots && s.plot_output_dir.is_none() {
                s.plot_output_dir = Some(prompt_path_required(
                    "Plot output directory (required for debug control plots)",
                )?);
            }
            Ok(())
        },
    },
    EditableField {
        label: "Peak detection",
        show: |s| s.peak_detection.to_string(),
        edit: |s| {
            s.peak_detection = prompt_confirm(
                "Enable peak-based median selection for single-stain controls?",
                s.peak_detection,
            )?;
            Ok(())
        },
    },
    EditableField {
        label: "Peak threshold",
        show: |s| format!("{:.3}", s.peak_threshold),
        edit: |s| {
            s.peak_threshold = prompt_f64_with_default(
                "Peak detection threshold (fraction of max density)",
                s.peak_threshold,
            )?;
            Ok(())
        },
    },
    EditableField {
        label: "Autofluorescence mode",
        show: |s| s.autofluorescence_mode.clone(),
        edit: |s| {
            let options = vec!["universal", "negative-events", "hybrid"];
            let start = options
                .iter()
                .position(|o| *o == s.autofluorescence_mode)
                .unwrap_or(0);
            s.autofluorescence_mode = Select::new("Autofluorescence mode", options)
                .with_starting_cursor(start)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?
                .to_string();
            Ok(())
        },
    },
];

/// Apply the saved state (or a state produced by editing) by calling `run_unmix_command` with it,
/// then persist the state to the cwd so subsequent runs see the latest configuration.
fn run_from_state(mut state: SavedState) -> Result<()> {
    // Derive a default mixing-matrix export path if the user hasn't set one. Saving the matrix by
    // default makes it cheap to re-run with different parameters and preserves a machine-readable
    // record of what was used.
    if state.export_mixing_matrix.is_none() {
        state.export_mixing_matrix = Some(default_mixing_matrix_path(&state));
    }

    let result = commands::run_unmix_command(
        &state.stained,
        state.unstained.as_ref(),
        state.controls.as_ref(),
        state.mixing_matrix.as_ref(),
        state.use_spill,
        state.single_stain_controls.as_ref(),
        &state.detectors,
        &state.endmembers,
        &state.autofluorescence,
        state.cutoff_percentile,
        &state.strategy,
        state.output.as_ref(),
        state.plot,
        &state.plot_format,
        state.plot_output_dir.as_ref(),
        state.compare_ols,
        state.plot_both,
        state.peak_detection,
        state.peak_threshold,
        state.peak_bias,
        state.peak_bias_negative,
        state.use_negative_events,
        &state.autofluorescence_mode,
        state.af_weight,
        state.min_negative_events,
        state.auto_gate,
        state.debug_control_plots,
        state.export_mixing_matrix.as_ref(),
        state.control_assignments.as_deref(),
        &crate::qc_pipeline::QcCliOptions::default(),
    );

    // Persist even on error so the user can re-run and edit whichever setting tripped things up.
    // This is safe because we write atomically and never widen the file's permissions.
    match state::save(&state) {
        Ok(path) => println!(
            "Saved configuration to {} (rerun `tru-ols` to reuse or edit).",
            path.display()
        ),
        Err(e) => eprintln!("Warning: failed to save run configuration: {:#}", e),
    }

    result
}

/// Decide where to write the mixing-matrix CSV when the user hasn't set an explicit path.
/// Priority: alongside the output FCS → plot output directory → current working directory.
fn default_mixing_matrix_path(state: &SavedState) -> PathBuf {
    if let Some(out) = &state.output {
        if let Some(parent) = out.parent() {
            if !parent.as_os_str().is_empty() {
                return parent.join("mixing_matrix.csv");
            }
        }
    }
    if let Some(plot_dir) = &state.plot_output_dir {
        return plot_dir.join("mixing_matrix.csv");
    }
    std::env::current_dir()
        .map(|d| d.join("mixing_matrix.csv"))
        .unwrap_or_else(|_| PathBuf::from("mixing_matrix.csv"))
}

/// The original fresh-start prompt sequence, lifted out so both the first-run path and the
/// "start fresh" launch mode can share it.
fn collect_fresh_state() -> Result<SavedState> {
    // Step 1: Stained path
    let stained = prompt_stained_path()?;

    // Step 2: Mixing source (default: Controls directory = index 0)
    let mixing_source = prompt_mixing_source()?;

    let (
        controls,
        single_stain_controls,
        mixing_matrix,
        use_spill,
        unstained,
        mut detectors,
        mut endmembers,
    ) = match mixing_source {
        0 => {
            // Controls directory — unstained is auto-detected from filename; prompt only if missing.
            let controls_dir =
                prompt_path_required("Path to controls directory (single-stain + unstained)")?;
            let unstained_override =
                prompt_unstained_only_if_missing_in_folder(&controls_dir, "controls directory")?;
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
            // Single-stain controls directory — try the same unstained filename heuristic before prompting.
            let single_stain_dir =
                prompt_path_required("Path to single-stain controls directory")?;
            let unstained_override = prompt_unstained_only_if_missing_in_folder(
                &single_stain_dir,
                "single-stain controls directory",
            )?;
            let detectors = prompt_comma_list("Detector names", false)?;
            let endmembers = prompt_comma_list("Endmember names", false)?;
            (
                None,
                Some(single_stain_dir),
                None,
                false,
                unstained_override,
                detectors,
                endmembers,
            )
        }
        2 => {
            // CSV mixing matrix
            let matrix_path = prompt_path_required("Path to mixing matrix CSV file")?;
            let (det_csv, em_csv) =
                commands::mixing_matrix_csv_detector_endmember_lists(matrix_path.as_path())?;
            let from_csv_both = !det_csv.is_empty() && !em_csv.is_empty();
            let detectors = if det_csv.is_empty() {
                prompt_comma_list(
                    "Detector names (comma-separated; required for legacy numeric-only matrix CSV)",
                    true,
                )?
            } else {
                det_csv
            };
            let endmembers = if em_csv.is_empty() {
                prompt_comma_list(
                    "Endmember names (comma-separated; required for legacy numeric-only matrix CSV)",
                    true,
                )?
            } else {
                em_csv
            };
            if from_csv_both {
                println!("Detector and endmember names were read from the mixing matrix CSV.");
            }
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

    // Map each fluor endmember to a control file (controls dir or single-stain dir only)
    let control_assignments: Option<Vec<(String, PathBuf)>> =
        if mixing_source == 0 || mixing_source == 1 {
            let ss_dir: PathBuf = match mixing_source {
                0 => controls
                    .as_ref()
                    .expect("controls set for mixing_source 0")
                    .clone(),
                1 => single_stain_controls
                    .as_ref()
                    .expect("single_stain_controls set for mixing_source 1")
                    .clone(),
                _ => unreachable!(),
            };
            let sample_path = if stained.is_dir() {
                first_fcs_path_in_dir(&stained)?
            } else {
                stained.clone()
            };
            let stained_fcs = Fcs::open(
                sample_path
                    .to_str()
                    .context("Invalid stained sample path for auto-detect")?,
            )?;
            let (auto_detectors, auto_endmembers) =
                commands::auto_detect_from_single_stains(&ss_dir, &stained_fcs)?;
            if detectors.is_empty() {
                detectors = auto_detectors;
            }
            if endmembers.is_empty() {
                endmembers = auto_endmembers;
            }
            if !endmembers.iter().any(|e| e == &autofluorescence) {
                endmembers.push(autofluorescence.clone());
            }
            let mut candidates = commands::list_non_unstained_control_fcs(ss_dir.as_path())?;
            candidates.sort();
            let mut fluors: Vec<String> = endmembers
                .iter()
                .filter(|e| *e != &autofluorescence)
                .cloned()
                .collect();
            fluors.sort();
            println!("\nMatching controls to endmember (fluorophore) names:\n");
            let assigns = match auto_pair_fluors_to_control_paths(&fluors, &candidates) {
                Ok(pairs) => prompt_pairings_confirm_or_edit(pairs, &candidates)?,
                Err(e) => {
                    println!("{:#}", e);
                    println!("\nCould not infer pairings from file names alone. Use step-by-step selection for each endmember.\n");
                    let mut avail = candidates.clone();
                    prompt_fluor_control_assignments(&fluors, &mut avail)?
                }
            };
            Some(assigns)
        } else {
            None
        };

    // Step 4: Output path, control gating, then optional plot/OLS/debug advanced options
    let output = prompt_path("Output FCS file path", false)?;

    let auto_gate = if mixing_source == 0 || mixing_source == 1 {
        prompt_confirm(
            "Enable automated scatter and doublet gating for control files?",
            true,
        )?
    } else {
        true
    };

    let mut plot = false;
    let mut plot_format = "png".to_string();
    let mut plot_output_dir: Option<PathBuf> = None;
    let mut compare_ols = false;
    let mut plot_both = false;
    let mut debug_control_plots = false;

    if prompt_confirm(
        "Open advanced options (comparison plots, OLS comparison, debug control plots, extra paths)?",
        false,
    )? {
        plot = prompt_confirm("Generate comparison plots?", false)?;
        if plot {
            let options = vec!["png", "svg", "pdf"];
            plot_format = Select::new("Plot format", options)
                .with_starting_cursor(0)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Prompt cancelled or error: {}", e))?
                .to_string();
        }
        plot_output_dir = prompt_path("Directory for plot outputs", false)?;
        compare_ols = prompt_confirm("Also run standard OLS and compare?", false)?;
        plot_both = if compare_ols {
            prompt_confirm("Generate plots for both OLS and TRU-OLS?", false)?
        } else {
            false
        };
        if (mixing_source == 0 || mixing_source == 1)
            && prompt_confirm(
                "Generate debug control plots (FSC-A vs SSC-A at cleanup stages + per-endmember spectral)?",
                false,
            )?
        {
            if plot_output_dir.is_none() {
                let dir = prompt_path_required(
                    "Plot output directory (required for debug control plots)",
                )?;
                plot_output_dir = Some(dir);
            }
            debug_control_plots = true;
        }
    }

    // Step 5: Advanced options (peak / AF / export)
    let (
        peak_detection,
        peak_threshold,
        peak_bias,
        peak_bias_negative,
        use_negative_events,
        autofluorescence_mode,
        af_weight,
        min_negative_events,
        export_mixing_matrix,
    ) = if prompt_confirm(
        "Configure advanced options (peak detection, negative events, mixing matrix export)?",
        false,
    )? {
        let peak_detection = prompt_confirm(
            "Enable peak-based median selection for single-stain controls?",
            true,
        )?;
        let peak_threshold =
            prompt_f64_with_default("Peak detection threshold (fraction of max density)", 0.3)?;
        let peak_bias = prompt_f64_with_default(
            "Peak bias fraction for positive peaks (0.5 = upper 50%)",
            0.5,
        )?;
        let peak_bias_negative =
            prompt_f64_with_default("Peak bias fraction for negative peaks", 0.5)?;
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
        let export_mixing_matrix = prompt_path("Export mixing matrix to CSV path", false)?;
        (
            peak_detection,
            peak_threshold,
            peak_bias,
            peak_bias_negative,
            use_negative_events,
            autofluorescence_mode,
            af_weight,
            min_negative_events,
            export_mixing_matrix,
        )
    } else {
        (
            true,
            0.3,
            0.5,
            0.5,
            false,
            "universal".to_string(),
            0.7,
            100,
            None,
        )
    };

    Ok(SavedState {
        stained,
        mixing_source,
        controls,
        single_stain_controls,
        mixing_matrix,
        use_spill,
        unstained,
        detectors,
        endmembers,
        cutoff_percentile,
        strategy,
        autofluorescence,
        control_assignments,
        output,
        auto_gate,
        plot,
        plot_format,
        plot_output_dir,
        compare_ols,
        plot_both,
        debug_control_plots,
        peak_detection,
        peak_threshold,
        peak_bias,
        peak_bias_negative,
        use_negative_events,
        autofluorescence_mode,
        af_weight,
        min_negative_events,
        export_mixing_matrix,
    })
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
        assert_eq!(
            expanded, expected,
            "VAR/TSC samples should expand when VAR has spaces"
        );
        assert!(Path::new(&expanded).exists());
    }
}
