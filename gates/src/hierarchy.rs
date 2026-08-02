use crate::error::{GateError, Result};
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

    /// Add a root gate (gate with no parent)
    ///
    /// Ensures a gate is registered in the hierarchy as a root node.
    /// This is necessary for gates that don't have parents to appear in get_roots().
    pub fn add_root(&mut self, gate_id: impl Into<Arc<str>>) {
        let gate_id = gate_id.into();
        // Ensure the gate exists in the children map with an empty list
        // This makes it appear in get_roots() without having a parent
        self.children.entry(gate_id).or_default();
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
        if let Some(old_parent) = self.parents.get(&child_id)
            && let Some(siblings) = self.children.get_mut(old_parent)
        {
            siblings.retain(|id| id != &child_id);
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
        if let Some(parent_id) = self.parents.remove(gate_id)
            && let Some(siblings) = self.children.get_mut(&parent_id)
        {
            siblings.retain(|id| id.as_ref() != gate_id);
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

        for children in self.children.values() {
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

    /// Reparent a single gate to a new parent
    ///
    /// Moves a gate from its current parent to a new parent. This is equivalent
    /// to removing the gate from its current parent and adding it to the new parent.
    ///
    /// # Arguments
    /// * `gate_id` - The ID of the gate to reparent
    /// * `new_parent_id` - The ID of the new parent gate
    ///
    /// # Returns
    /// `Ok(())` if successful, or an error if:
    /// - The gate doesn't exist
    /// - The new parent doesn't exist
    /// - The operation would create a cycle
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_child("parent1", "child");
    /// hierarchy.reparent("child", "parent2")?;
    /// assert_eq!(hierarchy.get_parent("child").map(|s| s.as_ref()), Some("parent2"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn reparent(
        &mut self,
        gate_id: impl Into<Arc<str>>,
        new_parent_id: impl Into<Arc<str>>,
    ) -> Result<()> {
        let gate_id = gate_id.into();
        let new_parent_id = new_parent_id.into();

        // Check if gate exists (it might be a root)
        let all_gates: HashSet<Arc<str>> = self
            .children
            .keys()
            .chain(self.parents.keys())
            .cloned()
            .collect();

        if !all_gates.contains(&gate_id) {
            return Err(GateError::gate_not_found(
                gate_id.as_ref(),
                "gate not found in hierarchy",
            ));
        }

        if !all_gates.contains(&new_parent_id) && gate_id != new_parent_id {
            return Err(GateError::gate_not_found(
                new_parent_id.as_ref(),
                "new parent not found in hierarchy",
            ));
        }

        // Check for cycles
        if self.would_create_cycle(&new_parent_id, &gate_id) {
            return Err(GateError::hierarchy_cycle(
                new_parent_id.as_ref(),
                gate_id.as_ref(),
            ));
        }

        // Remove from current parent if it exists
        if let Some(old_parent) = self.parents.remove(&gate_id)
            && let Some(siblings) = self.children.get_mut(&old_parent)
        {
            siblings.retain(|id| id != &gate_id);
        }

        // Add to new parent
        self.children
            .entry(new_parent_id.clone())
            .or_default()
            .push(gate_id.clone());
        self.parents.insert(gate_id, new_parent_id);

        Ok(())
    }

    /// Reparent a gate and all its descendants to a new parent
    ///
    /// Moves an entire subtree (gate and all its descendants) to a new parent.
    /// This is useful for reorganizing large portions of the hierarchy.
    ///
    /// # Arguments
    /// * `gate_id` - The ID of the root gate of the subtree to move
    /// * `new_parent_id` - The ID of the new parent gate
    ///
    /// # Returns
    /// `Ok(())` if successful, or an error if the operation would create a cycle
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_child("parent1", "child");
    /// hierarchy.add_child("child", "grandchild");
    /// hierarchy.reparent_subtree("child", "parent2")?;
    /// // Both "child" and "grandchild" are now under "parent2"
    /// # Ok(())
    /// # }
    /// ```
    pub fn reparent_subtree(
        &mut self,
        gate_id: impl Into<Arc<str>>,
        new_parent_id: impl Into<Arc<str>>,
    ) -> Result<()> {
        let gate_id = gate_id.into();
        let new_parent_id = new_parent_id.into();

        // Get all descendants
        let descendants = self.get_descendants(gate_id.as_ref());

        // Check if new_parent_id is a descendant (would create cycle)
        if descendants.contains(&new_parent_id) {
            return Err(GateError::hierarchy_cycle(
                new_parent_id.as_ref(),
                gate_id.as_ref(),
            ));
        }

        // Check if gate_id is a descendant of new_parent_id (would create cycle)
        let new_parent_descendants = self.get_descendants(new_parent_id.as_ref());
        if new_parent_descendants.contains(&gate_id) {
            return Err(GateError::hierarchy_cycle(
                new_parent_id.as_ref(),
                gate_id.as_ref(),
            ));
        }

        // Reparent the root gate
        self.reparent(gate_id.as_ref(), new_parent_id.as_ref())?;

        Ok(())
    }

    /// Clone a subtree with new IDs
    ///
    /// Creates a copy of a subtree (gate and all its descendants) with new IDs
    /// generated by the provided mapper function. The cloned subtree is returned
    /// as a new hierarchy.
    ///
    /// # Arguments
    /// * `gate_id` - The ID of the root gate of the subtree to clone
    /// * `id_mapper` - Function that maps old IDs to new IDs
    ///
    /// # Returns
    /// A new `GateHierarchy` containing the cloned subtree, or an error if the gate doesn't exist
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_child("parent", "child");
    /// hierarchy.add_child("child", "grandchild");
    ///
    /// let cloned = hierarchy.clone_subtree("child", |id| format!("{}_copy", id))?;
    /// // cloned contains "child_copy" -> "grandchild_copy"
    /// # Ok(())
    /// # }
    /// ```
    pub fn clone_subtree<F>(&self, gate_id: &str, id_mapper: F) -> Result<Self>
    where
        F: Fn(&str) -> String,
    {
        let mut new_hierarchy = Self::new();

        // Get all nodes in subtree (including root)
        let mut subtree_nodes = vec![Arc::from(gate_id)];
        subtree_nodes.extend(self.get_descendants(gate_id));

        // Map old IDs to new IDs
        let id_map: HashMap<Arc<str>, Arc<str>> = subtree_nodes
            .iter()
            .map(|old_id| {
                let new_id = Arc::from(id_mapper(old_id.as_ref()).as_str());
                (old_id.clone(), new_id)
            })
            .collect();

        // Clone relationships
        for old_id in &subtree_nodes {
            if let Some(children) = self.children.get(old_id) {
                let new_parent_id = id_map.get(old_id).unwrap();
                for child in children {
                    if let Some(new_child_id) = id_map.get(child)
                        && !new_hierarchy.add_child(new_parent_id.clone(), new_child_id.clone())
                    {
                        return Err(GateError::hierarchy_error(
                            "Failed to add child in cloned hierarchy - possible cycle",
                        ));
                    }
                }
            }
        }

        Ok(new_hierarchy)
    }

    /// Move an entire subtree to a new parent
    ///
    /// This is an alias for `reparent_subtree` that makes the intent clearer.
    ///
    /// # Arguments
    /// * `gate_id` - The ID of the root gate of the subtree to move
    /// * `new_parent_id` - The ID of the new parent gate
    ///
    /// # Returns
    /// `Ok(())` if successful, or an error if the operation would create a cycle
    pub fn move_subtree(
        &mut self,
        gate_id: impl Into<Arc<str>>,
        new_parent_id: impl Into<Arc<str>>,
    ) -> Result<()> {
        self.reparent_subtree(gate_id, new_parent_id)
    }

    /// Delete a gate and all its descendants
    ///
    /// Removes a gate and all of its descendants from the hierarchy.
    /// Returns a list of all deleted gate IDs.
    ///
    /// # Arguments
    /// * `gate_id` - The ID of the gate to delete (along with all descendants)
    ///
    /// # Returns
    /// A vector of all deleted gate IDs (including the root gate and all descendants)
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_child("parent", "child");
    /// hierarchy.add_child("child", "grandchild");
    ///
    /// let deleted = hierarchy.delete_subtree("child");
    /// assert_eq!(deleted.len(), 2); // "child" and "grandchild"
    /// assert!(hierarchy.get_parent("child").is_none());
    /// ```
    pub fn delete_subtree(&mut self, gate_id: &str) -> Vec<Arc<str>> {
        let mut deleted = Vec::new();
        let mut to_delete = vec![Arc::from(gate_id)];

        // Collect all descendants
        to_delete.extend(self.get_descendants(gate_id));

        // Delete all nodes
        for id in &to_delete {
            self.remove_node(id.as_ref());
            deleted.push(id.clone());
        }

        deleted
    }

    /// Delete a gate but keep its children (reparent them)
    ///
    /// Removes a gate from the hierarchy but reparents all its children to
    /// a new parent (or makes them root nodes if no parent is specified).
    ///
    /// # Arguments
    /// * `gate_id` - The ID of the gate to delete
    /// * `new_parent_id` - Optional new parent for the children. If `None`, children become root nodes
    ///
    /// # Returns
    /// A vector of reparented child IDs, or an error if:
    /// - The gate doesn't exist
    /// - The new parent doesn't exist
    /// - Reparenting would create a cycle
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use std::sync::Arc;
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_child("parent", "child");
    /// hierarchy.add_child("child", "grandchild1");
    /// hierarchy.add_child("child", "grandchild2");
    ///
    /// let reparented = hierarchy.delete_node_keep_children("child", Some(Arc::from("parent")))?;
    /// assert_eq!(reparented.len(), 2);
    /// // grandchild1 and grandchild2 are now direct children of "parent"
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_node_keep_children(
        &mut self,
        gate_id: &str,
        new_parent_id: Option<Arc<str>>,
    ) -> Result<Vec<Arc<str>>> {
        // Get children before deletion
        let children: Vec<Arc<str>> = self
            .get_children(gate_id)
            .into_iter()
            .map(|id| id.clone())
            .collect();

        if children.is_empty() {
            // No children, just delete the node
            self.remove_node(gate_id);
            return Ok(Vec::new());
        }

        // Reparent children
        if let Some(ref new_parent) = new_parent_id {
            // Check if new parent exists
            let all_gates: HashSet<Arc<str>> = self
                .children
                .keys()
                .chain(self.parents.keys())
                .cloned()
                .collect();

            if !all_gates.contains(new_parent) && new_parent.as_ref() != gate_id {
                return Err(GateError::gate_not_found(
                    new_parent.as_ref(),
                    "new parent not found in hierarchy",
                ));
            }

            // Check for cycles
            for child in &children {
                if self.would_create_cycle(new_parent, child) {
                    return Err(GateError::hierarchy_cycle(
                        new_parent.as_ref(),
                        child.as_ref(),
                    ));
                }
            }

            // Reparent all children
            for child in &children {
                self.reparent(child.as_ref(), new_parent.clone())?;
            }
        } else {
            // Make children root nodes (remove their parent relationship)
            for child in &children {
                if self.parents.remove(child).is_some() {
                    // Also remove from old parent's children list
                    // (This is already handled by reparent, but we need to do it manually here)
                }
            }
        }

        // Now delete the node
        self.remove_node(gate_id);

        Ok(children)
    }

    /// Delete a single node, orphaning its children
    ///
    /// Removes a gate from the hierarchy. Its children become orphaned (root nodes).
    /// This is equivalent to `delete_node_keep_children(gate_id, None)`.
    ///
    /// # Arguments
    /// * `gate_id` - The ID of the gate to delete
    ///
    /// # Returns
    /// A vector of orphaned child IDs
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_child("parent", "child");
    /// hierarchy.add_child("child", "grandchild");
    ///
    /// let orphaned = hierarchy.delete_node("child")?;
    /// assert_eq!(orphaned.len(), 1); // "grandchild" is now orphaned
    /// assert!(hierarchy.is_root("grandchild"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_node(&mut self, gate_id: &str) -> Result<Vec<Arc<str>>> {
        self.delete_node_keep_children(gate_id, None)
    }

    /// Add a child-parent relationship, returning a Result
    ///
    /// This is a convenience wrapper around `add_child` that returns an error
    /// instead of `false` when the operation fails.
    ///
    /// # Arguments
    /// * `parent_id` - The ID of the parent gate
    /// * `child_id` - The ID of the child gate
    ///
    /// # Returns
    /// `Ok(())` if successful, or an error if the operation would create a cycle
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_gate_child("parent", "child")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_gate_child(
        &mut self,
        parent_id: impl Into<Arc<str>>,
        child_id: impl Into<Arc<str>>,
    ) -> Result<()> {
        let parent_id = parent_id.into();
        let child_id = child_id.into();

        if !self.add_child(parent_id.clone(), child_id.clone()) {
            return Err(GateError::hierarchy_cycle(
                parent_id.as_ref(),
                child_id.as_ref(),
            ));
        }

        Ok(())
    }

    /// Build a hierarchy from a list of gates and their relationships
    ///
    /// Creates a new hierarchy from a list of gates and their parent-child relationships.
    /// This is useful for constructing hierarchies programmatically.
    ///
    /// # Arguments
    /// * `relationships` - A slice of (parent_id, child_id) tuples defining the hierarchy
    ///
    /// # Returns
    /// A new `GateHierarchy` with the specified relationships, or an error if:
    /// - Any relationship would create a cycle
    /// - Relationships are invalid
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    /// use std::sync::Arc;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let relationships = vec![
    ///     (Arc::from("root"), Arc::from("child1")),
    ///     (Arc::from("root"), Arc::from("child2")),
    ///     (Arc::from("child1"), Arc::from("grandchild")),
    /// ];
    /// let hierarchy = GateHierarchy::from_relationships(&relationships)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_relationships(relationships: &[(Arc<str>, Arc<str>)]) -> Result<Self> {
        let mut hierarchy = Self::new();

        for (parent, child) in relationships {
            hierarchy.add_gate_child(parent.clone(), child.clone())?;
        }

        Ok(hierarchy)
    }

    /// Rebuild the hierarchy index from a set of gates, using each gate's
    /// `parent_id` field as the source of truth. This is the derived-index
    /// constructor: `Gate::parent_id` is authoritative (and serialized), while
    /// this index exists only for fast child/root/ancestor queries.
    ///
    /// Every gate is registered (roots via [`Self::add_root`], children via
    /// [`Self::add_gate_child`]). A `parent_id` referencing a gate not in the
    /// slice is treated as a root (defensive: a dangling parent shouldn't drop
    /// the gate from the tree). Returns an error only if the parent_ids form a
    /// cycle.
    pub fn from_gates(gates: &[crate::types::Gate]) -> Result<Self> {
        let mut hierarchy = Self::new();
        let ids: HashSet<&str> = gates.iter().map(|g| g.id.as_ref()).collect();

        for gate in gates {
            match &gate.parent_id {
                Some(parent) if ids.contains(parent.as_ref()) => {
                    hierarchy.add_gate_child(parent.clone(), gate.id.clone())?;
                }
                // None, or a dangling parent reference → treat as root.
                _ => hierarchy.add_root(gate.id.clone()),
            }
        }

        Ok(hierarchy)
    }

    /// Iterate gates in topological order (parents before children)
    ///
    /// Returns an iterator over gate IDs in topological order, where parents
    /// always come before their children. This is useful for processing gates
    /// in dependency order.
    ///
    /// # Returns
    /// An iterator over gate IDs in topological order, or an empty iterator if there are cycles
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_child("a", "b");
    /// hierarchy.add_child("a", "c");
    ///
    /// let order: Vec<_> = hierarchy.iter_topological().collect();
    /// // "a" will come before "b" and "c"
    /// ```
    pub fn iter_topological(&self) -> impl Iterator<Item = Arc<str>> {
        self.topological_sort().unwrap_or_default().into_iter()
    }

    /// Iterate gates in depth-first order starting from a root
    ///
    /// Returns an iterator over gate IDs in depth-first order, starting from
    /// the specified root gate and traversing down the tree.
    ///
    /// # Arguments
    /// * `root` - The root gate ID to start traversal from
    ///
    /// # Returns
    /// An iterator over gate IDs in depth-first order
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_child("root", "child1");
    /// hierarchy.add_child("root", "child2");
    /// hierarchy.add_child("child1", "grandchild");
    ///
    /// let order: Vec<_> = hierarchy.iter_dfs("root").collect();
    /// // Order: root, child1, grandchild, child2 (or similar DFS order)
    /// ```
    pub fn iter_dfs(&self, root: &str) -> impl Iterator<Item = Arc<str>> {
        let mut stack: Vec<Arc<str>> = vec![Arc::from(root)];
        let mut visited = HashSet::new();
        std::iter::from_fn(move || {
            while let Some(node) = stack.pop() {
                if visited.insert(node.clone()) {
                    // Add children to stack in reverse order to maintain left-to-right traversal
                    if let Some(children) = self.children.get(&node) {
                        for child in children.iter().rev() {
                            stack.push(child.clone());
                        }
                    }
                    return Some(node);
                }
            }
            None
        })
    }

    /// Validate the hierarchy structure
    ///
    /// Checks the hierarchy for common issues:
    /// - Cycles
    /// - Orphaned gates (gates with no parent and not in children map)
    /// - Inconsistent parent-child relationships
    ///
    /// # Returns
    /// `Ok(())` if the hierarchy is valid, or an error describing the issue
    ///
    /// # Example
    /// ```rust
    /// use flow_gates::GateHierarchy;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut hierarchy = GateHierarchy::new();
    /// hierarchy.add_child("parent", "child");
    /// hierarchy.validate()?; // Should pass
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate(&self) -> Result<()> {
        // Check for cycles using topological sort
        let all_gates: HashSet<Arc<str>> = self
            .children
            .keys()
            .chain(self.parents.keys())
            .cloned()
            .collect();

        if let Some(sorted) = self.topological_sort() {
            if sorted.len() != all_gates.len() {
                return Err(GateError::hierarchy_error(
                    "Topological sort failed - possible cycles detected",
                ));
            }
        } else {
            return Err(GateError::hierarchy_error("Cycles detected in hierarchy"));
        }

        // Check for inconsistent relationships
        for (parent, children) in &self.children {
            for child in children {
                if let Some(child_parent) = self.parents.get(child) {
                    if child_parent != parent {
                        return Err(GateError::hierarchy_error(format!(
                            "Inconsistent relationship: {} is child of {} but parent is {}",
                            child.as_ref(),
                            parent.as_ref(),
                            child_parent.as_ref()
                        )));
                    }
                } else {
                    return Err(GateError::hierarchy_error(format!(
                        "Child {} has no parent entry",
                        child.as_ref()
                    )));
                }
            }
        }

        Ok(())
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
