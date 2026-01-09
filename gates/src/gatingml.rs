use crate::types::{Gate, GateGeometry, GateNode};
use anyhow::{Context, Result};
use quick_xml::{
    Reader, Writer,
    events::{BytesEnd, BytesStart, Event},
};
use std::io::Cursor;

/// GatingML XML namespace constants
const GATINGML_NS_V2: &str = "http://www.isac-net.org/std/Gating-ML/v2.0/gating";
const GATINGML_NS_V1_5: &str = "http://www.isac-net.org/std/Gating-ML/v1.5/gating";
const DATA_TYPE_NS_V2: &str = "http://www.isac-net.org/std/Gating-ML/v2.0/datatypes";
const DATA_TYPE_NS_V1_5: &str = "http://www.isac-net.org/std/Gating-ML/v1.5/datatypes";

/// Detect Gating-ML version from XML content
fn detect_version(xml: &str) -> (String, String) {
    if xml.contains("v2.0") {
        (GATINGML_NS_V2.to_string(), DATA_TYPE_NS_V2.to_string())
    } else {
        (GATINGML_NS_V1_5.to_string(), DATA_TYPE_NS_V1_5.to_string())
    }
}

/// Convert gates to GatingML 2.0 XML format.
///
/// This function serializes gates to the GatingML 2.0 standard XML format,
/// which is widely used for exchanging gate definitions between flow cytometry
/// analysis tools.
///
/// # Arguments
///
/// * `gates` - A slice of gates to convert to GatingML format
///
/// # Returns
///
/// A string containing the GatingML XML representation of the gates.
///
/// # Example
///
/// ```rust
/// use flow_gates::{gates_to_gatingml, Gate};
///
/// let gates = vec![/* ... gates ... */];
/// let xml = gates_to_gatingml(&gates)?;
///
/// // Save to file
/// std::fs::write("gates.xml", xml)?;
/// ```
pub fn gates_to_gatingml(gates: &[Gate]) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // Write XML declaration
    writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        None,
    )))?;

    // Write root element with namespaces
    let mut gatingml_start = BytesStart::new("gating:Gating-ML");
    gatingml_start.push_attribute(("xmlns:gating", GATINGML_NS_V2));
    gatingml_start.push_attribute(("xmlns:data-type", DATA_TYPE_NS_V2));
    gatingml_start.push_attribute(("xmlns", GATINGML_NS_V2));
    writer.write_event(Event::Start(gatingml_start))?;

    // Write gates
    for gate in gates {
        write_gate_to_xml(&mut writer, gate)?;
    }

    // Close root element
    writer.write_event(Event::End(BytesEnd::new("gating:Gating-ML")))?;

    let result = writer.into_inner().into_inner();
    String::from_utf8(result).context("Failed to convert XML bytes to string")
}

/// Write a single gate to XML
fn write_gate_to_xml(writer: &mut Writer<Cursor<Vec<u8>>>, gate: &Gate) -> Result<()> {
    // Write gate element
    let mut gate_start = BytesStart::new("gating:Gate");
    gate_start.push_attribute(("gating:id", gate.id.as_ref()));
    gate_start.push_attribute(("gating:name", gate.name.as_str()));
    writer.write_event(Event::Start(gate_start))?;

    // Write gate type based on geometry
    match &gate.geometry {
        GateGeometry::Polygon { nodes, closed } => {
            write_polygon_gate(writer, nodes, *closed)?;
        }
        GateGeometry::Rectangle { min, max } => {
            write_rectangle_gate(writer, min, max)?;
        }
        GateGeometry::Ellipse {
            center,
            radius_x,
            radius_y,
            angle,
        } => {
            write_ellipse_gate(writer, center, *radius_x, *radius_y, *angle)?;
        }
    }

    // Close gate element
    writer.write_event(Event::End(BytesEnd::new("gating:Gate")))?;

    Ok(())
}

