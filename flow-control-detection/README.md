# flow-control-detection

Filename heuristics for classifying flow cytometry control files (unstained, single-stain, sample).

[MIT](LICENSE)

## Overview

`flow-control-detection` provides lightweight, FCS-independent suggestions for control roles and endmember↔filename matching before unmixing or compensation.

## Features

- Regex-based filename normalization and role inference (`ControlRole`)
- Batch `classify_controls` and `match_endmembers` for pairing display labels to control GUIDs
- No FCS parsing dependency

## Installation

```bash
cargo add flow-control-detection
```

Or add it directly to your `Cargo.toml`:

```toml
[dependencies]
flow-control-detection = "0.1.0"
```

## API Usage

### Classify files from filenames

```rust
use flow_control_detection::{
    classify_controls, ControlClassification, ControlRole, FileInfo,
};

fn example() {
    let files: Vec<FileInfo> = vec![
        FileInfo {
            guid: "1".into(),
            filename: "Unstained.fcs".into(),
        },
        FileInfo {
            guid: "2".into(),
            filename: "CD3_FITC.fcs".into(),
        },
    ];
    let classified: Vec<ControlClassification> = classify_controls(&files);

    for c in &classified {
        let role: ControlRole = c.suggested_role;
        let confidence: f32 = c.confidence;
        let label: &String = &c.display_label;
        let guid: &String = &c.guid;
        println!("{guid}: {role:?} ({confidence}) → {label}");
    }
}
```

### Match endmembers / detectors to single-stain controls

```rust
use anyhow::Result;
use flow_control_detection::{
    classify_controls, match_endmembers, ControlClassification, EndmemberMatch, FileInfo,
};

fn example(files: &[FileInfo], detector_names: &[String]) -> Result<()> {
    let classified: Vec<ControlClassification> = classify_controls(files);
    let matches: Vec<EndmemberMatch> = match_endmembers(&classified, detector_names)?;

    for m in matches {
        let endmember: String = m.endmember_name;
        let control_guid: String = m.control_guid;
        let detector: Option<String> = m.detector_name;
        let confidence: f32 = m.confidence;
        println!("{endmember} → {control_guid} ({confidence}) detector={detector:?}");
    }
    Ok(())
}
```

## Testing

```bash
cargo test -p flow-control-detection
```

## License

MIT

## Related crates

- **FCS I/O** → [`flow-fcs`](../fcs/) — reading/loading FCS file data
- **Spectral unmixing** → [`flow-tru-ols`](../tru-ols/) / [`tru-ols`](../tru-ols-cli/) CLI
- **Peak isolation** → [`flow-peak-detection`](../flow-peak-detection/) - identify a peak after a control file is chosen
- **QC** → [`peacoqc-rs`](../peacoqc-rs/)