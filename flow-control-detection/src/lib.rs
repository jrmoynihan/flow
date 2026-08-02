//! Filename heuristics for classifying flow cytometry control files.

use anyhow::Result;
use regex::Regex;
use std::sync::OnceLock;

/// Suggested role for a loaded FCS file in an unmix / compensation workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRole {
    Unstained,
    SingleStain,
    Sample,
    Unassigned,
}

/// Lightweight file descriptor for classification (no FCS dependency).
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub guid: String,
    pub filename: String,
}

/// Classification result for one file.
#[derive(Debug, Clone)]
pub struct ControlClassification {
    pub guid: String,
    pub suggested_role: ControlRole,
    pub confidence: f32,
    pub display_label: String,
}

/// Endmember ↔ control pairing suggestion.
#[derive(Debug, Clone)]
pub struct EndmemberMatch {
    pub endmember_name: String,
    pub control_guid: String,
    pub detector_name: Option<String>,
    pub confidence: f32,
}

fn unstained_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)unstained|un[\s_-]?stain|blank|af[\s_-]?only").unwrap())
}

fn full_stain_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bfull[\s_-]?stain").unwrap())
}

/// Lowercase alphanumerics only; other chars become spaces (so `Single-Stain` → `single stain`).
pub fn normalize_control_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_space = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_space = false;
        } else if !prev_space {
            out.push(' ');
            prev_space = true;
        }
    }
    out.trim().to_string()
}

/// Strong name signals for single-stain / reference controls (special chars ignored).
pub fn is_named_single_stain_control(filename: &str) -> bool {
    let n = normalize_control_filename(filename);
    if full_stain_re().is_match(filename) {
        return false;
    }
    if n.contains("reference control") || n.contains("single stain") {
        return true;
    }
    // Whole-token "reference" (e.g. "Reference Group_A3 …")
    n.split_whitespace().any(|tok| tok == "reference")
}

/// Fluorophore token patterns used for classification and extraction.
/// Keep in sync with [`extract_marker_and_fluor`].
fn fluor_alt() -> &'static str {
    // Longest-first where needed (Brilliant Violet before BV; Near IR before IR).
    r"(?:Brilliant\s*Violet|Super\s*Bright|eFluor|Near\s*IR|LIVE[\s/_-]?DEAD|BUV|BV|BB|RB|RY|RR|RV|Spark|Viability|PerCP(?:[\s/_-]?Cy\d+)?|PE(?:[\s/_-]?Cy\d+)?|APC(?:[\s/_-]?Cy\d+)?|FITC|AF\d+|LD)"
}

fn fluor_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(r"(?i)\b{}", fluor_alt())).expect("fluor_token_re")
    })
}

fn marker_fluor_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Marker token then separator then known fluor (no empty alternatives).
        Regex::new(&format!(
            r"(?i)\b([A-Za-z][A-Za-z0-9]*(?:[-/][A-Za-z0-9]+)?)\s*[_\- ]\s*({}[\w\.]*)",
            fluor_alt()
        ))
        .expect("marker_fluor_re")
    })
}

/// Clean filename → human-readable endmember label.
pub fn endmember_display_label(filename: &str) -> String {
    let stem = PathStem::from(filename);
    let mut s = stem.0;
    for junk in [
        ".fcs",
        ".FCS",
        "_compensated",
        "-compensated",
        "_unmixed",
        "-unmixed",
    ] {
        s = s.replace(junk, "");
    }
    s = s.replace('_', " ").replace('-', " ");
    let parts: Vec<_> = s.split_whitespace().filter(|p| !p.is_empty()).collect();
    parts.join(" ")
}