/// Write a polygon gate to XML
fn write_polygon_gate(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    nodes: &[GateNode],
    closed: bool,
) -> Result<()> {
    let mut polygon_start = BytesStart::new("gating:PolygonGate");
    if closed {
        polygon_start.push_attribute(("gating:closed", "true"));
    }
    writer.write_event(Event::Start(polygon_start))?;

    // Write vertices
    for node in nodes {
        write_vertex(writer, node)?;
    }

    writer.write_event(Event::End(BytesEnd::new("gating:PolygonGate")))?;
    Ok(())
}

/// Write a rectangle gate to XML
fn write_rectangle_gate(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    min: &GateNode,
    max: &GateNode,
) -> Result<()> {
    let rect_start = BytesStart::new("gating:RectangleGate");
    writer.write_event(Event::Start(rect_start))?;

    // Write min vertex
    write_vertex(writer, min)?;

    // Write max vertex
    write_vertex(writer, max)?;

    writer.write_event(Event::End(BytesEnd::new("gating:RectangleGate")))?;
    Ok(())
}

/// Write an ellipse gate to XML
fn write_ellipse_gate(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    center: &GateNode,
    radius_x: f32,
    radius_y: f32,
    angle: f32,
) -> Result<()> {
    let mut ellipse_start = BytesStart::new("gating:EllipseGate");
    ellipse_start.push_attribute(("gating:radiusX", radius_x.to_string().as_str()));
    ellipse_start.push_attribute(("gating:radiusY", radius_y.to_string().as_str()));
    ellipse_start.push_attribute(("gating:angle", angle.to_string().as_str()));
    writer.write_event(Event::Start(ellipse_start))?;

    // Write center vertex
    write_vertex(writer, center)?;

    writer.write_event(Event::End(BytesEnd::new("gating:EllipseGate")))?;
    Ok(())
}

/// Write a vertex (gate node) to XML
fn write_vertex(writer: &mut Writer<Cursor<Vec<u8>>>, node: &GateNode) -> Result<()> {
    let vertex_start = BytesStart::new("gating:vertex");
    writer.write_event(Event::Start(vertex_start))?;

    // Write coordinates
    for (param, value) in &node.coordinates {
        let mut coord_start = BytesStart::new("gating:coordinate");
        coord_start.push_attribute(("gating:parameter", param.as_ref()));
        coord_start.push_attribute(("gating:value", value.to_string().as_str()));
        writer.write_event(Event::Empty(coord_start))?;
    }

    writer.write_event(Event::End(BytesEnd::new("gating:vertex")))?;
    Ok(())
}

/// Parse GatingML XML format to gates (supports both v1.5 and v2.0).
///
/// This function deserializes gates from GatingML XML format, allowing
/// import of gate definitions from other flow cytometry analysis tools.
///
/// # Arguments
///
/// * `xml` - A string containing GatingML XML data
///
/// # Returns
///
/// A vector of gates parsed from the XML.
///
/// # Example
///
/// ```rust
/// use flow_gates::gatingml_to_gates;
///
/// // Read from file
/// let xml = std::fs::read_to_string("gates.xml")?;
/// let gates = gatingml_to_gates(&xml)?;
///
/// // Use the gates
/// for gate in gates {
///     println!("Gate: {}", gate.name);
/// }
/// ```
pub fn gatingml_to_gates(xml: &str) -> Result<Vec<Gate>> {
    let (gating_ns, _data_ns) = detect_version(xml);
    let is_v2 = gating_ns.contains("v2.0");

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut gates = Vec::new();
    let mut buf = Vec::new();
    let mut depth = 0u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let name = e.name();

                // Handle v2.0 format: gates wrapped in <gating:Gate>
                if name.as_ref() == b"gating:Gate" || name.as_ref() == b"Gate" {
                    if let Some(gate) = parse_gate_v2(&mut reader, e, &mut depth)? {
                        gates.push(gate);
                    }
                }
                // Handle v1.5 format: direct gate elements
                else if name.as_ref() == b"gating:RectangleGate"
                    || name.as_ref() == b"RectangleGate"
                {
                    if let Some(gate) = parse_rectangle_gate_v1_5(&mut reader, e, &mut depth)? {
                        gates.push(gate);
                    }
                } else if name.as_ref() == b"gating:PolygonGate" || name.as_ref() == b"PolygonGate"
                {
                    if let Some(gate) = parse_polygon_gate_v1_5(&mut reader, e, &mut depth)? {
                        gates.push(gate);
                    }
                } else if name.as_ref() == b"gating:EllipseGate" || name.as_ref() == b"EllipseGate"
                {
                    if let Some(gate) = parse_ellipse_gate_v1_5(&mut reader, e, &mut depth)? {
                        gates.push(gate);
                    }
                }
            }
            Ok(Event::End(_)) => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
        buf.clear();
    }

    Ok(gates)
}

