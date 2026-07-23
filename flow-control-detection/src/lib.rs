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

fn single_stain_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(single[\s_-]?stain|ssc?[\s_-]?|comp[\s_-]?control|reference|beads?)").unwrap()
    })
}

/// Clean filename → human-readable endmember label.
pub fn endmember_display_label(filename: &str) -> String {
    let stem = PathStem::from(filename);
    let mut s = stem.0;
    for junk in [
        ".fcs", ".FCS", "_compensated", "-compensated", "_unmixed", "-unmixed",
    ] {
        s = s.replace(junk, "");
    }
    s = s.replace('_', " ").replace('-', " ");
    let parts: Vec<_> = s.split_whitespace().filter(|p| !p.is_empty()).collect();
    parts.join(" ")
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
pub fn extract_marker_and_fluor(text: &str) -> Option<(String, String)> {
    let re = Regex::new(r"(?i)([A-Za-z0-9]+)\s*[_\- ]\s*((?:BV|BB|PE|APC|FITC|PerCP|AF|eFluor|Super\s*Bright)[\w\.]*)").ok()?;
    let caps = re.captures(text)?;
    Some((caps[1].to_string(), caps[2].to_string()))
}

/// Classify files as unstained / single-stain / sample / unassigned from filenames.
pub fn classify_controls(files: &[FileInfo]) -> Vec<ControlClassification> {
    files
        .iter()
        .map(|f| {
            let name = &f.filename;
            let (role, confidence) = if unstained_re().is_match(name) {
                (ControlRole::Unstained, 0.9)
            } else if single_stain_re().is_match(name) {
                (ControlRole::SingleStain, 0.75)
            } else if name.to_ascii_lowercase().contains("sample")
                || name.to_ascii_lowercase().contains("specimen")
            {
                (ControlRole::Sample, 0.6)
            } else {
                // Heuristic: fluorophore-like tokens → single stain
                if extract_marker_and_fluor(name).is_some()
                    || Regex::new(r"(?i)\b(BV|BB|PE|APC|FITC|PerCP)\d*")
                        .ok()
                        .is_some_and(|re| re.is_match(name))
                {
                    (ControlRole::SingleStain, 0.55)
                } else {
                    (ControlRole::Unassigned, 0.2)
                }
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
            let file_l = c.guid.to_ascii_lowercase(); // weak; prefer display
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
    fn classifies_unstained_and_bv_single_stain() {
        let files = vec![
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
        assert!(endmember_display_label("CD4_BV421_Cells.fcs").contains("BV421"));
    }
}
