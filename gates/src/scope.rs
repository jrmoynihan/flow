use crate::hierarchy::GateHierarchy;
use crate::types::{Gate, GateMode};
use std::sync::Arc;

/// Builder for querying and filtering gates
///
/// `GateQuery` provides a fluent API for filtering gates by various criteria
/// such as parameters, scope, and other properties.
///
/// # Example
///
/// ```rust,no_run
/// use flow_gates::{GateQuery, Gate};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // In practice, you would get gates from storage
/// // let gate1 = /* ... */;
/// // let gate2 = /* ... */;
/// // let gate3 = /* ... */;
/// // let gates = vec![&gate1, &gate2, &gate3];
/// // let filtered: Vec<&Gate> = GateQuery::new(gates.iter().copied())
/// //     .by_parameters("FSC-A", "SSC-A")
/// //     .by_scope(Some("file-guid"))
/// //     .collect();
/// # Ok(())
/// # }
/// ```
pub struct GateQuery<'a> {
    gates: Vec<&'a Gate>,
}

impl<'a> GateQuery<'a> {
    /// Create a new query builder from an iterator of gates
    ///
    /// # Arguments
    /// * `gates` - Iterator over gate references
    pub fn new(gates: impl Iterator<Item = &'a Gate>) -> Self {
        Self {
            gates: gates.collect(),
        }
    }

    /// Filter gates by parameters (channels)
    ///
    /// Only gates that operate on the specified x and y parameters are retained.
    ///
    /// # Arguments
    /// * `x` - X-axis parameter name
    /// * `y` - Y-axis parameter name
    pub fn by_parameters(mut self, x: &str, y: &str) -> Self {
        self.gates.retain(|gate| {
            gate.x_parameter_channel_name() == x && gate.y_parameter_channel_name() == y
        });
        self
    }

    /// Filter gates by scope (file GUID)
    ///
    /// Only gates that apply to the specified file (or are global) are retained.
    ///
    /// # Arguments
    /// * `file_guid` - Optional file GUID. If `None`, only global gates are returned.
    pub fn by_scope(mut self, file_guid: Option<&str>) -> Self {
        match file_guid {
            Some(guid) => self.gates.retain(|gate| gate.mode.applies_to(guid)),
            None => self.gates.retain(|gate| matches!(gate.mode, GateMode::Global)),
        }
        self
    }

    /// Collect the filtered gates into a vector
    ///
    /// # Returns
    /// A vector of gate references matching the query criteria
    pub fn collect(self) -> Vec<&'a Gate> {
        self.gates
    }
}

impl<'a> IntoIterator for GateQuery<'a> {
    type Item = &'a Gate;
    type IntoIter = std::vec::IntoIter<&'a Gate>;

    fn into_iter(self) -> Self::IntoIter {
        self.gates.into_iter()
    }
}

/// Helper functions for managing gate types
impl GateMode {
    /// Create a global type
    pub fn global() -> Self {
        GateMode::Global
    }

    /// Create a file-specific type
    pub fn file_specific(guid: impl Into<Arc<str>>) -> Self {
        GateMode::FileSpecific { guid: guid.into() }
    }

    /// Create a file group type
    pub fn file_group(guids: Vec<impl Into<Arc<str>>>) -> Self {
        GateMode::FileGroup {
            guids: guids.into_iter().map(|g| g.into()).collect(),
        }
    }

    /// Add a file to a group type (no-op for Global or FileSpecific)
    pub fn add_file(&mut self, guid: impl Into<Arc<str>>) {
        if let GateMode::FileGroup { guids } = self {
            let guid = guid.into();
            if !guids.contains(&guid) {
                guids.push(guid);
            }
        }
    }

    /// Remove a file from a group type (no-op for Global or FileSpecific)
    pub fn remove_file(&mut self, guid: &str) {
        if let GateMode::FileGroup { guids } = self {
            guids.retain(|g| g.as_ref() != guid);
        }
    }

    /// Get the list of file GUIDs this type applies to (None for Global)
    pub fn file_guids(&self) -> Option<Vec<&str>> {
        match self {
            GateMode::Global => None,
            GateMode::FileSpecific { guid } => Some(vec![guid.as_ref()]),
            GateMode::FileGroup { guids } => Some(guids.iter().map(|g| g.as_ref()).collect()),
        }
    }
}

/// Filter gates by type (scope)
///
/// This is a convenience function that filters gates by their scope.
/// Consider using `GateQuery` for more complex filtering needs.
///
/// # Arguments
/// * `gates` - Iterator over gate references
/// * `file_guid` - Optional file GUID. If `None`, only global gates are returned.
///
/// # Returns
/// A vector of gates matching the scope criteria
pub fn filter_gates_by_type<'a>(
    gates: impl Iterator<Item = &'a Gate>,
    file_guid: Option<&str>,
) -> Vec<&'a Gate> {
    match file_guid {
        Some(guid) => gates.filter(|gate| gate.mode.applies_to(guid)).collect(),
        None => gates
            .filter(|gate| matches!(gate.mode, GateMode::Global))
            .collect(),
    }
}

