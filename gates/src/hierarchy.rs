use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Manages the hierarchical relationships between gates.
///
/// Gate hierarchies represent parent-child relationships where child gates
/// are applied to events that pass through their parent gates. This enables
/// sequential gating strategies common in flow cytometry analysis.
///
/// The hierarchy is represented as a directed acyclic graph (DAG), preventing
/// cycles while allowing multiple parents per child (though this implementation
/// currently supports single-parent hierarchies).
///
/// # Example
///
/// ```rust
/// use flow_gates::GateHierarchy;
///
/// let mut hierarchy = GateHierarchy::new();
///
/// // Build hierarchy: root -> parent -> child
/// hierarchy.add_child("root", "parent");
/// hierarchy.add_child("parent", "child");
///
/// // Get ancestors
/// let ancestors = hierarchy.get_ancestors("child");
/// assert_eq!(ancestors.len(), 2);
///
/// // Get chain from root to child
/// let chain = hierarchy.get_chain_to_root("child");
/// assert_eq!(chain.len(), 3);
///
/// // Prevent cycles
/// assert!(!hierarchy.add_child("child", "root")); // Would create cycle
/// ```
#[derive(Debug, Clone, Default)]
pub struct GateHierarchy {
    /// Maps parent gate ID to list of child gate IDs
    children: HashMap<Arc<str>, Vec<Arc<str>>>,
    /// Maps child gate ID to parent gate ID
    parents: HashMap<Arc<str>, Arc<str>>,
}

impl GateHierarchy {
    /// Create a new empty hierarchy
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a child-parent relationship
    ///
    /// Returns `true` if the relationship was added, `false` if it would create a cycle
    pub fn add_child(
        &mut self,
        parent_id: impl Into<Arc<str>>,
        child_id: impl Into<Arc<str>>,
    ) -> bool {
        let parent_id = parent_id.into();
        let child_id = child_id.into();

        // Check for cycles before adding
        if self.would_create_cycle(&parent_id, &child_id) {
            return false;
        }

        // Remove child from previous parent if it exists
        if let Some(old_parent) = self.parents.get(&child_id) {
            if let Some(siblings) = self.children.get_mut(old_parent) {
                siblings.retain(|id| id != &child_id);
            }
        }

        // Add new relationship
        self.children
            .entry(parent_id.clone())
            .or_default()
            .push(child_id.clone());
        self.parents.insert(child_id, parent_id);

        true
    }

    /// Remove a gate and all its relationships
    ///
    /// Children of the removed gate become orphans (no parent)
    pub fn remove_node(&mut self, gate_id: &str) {
        // Remove as a child
        if let Some(parent_id) = self.parents.remove(gate_id) {
            if let Some(siblings) = self.children.get_mut(&parent_id) {
                siblings.retain(|id| id.as_ref() != gate_id);
            }
        }

        // Remove as a parent (orphan the children)
        if let Some(child_ids) = self.children.remove(gate_id) {
            for child_id in child_ids {
                self.parents.remove(&child_id);
            }
        }
    }

    /// Remove a parent-child relationship
    pub fn remove_child(&mut self, parent_id: &str, child_id: &str) {
        if let Some(children) = self.children.get_mut(parent_id) {
            children.retain(|id| id.as_ref() != child_id);
        }

        if self.parents.get(child_id).map(|p| p.as_ref()) == Some(parent_id) {
            self.parents.remove(child_id);
        }
    }

    /// Get the parent of a gate
    pub fn get_parent(&self, gate_id: &str) -> Option<&Arc<str>> {
        self.parents.get(gate_id)
    }

