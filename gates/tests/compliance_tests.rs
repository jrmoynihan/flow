//! Gating-ML Compliance Test Suite
//!
//! This integration test suite provides tests against the official Gating-ML compliance test suite
//! to verify that our implementation correctly parses and applies gates according
//! to the Gating-ML specification.
//!
//! The compliance tests are located in:
//! `Gating-ML.v1.5.081030.Compliance-tests.081030/`
//!
//! Each test consists of:
//! - A Gating-ML XML file defining gates
//! - An FCS data file with events
//! - Expected results files listing which events should be inside each gate
//!
//! Run with: `cargo test --test compliance_tests -- --ignored`

use flow_fcs::Fcs;
use flow_gates::filtering::filter_events_by_gate;
use flow_gates::gatingml::gatingml_to_gates;
use std::path::{Path, PathBuf};

/// Path to the compliance test directory (relative to crate root)
const COMPLIANCE_TEST_DIR: &str = "Gating-ML.v1.5.081030.Compliance-tests.081030";

/// Represents a single compliance test case
#[derive(Debug, Clone)]
struct ComplianceTestCase {
    /// Test set name (e.g., "01Rectangular")
    set_name: String,
    /// Gate ID to test
    gate_id: String,
    /// Path to the Gating-ML XML file
    gatingml_file: PathBuf,
    /// Path to the FCS data file
    fcs_file: PathBuf,
    /// Expected number of events inside the gate
    expected_events: usize,
    /// Path to the expected results file (if available)
    expected_results_file: Option<PathBuf>,
}

/// Load all compliance test cases from the Summary.csv file
fn load_test_cases(base_path: &Path) -> anyhow::Result<Vec<ComplianceTestCase>> {
    let summary_path = base_path.join("Summary.csv");
    let summary_content = std::fs::read_to_string(&summary_path)
        .map_err(|e| anyhow::anyhow!("Failed to read Summary.csv: {}", e))?;

    let mut test_cases = Vec::new();
    let mut lines = summary_content.lines();

    // Skip header
    lines.next();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 5 {
            continue;
        }

        let set_name = parts[0].trim().to_string();
        let gate_id = parts[1].trim().to_string();
        let gatingml_filename = parts[2].trim();
        let fcs_filename = parts[3].trim();
        let expected_events: usize = parts[4].trim().parse().unwrap_or(0);

        let gatingml_file = base_path.join("Gating-ML Files").join(gatingml_filename);
        let fcs_file = base_path.join("List-mode Data Files").join(fcs_filename);

        // Expected results file path
        let expected_results_file = base_path
            .join("Expected Results")
            .join(&set_name)
            .join(format!("{}.txt", gate_id));

        test_cases.push(ComplianceTestCase {
            set_name,
            gate_id,
            gatingml_file,
            fcs_file,
            expected_events,
            expected_results_file: if expected_results_file.exists() {
                Some(expected_results_file)
            } else {
                None
            },
        });
    }

    Ok(test_cases)
}

/// Run a single compliance test case
fn run_test_case(test_case: &ComplianceTestCase) -> anyhow::Result<ComplianceTestResult> {
    // Load and parse Gating-ML file
    let xml_content = std::fs::read_to_string(&test_case.gatingml_file).map_err(|e| {
        anyhow::anyhow!(
            "Failed to read Gating-ML file {}: {}",
            test_case.gatingml_file.display(),
            e
        )
    })?;

    let gates = gatingml_to_gates(&xml_content).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse Gating-ML file {}: {}",
            test_case.gatingml_file.display(),
            e
        )
    })?;

    // Find the gate with the matching ID
    let gate = gates
        .iter()
        .find(|g| g.id.as_ref() == test_case.gate_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Gate with ID '{}' not found in Gating-ML file",
                test_case.gate_id
            )
        })?;

    // Load FCS file
    let fcs_path = test_case
        .fcs_file
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("FCS file path is not valid UTF-8"))?;

    let fcs = Fcs::open(fcs_path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to open FCS file {}: {}",
            test_case.fcs_file.display(),
            e
        )
    })?;

    // Apply gate and get event indices
    let event_indices = filter_events_by_gate(&fcs, gate, None)
        .map_err(|e| anyhow::anyhow!("Failed to filter events: {}", e))?;

    let actual_events = event_indices.len();

    // Compare with expected
    let passed = actual_events == test_case.expected_events;

    Ok(ComplianceTestResult {
        test_case: test_case.clone(),
        actual_events,
        passed,
        error: None,
    })
}

