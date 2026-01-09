use crate::types::{Gate, GateMode};
use std::sync::Arc;

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

/// Filter gates by type
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