    /// Get the children of a gate
    pub fn get_children(&self, gate_id: &str) -> Vec<&Arc<str>> {
        self.children
            .get(gate_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Get all ancestors of a gate (parent, grandparent, etc.) in order from closest to root
    pub fn get_ancestors(&self, gate_id: &str) -> Vec<Arc<str>> {
        let mut ancestors = Vec::new();
        let mut current = gate_id;

        while let Some(parent) = self.parents.get(current) {
            ancestors.push(parent.clone());
            current = parent.as_ref();
        }

        ancestors
    }

    /// Get all descendants of a gate (children, grandchildren, etc.)
    pub fn get_descendants(&self, gate_id: &str) -> Vec<Arc<str>> {
        let mut descendants = Vec::new();
        let mut queue = VecDeque::new();

        if let Some(children) = self.children.get(gate_id) {
            for child in children {
                queue.push_back(child.clone());
            }
        }

        while let Some(node) = queue.pop_front() {
            descendants.push(node.clone());

            if let Some(children) = self.children.get(&node) {
                for child in children {
                    queue.push_back(child.clone());
                }
            }
        }

        descendants
    }

    /// Get the full chain from root to this gate (including the gate itself)
    pub fn get_chain_to_root(&self, gate_id: &str) -> Vec<Arc<str>> {
        let mut chain = self.get_ancestors(gate_id);
        chain.reverse(); // Root first
        chain.push(Arc::from(gate_id));
        chain
    }

    /// Get all root gates (gates with no parents)
    pub fn get_roots(&self) -> Vec<Arc<str>> {
        let all_gates: HashSet<_> = self.children.keys().chain(self.parents.keys()).collect();

        all_gates
            .into_iter()
            .filter(|gate_id| !self.parents.contains_key(*gate_id))
            .cloned()
            .collect()
    }

    /// Perform a topological sort of the gates
    ///
    /// Returns gates in an order where parents come before children
    /// Returns None if there are cycles
    pub fn topological_sort(&self) -> Option<Vec<Arc<str>>> {
        let mut result = Vec::new();
        let mut in_degree: HashMap<Arc<str>, usize> = HashMap::new();
        let mut queue = VecDeque::new();

        // Collect all gates
        let all_gates: HashSet<Arc<str>> = self
            .children
            .keys()
            .chain(self.parents.keys())
            .cloned()
            .collect();

        // Calculate in-degrees
        for gate in &all_gates {
            in_degree.insert(gate.clone(), 0);
        }

        for (_, children) in &self.children {
            for child in children {
                *in_degree.entry(child.clone()).or_insert(0) += 1;
            }
        }

        // Find gates with no incoming edges (roots)
        for (gate, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(gate.clone());
            }
        }

        // Process queue
        while let Some(gate) = queue.pop_front() {
            result.push(gate.clone());

            if let Some(children) = self.children.get(&gate) {
                for child in children {
                    if let Some(degree) = in_degree.get_mut(child) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(child.clone());
                        }
                    }
                }
            }
        }