/// Parse a gate in v2.0 format (wrapped in <gating:Gate>)
fn parse_gate_v2(
    reader: &mut Reader<&[u8]>,
    gate_start: &BytesStart,
    depth: &mut u32,
) -> Result<Option<Gate>> {
    let id = gate_start
        .attributes()
        .find(|attr| {
            let attr = attr.as_ref().unwrap();
            attr.key.as_ref() == b"gating:id" || attr.key.as_ref() == b"id"
        })
        .and_then(|attr| String::from_utf8(attr.unwrap().value.into_owned()).ok())
        .context("Gate missing id attribute")?;

    let name = gate_start
        .attributes()
        .find(|attr| {
            let attr = attr.as_ref().unwrap();
            attr.key.as_ref() == b"gating:name" || attr.key.as_ref() == b"name"
        })
        .and_then(|attr| String::from_utf8(attr.unwrap().value.into_owned()).ok())
        .unwrap_or_else(|| id.clone());

    // Parse gate geometry inside
    let geometry = parse_gate_geometry_v2(reader, depth)?;

    if let Some(geom) = geometry {
        // Extract parameters from geometry
        let (x_param, y_param) = extract_parameters_from_geometry(&geom)?;

        Ok(Some(Gate::new(id, name, geom, x_param, y_param)))
    } else {
        Ok(None)
    }
}

/// Parse gate geometry in v2.0 format
fn parse_gate_geometry_v2(
    reader: &mut Reader<&[u8]>,
    depth: &mut u32,
) -> Result<Option<GateGeometry>> {
    let start_depth = *depth;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                *depth += 1;
                let name = e.name();

                if name.as_ref() == b"gating:PolygonGate" || name.as_ref() == b"PolygonGate" {
                    return Ok(Some(parse_polygon_geometry_v2(reader, e, depth)?));
                } else if name.as_ref() == b"gating:RectangleGate"
                    || name.as_ref() == b"RectangleGate"
                {
                    return Ok(Some(parse_rectangle_geometry_v2(reader, e, depth)?));
                } else if name.as_ref() == b"gating:EllipseGate" || name.as_ref() == b"EllipseGate"
                {
                    return Ok(Some(parse_ellipse_geometry_v2(reader, e, depth)?));
                }
            }
            Ok(Event::End(_)) => {
                if *depth <= start_depth {
                    break;
                }
                *depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
        buf.clear();
    }

    Ok(None)
}