/// Filter gates by parameters (channels)
///
/// Returns only gates that operate on the specified x and y parameters.
///
/// # Arguments
/// * `gates` - Iterator over gate references
/// * `x_param` - X-axis parameter name
/// * `y_param` - Y-axis parameter name
///
/// # Returns
/// A vector of gates operating on the specified parameters
///
/// # Example
/// ```rust,no_run
/// use flow_gates::{filter_gates_by_parameters, Gate};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // In practice, you would get gates from storage
/// // let gate1 = /* ... */;
/// // let gate2 = /* ... */;
/// // let gate3 = /* ... */;
/// // let gates = vec![&gate1, &gate2, &gate3];
/// // let fsc_ssc_gates = filter_gates_by_parameters(gates.iter().copied(), "FSC-A", "SSC-A");
/// # Ok(())
/// # }
/// ```
pub fn filter_gates_by_parameters<'a>(
    gates: impl Iterator<Item = &'a Gate>,
    x_param: &str,
    y_param: &str,
) -> Vec<&'a Gate> {
    gates
        .filter(|gate| {
            gate.x_parameter_channel_name() == x_param && gate.y_parameter_channel_name() == y_param
        })
        .collect()
}

/// Filter gates by scope
///
/// Returns gates that apply to the specified file (or are global if `file_guid` is `None`).
///
/// # Arguments
/// * `gates` - Iterator over gate references
/// * `file_guid` - Optional file GUID. If `None`, only global gates are returned.
///
/// # Returns
/// A vector of gates matching the scope criteria
///
/// # Example
/// ```rust,no_run
/// use flow_gates::{filter_gates_by_scope, Gate};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // In practice, you would get gates from storage
/// // let gate1 = /* ... */;
/// // let gate2 = /* ... */;
/// // let gate3 = /* ... */;
/// // let gates = vec![&gate1, &gate2, &gate3];
/// // let file_gates = filter_gates_by_scope(gates.iter().copied(), Some("file-123"));
/// // let global_gates = filter_gates_by_scope(gates.iter().copied(), None);
/// # Ok(())
/// # }
/// ```
pub fn filter_gates_by_scope<'a>(
    gates: impl Iterator<Item = &'a Gate>,
    file_guid: Option<&str>,
) -> Vec<&'a Gate> {
    filter_gates_by_type(gates, file_guid)
}

/// Filter a hierarchy by parameters, rebuilding relationships
///
/// Creates a new hierarchy containing only gates that operate on the specified
/// parameters, preserving parent-child relationships where both gates match.
///
/// # Arguments
/// * `hierarchy` - The original hierarchy
/// * `gates_map` - Map of gate IDs to gate references
/// * `x_param` - X-axis parameter name
/// * `y_param` - Y-axis parameter name
///
/// # Returns
/// A new hierarchy containing only gates matching the parameters
///
/// # Example
/// ```rust,no_run
/// use flow_gates::{filter_hierarchy_by_parameters, GateHierarchy, Gate};
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let hierarchy = GateHierarchy::new();
/// let mut gates_map: HashMap<Arc<str>, &Gate> = HashMap::new();
/// // ... populate gates_map ...
///
/// let filtered = filter_hierarchy_by_parameters(&hierarchy, &gates_map, "FSC-A", "SSC-A");
/// # Ok(())
/// # }
/// ```
pub fn filter_hierarchy_by_parameters(
    hierarchy: &GateHierarchy,
    gates_map: &std::collections::HashMap<Arc<str>, &Gate>,
    x_param: &str,
    y_param: &str,
) -> GateHierarchy {
    let mut new_hierarchy = GateHierarchy::new();

    // Get all gates matching parameters
    let matching_gates: std::collections::HashSet<Arc<str>> = gates_map
        .iter()
        .filter(|(_, gate)| {
            gate.x_parameter_channel_name() == x_param
                && gate.y_parameter_channel_name() == y_param
        })
        .map(|(id, _)| id.clone())
        .collect();

    // Rebuild relationships for matching gates using public API
    // Get all gates in hierarchy and check their relationships
    if let Some(sorted_gates) = hierarchy.topological_sort() {
        for gate_id in sorted_gates {
            if matching_gates.contains(&gate_id) {
                // Check if this gate has a parent that also matches
                if let Some(parent_id) = hierarchy.get_parent(gate_id.as_ref()) {
                    if matching_gates.contains(parent_id) {
                        let _ = new_hierarchy.add_child(parent_id.clone(), gate_id.clone());
                    }
                }
            }
        }
    }

    new_hierarchy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_type_applies_to_all() {
        let gate_type = GateMode::global();
        assert!(gate_type.applies_to("any-guid"));
        assert!(gate_type.applies_to("another-guid"));
    }

    #[test]
    fn test_file_specific_type() {
        let gate_type = GateMode::file_specific("test-guid");
        assert!(gate_type.applies_to("test-guid"));
        assert!(!gate_type.applies_to("other-guid"));
    }

    #[test]
    fn test_file_group_type() {
        let mut gate_type = GateMode::file_group(vec!["guid1", "guid2"]);
        assert!(gate_type.applies_to("guid1"));
        assert!(gate_type.applies_to("guid2"));
        assert!(!gate_type.applies_to("guid3"));

        gate_type.add_file("guid3");
        assert!(gate_type.applies_to("guid3"));

        gate_type.remove_file("guid1");
        assert!(!gate_type.applies_to("guid1"));
    }
}
