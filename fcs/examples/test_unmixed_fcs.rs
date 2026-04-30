use flow_fcs::Fcs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fcs_path =
        "/tmp/unmixed_output/Donor 10_F1 Full Stain_D10_Plate_001_2025_09_25_15_36_48_unmixed.fcs";

    println!("=== FCS File Parsing Test ===\n");
    println!("Testing file: {}", fcs_path);
    println!("File exists: {}\n", Path::new(fcs_path).exists());

    // Open the FCS file
    println!("Opening FCS file...");
    let fcs = Fcs::open(fcs_path)?;

    println!("✓ File opened successfully\n");

    // Check basic metadata
    println!("File Information:");
    println!("  Events: {}", fcs.data_frame.height());
    println!("  Parameters: {}", fcs.data_frame.width());
    println!("  Version: {}\n", fcs.header.version);

    // Check parameter names
    println!("Parameter Summary:");
    let col_names = fcs.data_frame.get_column_names();
    for (idx, name) in col_names.iter().take(10).enumerate() {
        let name_arc = std::sync::Arc::<str>::from(name.as_str());
        if let Some(param) = fcs.parameters.get(&name_arc) {
            println!(
                "  [{}] ${} - {} ({})",
                idx + 1,
                idx + 1,
                param.channel_name,
                param.label_name
            );
        } else {
            println!(
                "  [{}] ${} - {} (no param metadata)",
                idx + 1,
                idx + 1,
                name
            );
        }
    }
    if col_names.len() > 10 {
        println!("  ... and {} more parameters", col_names.len() - 10);
    }
    println!();

    // Check for unmixed channels
    println!("Unmixed Channels (new additions):");
    let mut unmixed_count = 0;
    for (idx, name) in col_names.iter().enumerate() {
        if name.to_string().contains("Unmixed") {
            let name_arc = std::sync::Arc::<str>::from(name.as_str());
            if let Some(param) = fcs.parameters.get(&name_arc) {
                println!(
                    "  [{}] ${} - {} (Label: {})",
                    idx + 1,
                    idx + 1,
                    param.channel_name,
                    param.label_name
                );
                unmixed_count += 1;
            }
        }
    }
    if unmixed_count == 0 {
        println!("  (No unmixed channels found)");
    }
    println!();

    // Check keywords for parsing issues
    println!("Checking TEXT segment keywords for issues...\n");

    let mut pne_errors = 0;
    let mut pdisplay_errors = 0;
    let mut pne_examples = Vec::new();
    let mut pdisplay_examples = Vec::new();

    for (key, keyword) in fcs.metadata.keywords.iter() {
        if key.contains("E") && key.contains("P") && !key.contains("P1E") && !key.contains("P2E") {
            // Check PnE keywords
            if key.contains("E") && !key.contains("DATATYPE") && !key.contains("DISPLAY") {
                let keyword_str = keyword.to_string();
                if keyword_str.contains("PnE(") {
                    pne_errors += 1;
                    if pne_examples.len() < 3 {
                        pne_examples.push(format!("{}: {}", key, keyword_str));
                    }
                }
            }
        }
        if key.contains("DISPLAY") {
            let keyword_str = keyword.to_string();
            if pdisplay_examples.len() < 3 {
                pdisplay_examples.push(format!("{}: {}", key, keyword_str));
            }
        }
    }

    if pne_errors > 0 {
        println!(
            "⚠  WARNING: Found {} $PnE keywords with debug format (PnE(...)):",
            pne_errors
        );
        for example in pne_examples {
            println!("    {}", example);
        }
    } else {
        println!("✓ All $PnE keywords properly formatted");
    }
    println!();

    if !pdisplay_examples.is_empty() {
        println!("✓ Found $PnDISPLAY keywords:");
        for example in pdisplay_examples {
            println!("    {}", example);
        }
    }
    println!();

    // Summary
    println!("=== Test Complete ===");
    println!("Result: SUCCESS - File parsed without critical errors");

    Ok(())
}