/// Parse rectangle gate in v1.5 format
fn parse_rectangle_gate_v1_5(
    reader: &mut Reader<&[u8]>,
    gate_start: &BytesStart,
    depth: &mut u32,
) -> Result<Option<Gate>> {
    let id = gate_start
        .attributes()
        .find(|attr| {
            let attr = attr.as_ref().unwrap();
            attr.key.as_ref() == b"gating:id" || attr.key.as_ref() == b"id"
        })
        .and_then(|attr| String::from_utf8(attr.unwrap().value.into_owned()).ok())
        .context("RectangleGate missing id attribute")?;

    let start_depth = *depth;
    let mut dimensions = Vec::new();
    let mut min_coords: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    let mut max_coords: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                *depth += 1;
                let name = e.name();

                if name.as_ref() == b"gating:dimension" || name.as_ref() == b"dimension" {
                    let (param, min_val, max_val) = parse_dimension_v1_5(reader, e, depth)?;
                    let param_clone = param.clone();
                    dimensions.push(param_clone.clone());
                    if let Some(min) = min_val {
                        min_coords.insert(param_clone.clone(), min);
                    }
                    if let Some(max) = max_val {
                        max_coords.insert(param_clone, max);
                    }
                }
            }
            Ok(Event::End(_)) => {
                if *depth <= start_depth {
                    break;
                }
                *depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
        buf.clear();
    }

    // Rectangle gates can have 1 or more dimensions
    // For 1D gates, we'll use the same parameter for both x and y
    // For 2D+ gates, we use the first two dimensions
    if dimensions.is_empty() {
        return Ok(None);
    }

    let x_param = dimensions[0].clone();
    let y_param = if dimensions.len() >= 2 {
        dimensions[1].clone()
    } else {
        dimensions[0].clone() // Use same parameter for 1D gates
    };

    let min_x = min_coords
        .get(&x_param)
        .copied()
        .unwrap_or(f32::NEG_INFINITY);
    let min_y = min_coords
        .get(&y_param)
        .copied()
        .unwrap_or(f32::NEG_INFINITY);
    let max_x = max_coords.get(&x_param).copied().unwrap_or(f32::INFINITY);
    let max_y = max_coords.get(&y_param).copied().unwrap_or(f32::INFINITY);

    let min_node = GateNode::new("min")
        .with_coordinate(x_param.clone(), min_x)
        .with_coordinate(y_param.clone(), min_y);
    let max_node = GateNode::new("max")
        .with_coordinate(x_param.clone(), max_x)
        .with_coordinate(y_param.clone(), max_y);

    let id_clone = id.clone();
    Ok(Some(Gate::new(
        id,
        id_clone, // Use ID as name if no name provided
        GateGeometry::Rectangle {
            min: min_node,
            max: max_node,
        },
        x_param,
        y_param,
    )))
}

/// Parse dimension element in v1.5 format
fn parse_dimension_v1_5(
    reader: &mut Reader<&[u8]>,
    dim_start: &BytesStart,
    depth: &mut u32,
) -> Result<(String, Option<f32>, Option<f32>)> {
    let min_val = dim_start
        .attributes()
        .find(|attr| {
            let attr = attr.as_ref().unwrap();
            attr.key.as_ref() == b"gating:min" || attr.key.as_ref() == b"min"
        })
        .and_then(|attr| {
            String::from_utf8(attr.unwrap().value.into_owned())
                .ok()?
                .parse::<f32>()
                .ok()
        });

    let max_val = dim_start
        .attributes()
        .find(|attr| {
            let attr = attr.as_ref().unwrap();
            attr.key.as_ref() == b"gating:max" || attr.key.as_ref() == b"max"
        })
        .and_then(|attr| {
            String::from_utf8(attr.unwrap().value.into_owned())
                .ok()?
                .parse::<f32>()
                .ok()
        });

    let start_depth = *depth;
    let mut param_name = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                *depth += 1;
                let name = e.name();

                if name.as_ref() == b"data-type:parameter" || name.as_ref() == b"parameter" {
                    param_name = e
                        .attributes()
                        .find(|attr| {
                            let attr = attr.as_ref().unwrap();
                            attr.key.as_ref() == b"data-type:name" || attr.key.as_ref() == b"name"
                        })
                        .and_then(|attr| String::from_utf8(attr.unwrap().value.into_owned()).ok());
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = e.name();

                if name.as_ref() == b"data-type:parameter" || name.as_ref() == b"parameter" {
                    param_name = e
                        .attributes()
                        .find(|attr| {
                            let attr = attr.as_ref().unwrap();
                            attr.key.as_ref() == b"data-type:name" || attr.key.as_ref() == b"name"
                        })
                        .and_then(|attr| String::from_utf8(attr.unwrap().value.into_owned()).ok());
                    // Empty element means we found the parameter, can break if we have it
                    if param_name.is_some() {
                        break;
                    }
                }
            }
            Ok(Event::End(_)) => {
                if *depth <= start_depth {
                    break;
                }
                *depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
        buf.clear();
    }

    let param = param_name.context("Dimension missing parameter name")?;
    Ok((param, min_val, max_val))
}

