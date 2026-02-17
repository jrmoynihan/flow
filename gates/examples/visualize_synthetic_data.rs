//! Example: Visualize synthetic test data using flow-plots
//!
//! This example demonstrates the synthetic FCS file generation and creates
//! scatter plots for each test scenario.

use flow_fcs::Fcs;
use flow_plots::{DensityPlot, DensityPlotOptions, BasePlotOptions, AxisOptions, Plot};
use flow_plots::render::RenderConfig;
use std::fs;
use std::path::PathBuf;

// Import test helpers - copy the necessary parts inline for the example
mod test_helpers {
    pub use flow_fcs::{
        Fcs, Header, Metadata, Parameter, TransformType,
        file::AccessWrapper,
        parameter::ParameterMap,
    };
    use polars::prelude::*;
    use std::sync::Arc;
    use std::fs::File;
    use std::io::Write;

    #[derive(Debug, Clone, Copy)]
    pub enum TestScenario {
        SinglePopulation,
        MultiPopulation,
        WithDoublets,
        NoisyData,
        WithDebris,
    }

    pub fn create_synthetic_fcs(n_events: usize, scenario: TestScenario) -> Result<Fcs, Box<dyn std::error::Error>> {
        let temp_path = std::env::temp_dir().join(format!("test_fcs_{}.tmp", std::process::id()));
        {
            let mut f = File::create(&temp_path)?;
            f.write_all(b"test")?;
        }

        let (fsc_a, fsc_h, fsc_w, ssc_a, ssc_h) = match scenario {
            TestScenario::SinglePopulation => generate_single_population(n_events),
            TestScenario::MultiPopulation => generate_multi_population(n_events),
            TestScenario::WithDoublets => generate_with_doublets(n_events),
            TestScenario::NoisyData => generate_noisy_data(n_events),
            TestScenario::WithDebris => generate_with_debris(n_events),
        };

        let mut columns = Vec::new();
        columns.push(Column::new("FSC-A".into(), fsc_a));
        columns.push(Column::new("FSC-H".into(), fsc_h));
        columns.push(Column::new("FSC-W".into(), fsc_w));
        columns.push(Column::new("SSC-A".into(), ssc_a));
        columns.push(Column::new("SSC-H".into(), ssc_h));

        let df = DataFrame::new(columns).expect("Failed to create test DataFrame");

        let mut params = ParameterMap::default();
        params.insert("FSC-A".into(), Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear));
        params.insert("FSC-H".into(), Parameter::new(&2, "FSC-H", "FSC-H", &TransformType::Linear));
        params.insert("FSC-W".into(), Parameter::new(&3, "FSC-W", "FSC-W", &TransformType::Linear));
        params.insert("SSC-A".into(), Parameter::new(&4, "SSC-A", "SSC-A", &TransformType::Linear));
        params.insert("SSC-H".into(), Parameter::new(&5, "SSC-H", "SSC-H", &TransformType::Linear));

