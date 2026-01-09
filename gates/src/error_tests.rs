#[cfg(test)]
mod tests {
    use crate::error::{GateError, Result};
    use crate::types::{GateGeometry, GateNode};

    #[test]
    fn test_invalid_geometry_error() {
        let error = GateError::invalid_geometry("Polygon has no valid coordinates");
        assert!(matches!(error, GateError::InvalidGeometry { .. }));
        assert!(error.to_string().contains("Invalid geometry"));
        assert!(error.to_string().contains("Polygon has no valid coordinates"));
    }

    #[test]
    fn test_missing_parameter_error() {
        let error = GateError::missing_parameter("FSC-A", "Polygon center");
        assert!(matches!(error, GateError::MissingParameter { .. }));
        assert!(error.to_string().contains("Missing parameter"));
        assert!(error.to_string().contains("FSC-A"));
        assert!(error.to_string().contains("Polygon center"));
    }

    #[test]
    fn test_invalid_coordinate_error() {
        let error = GateError::invalid_coordinate("x", f32::NAN);
        assert!(matches!(error, GateError::InvalidCoordinate { .. }));
        assert!(error.to_string().contains("Invalid coordinate"));
        assert!(error.to_string().contains("x"));
    }

    #[test]
    fn test_filtering_error() {
        let error = GateError::filtering_error("Failed to find parameter FSC-A");
        assert!(matches!(error, GateError::FilteringError { .. }));
        assert!(error.to_string().contains("Filtering error"));
        assert!(error.to_string().contains("Failed to find parameter FSC-A"));
    }

    #[test]
    fn test_hierarchy_error() {
        let error = GateError::hierarchy_error("Gate not found in hierarchy");
        assert!(matches!(error, GateError::HierarchyError { .. }));
        assert!(error.to_string().contains("Hierarchy error"));
        assert!(error.to_string().contains("Gate not found in hierarchy"));
    }

    #[test]
    fn test_index_error() {
        let error = GateError::index_error("X and Y arrays must have the same length");
        assert!(matches!(error, GateError::IndexError { .. }));
        assert!(error.to_string().contains("Index error"));
        assert!(error.to_string().contains("X and Y arrays must have the same length"));
    }

    #[test]
    fn test_rectangle_missing_parameter() {
        let min_node = GateNode::new("min").with_coordinate("x", 10.0);
        // Missing y coordinate
        let max_node = GateNode::new("max")
            .with_coordinate("x", 30.0)
            .with_coordinate("y", 40.0);

        let rect = GateGeometry::Rectangle {
            min: min_node,
            max: max_node,
        };

        let result = rect.calculate_center("x", "y");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GateError::MissingParameter { .. }
        ));
    }

    #[test]
    fn test_ellipse_missing_parameter() {
        let center_node = GateNode::new("center").with_coordinate("x", 50.0);
        // Missing y coordinate

        let ellipse = GateGeometry::Ellipse {
            center: center_node,
            radius_x: 20.0,
            radius_y: 30.0,
            angle: 0.0,
        };

        let result = ellipse.calculate_center("x", "y");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GateError::MissingParameter { .. }
        ));
    }

    #[test]
    fn test_polygon_no_valid_coordinates() {
        // Polygon with nodes but no valid coordinates for the parameters
        let node1 = GateNode::new("node1").with_coordinate("other_param", 10.0);
        let node2 = GateNode::new("node2").with_coordinate("other_param", 20.0);

        let polygon = GateGeometry::Polygon {
            nodes: vec![node1, node2],
            closed: true,
        };

        let result = polygon.calculate_center("x", "y");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GateError::InvalidGeometry { .. }
        ));
    }

    #[test]
    fn test_polygon_empty_nodes() {
        let polygon = GateGeometry::Polygon {
            nodes: vec![],
            closed: true,
        };

        let result = polygon.calculate_center("x", "y");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GateError::InvalidGeometry { .. }
        ));
    }

    #[test]
    fn test_rectangle_invalid_bounds() {
        // Rectangle where min > max
        let min_node = GateNode::new("min")
            .with_coordinate("x", 30.0)
            .with_coordinate("y", 40.0);
        let max_node = GateNode::new("max")
            .with_coordinate("x", 10.0)
            .with_coordinate("y", 20.0);

        let rect = GateGeometry::Rectangle {
            min: min_node,
            max: max_node,
        };

        let result = rect.is_valid("x", "y");
        assert!(result.is_ok());
        // Should return false for invalid rectangle
        assert!(!result.unwrap());
    }

    #[test]
    fn test_ellipse_invalid_radius() {
        let center_node = GateNode::new("center")
            .with_coordinate("x", 50.0)
            .with_coordinate("y", 60.0);

        let ellipse = GateGeometry::Ellipse {
            center: center_node,
            radius_x: -10.0, // Negative radius
            radius_y: 30.0,
            angle: 0.0,
        };

        let result = ellipse.is_valid("x", "y");
        assert!(result.is_ok());
        // Should return false for invalid ellipse
        assert!(!result.unwrap());
    }

    #[test]
    fn test_error_with_context() {
        let error = GateError::invalid_geometry("Polygon has no valid coordinates");
        let error_with_context = error.with_context("While processing gate 'test-gate'");

        assert!(error_with_context.to_string().contains("While processing gate 'test-gate'"));
        assert!(error_with_context.to_string().contains("Polygon has no valid coordinates"));
    }

    #[test]
    fn test_error_display() {
        let errors = vec![
            GateError::invalid_geometry("Test message"),
            GateError::missing_parameter("param", "context"),
            GateError::invalid_coordinate("coord", 42.0),
            GateError::filtering_error("Filter failed"),
            GateError::hierarchy_error("Hierarchy issue"),
            GateError::index_error("Index problem"),
        ];

        for error in errors {
            let display = error.to_string();
            assert!(!display.is_empty());
            // All errors should have descriptive messages
            assert!(display.len() > 10);
        }
    }

    #[test]
    fn test_serialization_error_conversion() {
        // Test that serde_json::Error converts to GateError
        let invalid_json = "{ invalid json }";
        let serde_result: std::result::Result<serde_json::Value, serde_json::Error> =
            serde_json::from_str(invalid_json);
        assert!(serde_result.is_err());

        // Convert to GateError using From trait
        let gate_error: GateError = serde_result.unwrap_err().into();
        assert!(matches!(gate_error, GateError::SerializationError(_)));
    }

    #[test]
    fn test_event_index_build_error() {
        // Test EventIndex build with mismatched array lengths
        use crate::filtering::EventIndex;

        let x_values = vec![1.0, 2.0, 3.0];
        let y_values = vec![1.0, 2.0]; // Different length

        let result = EventIndex::build(&x_values, &y_values);
        assert!(result.is_err());
        // Check error type by matching on the result
        match result {
            Err(GateError::IndexError { .. }) => {
                // Expected error type
            }
            Err(e) => panic!("Expected IndexError, got: {}", e),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }
}