/// Parse polygon gate in v1.5 format
fn parse_polygon_gate_v1_5(
    reader: &mut Reader<&[u8]>,
    gate_start: &BytesStart,
    depth: &mut u32,
) -> Result<Option<Gate>> {
    let id = gate_start
        .attributes()
        .find(|attr| {
            let attr = attr.as_ref().unwrap();
            attr.key.as_ref() == b"gating:id" || attr.key.as_ref() == b"id"
        })
        .and_then(|attr| String::from_utf8(attr.unwrap().value.into_owned()).ok())
        .context("PolygonGate missing id attribute")?;

    let start_depth = *depth;
    let mut dimensions = Vec::new();
    let mut vertices = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                *depth += 1;
                let name = e.name();

                if name.as_ref() == b"gating:dimension" || name.as_ref() == b"dimension" {
                    let (param, _, _) = parse_dimension_v1_5(reader, e, depth)?;
                    dimensions.push(param);
                } else if name.as_ref() == b"gating:vertex" || name.as_ref() == b"vertex" {
                    if let Some(vertex) = parse_vertex_v1_5(reader, e, depth, &dimensions)? {
                        vertices.push(vertex);
                    }
                }
            }
            Ok(Event::End(_)) => {
                if *depth <= start_depth {
                    break;
                }
                *depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
        buf.clear();
    }

    if dimensions.len() < 2 || vertices.is_empty() {
        return Ok(None);
    }

    let x_param = dimensions[0].clone();
    let y_param = dimensions[1].clone();

    let id_clone = id.clone();
    Ok(Some(Gate::new(
        id,
        id_clone,
        GateGeometry::Polygon {
            nodes: vertices,
            closed: true, // Default to closed for v1.5
        },
        x_param,
        y_param,
    )))
}

/// Parse vertex in v1.5 format
fn parse_vertex_v1_5(
    reader: &mut Reader<&[u8]>,
    _vertex_start: &BytesStart,
    depth: &mut u32,
    dimensions: &[String],
) -> Result<Option<GateNode>> {
    let start_depth = *depth;
    let mut coord_values = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.name();

                if name.as_ref() == b"gating:coordinate" || name.as_ref() == b"coordinate" {
                    let value = e
                        .attributes()
                        .find(|attr| {
                            let attr = attr.as_ref().unwrap();
                            attr.key.as_ref() == b"data-type:value" || attr.key.as_ref() == b"value"
                        })
                        .and_then(|attr| {
                            String::from_utf8(attr.unwrap().value.into_owned())
                                .ok()?
                                .parse::<f32>()
                                .ok()
                        });

                    if let Some(val) = value {
                        coord_values.push(val);
                    }
                }
            }
            Ok(Event::End(_)) => {
                if *depth <= start_depth {
                    break;
                }
                *depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
        buf.clear();
    }

    // In v1.5, coordinates are ordered by dimension
    if coord_values.len() >= 2 && coord_values.len() <= dimensions.len() {
        let mut node = GateNode::new(format!("vertex_{}", coord_values.len()));
        for (i, value) in coord_values.iter().enumerate() {
            if i < dimensions.len() {
                node.set_coordinate(dimensions[i].clone(), *value);
            }
        }
        Ok(Some(node))
    } else {
        Ok(None)
    }
}

/// Parse ellipse gate in v1.5 format (simplified - ellipsoids need more work)
fn parse_ellipse_gate_v1_5(
    _reader: &mut Reader<&[u8]>,
    _gate_start: &BytesStart,
    _depth: &mut u32,
) -> Result<Option<Gate>> {
    // TODO: Implement ellipse/ellipsoid parsing for v1.5
    // This is more complex as v1.5 uses covariance matrices
    Ok(None)
}

/// Parse polygon geometry in v2.0 format
fn parse_polygon_geometry_v2(
    _reader: &mut Reader<&[u8]>,
    _poly_start: &BytesStart,
    _depth: &mut u32,
) -> Result<GateGeometry> {
    // TODO: Implement v2.0 polygon parsing
    todo!("v2.0 polygon parsing")
}