        Ok(Fcs {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
        })
    }

    fn generate_single_population(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
        use rand_distr::{Distribution, Normal};
        let mut rng = rand::rng();
        
        // Use Gaussian distributions for realistic flow cytometry data
        // Reduced FSC mean by 35%: 50000 * 0.65 = 32500
        // Narrowed distribution: reduced std dev by ~40% for more concentration
        let fsc_dist = Normal::new(32500.0, 7200.0).unwrap();
        let ssc_dist = Normal::new(30000.0, 5400.0).unwrap();
        
        let mut fsc_a = Vec::with_capacity(n_events);
        let mut fsc_h = Vec::with_capacity(n_events);
        let mut fsc_w = Vec::with_capacity(n_events);
        let mut ssc_a = Vec::with_capacity(n_events);
        let mut ssc_h = Vec::with_capacity(n_events);
        
        for _ in 0..n_events {
            // Generate correlated FSC-A and FSC-H (height slightly less than area)
            let fsc_a_val = (fsc_dist.sample(&mut rng) as f32).max(1000.0);
            fsc_a.push(fsc_a_val);
            
            // FSC-H is correlated with FSC-A but slightly lower with some noise
            let fsc_h_val = fsc_a_val * 0.92 + (Normal::new(0.0, fsc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32);
            fsc_h.push(fsc_h_val.max(0.0));
            
            // FSC-W is typically 30-40% of FSC-A
            let fsc_w_val = fsc_a_val * 0.35 + (Normal::new(0.0, fsc_a_val as f64 * 0.03).unwrap().sample(&mut rng) as f32);
            fsc_w.push(fsc_w_val.max(0.0));
            
            // SSC-A is correlated but independent
            let ssc_a_val = (ssc_dist.sample(&mut rng) as f32).max(500.0);
            ssc_a.push(ssc_a_val);
            
            // SSC-H is correlated with SSC-A
            let ssc_h_val = ssc_a_val * 0.94 + (Normal::new(0.0, ssc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32);
            ssc_h.push(ssc_h_val.max(0.0));
        }
        
        (fsc_a, fsc_h, fsc_w, ssc_a, ssc_h)
    }

    fn generate_multi_population(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
        use rand_distr::{Distribution, Normal};
        let mut rng = rand::rng();
        
        // Two distinct populations with Gaussian distributions
        // Narrowed distributions for more concentration
        let pop1_fsc_dist = Normal::new(30000.0, 4800.0).unwrap();
        let pop1_ssc_dist = Normal::new(20000.0, 3600.0).unwrap();
        let pop2_fsc_dist = Normal::new(70000.0, 6000.0).unwrap();
        let pop2_ssc_dist = Normal::new(50000.0, 4800.0).unwrap();
        
        let n_pop1 = n_events / 2;
        let n_pop2 = n_events - n_pop1;
        
        let mut fsc_a = Vec::with_capacity(n_events);
        let mut fsc_h = Vec::with_capacity(n_events);
        let mut fsc_w = Vec::with_capacity(n_events);
        let mut ssc_a = Vec::with_capacity(n_events);
        let mut ssc_h = Vec::with_capacity(n_events);
        
        // Population 1: smaller cells
        for _ in 0..n_pop1 {
            let fsc_a_val = (pop1_fsc_dist.sample(&mut rng) as f32).max(1000.0);
            fsc_a.push(fsc_a_val);
            fsc_h.push((fsc_a_val * 0.92 + (Normal::new(0.0, fsc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
            fsc_w.push((fsc_a_val * 0.35 + (Normal::new(0.0, fsc_a_val as f64 * 0.03).unwrap().sample(&mut rng) as f32)).max(0.0));
            
            let ssc_a_val = (pop1_ssc_dist.sample(&mut rng) as f32).max(500.0);
            ssc_a.push(ssc_a_val);
            ssc_h.push((ssc_a_val * 0.94 + (Normal::new(0.0, ssc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
        }
        
        // Population 2: larger cells
        for _ in 0..n_pop2 {
            let fsc_a_val = (pop2_fsc_dist.sample(&mut rng) as f32).max(1000.0);
            fsc_a.push(fsc_a_val);
            fsc_h.push((fsc_a_val * 0.92 + (Normal::new(0.0, fsc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
            fsc_w.push((fsc_a_val * 0.35 + (Normal::new(0.0, fsc_a_val as f64 * 0.03).unwrap().sample(&mut rng) as f32)).max(0.0));
            
            let ssc_a_val = (pop2_ssc_dist.sample(&mut rng) as f32).max(500.0);
            ssc_a.push(ssc_a_val);
            ssc_h.push((ssc_a_val * 0.94 + (Normal::new(0.0, ssc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
        }
        
        (fsc_a, fsc_h, fsc_w, ssc_a, ssc_h)
    }

    fn generate_with_doublets(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
        use rand_distr::{Distribution, Normal};
        let mut rng = rand::rng();
        
        // Reduced FSC mean by 35%: 50000 * 0.65 = 32500
        // Narrowed distribution for more concentration
        let singlet_fsc_dist = Normal::new(32500.0, 7200.0).unwrap();
        let singlet_ssc_dist = Normal::new(30000.0, 5400.0).unwrap();
        
        let n_doublets = n_events / 10;
        let n_singlets = n_events - n_doublets;
        
        let mut fsc_a = Vec::with_capacity(n_events);
        let mut fsc_h = Vec::with_capacity(n_events);
        let mut fsc_w = Vec::with_capacity(n_events);
        let mut ssc_a = Vec::with_capacity(n_events);
        let mut ssc_h = Vec::with_capacity(n_events);
        
        // Singlets: normal distribution
        for _ in 0..n_singlets {
            let fsc_a_val = (singlet_fsc_dist.sample(&mut rng) as f32).max(1000.0);
            fsc_a.push(fsc_a_val);
            fsc_h.push((fsc_a_val * 0.92 + (Normal::new(0.0, fsc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
            fsc_w.push((fsc_a_val * 0.35 + (Normal::new(0.0, fsc_a_val as f64 * 0.03).unwrap().sample(&mut rng) as f32)).max(0.0));
            
            let ssc_a_val = (singlet_ssc_dist.sample(&mut rng) as f32).max(500.0);
            ssc_a.push(ssc_a_val);
            ssc_h.push((ssc_a_val * 0.94 + (Normal::new(0.0, ssc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
        }
        
        // Doublets: higher FSC-A, lower FSC-H/FSC-A ratio (wider)
        for _ in 0..n_doublets {
            let fsc_a_val = (singlet_fsc_dist.sample(&mut rng) as f32).max(1000.0) * 1.8;
            fsc_a.push(fsc_a_val);
            // Doublets have lower height/area ratio (wider cells)
            fsc_h.push((fsc_a_val * 0.65 + (Normal::new(0.0, fsc_a_val as f64 * 0.08).unwrap().sample(&mut rng) as f32)).max(0.0));
            fsc_w.push((fsc_a_val * 0.42 + (Normal::new(0.0, fsc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
            
            let ssc_a_val = (singlet_ssc_dist.sample(&mut rng) as f32).max(500.0) * 1.5;
            ssc_a.push(ssc_a_val);
            ssc_h.push((ssc_a_val * 0.94 + (Normal::new(0.0, ssc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
        }
        
        (fsc_a, fsc_h, fsc_w, ssc_a, ssc_h)
    }

    fn generate_noisy_data(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
        use rand_distr::{Distribution, Normal};
        let mut rng = rand::rng();
        
        // Narrowed distributions (still wider than others, but more concentrated)
        let fsc_dist = Normal::new(50000.0, 15000.0).unwrap();
        let ssc_dist = Normal::new(30000.0, 12000.0).unwrap();
        
        let mut fsc_a = Vec::with_capacity(n_events);
        let mut fsc_h = Vec::with_capacity(n_events);
        let mut fsc_w = Vec::with_capacity(n_events);
        let mut ssc_a = Vec::with_capacity(n_events);
        let mut ssc_h = Vec::with_capacity(n_events);
        
        for _ in 0..n_events {
            let fsc_a_val = (fsc_dist.sample(&mut rng) as f32).max(1000.0);
            fsc_a.push(fsc_a_val);
            // Higher noise in correlations
            fsc_h.push((fsc_a_val * 0.88 + (Normal::new(0.0, fsc_a_val as f64 * 0.12).unwrap().sample(&mut rng) as f32)).max(0.0));
            fsc_w.push((fsc_a_val * 0.33 + (Normal::new(0.0, fsc_a_val as f64 * 0.08).unwrap().sample(&mut rng) as f32)).max(0.0));
            
            let ssc_a_val = (ssc_dist.sample(&mut rng) as f32).max(500.0);
            ssc_a.push(ssc_a_val);
            ssc_h.push((ssc_a_val * 0.90 + (Normal::new(0.0, ssc_a_val as f64 * 0.12).unwrap().sample(&mut rng) as f32)).max(0.0));
        }
        
        (fsc_a, fsc_h, fsc_w, ssc_a, ssc_h)
    }

    fn generate_with_debris(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
        use rand::Rng;
        use rand_distr::{Distribution, Normal};
        let mut rng = rand::rng();
        
        // Narrowed main population distribution for more concentration
        // Debris population keeps original wider distribution
        let main_fsc_dist = Normal::new(50000.0, 7200.0).unwrap();
        let main_ssc_dist = Normal::new(30000.0, 5400.0).unwrap();
        let debris_fsc_dist = Normal::new(2000.0, 1500.0).unwrap();
        let debris_ssc_dist = Normal::new(1500.0, 1000.0).unwrap();
        
        // 10% debris, 90% main population (increased main population density)
        let n_debris = (n_events as f64 * 0.10) as usize;
        let n_main = n_events - n_debris;
        
        let mut fsc_a = Vec::with_capacity(n_events);
        let mut fsc_h = Vec::with_capacity(n_events);
        let mut fsc_w = Vec::with_capacity(n_events);
        let mut ssc_a = Vec::with_capacity(n_events);
        let mut ssc_h = Vec::with_capacity(n_events);
        
        for _ in 0..n_main {
            let fsc_a_val = (main_fsc_dist.sample(&mut rng) as f32).max(1000.0);
            fsc_a.push(fsc_a_val);
            fsc_h.push((fsc_a_val * 0.92 + (Normal::new(0.0, fsc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
            fsc_w.push((fsc_a_val * 0.35 + (Normal::new(0.0, fsc_a_val as f64 * 0.03).unwrap().sample(&mut rng) as f32)).max(0.0));
            let ssc_a_val = (main_ssc_dist.sample(&mut rng) as f32).max(500.0);
            ssc_a.push(ssc_a_val);
            ssc_h.push((ssc_a_val * 0.94 + (Normal::new(0.0, ssc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
        }
        
        for _ in 0..n_debris {
            let fsc_a_val = (debris_fsc_dist.sample(&mut rng) as f32).max(100.0);
            fsc_a.push(fsc_a_val);
            fsc_h.push((fsc_a_val * 0.85 + (Normal::new(0.0, fsc_a_val as f64 * 0.15).unwrap().sample(&mut rng) as f32)).max(0.0));
            fsc_w.push((fsc_a_val * 0.40 + (Normal::new(0.0, fsc_a_val as f64 * 0.10).unwrap().sample(&mut rng) as f32)).max(0.0));
            let ssc_a_val = (debris_ssc_dist.sample(&mut rng) as f32).max(50.0);
            ssc_a.push(ssc_a_val);
            ssc_h.push((ssc_a_val * 0.90 + (Normal::new(0.0, ssc_a_val as f64 * 0.15).unwrap().sample(&mut rng) as f32)).max(0.0));
        }
        
        (fsc_a, fsc_h, fsc_w, ssc_a, ssc_h)
    }
}
use test_helpers::{create_synthetic_fcs, TestScenario};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create output directory
    let output_dir = PathBuf::from("gates/examples/synthetic_plots");
    fs::create_dir_all(&output_dir)?;

    // Generate and plot each scenario
    let scenarios = vec![
        (TestScenario::SinglePopulation, "single_population"),
        (TestScenario::MultiPopulation, "multi_population"),
        (TestScenario::WithDoublets, "with_doublets"),
        (TestScenario::NoisyData, "noisy_data"),
        (TestScenario::WithDebris, "with_debris"),
    ];

    for (scenario, name) in scenarios {
        println!("Generating plot for: {}", name);
        
        // Create synthetic FCS file with higher event count for better visualization
        // Triple the events: 60k for most scenarios, 90k for with_debris
        let n_events = if matches!(scenario, TestScenario::WithDebris) {
            90000  // Triple: 30k -> 90k for with_debris
        } else {
            60000  // Triple: 20k -> 60k for other scenarios
        };
        let fcs = create_synthetic_fcs(n_events, scenario)?;
        
        // Extract FSC-A and SSC-A data
        let fsc_a = fcs.get_parameter_events_slice("FSC-A")?;
        let ssc_a = fcs.get_parameter_events_slice("SSC-A")?;
        
        // Create plot data pairs
        let plot_data: Vec<(f32, f32)> = fsc_a
            .iter()
            .zip(ssc_a.iter())
            .map(|(&x, &y)| (x, y))
            .collect();
        
        // Create plot options
        let x_axis = AxisOptions::new()
            .range(0.0..=100_000.0)
            .label("FSC-A".to_string())
            .build()?;
        
        let y_axis = AxisOptions::new()
            .range(0.0..=100_000.0)
            .label("SSC-A".to_string())
            .build()?;
        
        let base = BasePlotOptions::new()
            .width(800u32)
            .height(600u32)
            .title(format!("Synthetic Data: {}", name.replace("_", " ")))
            .build()?;
        
        let plot_options = DensityPlotOptions::new()
            .base(base)
            .x_axis(x_axis)
            .y_axis(y_axis)
            .build()?;
        
        // Render plot
        let plot = DensityPlot::new();
        let mut render_config = RenderConfig::default();
        let image_bytes = plot.render(plot_data, &plot_options, &mut render_config)?;
        
        // Save to file
        let output_path = output_dir.join(format!("{}.png", name));
        fs::write(&output_path, image_bytes)?;
        println!("  Saved to: {}", output_path.display());
    }
    
    println!("\nAll plots generated successfully!");
    Ok(())
}