        // Check if all gates were processed (no cycles)
        if result.len() == all_gates.len() {
            Some(result)
        } else {
            None // Cycle detected
        }
    }

    /// Check if adding a parent-child relationship would create a cycle
    fn would_create_cycle(&self, parent_id: &Arc<str>, child_id: &Arc<str>) -> bool {
        // If parent is already a descendant of child, adding this edge would create a cycle
        let descendants = self.get_descendants(child_id.as_ref());
        descendants.contains(parent_id)
    }

    /// Get the depth of a gate in the hierarchy (root = 0)
    pub fn get_depth(&self, gate_id: &str) -> usize {
        self.get_ancestors(gate_id).len()
    }

    /// Check if a gate is a root (has no parent)
    pub fn is_root(&self, gate_id: &str) -> bool {
        !self.parents.contains_key(gate_id)
    }

    /// Check if a gate is a leaf (has no children)
    pub fn is_leaf(&self, gate_id: &str) -> bool {
        self.children
            .get(gate_id)
            .map(|c| c.is_empty())
            .unwrap_or(true)
    }

    /// Get all leaf gates (gates with no children)
    pub fn get_leaves(&self) -> Vec<Arc<str>> {
        let all_gates: HashSet<Arc<str>> = self
            .children
            .keys()
            .chain(self.parents.keys())
            .cloned()
            .collect();

        all_gates
            .into_iter()
            .filter(|gate_id| self.is_leaf(gate_id.as_ref()))
            .collect()
    }

    /// Clear all relationships
    pub fn clear(&mut self) {
        self.children.clear();
        self.parents.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_child() {
        let mut hierarchy = GateHierarchy::new();
        assert!(hierarchy.add_child("parent", "child"));

        assert_eq!(
            hierarchy.get_parent("child").map(|s| s.as_ref()),
            Some("parent")
        );
        assert_eq!(hierarchy.get_children("parent").len(), 1);
    }

    #[test]
    fn test_cycle_detection() {
        let mut hierarchy = GateHierarchy::new();
        hierarchy.add_child("a", "b");
        hierarchy.add_child("b", "c");

        // Try to create a cycle: c -> a
        assert!(!hierarchy.add_child("c", "a"));
    }

    #[test]
    fn test_ancestors() {
        let mut hierarchy = GateHierarchy::new();
        hierarchy.add_child("root", "parent");
        hierarchy.add_child("parent", "child");
        hierarchy.add_child("child", "grandchild");

        let ancestors = hierarchy.get_ancestors("grandchild");
        assert_eq!(ancestors.len(), 3);
        assert_eq!(ancestors[0].as_ref(), "child");
        assert_eq!(ancestors[1].as_ref(), "parent");
        assert_eq!(ancestors[2].as_ref(), "root");
    }

    #[test]
    fn test_descendants() {
        let mut hierarchy = GateHierarchy::new();
        hierarchy.add_child("root", "child1");
        hierarchy.add_child("root", "child2");
        hierarchy.add_child("child1", "grandchild1");

        let descendants = hierarchy.get_descendants("root");
        assert_eq!(descendants.len(), 3);
    }

    #[test]
    fn test_topological_sort() {
        let mut hierarchy = GateHierarchy::new();
        hierarchy.add_child("a", "b");
        hierarchy.add_child("a", "c");
        hierarchy.add_child("b", "d");

        let sorted = hierarchy.topological_sort().expect("no cycles");

        // "a" should come before "b" and "c"
        let a_pos = sorted.iter().position(|s| s.as_ref() == "a").expect("a");
        let b_pos = sorted.iter().position(|s| s.as_ref() == "b").expect("b");
        let c_pos = sorted.iter().position(|s| s.as_ref() == "c").expect("c");
        let d_pos = sorted.iter().position(|s| s.as_ref() == "d").expect("d");

        assert!(a_pos < b_pos);
        assert!(a_pos < c_pos);
        assert!(b_pos < d_pos);
    }

    #[test]
    fn test_remove_node() {
        let mut hierarchy = GateHierarchy::new();
        hierarchy.add_child("parent", "child1");
        hierarchy.add_child("child1", "grandchild");

        hierarchy.remove_node("child1");

        assert!(hierarchy.get_parent("child1").is_none());
        assert!(hierarchy.get_children("parent").is_empty());
        assert!(hierarchy.get_parent("grandchild").is_none()); // Orphaned
    }

    #[test]
    fn test_depth_and_roots() {
        let mut hierarchy = GateHierarchy::new();
        hierarchy.add_child("root1", "child1");
        hierarchy.add_child("root2", "child2");
        hierarchy.add_child("child1", "grandchild");

        assert_eq!(hierarchy.get_depth("root1"), 0);
        assert_eq!(hierarchy.get_depth("child1"), 1);
        assert_eq!(hierarchy.get_depth("grandchild"), 2);

        let roots = hierarchy.get_roots();
        assert_eq!(roots.len(), 2);
        assert!(roots.iter().any(|s| s.as_ref() == "root1"));
        assert!(roots.iter().any(|s| s.as_ref() == "root2"));
    }
}
