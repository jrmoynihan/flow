use peacoqc_rs::{SimpleFcs, qc::*, error::Result};
use polars::prelude::*;
use std::collections::HashMap;

/// Example demonstrating how to use the PeacoQC library
fn main() -> Result<()> {
    println!("=== PeacoQC Rust Example ===\n");
    
    // 1. Create synthetic FCS data
    let fcs = create_synthetic_data()?;
    
    println!("Loaded FCS data:");
    println!("  Events: {}", fcs.n_events());
    println!("  Channels: {:?}\n", fcs.channel_names());
    
    // 2. Remove margin events
    println!("Step 1: Removing margin events...");
    let margin_config = MarginConfig {
        channels: vec!["FSC-A".to_string(), "SSC-A".to_string()],
        ..Default::default()
    };
    
    let margin_result = remove_margins(&fcs, &margin_config)?;
    println!("  Removed: {:.2}%", margin_result.percentage_removed);
    
    // 3. Remove doublets
    println!("\nStep 2: Removing doublets...");
    let doublet_config = DoubletConfig::default();
    let doublet_result = remove_doublets(&fcs, &doublet_config)?;
    println!("  Removed: {:.2}%", doublet_result.percentage_removed);
    println!("  Threshold: {:.3}", doublet_result.threshold);
    
    // 4. Run full PeacoQC analysis
    println!("\nStep 3: Running PeacoQC analysis...");
    let peacoqc_config = PeacoQCConfig {
        channels: vec!["FL1-A".to_string(), "FL2-A".to_string()],
        determine_good_cells: QCMode::MAD,
        ..Default::default()
    };
    
    let peacoqc_result = peacoqc(&fcs, &peacoqc_config)?;
    
    println!("\n=== PeacoQC Results ===");
    println!("  Total removed: {:.2}%", peacoqc_result.percentage_removed);
    if let Some(mad_pct) = peacoqc_result.mad_percentage {
        println!("  MAD removed: {:.2}%", mad_pct);
    }
    println!("  Consecutive removed: {:.2}%", peacoqc_result.consecutive_percentage);
    println!("  Number of bins: {}", peacoqc_result.n_bins);
    println!("  Events per bin: {}", peacoqc_result.events_per_bin);
    
    // 5. Show peak detection results
    println!("\n=== Peak Detection Results ===");
    for (channel, peak_frame) in &peacoqc_result.peaks {
        println!("  {}: {} peaks detected", channel, peak_frame.peaks.len());
        
        // Show clusters
        let mut clusters: HashMap<usize, Vec<f64>> = HashMap::new();
        for peak in &peak_frame.peaks {
            clusters.entry(peak.cluster)
                .or_insert_with(Vec::new)
                .push(peak.peak_value);
        }
        
        for (cluster_id, values) in clusters {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            println!("    Cluster {}: {:.2} (n={})", cluster_id, mean, values.len());
        }
    }
    
    // 6. Apply the filter
    println!("\n=== Applying Filter ===");
    let n_kept = peacoqc_result.good_cells.iter().filter(|&&x| x).count();
    println!("  Keeping {} / {} events", n_kept, fcs.n_events());
    
    Ok(())
}

/// Create synthetic FCS data for demonstration
fn create_synthetic_data() -> Result<SimpleFcs> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    let n_events = 10000;
    
    // FSC-A: Normal distribution around 50000
    let fsc_a: Vec<f64> = (0..n_events)
        .map(|_| 50000.0 + rng.gen::<f64>() * 20000.0)
        .collect();
    
    // FSC-H: Correlated with FSC-A (singlets)
    let fsc_h: Vec<f64> = fsc_a.iter()
        .map(|&a| a * 0.8 + rng.gen::<f64>() * 5000.0)
        .collect();
    
    // SSC-A: Normal distribution
    let ssc_a: Vec<f64> = (0..n_events)
        .map(|_| 30000.0 + rng.gen::<f64>() * 15000.0)
        .collect();
    
    // FL1-A: Bimodal distribution (two populations)
    let mut fl1_a = Vec::new();
    for _ in 0..n_events {
        let val = if rng.gen::<f64>() < 0.6 {
            // Population 1: low expression
            1000.0 + rng.gen::<f64>() * 500.0
        } else {
            // Population 2: high expression
            5000.0 + rng.gen::<f64>() * 1000.0
        };
        fl1_a.push(val);
    }
    
    // FL2-A: Single population
    let fl2_a: Vec<f64> = (0..n_events)
        .map(|_| 2000.0 + rng.gen::<f64>() * 800.0)
        .collect();
    
    // Create DataFrame
    let df = DataFrame::new(vec![
        Series::new("FSC-A".into(), fsc_a),
        Series::new("FSC-H".into(), fsc_h),
        Series::new("SSC-A".into(), ssc_a),
        Series::new("FL1-A".into(), fl1_a),
        Series::new("FL2-A".into(), fl2_a),
    ])?;
    
    // Create parameter metadata
    let mut metadata = HashMap::new();
    for channel in ["FSC-A", "FSC-H", "SSC-A", "FL1-A", "FL2-A"] {
        metadata.insert(
            channel.to_string(),
            peacoqc_rs::fcs::ParameterMetadata {
                min_range: 0.0,
                max_range: 262144.0,
                name: channel.to_string(),
            },
        );
    }
    
    Ok(SimpleFcs {
        data_frame: df,
        parameter_metadata: metadata,
    })
}
