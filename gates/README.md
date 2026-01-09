# flow-gates

A comprehensive Rust library for working with gates in flow cytometry data analysis. This library provides tools for creating, managing, and applying gates to flow cytometry data, supporting the GatingML 2.0 standard for gate definitions and hierarchies.

## Features

- **Multiple Gate Types**: Polygon, Rectangle, and Ellipse geometries
- **Gate Hierarchies**: Parent-child relationships for sequential gating strategies
- **Efficient Event Filtering**: Spatial indexing (R*-tree) for fast point-in-gate queries
- **Comprehensive Statistics**: Detailed statistical analysis of gated populations
- **GatingML 2.0 Support**: Import/export gates in standard XML format
- **Thread-Safe Storage**: Concurrent gate management with optional persistence
- **Zero-Copy Operations**: Efficient data access using slices where possible

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
flow-gates = { path = "../flow-gates" }
flow-fcs = { path = "../flow-fcs" }  # Required for FCS file support
```

## Quick Start

### Creating a Gate

```rust
use flow_gates::*;
use flow_gates::geometry::*;

// Create a polygon gate from coordinates
let coords = vec![
    (100.0, 200.0),
    (300.0, 200.0),
    (300.0, 400.0),
    (100.0, 400.0),
];
let geometry = create_polygon_geometry(coords, "FSC-A", "SSC-A")?;

let gate = Gate::new(
    "lymphocytes",
    "Lymphocytes",
    geometry,
    "FSC-A",
    "SSC-A",
);
```

### Filtering Events

```rust
use flow_gates::{filter_events_by_gate, Gate};
use flow_fcs::Fcs;

// Load FCS file
let fcs = Fcs::from_file("data.fcs")?;

// Filter events by gate
let event_indices = filter_events_by_gate(&fcs, &gate, None)?;

println!("Found {} events in gate", event_indices.len());
```

### Calculating Statistics

```rust
use flow_gates::GateStatistics;

let stats = GateStatistics::calculate(&fcs, &gate)?;

println!("Event count: {}", stats.event_count);
println!("Percentage: {:.2}%", stats.percentage);
println!("X parameter mean: {:.2}", stats.x_stats.mean);
println!("Y parameter median: {:.2}", stats.y_stats.median);
```

## Core Concepts

### Gates

A `Gate` represents a region of interest in 2D parameter space. Each gate has:

- **Geometry**: The shape (polygon, rectangle, or ellipse)
- **Parameters**: Two channels (x and y) the gate operates on
- **Mode**: Scope (global, file-specific, or file group)
- **ID and Name**: Unique identifier and human-readable name

### Gate Types

#### Polygon Gates

Polygon gates are defined by a series of vertices forming a closed or open polygon:

```rust
use flow_gates::{Gate, GateGeometry, GateNode, geometry::*};

let coords = vec![
    (100.0, 200.0),
    (300.0, 200.0),
    (300.0, 400.0),
    (100.0, 400.0),
];
let geometry = create_polygon_geometry(coords, "FSC-A", "SSC-A")?;

let gate = Gate::new("polygon-gate", "Polygon", geometry, "FSC-A", "SSC-A");
```

#### Rectangle Gates

Rectangle gates are axis-aligned rectangular regions:

```rust
let coords = vec![(100.0, 200.0), (500.0, 600.0)];
let geometry = create_rectangle_geometry(coords, "FSC-A", "SSC-A")?;

let gate = Gate::new("rect-gate", "Rectangle", geometry, "FSC-A", "SSC-A");
```

#### Ellipse Gates

Ellipse gates are elliptical regions with optional rotation:

```rust
let coords = vec![
    (300.0, 400.0),  // Center
    (500.0, 400.0),  // Right point (defines radius_x and angle)
    (300.0, 600.0),  // Top point (defines radius_y)
];
let geometry = create_ellipse_geometry(coords, "FSC-A", "SSC-A")?;

let gate = Gate::new("ellipse-gate", "Ellipse", geometry, "FSC-A", "SSC-A");
```

### Gate Modes

Gates can be scoped to apply globally or to specific files:

```rust
use flow_gates::GateMode;

// Global gate (applies to all files)
let global_gate = Gate::new(/* ... */);
// Gate mode defaults to Global

// File-specific gate
let mut file_gate = Gate::new(/* ... */);
file_gate.mode = GateMode::FileSpecific { guid: "file-123".into() };

// File group gate
let mut group_gate = Gate::new(/* ... */);
group_gate.mode = GateMode::FileGroup {
    guids: vec!["file-1".into(), "file-2".into()],
};
```

## Advanced Usage

### Gate Hierarchies

Gate hierarchies allow sequential gating where child gates are applied to events that pass parent gates:

```rust
use flow_gates::GateHierarchy;

let mut hierarchy = GateHierarchy::new();

