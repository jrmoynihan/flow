#[cfg(test)]
mod tests {
    use crate::types::{GateGeometry, GateNode};

    #[test]
    fn test_rectangle_center() {
        let min_node = GateNode::new("min")
            .with_coordinate("x", 10.0)
            .with_coordinate("y", 20.0);
        let max_node = GateNode::new("max")
            .with_coordinate("x", 30.0)
            .with_coordinate("y", 40.0);

        let rect = GateGeometry::Rectangle {
            min: min_node,
            max: max_node,
        };

        let center = rect.calculate_center("x", "y").unwrap();
        assert_eq!(center, (20.0, 30.0));
    }

    #[test]
    fn test_rectangle_containment() {
        let min_node = GateNode::new("min")
            .with_coordinate("x", 10.0)
            .with_coordinate("y", 20.0);
        let max_node = GateNode::new("max")
            .with_coordinate("x", 30.0)
            .with_coordinate("y", 40.0);

        let rect = GateGeometry::Rectangle {
            min: min_node,
            max: max_node,
        };

        // Point inside
        assert!(rect.contains_point(20.0, 30.0, "x", "y").unwrap());

        // Point outside
        assert!(!rect.contains_point(5.0, 30.0, "x", "y").unwrap());
        assert!(!rect.contains_point(20.0, 50.0, "x", "y").unwrap());

        // Point on boundary
        assert!(rect.contains_point(10.0, 20.0, "x", "y").unwrap());
        assert!(rect.contains_point(30.0, 40.0, "x", "y").unwrap());
    }

    #[test]
    fn test_rectangle_bounds() {
        let min_node = GateNode::new("min")
            .with_coordinate("x", 10.0)
            .with_coordinate("y", 20.0);
        let max_node = GateNode::new("max")
            .with_coordinate("x", 30.0)
            .with_coordinate("y", 40.0);

        let rect = GateGeometry::Rectangle {
            min: min_node,
            max: max_node,
        };

        let bounds = rect.bounding_box("x", "y").unwrap();
        assert_eq!(bounds, (10.0, 20.0, 30.0, 40.0));
    }

    #[test]
    fn test_rectangle_validation() {
        // Valid rectangle
        let min_node = GateNode::new("min")
            .with_coordinate("x", 10.0)
            .with_coordinate("y", 20.0);
        let max_node = GateNode::new("max")
            .with_coordinate("x", 30.0)
            .with_coordinate("y", 40.0);

        let rect = GateGeometry::Rectangle {
            min: min_node,
            max: max_node,
        };

        assert!(rect.is_valid("x", "y").unwrap());

        // Invalid rectangle (min > max)
        let invalid_min = GateNode::new("min")
            .with_coordinate("x", 30.0)
            .with_coordinate("y", 40.0);
        let invalid_max = GateNode::new("max")
            .with_coordinate("x", 10.0)
            .with_coordinate("y", 20.0);

        let invalid_rect = GateGeometry::Rectangle {
            min: invalid_min,
            max: invalid_max,
        };

        assert!(!invalid_rect.is_valid("x", "y").unwrap());
    }

    #[test]
    fn test_ellipse_center() {
        let center_node = GateNode::new("center")
            .with_coordinate("x", 50.0)
            .with_coordinate("y", 60.0);

        let ellipse = GateGeometry::Ellipse {
            center: center_node,
            radius_x: 20.0,
            radius_y: 30.0,
            angle: 0.0,
        };

        let center = ellipse.calculate_center("x", "y").unwrap();
        assert_eq!(center, (50.0, 60.0));
    }

    #[test]
    fn test_ellipse_containment() {
        let center_node = GateNode::new("center")
            .with_coordinate("x", 50.0)
            .with_coordinate("y", 60.0);

        let ellipse = GateGeometry::Ellipse {
            center: center_node,
            radius_x: 20.0,
            radius_y: 30.0,
            angle: 0.0,
        };

        // Point inside
        assert!(ellipse.contains_point(50.0, 60.0, "x", "y").unwrap());
        assert!(ellipse.contains_point(60.0, 60.0, "x", "y").unwrap());
        assert!(ellipse.contains_point(50.0, 80.0, "x", "y").unwrap());

        // Point outside
        assert!(!ellipse.contains_point(80.0, 60.0, "x", "y").unwrap());
        assert!(!ellipse.contains_point(50.0, 100.0, "x", "y").unwrap());
    }