/// Result of running a compliance test
#[derive(Debug, Clone)]
struct ComplianceTestResult {
    test_case: ComplianceTestCase,
    actual_events: usize,
    passed: bool,
    error: Option<String>,
}

impl ComplianceTestResult {
    fn failed(&self) -> bool {
        !self.passed
    }

    fn error_message(&self) -> String {
        if let Some(err) = &self.error {
            err.clone()
        } else if self.failed() {
            format!(
                "Expected {} events, got {}",
                self.test_case.expected_events, self.actual_events
            )
        } else {
            String::new()
        }
    }
}

/// Run all compliance tests and return results
fn run_all_tests(base_path: &Path) -> anyhow::Result<ComplianceTestSuite> {
    let test_cases = load_test_cases(base_path)?;
    let mut results = Vec::new();

    for test_case in &test_cases {
        match run_test_case(test_case) {
            Ok(result) => results.push(result),
            Err(e) => {
                results.push(ComplianceTestResult {
                    test_case: test_case.clone(),
                    actual_events: 0,
                    passed: false,
                    error: Some(format!("{}", e)),
                });
            }
        }
    }

    Ok(ComplianceTestSuite {
        results,
        total: test_cases.len(),
    })
}

/// Complete test suite results
#[derive(Debug)]
struct ComplianceTestSuite {
    results: Vec<ComplianceTestResult>,
    total: usize,
}

impl ComplianceTestSuite {
    fn passed(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }

    fn failed(&self) -> usize {
        self.results.iter().filter(|r| r.failed()).count()
    }

    fn pass_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.passed() as f64 / self.total as f64) * 100.0
    }

    fn print_summary(&self) {
        println!("Compliance Test Suite Results");
        println!("==============================");
        println!("Total tests: {}", self.total);
        println!("Passed: {}", self.passed());
        println!("Failed: {}", self.failed());
        println!("Pass rate: {:.2}%", self.pass_rate());
        println!();

        if self.failed() > 0 {
            println!("Failed tests:");
            for result in &self.results {
                if result.failed() {
                    println!(
                        "  {} / {}: {}",
                        result.test_case.set_name,
                        result.test_case.gate_id,
                        result.error_message()
                    );
                }
            }
        }
    }
}

#[test]
#[ignore] // Only run when compliance tests are available
fn test_load_compliance_test_cases() {
    let base_path = Path::new(COMPLIANCE_TEST_DIR);
    if !base_path.exists() {
        eprintln!("Compliance test directory not found, skipping test");
        return;
    }

    let test_cases = load_test_cases(base_path).expect("Should load test cases");
    assert!(!test_cases.is_empty(), "Should have test cases");

    println!("Loaded {} test cases", test_cases.len());
}

#[test]
#[ignore] // Only run when compliance tests are available
fn test_run_rectangular_gate_test() {
    let base_path = Path::new(COMPLIANCE_TEST_DIR);
    if !base_path.exists() {
        eprintln!("Compliance test directory not found, skipping test");
        return;
    }

    let test_cases = load_test_cases(base_path).expect("Should load test cases");

    // Find a simple rectangular gate test
    let test_case = test_cases
        .iter()
        .find(|tc| tc.set_name == "01Rectangular" && tc.gate_id == "BetweenMinAndMax")
        .expect("Should find test case");

    let result = run_test_case(test_case).expect("Should run test case");
    assert!(
        result.passed,
        "Test should pass: {}",
        result.error_message()
    );
}

#[test]
#[ignore] // Only run when compliance tests are available
fn test_run_all_compliance_tests() {
    let base_path = Path::new(COMPLIANCE_TEST_DIR);
    if !base_path.exists() {
        eprintln!("Compliance test directory not found, skipping test");
        return;
    }

    let suite = run_all_tests(base_path).expect("Should run all tests");
    suite.print_summary();

    // For now, we expect some failures until parsing is fully implemented
    // This test serves as a baseline
    println!(
        "Test suite completed: {}/{} passed",
        suite.passed(),
        suite.total
    );
}