/// Infer cells vs beads from filename tokens (SpectroFlo-style control type).
pub fn infer_control_material(filename: &str) -> ControlMaterial {
    let n = normalize_control_filename(filename);
    if n.split_whitespace().any(|t| t == "beads" || t == "bead") {
        ControlMaterial::Beads
    } else if n.split_whitespace().any(|t| t == "cells" || t == "cell") {
        ControlMaterial::Cells
    } else {
        ControlMaterial::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMaterial {
    Unknown,
    Cells,
    Beads,
}

struct PathStem(String);
impl PathStem {
    fn from(filename: &str) -> Self {
        let name = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
        let stem = name.rsplit_once('.').map(|(a, _)| a).unwrap_or(name);
        Self(stem.to_string())
    }
}

/// Parse marker/fluorophore tokens from filename or $PnS-like text.
///
/// Returns `(marker, fluor)` e.g. `("CD14", "FITC")`, `("HLA-DR", "Spark")`.
pub fn extract_marker_and_fluor(text: &str) -> Option<(String, String)> {
    let caps = marker_fluor_re().captures(text)?;
    let marker = caps.get(1)?.as_str().to_string();
    let fluor_raw = caps.get(2)?.as_str();
    // Strip trailing material / plate tokens stuck to fluor ("FITC_Cells").
    let fluor = fluor_raw
        .split(['_', ' '])
        .next()
        .unwrap_or(fluor_raw)
        .trim_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_string();
    if fluor.is_empty() {
        return None;
    }
    // Reject false markers that are plate / group noise.
    let marker_l = marker.to_ascii_lowercase();
    if matches!(
        marker_l.as_str(),
        "group" | "reference" | "donor" | "plate" | "well" | "tube" | "sample"
    ) {
        // Prefer fluor-only: still return if we found a real fluor with a weak marker —
        // try a second pass that finds marker immediately before fluor.
        return find_marker_before_fluor(text, &fluor);
    }
    Some((marker, fluor))
}

fn find_marker_before_fluor(text: &str, fluor: &str) -> Option<(String, String)> {
    let fluor_re = Regex::new(&format!(r"(?i)\b({})\b", regex::escape(fluor))).ok()?;
    let m = fluor_re.find(text)?;
    let before = &text[..m.start()];
    // Last alphanumeric token before fluor (allow HLA-DR style).
    let token_re = Regex::new(r"(?i)([A-Za-z][A-Za-z0-9]*(?:[-/][A-Za-z0-9]+)?)\s*[_\- ]*\s*$").ok()?;
    let caps = token_re.captures(before)?;
    let marker = caps[1].to_string();
    let marker_l = marker.to_ascii_lowercase();
    if matches!(
        marker_l.as_str(),
        "group" | "reference" | "donor" | "plate" | "well" | "tube" | "sample" | "a1"
            | "a2" | "a3" | "b1" | "b2" | "c1" | "c2" | "d1" | "d9" | "e1" | "f1"
    ) || marker_l.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some((marker, fluor.to_string()))
}

/// Classify files as unstained / single-stain / sample / unassigned from filenames.
pub fn classify_controls(files: &[FileInfo]) -> Vec<ControlClassification> {
    files
        .iter()
        .map(|f| {
            let name = &f.filename;
            let (role, confidence) = if unstained_re().is_match(name) {
                (ControlRole::Unstained, 0.95)
            } else if full_stain_re().is_match(name)
                || name.to_ascii_lowercase().contains("sample")
                || name.to_ascii_lowercase().contains("specimen")
            {
                // Fully stained panels / samples are never auto single-stains.
                (ControlRole::Sample, 0.75)
            } else if is_named_single_stain_control(name) {
                let n = normalize_control_filename(name);
                let conf = if n.contains("reference control") || n.contains("single stain") {
                    0.92
                } else {
                    0.88 // token "reference"
                };
                (ControlRole::SingleStain, conf)
            } else if extract_marker_and_fluor(name).is_some() {
                // Real marker+fluor in filename (CD14_FITC, …) without "Reference".
                (ControlRole::SingleStain, 0.72)
            } else if fluor_token_re().is_match(name)
                && !normalize_control_filename(name).contains("donor")
            {
                (ControlRole::SingleStain, 0.55)
            } else if normalize_control_filename(name).contains("bead")
                && is_named_single_stain_control(name)
            {
                (ControlRole::SingleStain, 0.7)
            } else {
                (ControlRole::Unassigned, 0.2)
            };
            ControlClassification {
                guid: f.guid.clone(),
                suggested_role: role,
                confidence,
                display_label: endmember_display_label(name),
            }
        })
        .collect()
}

/// Fuzzy-match endmember / detector names to single-stain control files.
pub fn match_endmembers(
    controls: &[ControlClassification],
    detector_names: &[String],
) -> Result<Vec<EndmemberMatch>> {
    let singles: Vec<_> = controls
        .iter()
        .filter(|c| c.suggested_role == ControlRole::SingleStain)
        .collect();
    let mut out = Vec::new();
    for det in detector_names {
        let det_l = det.to_ascii_lowercase();
        let mut best: Option<(&ControlClassification, f32)> = None;
        for c in &singles {
            let label_l = c.display_label.to_ascii_lowercase();
            let file_l = c.guid.to_ascii_lowercase();
            let score = if label_l.contains(&det_l) || det_l.contains(&label_l) {
                0.85
            } else if det_l
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .filter(|s| s.len() >= 3)
                .any(|tok| label_l.contains(tok))
            {
                0.65
            } else if file_l.contains(&det_l) {
                0.4
            } else {
                continue;
            };
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((c, score));
            }
        }
        if let Some((c, confidence)) = best {
            out.push(EndmemberMatch {
                endmember_name: c.display_label.clone(),
                control_guid: c.guid.clone(),
                detector_name: Some(det.clone()),
                confidence,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_unstained_and_marker_fluor() {
        let files = [
            FileInfo {
                guid: "1".into(),
                filename: "Unstained_Cells.fcs".into(),
            },
            FileInfo {
                guid: "2".into(),
                filename: "CD4_BV421_Cells.fcs".into(),
            },
        ];
        let c = classify_controls(&files);
        assert_eq!(c[0].suggested_role, ControlRole::Unstained);
        assert_eq!(c[1].suggested_role, ControlRole::SingleStain);
    }

    #[test]
    fn reference_and_single_stain_phrases_normalize() {
        assert!(is_named_single_stain_control(
            "Reference Group_A3 CD4 BUV496 (Beads)_Plate.fcs"
        ));
        assert!(is_named_single_stain_control("Reference-Control_Tube.fcs"));
        assert!(is_named_single_stain_control("Single_Stain_CD8.fcs"));
        assert!(is_named_single_stain_control("Single-Stain CD4.fcs"));
        assert!(!is_named_single_stain_control(
            "Donor7_Full_Stain_panel.fcs"
        ));
    }

    #[test]
    fn full_stain_donors_are_samples_not_controls() {
        let files = [
            FileInfo {
                guid: "d".into(),
                filename: "Donor 9_F1 Full Stain_Plate.fcs".into(),
            },
            FileInfo {
                guid: "r".into(),
                filename: "Reference Group_A2 HLA-DR DQ Spark UV 387 (Beads).fcs".into(),
            },
        ];
        let c = classify_controls(&files);
        assert_eq!(c[0].suggested_role, ControlRole::Sample);
        assert_eq!(c[1].suggested_role, ControlRole::SingleStain);
        assert!(c[1].confidence >= 0.85);
    }

    #[test]
    fn extract_known_fluors_not_group_or_donor() {
        let (m, f) = extract_marker_and_fluor("CD14_FITC_Cells.fcs").expect("FITC");
        assert_eq!(m, "CD14");
        assert_eq!(f, "FITC");

        let (m, f) = extract_marker_and_fluor("CD8_RB545_Beads.fcs").expect("RB");
        assert_eq!(m, "CD8");
        assert!(f.starts_with("RB"));

        let (m, f) =
            extract_marker_and_fluor("Reference Group_A2 HLA-DR DQ Spark UV 387 (Beads).fcs")
                .expect("Spark");
        assert!(m.contains("HLA") || m == "DQ" || m.contains("DR"));
        assert!(f.to_ascii_lowercase().starts_with("spark"));

        assert!(
            extract_marker_and_fluor("Donor 9_F1 Full Stain.fcs").is_none()
                || extract_marker_and_fluor("Donor 9_F1 Full Stain.fcs")
                    .map(|(m, _)| m != "9" && m.to_ascii_lowercase() != "donor")
                    .unwrap_or(true)
        );
    }

    #[test]
    fn classify_includes_all_reference_controls() {
        let files = [
            FileInfo {
                guid: "a".into(),
                filename: "Reference Group_A1.fcs".into(),
            },
            FileInfo {
                guid: "b".into(),
                filename: "Reference Control_unstained.fcs".into(),
            },
        ];
        let c = classify_controls(&files);
        assert_eq!(c[0].suggested_role, ControlRole::SingleStain);
        assert!(c[0].confidence >= 0.85);
        assert_eq!(c[1].suggested_role, ControlRole::Unstained);
    }
}