// Build hierarchy: root -> parent -> child
hierarchy.add_child("root-gate", "parent-gate");
hierarchy.add_child("parent-gate", "child-gate");

// Get chain from root to a specific gate
let chain = hierarchy.get_chain_to_root("child-gate");
// Returns: ["root-gate", "parent-gate", "child-gate"]

// Get ancestors
let ancestors = hierarchy.get_ancestors("child-gate");
// Returns: ["parent-gate", "root-gate"]

// Get descendants
let descendants = hierarchy.get_descendants("root-gate");
// Returns: ["parent-gate", "child-gate"]
```

### Hierarchical Event Filtering

Filter events through a chain of gates:

```rust
use flow_gates::{filter_events_by_hierarchy, GateHierarchy};

// Build gate chain from hierarchy
let gate_chain: Vec<&Gate> = hierarchy
    .get_chain_to_root("child-gate")
    .iter()
    .filter_map(|id| storage.get(id.as_ref()))
    .collect();

// Filter through hierarchy
let indices = filter_events_by_hierarchy(&fcs, &gate_chain, None, None)?;
```

### Spatial Indexing for Performance

For repeated filtering operations, use a spatial index:

```rust
use flow_gates::{EventIndex, filter_events_by_gate};

// Build index once
let x_slice = fcs.get_parameter_events_slice("FSC-A")?;
let y_slice = fcs.get_parameter_events_slice("SSC-A")?;
let index = EventIndex::build(x_slice, y_slice)?;

// Reuse index for multiple gates (much faster!)
let indices1 = filter_events_by_gate(&fcs, &gate1, Some(&index))?;
let indices2 = filter_events_by_gate(&fcs, &gate2, Some(&index))?;
let indices3 = filter_events_by_gate(&fcs, &gate3, Some(&index))?;
```

### Gate Storage

Thread-safe gate storage with optional persistence:

```rust
use flow_gates::gate_storage::GateStorage;
use std::path::PathBuf;

// Create storage with auto-save
let storage = GateStorage::with_save_path(PathBuf::from("gates.json"));

// Load existing gates
storage.load()?;

// Insert gates
storage.insert(gate1);
storage.insert(gate2);

// Query gates
let file_gates = storage.gates_for_file("file-guid");
let param_gates = storage.gates_for_parameters("FSC-A", "SSC-A");
let specific_gates = storage.gates_for_file_and_parameters(
    "file-guid",
    "FSC-A",
    "SSC-A",
);

// Manual save (auto-save is enabled by default)
storage.save()?;
```

### GatingML Import/Export

Export gates to GatingML 2.0 format:

```rust
use flow_gates::gates_to_gatingml;

let gates = vec![gate1, gate2, gate3];
let xml = gates_to_gatingml(&gates)?;

// Save to file
std::fs::write("gates.xml", xml)?;
```

Import gates from GatingML format:

```rust
use flow_gates::gatingml_to_gates;

let xml = std::fs::read_to_string("gates.xml")?;
let gates = gatingml_to_gates(&xml)?;
```

## Application Integration Examples

### Example 1: Basic Gate Application

```rust
use flow_gates::*;
use flow_fcs::Fcs;

fn apply_gate_to_file(fcs_path: &str, gate: &Gate) -> Result<Vec<usize>> {
    // Load FCS file
    let fcs = Fcs::from_file(fcs_path)?;
    
    // Filter events
    let indices = filter_events_by_gate(&fcs, gate, None)?;
    
    Ok(indices)
}
```

### Example 2: Hierarchical Gating Pipeline

```rust
use flow_gates::*;
use flow_fcs::Fcs;

fn hierarchical_gating(
    fcs: &Fcs,
    hierarchy: &GateHierarchy,
    storage: &GateStorage,
    target_gate_id: &str,
) -> Result<Vec<usize>> {
    // Get gate chain from hierarchy
    let chain_ids = hierarchy.get_chain_to_root(target_gate_id);
    
    // Resolve gates from storage
    let gate_chain: Vec<&Gate> = chain_ids
        .iter()
        .filter_map(|id| storage.get(id.as_ref()))
        .collect();
    
    // Filter through hierarchy
    filter_events_by_hierarchy(fcs, &gate_chain, None, None)
}
```

### Example 3: Batch Processing with Caching

```rust
use flow_gates::*;
use flow_fcs::Fcs;
use std::sync::Arc;
use std::collections::HashMap;

struct SimpleFilterCache {
    cache: Arc<dashmap::DashMap<FilterCacheKey, Arc<Vec<usize>>>>,
}

impl FilterCache for SimpleFilterCache {
    fn get(&self, key: &FilterCacheKey) -> Option<Arc<Vec<usize>>> {
        self.cache.get(key).map(|entry| entry.value().clone())
    }
    
