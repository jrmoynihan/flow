//! Test helpers for automated gating tests
//!
//! Provides utilities to create synthetic FCS files with realistic scatter patterns
//! for testing automated gating algorithms.

use flow_fcs::{
    Fcs, Header, Metadata, Parameter, TransformType,
    file::AccessWrapper,
    parameter::ParameterMap,
};
use polars::prelude::*;
use std::sync::Arc;
use std::fs::File;
use std::io::Write;

/// Create a synthetic FCS file with realistic scatter patterns
///
/// # Arguments
/// * `n_events` - Number of events to generate
/// * `scenario` - Test scenario type
///
/// # Returns
/// Fcs struct with synthetic data
pub fn create_synthetic_fcs(n_events: usize, scenario: TestScenario) -> Result<Fcs, Box<dyn std::error::Error>> {
    // Create a temporary file for testing
    let temp_path = std::env::temp_dir().join(format!("test_fcs_{}.tmp", std::process::id()));
    {
        let mut f = File::create(&temp_path)?;
        f.write_all(b"test")?;
    }

    // Generate data based on scenario
    let (fsc_a, fsc_h, fsc_w, ssc_a, ssc_h) = match scenario {
        TestScenario::SinglePopulation => generate_single_population(n_events),
        TestScenario::MultiPopulation => generate_multi_population(n_events),
        TestScenario::WithDoublets => generate_with_doublets(n_events),
        TestScenario::NoisyData => generate_noisy_data(n_events),
        TestScenario::WithDebris => generate_with_debris(n_events),
    };

    // Create DataFrame with scatter channels
    let mut columns = Vec::new();
    columns.push(Column::new("FSC-A".into(), fsc_a));
    columns.push(Column::new("FSC-H".into(), fsc_h));
    columns.push(Column::new("FSC-W".into(), fsc_w));
    columns.push(Column::new("SSC-A".into(), ssc_a));
    columns.push(Column::new("SSC-H".into(), ssc_h));

    let height = n_events;
    let df = DataFrame::new(height, columns).expect("Failed to create test DataFrame");

    // Create parameter map
    let mut params = ParameterMap::default();
    params.insert(
        "FSC-A".into(),
        Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
    );
    params.insert(
        "FSC-H".into(),
        Parameter::new(&2, "FSC-H", "FSC-H", &TransformType::Linear),
    );
    params.insert(
        "FSC-W".into(),
        Parameter::new(&3, "FSC-W", "FSC-W", &TransformType::Linear),
    );
    params.insert(
        "SSC-A".into(),
        Parameter::new(&4, "SSC-A", "SSC-A", &TransformType::Linear),
    );
    params.insert(
        "SSC-H".into(),
        Parameter::new(&5, "SSC-H", "SSC-H", &TransformType::Linear),
    );

    Ok(Fcs {
        header: Header::new(),
        metadata: Metadata::new(),
        parameters: params,
        data_frame: Arc::new(df),
        file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
    })
}

/// Test scenario types
#[derive(Debug, Clone, Copy)]
pub enum TestScenario {
    /// Single population scatter (ellipse fit test)
    SinglePopulation,
    /// Multi-population scatter (clustering test)
    MultiPopulation,
    /// Data with doublet patterns (doublet detection test)
    WithDoublets,
    /// Noisy data (edge case test)
    NoisyData,
    /// Data with debris population near origin (low FSC/SSC)
    WithDebris,
}

/// Generate single population scatter data
///
/// Creates a tight cluster around a center point with Gaussian distributions.
fn generate_single_population(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    use rand_distr::{Distribution, Normal};
    let mut rng = rand::rng();
    
    // Use Gaussian distributions for realistic flow cytometry data
    let fsc_dist = Normal::new(50000.0, 12000.0).unwrap();
    let ssc_dist = Normal::new(30000.0, 9000.0).unwrap();
    
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

/// Generate multi-population scatter data
///
/// Creates two distinct populations with different scatter characteristics.
fn generate_multi_population(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    use rand_distr::{Distribution, Normal};
    let mut rng = rand::rng();
    
    // Two distinct populations with Gaussian distributions
    let pop1_fsc_dist = Normal::new(30000.0, 8000.0).unwrap();
    let pop1_ssc_dist = Normal::new(20000.0, 6000.0).unwrap();
    let pop2_fsc_dist = Normal::new(70000.0, 10000.0).unwrap();
    let pop2_ssc_dist = Normal::new(50000.0, 8000.0).unwrap();
    
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

/// Generate data with doublet patterns
///
/// Creates singlet population plus doublet population with higher FSC-A/FSC-H ratios.
fn generate_with_doublets(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    use rand_distr::{Distribution, Normal};
    let mut rng = rand::rng();
    
    let singlet_fsc_dist = Normal::new(50000.0, 12000.0).unwrap();
    let singlet_ssc_dist = Normal::new(30000.0, 9000.0).unwrap();
    
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

/// Generate noisy data (edge case)
///
/// Creates data with high noise and less clear patterns.
fn generate_noisy_data(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    use rand_distr::{Distribution, Normal};
    let mut rng = rand::rng();
    
    // Wider distributions for noisy data
    let fsc_dist = Normal::new(50000.0, 25000.0).unwrap();
    let ssc_dist = Normal::new(30000.0, 20000.0).unwrap();
    
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

/// Generate data with debris population
///
/// Creates main population plus debris population near origin (low FSC/SSC).
fn generate_with_debris(n_events: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    use rand_distr::{Distribution, Normal};
    let mut rng = rand::rng();
    
    // Main population
    let main_fsc_dist = Normal::new(50000.0, 12000.0).unwrap();
    let main_ssc_dist = Normal::new(30000.0, 9000.0).unwrap();
    
    // Debris population: very low FSC/SSC near origin
    let debris_fsc_dist = Normal::new(2000.0, 1500.0).unwrap();
    let debris_ssc_dist = Normal::new(1500.0, 1000.0).unwrap();
    
    // 15% debris, 85% main population
    let n_debris = (n_events as f64 * 0.15) as usize;
    let n_main = n_events - n_debris;
    
    let mut fsc_a = Vec::with_capacity(n_events);
    let mut fsc_h = Vec::with_capacity(n_events);
    let mut fsc_w = Vec::with_capacity(n_events);
    let mut ssc_a = Vec::with_capacity(n_events);
    let mut ssc_h = Vec::with_capacity(n_events);
    
    // Main population
    for _ in 0..n_main {
        let fsc_a_val = (main_fsc_dist.sample(&mut rng) as f32).max(1000.0);
        fsc_a.push(fsc_a_val);
        fsc_h.push((fsc_a_val * 0.92 + (Normal::new(0.0, fsc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
        fsc_w.push((fsc_a_val * 0.35 + (Normal::new(0.0, fsc_a_val as f64 * 0.03).unwrap().sample(&mut rng) as f32)).max(0.0));
        
        let ssc_a_val = (main_ssc_dist.sample(&mut rng) as f32).max(500.0);
        ssc_a.push(ssc_a_val);
        ssc_h.push((ssc_a_val * 0.94 + (Normal::new(0.0, ssc_a_val as f64 * 0.05).unwrap().sample(&mut rng) as f32)).max(0.0));
    }
    
    // Debris: very low values near origin
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
