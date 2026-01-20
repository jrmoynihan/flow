//! Gate linking system for tracking gate references.
//!
//! This module provides `GateLinks`, a structure for tracking which gates reference
//! (link to) other gates. This is separate from the hierarchy system - links represent
//! gate reuse/references (e.g., in boolean gates), not parent-child relationships.
//!
//! # Linking vs Hierarchy
//!
//! - **Hierarchy**: Parent-child relationships for sequential gating (filtering order)
//! - **Linking**: Gate references for reuse (e.g., boolean gates referencing other gates)
//!
//! # Example
//!
//! ```rust
//! use flow_gates::GateLinks;
//!
//! let mut links = GateLinks::new();
//! links.add_link("target-gate", "boolean-gate");
//! links.add_link("target-gate", "another-gate");
//!
//! let referencing_gates = links.get_links("target-gate");
//! assert_eq!(referencing_gates.len(), 2);
//! ```

use std::collections::HashMap;
use std::sync::Arc;

/// Manages gate linking relationships.
///
/// `GateLinks` tracks which gates reference (link to) other gates. This is useful
/// for tracking gate reuse, especially in boolean operations where gates reference
/// other gates.
///
/// Unlike `GateHierarchy`, links don't represent filtering order - they represent
/// references/reuse of gate definitions.
#[derive(Debug, Clone, Default)]
pub struct GateLinks {
    /// Maps target gate ID to list of gate IDs that reference/link to it
    links: HashMap<Arc<str>, Vec<Arc<str>>>,
}

impl GateLinks {
    /// Create a new empty `GateLinks` structure
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateLinks;
    ///
    /// let links = GateLinks::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a link from a linking gate to a target gate
    ///
    /// This records that `linking_gate_id` references/links to `target_gate_id`.
    ///
    /// # Arguments
    /// * `target_gate_id` - The gate being referenced/linked to
    /// * `linking_gate_id` - The gate that references the target gate
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateLinks;
    ///
    /// let mut links = GateLinks::new();
    /// links.add_link("target", "linker1");
    /// links.add_link("target", "linker2");
    /// ```
    pub fn add_link(
        &mut self,
        target_gate_id: impl Into<Arc<str>>,
        linking_gate_id: impl Into<Arc<str>>,
    ) {
        let target_gate_id = target_gate_id.into();
        let linking_gate_id = linking_gate_id.into();

        self.links
            .entry(target_gate_id)
            .or_default()
            .push(linking_gate_id);
    }

    /// Remove a link from a linking gate to a target gate
    ///
    /// Removes the record that `linking_gate_id` references `target_gate_id`.
    /// If the link doesn't exist, this is a no-op.
    ///
    /// # Arguments
    /// * `target_gate_id` - The gate being referenced
    /// * `linking_gate_id` - The gate that references the target gate
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateLinks;
    ///
    /// let mut links = GateLinks::new();
    /// links.add_link("target", "linker");
    /// links.remove_link("target", "linker");
    /// assert!(!links.is_linked("target"));
    /// ```
    pub fn remove_link(
        &mut self,
        target_gate_id: &str,
        linking_gate_id: &str,
    ) {
        if let Some(linkers) = self.links.get_mut(target_gate_id) {
            linkers.retain(|id| id.as_ref() != linking_gate_id);
            if linkers.is_empty() {
                self.links.remove(target_gate_id);
            }
        }
    }

    /// Get all gates that link to the specified gate
    ///
    /// Returns a vector of gate IDs that reference the target gate.
    ///
    /// # Arguments
    /// * `gate_id` - The gate ID to get links for
    ///
    /// # Returns
    /// A vector of gate IDs that link to the specified gate
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateLinks;
    ///
    /// let mut links = GateLinks::new();
    /// links.add_link("target", "linker1");
    /// links.add_link("target", "linker2");
    ///
    /// let linkers = links.get_links("target");
    /// assert_eq!(linkers.len(), 2);
    /// ```
    pub fn get_links(&self, gate_id: &str) -> Vec<&Arc<str>> {
        self.links
            .get(gate_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check if a gate is linked (has any gates referencing it)
    ///
    /// # Arguments
    /// * `gate_id` - The gate ID to check
    ///
    /// # Returns
    /// `true` if any gates link to this gate, `false` otherwise
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateLinks;
    ///
    /// let mut links = GateLinks::new();
    /// assert!(!links.is_linked("target"));
    ///
    /// links.add_link("target", "linker");
    /// assert!(links.is_linked("target"));
    /// ```
    pub fn is_linked(&self, gate_id: &str) -> bool {
        self.links
            .get(gate_id)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    /// Get the number of gates linking to the specified gate
    ///
    /// # Arguments
    /// * `gate_id` - The gate ID to get link count for
    ///
    /// # Returns
    /// The number of gates that link to the specified gate
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateLinks;
    ///
    /// let mut links = GateLinks::new();
    /// links.add_link("target", "linker1");
    /// links.add_link("target", "linker2");
    ///
    /// assert_eq!(links.get_link_count("target"), 2);
    /// ```
    pub fn get_link_count(&self, gate_id: &str) -> usize {
        self.links
            .get(gate_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Clear all links
    ///
    /// Removes all linking relationships.
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateLinks;
    ///
    /// let mut links = GateLinks::new();
    /// links.add_link("target", "linker");
    /// links.clear();
    /// assert!(!links.is_linked("target"));
    /// ```
    pub fn clear(&mut self) {
        self.links.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_link() {
        let mut links = GateLinks::new();
        links.add_link("target", "linker1");
        links.add_link("target", "linker2");

        assert!(links.is_linked("target"));
        assert_eq!(links.get_link_count("target"), 2);
    }

    #[test]
    fn test_remove_link() {
        let mut links = GateLinks::new();
        links.add_link("target", "linker1");
        links.add_link("target", "linker2");

        links.remove_link("target", "linker1");

        assert!(links.is_linked("target"));
        assert_eq!(links.get_link_count("target"), 1);
    }

    #[test]
    fn test_get_links() {
        let mut links = GateLinks::new();
        links.add_link("target", "linker1");
        links.add_link("target", "linker2");

        let linkers = links.get_links("target");
        assert_eq!(linkers.len(), 2);
    }

    #[test]
    fn test_is_linked() {
        let mut links = GateLinks::new();
        assert!(!links.is_linked("target"));

        links.add_link("target", "linker");
        assert!(links.is_linked("target"));
    }

    #[test]
    fn test_get_link_count() {
        let mut links = GateLinks::new();
        assert_eq!(links.get_link_count("target"), 0);

        links.add_link("target", "linker1");
        links.add_link("target", "linker2");
        assert_eq!(links.get_link_count("target"), 2);
    }

    #[test]
    fn test_clear() {
        let mut links = GateLinks::new();
        links.add_link("target", "linker");
        links.clear();

        assert!(!links.is_linked("target"));
        assert_eq!(links.get_link_count("target"), 0);
    }
}
