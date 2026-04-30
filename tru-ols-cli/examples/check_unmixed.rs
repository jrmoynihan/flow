use flow_fcs::Fcs;
use std::env;

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/tru_ols_output.fcs".to_string());

    match Fcs::open(&path) {
        Ok(fcs) => {
            println!("✓ Successfully read unmixed FCS file");
            println!("Number of parameters: {}\n", fcs.parameters.len());

            println!("Parameter names:");
            let mut params_sorted: Vec<_> = fcs.parameters.iter().collect();
            params_sorted.sort_by_key(|(_name, param)| param.parameter_number);
            for (i, (_key, param)) in params_sorted.iter().enumerate() {
                let name = param.channel_name.as_ref();
                let label = param.label_name.as_ref();
                if label.is_empty() || label == name {
                    println!("  {:2}: {}", i + 1, name);
                } else {
                    println!("  {:2}: {} ({})", i + 1, name, label);
                }
            }

            // Count parameters
            let unmixed_count = fcs
                .parameters
                .values()
                .filter(|p| p.channel_name.starts_with("Unmixed_"))
                .count();

            let scatter_time = fcs
                .parameters
                .values()
                .filter(|p| {
                    let upper = p.channel_name.to_uppercase();
                    upper.contains("FSC") || upper.contains("SSC") || upper.contains("TIME")
                })
                .count();

            let original_fluor = fcs
                .parameters
                .values()
                .filter(|p| {
                    let name = p.channel_name.as_ref();
                    !name.starts_with("Unmixed_") && {
                        let upper = name.to_uppercase();
                        !upper.contains("FSC") && !upper.contains("SSC") && !upper.contains("TIME")
                    }
                })
                .count();

            println!("\nParameter summary:");
            println!("  Scatter/Time parameters: {}", scatter_time);
            println!("  Unmixed channels: {}", unmixed_count);
            println!(
                "  Original fluorescent channels remaining: {}",
                original_fluor
            );

            if original_fluor > 0 {
                println!(
                    "\n⚠ WARNING: Found {} original fluorescent channels that should have been filtered!",
                    original_fluor
                );
                println!("Names:");
                for param in fcs.parameters.values() {
                    let name = param.channel_name.as_ref();
                    if !name.starts_with("Unmixed_") && {
                        let upper = name.to_uppercase();
                        !upper.contains("FSC") && !upper.contains("SSC") && !upper.contains("TIME")
                    } {
                        println!("    - {}", name);
                    }
                }
                std::process::exit(1);
            } else {
                println!("\n✓ SUCCESS: All original fluorescent channels properly filtered!");
            }
        }
        Err(e) => {
            eprintln!("Error reading FCS: {}", e);
            std::process::exit(1);
        }
    }
}