    #[test]
    fn test_ellipse_validation() {
        // Valid ellipse
        let center_node = GateNode::new("center")
            .with_coordinate("x", 50.0)
            .with_coordinate("y", 60.0);

        let ellipse = GateGeometry::Ellipse {
            center: center_node,
            radius_x: 20.0,
            radius_y: 30.0,
            angle: 0.0,
        };

        assert!(ellipse.is_valid("x", "y").unwrap());

        // Invalid ellipse (negative radius)
        let invalid_center_node = GateNode::new("center")
            .with_coordinate("x", 50.0)
            .with_coordinate("y", 60.0);

        let invalid_ellipse = GateGeometry::Ellipse {
            center: invalid_center_node,
            radius_x: -10.0,
            radius_y: 30.0,
            angle: 0.0,
        };

        assert!(!invalid_ellipse.is_valid("x", "y").unwrap());
    }

    #[test]
    fn test_rotated_ellipse_bounds() {
        let angle = std::f32::consts::FRAC_PI_4;
        let center_node = GateNode::new("center")
            .with_coordinate("x", 50.0)
            .with_coordinate("y", 60.0);

        let ellipse = GateGeometry::Ellipse {
            center: center_node,
            radius_x: 20.0,
            radius_y: 10.0,
            angle,
        };

        let bounds = ellipse.bounding_box("x", "y").unwrap();
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();
        let extent_x = ((20.0 * cos_angle).powi(2) + (10.0 * sin_angle).powi(2)).sqrt();
        let extent_y = ((20.0 * sin_angle).powi(2) + (10.0 * cos_angle).powi(2)).sqrt();

        assert!((bounds.0 - (50.0 - extent_x)).abs() < 1e-5);
        assert!((bounds.1 - (60.0 - extent_y)).abs() < 1e-5);
        assert!((bounds.2 - (50.0 + extent_x)).abs() < 1e-5);
        assert!((bounds.3 - (60.0 + extent_y)).abs() < 1e-5);
    }

    #[test]
    fn test_polygon_center() {
        let nodes = vec![
            GateNode::new("node1")
                .with_coordinate("x", 10.0)
                .with_coordinate("y", 10.0),
            GateNode::new("node2")
                .with_coordinate("x", 30.0)
                .with_coordinate("y", 10.0),
            GateNode::new("node3")
                .with_coordinate("x", 20.0)
                .with_coordinate("y", 30.0),
        ];

        let polygon = GateGeometry::Polygon {
            nodes,
            closed: true,
        };

        let center = polygon.calculate_center("x", "y").unwrap();
        assert_eq!(center, (20.0, 16.6666665)); // (10+30+20)/3, (10+10+30)/3
    }

    #[test]
    fn test_polygon_validation() {
        // Valid polygon (3+ nodes)
        let nodes = vec![
            GateNode::new("node1")
                .with_coordinate("x", 10.0)
                .with_coordinate("y", 10.0),
            GateNode::new("node2")
                .with_coordinate("x", 30.0)
                .with_coordinate("y", 10.0),
            GateNode::new("node3")
                .with_coordinate("x", 20.0)
                .with_coordinate("y", 30.0),
        ];

        let polygon = GateGeometry::Polygon {
            nodes,
            closed: true,
        };

        assert!(polygon.is_valid("x", "y").unwrap());

        // Invalid polygon (< 3 nodes)
        let invalid_nodes = vec![
            GateNode::new("node1")
                .with_coordinate("x", 10.0)
                .with_coordinate("y", 10.0),
        ];

        let invalid_polygon = GateGeometry::Polygon {
            nodes: invalid_nodes,
            closed: true,
        };

        assert!(!invalid_polygon.is_valid("x", "y").unwrap());
    }
}