/// Parse rectangle geometry in v2.0 format
fn parse_rectangle_geometry_v2(
    _reader: &mut Reader<&[u8]>,
    _rect_start: &BytesStart,
    _depth: &mut u32,
) -> Result<GateGeometry> {
    // TODO: Implement v2.0 rectangle parsing
    todo!("v2.0 rectangle parsing")
}

/// Parse ellipse geometry in v2.0 format
fn parse_ellipse_geometry_v2(
    _reader: &mut Reader<&[u8]>,
    _ellipse_start: &BytesStart,
    _depth: &mut u32,
) -> Result<GateGeometry> {
    // TODO: Implement v2.0 ellipse parsing
    todo!("v2.0 ellipse parsing")
}

/// Extract x and y parameters from geometry
fn extract_parameters_from_geometry(geometry: &GateGeometry) -> Result<(String, String)> {
    let node = match geometry {
        GateGeometry::Polygon { nodes, .. } => nodes.first(),
        GateGeometry::Rectangle { min, .. } => Some(min),
        GateGeometry::Ellipse { center, .. } => Some(center),
    };

    if let Some(first_node) = node {
        let params: Vec<String> = first_node
            .coordinates
            .keys()
            .map(|k| k.as_ref().to_string())
            .collect();
        if params.len() >= 2 {
            return Ok((params[0].clone(), params[1].clone()));
        }
    }
    Err(anyhow::anyhow!(
        "Could not extract parameters from geometry"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GateGeometry, GateNode};

    fn create_test_gate() -> Gate {
        let node1 = GateNode::new("node1".to_string())
            .with_coordinate("FSC-A", 1000.0)
            .with_coordinate("SSC-A", 2000.0);
        let node2 = GateNode::new("node2".to_string())
            .with_coordinate("FSC-A", 3000.0)
            .with_coordinate("SSC-A", 4000.0);

        Gate::new(
            "test-gate",
            "Test Gate",
            GateGeometry::Polygon {
                nodes: vec![node1, node2],
                closed: true,
            },
            "FSC-A",
            "SSC-A",
        )
    }

    #[test]
    fn test_gates_to_gatingml() {
        let gate = create_test_gate();
        let gates = vec![gate];

        let xml = gates_to_gatingml(&gates).unwrap();

        // Basic validation that XML was generated
        assert!(xml.contains("Gating-ML"));
        assert!(xml.contains("test-gate"));
        assert!(xml.contains("Test Gate"));
        assert!(xml.contains("FSC-A"));
        assert!(xml.contains("SSC-A"));
    }

    #[test]
    fn test_rectangle_gate_to_gatingml() {
        let min_node = GateNode::new("min".to_string())
            .with_coordinate("FSC-A", 1000.0)
            .with_coordinate("SSC-A", 2000.0);
        let max_node = GateNode::new("max".to_string())
            .with_coordinate("FSC-A", 5000.0)
            .with_coordinate("SSC-A", 6000.0);

        let gate = Gate::new(
            "rect-gate",
            "Rectangle Gate",
            GateGeometry::Rectangle {
                min: min_node,
                max: max_node,
            },
            "FSC-A",
            "SSC-A",
        );

        let gates = vec![gate];
        let xml = gates_to_gatingml(&gates).unwrap();

        assert!(xml.contains("RectangleGate"));
        assert!(xml.contains("rect-gate"));
    }

    #[test]
    fn test_ellipse_gate_to_gatingml() {
        let center_node = GateNode::new("center".to_string())
            .with_coordinate("FSC-A", 3000.0)
            .with_coordinate("SSC-A", 4000.0);

        let gate = Gate::new(
            "ellipse-gate",
            "Ellipse Gate",
            GateGeometry::Ellipse {
                center: center_node,
                radius_x: 1000.0,
                radius_y: 2000.0,
                angle: 0.0,
            },
            "FSC-A",
            "SSC-A",
        );

        let gates = vec![gate];
        let xml = gates_to_gatingml(&gates).unwrap();

        assert!(xml.contains("EllipseGate"));
        assert!(xml.contains("ellipse-gate"));
        assert!(xml.contains("radiusX=\"1000\""));
        assert!(xml.contains("radiusY=\"2000\""));
    }
}