    fn insert(&self, key: FilterCacheKey, value: Arc<Vec<usize>>) {
        self.cache.insert(key, value);
    }
}

fn batch_process_with_cache(
    fcs: &Fcs,
    gates: &[Gate],
    file_guid: &str,
) -> Result<HashMap<String, Vec<usize>>> {
    let cache = SimpleFilterCache {
        cache: Arc::new(dashmap::DashMap::new()),
    };
    
    let mut results = HashMap::new();
    
    for gate in gates {
        let chain = vec![gate];
        let indices = filter_events_by_hierarchy(
            fcs,
            &chain,
            Some(&cache),
            Some(file_guid),
        )?;
        
        results.insert(gate.id.to_string(), indices);
    }
    
    Ok(results)
}
```

### Example 4: Statistics Dashboard

```rust
use flow_gates::*;
use flow_fcs::Fcs;

fn generate_statistics_report(
    fcs: &Fcs,
    gates: &[Gate],
) -> Result<Vec<(String, GateStatistics)>> {
    let mut report = Vec::new();
    
    for gate in gates {
        let stats = GateStatistics::calculate(fcs, gate)?;
        report.push((gate.name.clone(), stats));
    }
    
    Ok(report)
}

fn print_statistics_report(report: &[(String, GateStatistics)]) {
    for (name, stats) in report {
        println!("Gate: {}", name);
        println!("  Events: {}", stats.event_count);
        println!("  Percentage: {:.2}%", stats.percentage);
        println!("  Centroid: ({:.2}, {:.2})", stats.centroid.0, stats.centroid.1);
        println!("  X Parameter:");
        println!("    Mean: {:.2}", stats.x_stats.mean);
        println!("    Median: {:.2}", stats.x_stats.median);
        println!("    Std Dev: {:.2}", stats.x_stats.std_dev);
        println!("  Y Parameter:");
        println!("    Mean: {:.2}", stats.y_stats.mean);
        println!("    Median: {:.2}", stats.y_stats.median);
        println!("    Std Dev: {:.2}", stats.y_stats.std_dev);
        println!();
    }
}
```

### Example 5: Interactive Gate Editor Integration

```rust
use flow_gates::*;
use flow_gates::geometry::*;

// User draws polygon on plot
fn create_gate_from_user_drawing(
    points: Vec<(f32, f32)>,
    x_param: &str,
    y_param: &str,
    gate_id: &str,
    gate_name: &str,
) -> Result<Gate> {
    // Create geometry from user-drawn points
    let geometry = create_polygon_geometry(points, x_param, y_param)?;
    
    // Create gate
    let gate = Gate::new(gate_id, gate_name, geometry, x_param, y_param);
    
    // Validate
    if !gate.geometry.is_valid(x_param, y_param)? {
        return Err(GateError::invalid_geometry("Invalid gate geometry"));
    }
    
    Ok(gate)
}

// Update gate after user edits
fn update_gate_geometry(
    gate: &mut Gate,
    new_points: Vec<(f32, f32)>,
) -> Result<()> {
    let geometry = create_polygon_geometry(
        new_points,
        gate.x_parameter_channel_name(),
        gate.y_parameter_channel_name(),
    )?;
    
    gate.geometry = geometry;
    
    Ok(())
}
```

## Performance Considerations

### Spatial Indexing

For repeated filtering operations on the same dataset, use `EventIndex`:

- **Build time**: O(n log n) - one-time cost
- **Query time**: O(log n) per gate - much faster than O(n) linear scan
- **Memory**: O(n) - stores all event points

### Caching

Implement the `FilterCache` trait for your application to cache filter results:

```rust
use flow_gates::{FilterCache, FilterCacheKey};
use std::sync::Arc;

struct MyFilterCache {
    // Your cache implementation
}

impl FilterCache for MyFilterCache {
    fn get(&self, key: &FilterCacheKey) -> Option<Arc<Vec<usize>>> {
        // Retrieve from cache
    }
    
    fn insert(&self, key: FilterCacheKey, value: Arc<Vec<usize>>) {
        // Store in cache
    }
}
```

## Error Handling

The library uses `GateError` for all error conditions. Most operations return `Result<T, GateError>`:

```rust
use flow_gates::{GateError, Result};

match create_polygon_geometry(coords, "FSC-A", "SSC-A") {
    Ok(geometry) => {
        // Use geometry
    }
    Err(GateError::InvalidGeometry { message }) => {
        eprintln!("Invalid geometry: {}", message);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

## Thread Safety

Most types in this library are thread-safe:

- `GateStorage`: Thread-safe concurrent access
- `EventIndex`: Immutable after construction, safe to share
- `Gate`, `GateGeometry`, `GateNode`: Clone to share between threads
- `GateHierarchy`: Use synchronization primitives for concurrent access

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

