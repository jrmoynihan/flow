use std::fs;
use std::io::{self, BufRead};

#[derive(Debug)]
struct BenchmarkResult {
    name: String,
    min: f64,
    median: f64,
    max: f64,
}

fn parse_benchmark_line(line: &str) -> Option<BenchmarkResult> {
    // Look for lines like: "time:   [121.34 µs 132.77 µs 146.17 µs]"
    if !line.contains("time:") || !line.contains("µs") {
        return None;
    }

    // Extract numbers between brackets
    let start = line.find('[')?;
    let end = line.find(']')?;
    let content = &line[start + 1..end];
    
    // Split by "µs" and extract numbers
    let parts: Vec<&str> = content.split("µs").collect();
    if parts.len() < 3 {
        return None;
    }
    
    let min = parts[0].trim().parse().ok()?;
    let median = parts[1].trim().parse().ok()?;
    let max = parts[2].trim().parse().ok()?;
    
    Some(BenchmarkResult {
        name: String::new(), // Will be set from context
        min,
        median,
        max,
    })
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let file_path = args.get(1).map(|s| s.as_str()).unwrap_or("/tmp/bench_results.txt");
    
    let file = fs::File::open(file_path)?;
    let reader = io::BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;
    
    let mut current_benchmark = String::new();
    let mut results: Vec<(String, BenchmarkResult)> = Vec::new();
    
    for line in &lines {
        if line.starts_with("Benchmarking dataframe_parsing/") {
            // Extract benchmark name
            if let Some(name) = line.strip_prefix("Benchmarking dataframe_parsing/") {
                current_benchmark = name.split(':').next().unwrap_or("").to_string();
            }
        } else if let Some(result) = parse_benchmark_line(line) {
            if !current_benchmark.is_empty() {
                results.push((current_benchmark.clone(), result));
            }
        }
    }
    
    // Filter for 100k events results
    let results_100k: Vec<_> = results
        .iter()
        .filter(|(name, _)| name.contains("100000_events"))
        .collect();
    
    println!("=== Benchmark Analysis (100k events, 8 parameters) ===\n");
    
    // Compare uniform_f32_parallel
    let f32_par_inline = results_100k.iter().find(|(n, _)| n.contains("uniform_f32_parallel_inline"));
    let f32_par_no = results_100k.iter().find(|(n, _)| n.contains("uniform_f32_parallel_no_hints"));
    
    if let (Some((_, inline)), Some((_, no_hints))) = (f32_par_inline, f32_par_no) {
        let improvement = ((no_hints.median - inline.median) / no_hints.median) * 100.0;
        println!("Uniform Float32 Parallel:");
        println!("  With #[inline]:    {:.2} µs", inline.median);
        println!("  Without hints:     {:.2} µs", no_hints.median);
        println!("  Improvement:       {:.1}%", improvement);
        println!();
    }
    
    // Compare uniform_f32_sequential
    let f32_seq_inline = results_100k.iter().find(|(n, _)| n.contains("uniform_f32_sequential_inline"));
    let f32_seq_no = results_100k.iter().find(|(n, _)| n.contains("uniform_f32_sequential_no_hints"));
    
    if let (Some((_, inline)), Some((_, no_hints))) = (f32_seq_inline, f32_seq_no) {
        let improvement = ((no_hints.median - inline.median) / no_hints.median) * 100.0;
        println!("Uniform Float32 Sequential:");
        println!("  With #[inline]:    {:.2} µs", inline.median);
        println!("  Without hints:     {:.2} µs", no_hints.median);
        println!("  Improvement:       {:.1}%", improvement);
        println!();
    }
    
    // Compare parallel vs sequential for float32
    if let (Some((_, par)), Some((_, seq))) = (f32_par_inline, f32_seq_inline) {
        let speedup = par.median / seq.median;
        println!("Float32: Parallel vs Sequential:");
        println!("  Parallel:          {:.2} µs", par.median);
        println!("  Sequential:        {:.2} µs", seq.median);
        println!("  Sequential is {:.2}x faster", speedup);
        println!();
    }
    
    // Compare uniform_i16_parallel
    let i16_par_inline = results_100k.iter().find(|(n, _)| n.contains("uniform_i16_parallel_inline"));
    let i16_par_no = results_100k.iter().find(|(n, _)| n.contains("uniform_i16_parallel_no_hints"));
    
    if let (Some((_, inline)), Some((_, no_hints))) = (i16_par_inline, i16_par_no) {
        let improvement = ((no_hints.median - inline.median) / no_hints.median) * 100.0;
        println!("Uniform Int16 Parallel:");
        println!("  With #[inline]:    {:.2} µs", inline.median);
        println!("  Without hints:     {:.2} µs", no_hints.median);
        println!("  Improvement:       {:.1}%", improvement);
        println!();
    }
    
    // Compare uniform_i16_sequential
    let i16_seq_inline = results_100k.iter().find(|(n, _)| n.contains("uniform_i16_sequential_inline"));
    let i16_seq_no = results_100k.iter().find(|(n, _)| n.contains("uniform_i16_sequential_no_hints"));
    
    if let (Some((_, inline)), Some((_, no_hints))) = (i16_seq_inline, i16_seq_no) {
        let improvement = ((no_hints.median - inline.median) / no_hints.median) * 100.0;
        println!("Uniform Int16 Sequential:");
        println!("  With #[inline]:    {:.2} µs", inline.median);
        println!("  Without hints:     {:.2} µs", no_hints.median);
        println!("  Improvement:       {:.1}%", improvement);
        println!();
    }
    
    // Compare parallel vs sequential for int16
    if let (Some((_, par)), Some((_, seq))) = (i16_par_inline, i16_seq_inline) {
        let speedup = seq.median / par.median;
        println!("Int16: Parallel vs Sequential:");
        println!("  Parallel:          {:.2} µs", par.median);
        println!("  Sequential:        {:.2} µs", seq.median);
        println!("  Parallel is {:.2}x faster", speedup);
        println!();
    }
    
    // Compare variable_width
    let var_cold = results_100k.iter().find(|(n, _)| n.contains("variable_width_sequential_cold"));
    let var_no = results_100k.iter().find(|(n, _)| n.contains("variable_width_sequential_no_hints"));
    
    if let (Some((_, cold)), Some((_, no_hints))) = (var_cold, var_no) {
        let improvement = ((no_hints.median - cold.median) / no_hints.median) * 100.0;
        println!("Variable Width Sequential:");
        println!("  With #[cold]:      {:.2} µs", cold.median);
        println!("  Without hints:     {:.2} µs", no_hints.median);
        println!("  Improvement:       {:.1}%", improvement);
    }
    
    Ok(())
}

