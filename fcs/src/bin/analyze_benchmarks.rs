use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};

#[derive(Debug, Clone)]
struct BenchmarkResult {
    median: f64,
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

    let median = parts[1].trim().parse().ok()?;

    Some(BenchmarkResult { median })
}

fn extract_event_count(name: &str) -> Option<usize> {
    // Extract number from names like "dataframe_parsing/uniform_f32_parallel_inline/10000_events"
    // Format: .../XXXXX_events
    if let Some(events_part) = name.split('/').last() {
        if events_part.ends_with("_events") {
            if let Some(num_str) = events_part.strip_suffix("_events") {
                return num_str.parse().ok();
            }
        }
    }
    None
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let file_path = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("/tmp/bench_results.txt");

    let file = fs::File::open(file_path)?;
    let reader = io::BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    let mut current_benchmark = String::new();
    let mut results: Vec<(String, BenchmarkResult)> = Vec::new();

    for line in lines.iter() {
        // Check if this line is a benchmark name (starts with "dataframe_parsing/")
        if line.starts_with("dataframe_parsing/") && !line.contains("time:") {
            // Extract the full benchmark name
            current_benchmark = line.trim().to_string();
        } else if let Some(result) = parse_benchmark_line(line) {
            // If we have a current benchmark name, associate this result with it
            if !current_benchmark.is_empty() {
                results.push((current_benchmark.clone(), result));
                current_benchmark.clear(); // Clear after use
            }
        }
    }

    // Organize results by event count and type
    let mut f32_parallel: HashMap<usize, f64> = HashMap::new();
    let mut f32_sequential: HashMap<usize, f64> = HashMap::new();
    let mut i16_parallel: HashMap<usize, f64> = HashMap::new();
    let mut i16_sequential: HashMap<usize, f64> = HashMap::new();

    for (name, result) in &results {
        if let Some(event_count) = extract_event_count(name) {
            // Match the full path format: dataframe_parsing/.../events
            if name.contains("/uniform_f32_parallel_inline/") {
                f32_parallel.insert(event_count, result.median);
            } else if name.contains("/uniform_f32_sequential_inline/") {
                f32_sequential.insert(event_count, result.median);
            } else if name.contains("/uniform_i16_parallel_inline/") {
                i16_parallel.insert(event_count, result.median);
            } else if name.contains("/uniform_i16_sequential_inline/") {
                i16_sequential.insert(event_count, result.median);
            }
        }
    }

    // Sort event counts
    let mut event_counts: Vec<usize> = f32_parallel.keys().copied().collect();
    event_counts.sort();

    println!("=== Parallelization Threshold Analysis ===\n");
    println!("Testing event counts: {:?}\n", event_counts);

    // Float32 analysis
    println!("=== Float32 (4 bytes) Analysis ===");
    println!(
        "{:<12} {:<15} {:<15} {:<15} {:<15}",
        "Events", "Parallel (µs)", "Sequential (µs)", "Speedup", "Winner"
    );
    println!("{}", "-".repeat(75));

    let mut f32_sequential_always_faster = true;
    let mut f32_parallel_wins_at: Option<usize> = None;
    for &events in &event_counts {
        if let (Some(&par), Some(&seq)) = (f32_parallel.get(&events), f32_sequential.get(&events)) {
            let speedup = par / seq; // >1 means sequential is faster
            let winner = if speedup > 1.0 {
                "Sequential"
            } else {
                "Parallel"
            };
            println!(
                "{:<12} {:<15.2} {:<15.2} {:<15.2}x {:<15}",
                events, par, seq, speedup, winner
            );

            // Check if parallel ever wins
            if speedup < 1.0 {
                f32_sequential_always_faster = false;
                if f32_parallel_wins_at.is_none() {
                    f32_parallel_wins_at = Some(events);
                }
            }
        }
    }

    if f32_sequential_always_faster {
        println!(
            "\n✓ Float32: Sequential is ALWAYS faster (tested up to {} events)",
            event_counts.last().unwrap_or(&0)
        );
    } else if let Some(events) = f32_parallel_wins_at {
        println!(
            "\n⚠ Float32: Parallel becomes faster at {} events (unexpected!)",
            events
        );
    }

    println!("\n=== Int16 (2 bytes) Analysis ===");
    println!(
        "{:<12} {:<15} {:<15} {:<15} {:<15}",
        "Events", "Parallel (µs)", "Sequential (µs)", "Speedup", "Winner"
    );
    println!("{}", "-".repeat(75));

    let mut i16_threshold: Option<usize> = None;
    for &events in &event_counts {
        if let (Some(&par), Some(&seq)) = (i16_parallel.get(&events), i16_sequential.get(&events)) {
            let speedup = seq / par; // How much faster parallel is
            let winner = if speedup > 1.0 {
                "Parallel"
            } else {
                "Sequential"
            };
            println!(
                "{:<12} {:<15.2} {:<15.2} {:<15.2}x {:<15}",
                events, par, seq, speedup, winner
            );

            // Find first point where parallel is faster
            if i16_threshold.is_none() && speedup > 1.0 {
                i16_threshold = Some(events);
            }
        }
    }

    if let Some(threshold) = i16_threshold {
        println!(
            "\n✓ Int16: Parallel becomes faster at {} events ({}k values)",
            threshold,
            threshold * 8 / 1000
        );
    } else {
        println!("\n⚠ Int16: Parallel is never faster in tested range");
    }

    // Calculate recommended threshold
    println!("\n=== Recommended Threshold ===");
    if let Some(threshold) = i16_threshold {
        let values_threshold = threshold * 8; // 8 parameters
        println!(
            "Recommended PARALLEL_THRESHOLD: {} values ({} events × 8 params)",
            values_threshold, threshold
        );
        println!("\nThis means:");
        println!("  - Float32: Always use sequential");
        println!(
            "  - Int16/Int32/Float64: Use parallel for datasets with ≥{} values",
            values_threshold
        );
    } else {
        println!("⚠ Could not determine threshold - parallel may not be beneficial");
        println!("Recommendation: Use sequential for all data types");
    }

    Ok(())
}
