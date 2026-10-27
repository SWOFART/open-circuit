//! Deterministic schematic connectivity derived from persisted geometry.
//!
//! This module builds conductor nodes and electrical islands only. It does not solve the
//! circuit, distribute current through individual wire segments, or serialize derived state.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::{
    Diagnostic, DiagnosticCode, DiagnosticReference, DiagnosticSeverity, GeometryEnvelope,
    GridPoint, PinId, Schematic, StableId, component_catalog,
};

/// A stable terminal identity within a schematic.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PinRef {
    pub component_id: StableId,
    pub pin_id: PinId,
}

/// A derived conductor node. Its index is stable for the same electrical connectivity and IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetNode {
    pub index: usize,
    pub pins: Vec<PinRef>,
    pub wire_ids: Vec<StableId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceKind {
    ExplicitMarker,
    Automatic,
}

/// Component branches connect nodes into an electrical island without merging the nodes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElectricalIsland {
    pub node_indices: Vec<usize>,
    pub component_ids: Vec<StableId>,
    pub reference_node: Option<usize>,
    pub reference_kind: Option<ReferenceKind>,
}

/// Runtime topology and non-fatal diagnostics. This is never part of the saved schematic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Netlist {
    pub nodes: Vec<NetNode>,
    pub islands: Vec<ElectricalIsland>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Derive conductor nodes, electrical islands, and numerical references.
///
/// Errors from schematic and pin-geometry validation are returned before topology is built.
/// Warnings such as automatic local references are retained on the successful netlist.
pub fn derive_netlist(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
) -> Result<Netlist, Vec<Diagnostic>> {
    derive_netlist_with_open(schematic, geometry, &HashSet::new())
}

/// Like `derive_netlist`, for a circuit in which the components of `open` have lost their
/// branch (the `open` fault of docs/contracts/stage-5-assignments.md). An open component's
/// pins stay in the netlist, so the voltage across them can be reported, but they no longer
/// connect its nodes into one island.
pub fn derive_netlist_with_open(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    open: &HashSet<StableId>,
) -> Result<Netlist, Vec<Diagnostic>> {
    let mut diagnostics = schematic.validate();
    diagnostics.extend(geometry.validate_for(schematic));
    if has_errors(&diagnostics) {
        sort_diagnostics(&mut diagnostics);
        return Err(diagnostics);
    }

    let (segments, wire_segment_groups) = make_segments(schematic);
    let pins = make_pins(schematic, geometry);
    let mut conductors = DisjointSet::new(segments.len() + pins.len());

    // Every route is one continuous conductor, including a route that crosses itself.
    for segment_group in &wire_segment_groups {
        union_all(&mut conductors, segment_group);
    }

    // Contacts between different routes are exact-grid only. A perpendicular interior
    // crossing stays separate; an endpoint contact, pin, or junction provides the connection.
    for left in 0..segments.len() {
        for right in (left + 1)..segments.len() {
            if segments[left].wire_id != segments[right].wire_id
                && segments_connect_automatically(&segments[left], &segments[right])
            {
                conductors.union(left, right);
            }
        }
    }

    let pin_offset = segments.len();
    for (pin_index, pin) in pins.iter().enumerate() {
        for (segment_index, segment) in segments.iter().enumerate() {
            if point_on_segment(pin.position, segment) {
                conductors.union(pin_offset + pin_index, segment_index);
            }
        }
    }
    for left in 0..pins.len() {
        for right in (left + 1)..pins.len() {
            if pins[left].position == pins[right].position {
                conductors.union(pin_offset + left, pin_offset + right);
            }
        }
    }

    for junction in &schematic.junctions {
        let entities = entities_at(junction.position, &segments, &pins, pin_offset);
        if entities.is_empty() {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidGeometry,
                vec![DiagnosticReference::Junction {
                    junction_id: junction.id.clone(),
                }],
            ));
        } else {
            union_all(&mut conductors, &entities);
        }
    }

    let mut explicit_reference_entities = Vec::new();
    for marker in &schematic.reference_markers {
        let entities = entities_at(marker.position, &segments, &pins, pin_offset);
        if entities.is_empty() {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DetachedReferenceMarker,
                vec![DiagnosticReference::ReferenceMarker {
                    reference_marker_id: marker.id.clone(),
                }],
            ));
        } else {
            explicit_reference_entities.extend(entities);
        }
    }

    // All explicit markers denote the same global reference and therefore intentionally
    // connect the marked conductors. A reference marker never creates a conductor by itself.
    if let Some(first) = explicit_reference_entities.first().copied() {
        for entity in explicit_reference_entities.iter().copied().skip(1) {
            conductors.union(first, entity);
        }
    }

    if has_errors(&diagnostics) {
        sort_diagnostics(&mut diagnostics);
        return Err(diagnostics);
    }

    let (mut nodes, root_to_node) = make_nodes(&mut conductors, &segments, &pins, pin_offset);
    let mut pin_to_node = HashMap::new();
    for node in &nodes {
        for pin in &node.pins {
            pin_to_node.insert(pin.clone(), node.index);
        }
    }

    let mut island_sets = DisjointSet::new(nodes.len());
    let mut component_nodes = Vec::<(StableId, Vec<usize>)>::new();
    for component in &schematic.components {
        if open.contains(component.id()) {
            continue;
        }
        let mut component_pin_nodes = pins
            .iter()
            .filter(|pin| pin.reference.component_id == *component.id())
            .filter_map(|pin| pin_to_node.get(&pin.reference).copied())
            .collect::<Vec<_>>();
        component_pin_nodes.sort_unstable();
        component_pin_nodes.dedup();
        if let Some(first) = component_pin_nodes.first().copied() {
            for node in component_pin_nodes.iter().copied().skip(1) {
                island_sets.union(first, node);
            }
            component_nodes.push((component.id().clone(), component_pin_nodes));
        }
    }

    let explicit_reference_nodes = explicit_reference_entities
        .iter()
        .filter_map(|entity| root_to_node.get(&conductors.find(*entity)).copied())
        .collect::<BTreeSet<_>>();

    let mut grouped_islands = BTreeMap::<usize, Vec<usize>>::new();
    for node_index in 0..nodes.len() {
        let root = island_sets.find(node_index);
        grouped_islands.entry(root).or_default().push(node_index);
    }

    let mut islands = Vec::with_capacity(grouped_islands.len());
    for node_indices in grouped_islands.into_values() {
        let mut component_ids = component_nodes
            .iter()
            .filter(|(_, component_nodes)| {
                component_nodes
                    .iter()
                    .any(|node| node_indices.binary_search(node).is_ok())
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        component_ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        component_ids.dedup();

        let pins_in_island = node_indices
            .iter()
            .flat_map(|index| nodes[*index].pins.iter())
            .collect::<Vec<_>>();
        let explicit_reference_node = node_indices
            .iter()
            .copied()
            .find(|node| explicit_reference_nodes.contains(node));

        let (reference_node, reference_kind) = if let Some(node) = explicit_reference_node {
            (Some(node), Some(ReferenceKind::ExplicitMarker))
        } else if let Some(reference_pin) = pins_in_island.iter().min_by(|left, right| {
            left.component_id
                .as_str()
                .cmp(right.component_id.as_str())
                .then_with(|| left.pin_id.as_str().cmp(right.pin_id.as_str()))
        }) {
            let node = pin_to_node[*reference_pin];
            diagnostics.push(Diagnostic::warning(
                DiagnosticCode::NoGroundReference,
                vec![DiagnosticReference::pin(
                    reference_pin.component_id.clone(),
                    reference_pin.pin_id,
                )],
            ));
            (Some(node), Some(ReferenceKind::Automatic))
        } else {
            let mut references = node_indices
                .iter()
                .flat_map(|index| nodes[*index].wire_ids.iter().cloned())
                .map(|wire_id| DiagnosticReference::Wire { wire_id })
                .collect::<Vec<_>>();
            sort_references(&mut references);
            diagnostics.push(Diagnostic::error(DiagnosticCode::FloatingNode, references));
            (
                explicit_reference_node,
                explicit_reference_node.map(|_| ReferenceKind::ExplicitMarker),
            )
        };

        islands.push(ElectricalIsland {
            node_indices,
            component_ids,
            reference_node,
            reference_kind,
        });
    }

    islands.sort_by_key(|island| island.node_indices.first().copied().unwrap_or(usize::MAX));
    nodes.shrink_to_fit();
    sort_diagnostics(&mut diagnostics);
    if has_errors(&diagnostics) {
        Err(diagnostics)
    } else {
        Ok(Netlist {
            nodes,
            islands,
            diagnostics,
        })
    }
}

#[derive(Clone)]
struct Segment {
    wire_id: StableId,
    start: GridPoint,
    end: GridPoint,
}

#[derive(Clone)]
struct Pin {
    reference: PinRef,
    position: GridPoint,
}

fn make_segments(schematic: &Schematic) -> (Vec<Segment>, Vec<Vec<usize>>) {
    let mut wires = schematic.wires.iter().collect::<Vec<_>>();
    wires.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));

    let mut segments = Vec::new();
    let mut groups = Vec::new();
    for wire in wires {
        let mut group = Vec::new();
        for pair in wire.points.windows(2) {
            group.push(segments.len());
            segments.push(Segment {
                wire_id: wire.id.clone(),
                start: pair[0],
                end: pair[1],
            });
        }
        groups.push(group);
    }
    (segments, groups)
}

fn make_pins(schematic: &Schematic, geometry: &GeometryEnvelope) -> Vec<Pin> {
    let catalog = component_catalog();
    let mut components = schematic.components.iter().collect::<Vec<_>>();
    components.sort_by(|left, right| left.id().as_str().cmp(right.id().as_str()));
    let mut pins = Vec::new();

    for component in components {
        let Some(component_geometry) = geometry
            .components
            .iter()
            .find(|entry| entry.component_id == *component.id())
        else {
            continue;
        };
        let Some(definition) = catalog
            .component_types
            .iter()
            .find(|entry| entry.type_id == component.type_id())
        else {
            continue;
        };

        for pin_definition in &definition.pins {
            let pin_id = pin_definition.id;
            let Some(resolved) = component_geometry
                .pins
                .iter()
                .find(|pin| pin.pin_id == pin_id.as_str())
            else {
                continue;
            };
            pins.push(Pin {
                reference: PinRef {
                    component_id: component.id().clone(),
                    pin_id,
                },
                position: resolved.position,
            });
        }
    }
    pins
}

fn segments_connect_automatically(left: &Segment, right: &Segment) -> bool {
    let left_horizontal = left.start.y == left.end.y;
    let right_horizontal = right.start.y == right.end.y;

    if left_horizontal == right_horizontal {
        if left_horizontal {
            left.start.y == right.start.y
                && intervals_overlap(left.start.x, left.end.x, right.start.x, right.end.x)
        } else {
            left.start.x == right.start.x
                && intervals_overlap(left.start.y, left.end.y, right.start.y, right.end.y)
        }
    } else {
        let (horizontal, vertical) = if left_horizontal {
            (left, right)
        } else {
            (right, left)
        };
        let crossing = GridPoint {
            x: vertical.start.x,
            y: horizontal.start.y,
        };
        if !point_on_segment(crossing, horizontal) || !point_on_segment(crossing, vertical) {
            return false;
        }

        is_endpoint(crossing, horizontal) || is_endpoint(crossing, vertical)
    }
}

fn intervals_overlap(left_start: i32, left_end: i32, right_start: i32, right_end: i32) -> bool {
    left_start.min(left_end).max(right_start.min(right_end))
        <= left_start.max(left_end).min(right_start.max(right_end))
}

fn is_endpoint(point: GridPoint, segment: &Segment) -> bool {
    point == segment.start || point == segment.end
}

fn point_on_segment(point: GridPoint, segment: &Segment) -> bool {
    if segment.start.x == segment.end.x {
        point.x == segment.start.x
            && point.y >= segment.start.y.min(segment.end.y)
            && point.y <= segment.start.y.max(segment.end.y)
    } else {
        point.y == segment.start.y
            && point.x >= segment.start.x.min(segment.end.x)
            && point.x <= segment.start.x.max(segment.end.x)
    }
}

fn entities_at(
    point: GridPoint,
    segments: &[Segment],
    pins: &[Pin],
    pin_offset: usize,
) -> Vec<usize> {
    let mut entities = segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| point_on_segment(point, segment).then_some(index))
        .collect::<Vec<_>>();
    entities.extend(
        pins.iter()
            .enumerate()
            .filter_map(|(index, pin)| (pin.position == point).then_some(pin_offset + index)),
    );
    entities
}

fn make_nodes(
    conductors: &mut DisjointSet,
    segments: &[Segment],
    pins: &[Pin],
    pin_offset: usize,
) -> (Vec<NetNode>, HashMap<usize, usize>) {
    let mut grouped = BTreeMap::<usize, NodeBuilder>::new();
    for (index, segment) in segments.iter().enumerate() {
        grouped
            .entry(conductors.find(index))
            .or_default()
            .wire_ids
            .insert(segment.wire_id.as_str().to_owned(), segment.wire_id.clone());
    }
    for (index, pin) in pins.iter().enumerate() {
        grouped
            .entry(conductors.find(pin_offset + index))
            .or_default()
            .pins
            .push(pin.reference.clone());
    }

    let mut builders = grouped.into_iter().collect::<Vec<_>>();
    for (_, builder) in &mut builders {
        builder.pins.sort_by(|left, right| {
            left.component_id
                .as_str()
                .cmp(right.component_id.as_str())
                .then_with(|| left.pin_id.as_str().cmp(right.pin_id.as_str()))
        });
    }
    builders.sort_by_key(|(_, builder)| node_sort_key(builder));

    let mut root_to_node = HashMap::new();
    let mut nodes = Vec::with_capacity(builders.len());
    for (index, (root, builder)) in builders.into_iter().enumerate() {
        root_to_node.insert(root, index);
        let wire_ids = builder.wire_ids.into_values().collect::<Vec<_>>();
        nodes.push(NetNode {
            index,
            pins: builder.pins,
            wire_ids,
        });
    }
    (nodes, root_to_node)
}

#[derive(Default)]
struct NodeBuilder {
    pins: Vec<PinRef>,
    wire_ids: BTreeMap<String, StableId>,
}

fn node_sort_key(builder: &NodeBuilder) -> (u8, String, String, i32, i32) {
    if let Some(pin) = builder.pins.first() {
        (
            0,
            pin.component_id.as_str().to_owned(),
            pin.pin_id.as_str().to_owned(),
            0,
            0,
        )
    } else {
        (
            1,
            builder
                .wire_ids
                .first_key_value()
                .map_or_else(String::new, |(id, _)| id.clone()),
            String::new(),
            0,
            0,
        )
    }
}

fn union_all(sets: &mut DisjointSet, entities: &[usize]) {
    if let Some(first) = entities.first().copied() {
        for entity in entities.iter().copied().skip(1) {
            sets.union(first, entity);
        }
    }
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn sort_references(references: &mut Vec<DiagnosticReference>) {
    references.sort_by_key(|reference| format!("{reference:?}"));
    references.dedup();
}

fn sort_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    for diagnostic in diagnostics.iter_mut() {
        sort_references(&mut diagnostic.references);
    }
    diagnostics.sort_by_key(|diagnostic| format!("{diagnostic:?}"));
    diagnostics.dedup();
}

struct DisjointSet {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, item: usize) -> usize {
        if self.parent[item] != item {
            self.parent[item] = self.find(self.parent[item]);
        }
        self.parent[item]
    }

    fn union(&mut self, left: usize, right: usize) {
        let mut left_root = self.find(left);
        let mut right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        if self.rank[left_root] < self.rank[right_root] {
            std::mem::swap(&mut left_root, &mut right_root);
        }
        self.parent[right_root] = left_root;
        if self.rank[left_root] == self.rank[right_root] {
            self.rank[left_root] += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ComponentSpec;
    use serde_json::from_str;

    const SCHEMATIC_JSON: &str =
        include_str!("../../../tests/golden/stage-1-divider.schematic.json");
    const GEOMETRY_JSON: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");

    fn divider() -> (Schematic, GeometryEnvelope) {
        (
            from_str(SCHEMATIC_JSON).expect("divider schematic should parse"),
            from_str(GEOMETRY_JSON).expect("divider geometry should parse"),
        )
    }

    fn pin_node<'a>(netlist: &'a Netlist, component: &str, pin_id: PinId) -> &'a NetNode {
        netlist
            .nodes
            .iter()
            .find(|node| {
                node.pins
                    .iter()
                    .any(|pin| pin.component_id.as_str() == component && pin.pin_id == pin_id)
            })
            .expect("pin should belong to a net node")
    }

    fn crossing_fixture(with_junction: bool) -> (Schematic, GeometryEnvelope) {
        let mut schematic: Schematic = from_str(
            r#"{
              "schemaVersion":1,
              "components":[
                {"id":"00000000-0000-4000-8000-000000000001","typeId":"resistor","position":{"x":-2,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":1000}},
                {"id":"00000000-0000-4000-8000-000000000002","typeId":"resistor","position":{"x":2,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":1000}},
                {"id":"00000000-0000-4000-8000-000000000003","typeId":"resistor","position":{"x":0,"y":-2},"rotationDeg":90,"parameters":{"resistanceOhms":1000}},
                {"id":"00000000-0000-4000-8000-000000000004","typeId":"resistor","position":{"x":0,"y":2},"rotationDeg":90,"parameters":{"resistanceOhms":1000}}
              ],
              "wires":[
                {"id":"00000000-0000-4000-8000-000000000005","points":[{"x":-1,"y":0},{"x":1,"y":0}]},
                {"id":"00000000-0000-4000-8000-000000000006","points":[{"x":0,"y":-1},{"x":0,"y":1}]}
              ],
              "junctions":[],"referenceMarkers":[]
            }"#,
        )
        .expect("crossing schematic should parse");
        let geometry: GeometryEnvelope = from_str(
            r#"{
              "components":[
                {"componentId":"00000000-0000-4000-8000-000000000001","pins":[{"pinId":"pin1","position":{"x":-3,"y":0}},{"pinId":"pin2","position":{"x":-1,"y":0}}]},
                {"componentId":"00000000-0000-4000-8000-000000000002","pins":[{"pinId":"pin1","position":{"x":1,"y":0}},{"pinId":"pin2","position":{"x":3,"y":0}}]},
                {"componentId":"00000000-0000-4000-8000-000000000003","pins":[{"pinId":"pin1","position":{"x":0,"y":-3}},{"pinId":"pin2","position":{"x":0,"y":-1}}]},
                {"componentId":"00000000-0000-4000-8000-000000000004","pins":[{"pinId":"pin1","position":{"x":0,"y":1}},{"pinId":"pin2","position":{"x":0,"y":3}}]}
              ]
            }"#,
        )
        .expect("crossing geometry should parse");

        if with_junction {
            schematic.junctions.push(crate::JunctionSpec {
                id: StableId::parse("00000000-0000-4000-8000-000000000007").unwrap(),
                position: GridPoint { x: 0, y: 0 },
            });
        }
        (schematic, geometry)
    }

    #[test]
    fn pin_on_segment_and_t_junctions_join_conductors() {
        let (mut schematic, mut geometry) = divider();
        geometry.components[1].pins[0].position = GridPoint { x: -1, y: 2 };
        schematic.wires.push(crate::WireSpec {
            id: StableId::parse("00000000-0000-4000-8000-000000000008").unwrap(),
            points: vec![GridPoint { x: 3, y: 2 }, GridPoint { x: 2, y: 2 }],
        });

        let netlist = derive_netlist(&schematic, &geometry).expect("topology should be valid");
        let pin_node = pin_node(
            &netlist,
            "00000000-0000-4000-8000-000000000002",
            PinId::Pin1,
        );
        assert!(
            pin_node
                .wire_ids
                .iter()
                .any(|wire| wire.as_str() == "00000000-0000-4000-8000-000000000004")
        );
        assert!(pin_node.pins.iter().any(|pin| pin.component_id.as_str()
            == "00000000-0000-4000-8000-000000000001"
            && pin.pin_id == PinId::Pin1));
        assert!(
            pin_node
                .wire_ids
                .iter()
                .any(|wire| wire.as_str() == "00000000-0000-4000-8000-000000000008")
        );
    }

    #[test]
    fn interior_crossings_stay_separate_until_a_junction_is_present() {
        let (schematic, geometry) = crossing_fixture(false);
        let no_junction = derive_netlist(&schematic, &geometry).expect("crossing nets are valid");
        assert_eq!(no_junction.islands.len(), 2);
        assert_ne!(
            pin_node(
                &no_junction,
                "00000000-0000-4000-8000-000000000001",
                PinId::Pin1
            )
            .index,
            pin_node(
                &no_junction,
                "00000000-0000-4000-8000-000000000003",
                PinId::Pin1
            )
            .index,
        );

        let (schematic, geometry) = crossing_fixture(true);
        let with_junction =
            derive_netlist(&schematic, &geometry).expect("junction should join nets");
        assert_eq!(with_junction.islands.len(), 1);
    }

    #[test]
    fn separate_explicit_reference_markers_join_the_marked_nets() {
        let (mut schematic, geometry) = crossing_fixture(false);
        schematic.reference_markers = vec![
            crate::ReferenceMarkerSpec {
                id: StableId::parse("00000000-0000-4000-8000-000000000007").unwrap(),
                position: GridPoint { x: -1, y: 0 },
            },
            crate::ReferenceMarkerSpec {
                id: StableId::parse("00000000-0000-4000-8000-000000000008").unwrap(),
                position: GridPoint { x: 0, y: -1 },
            },
        ];

        let netlist = derive_netlist(&schematic, &geometry).expect("marked nets should connect");
        assert_eq!(netlist.islands.len(), 1);
        assert!(netlist.islands[0].reference_node.is_some());
        assert_eq!(
            netlist.islands[0].reference_kind,
            Some(ReferenceKind::ExplicitMarker)
        );
    }

    #[test]
    fn coincident_endpoints_and_collinear_overlaps_share_nodes() {
        let (mut schematic, geometry) = divider();
        schematic.wires.push(crate::WireSpec {
            id: StableId::parse("00000000-0000-4000-8000-000000000008").unwrap(),
            points: vec![GridPoint { x: 6, y: 0 }, GridPoint { x: 8, y: 0 }],
        });
        let netlist = derive_netlist(&schematic, &geometry).expect("overlap should connect");
        let middle = pin_node(
            &netlist,
            "00000000-0000-4000-8000-000000000002",
            PinId::Pin2,
        );
        assert!(
            middle
                .wire_ids
                .iter()
                .any(|wire| wire.as_str() == "00000000-0000-4000-8000-000000000005")
        );
        assert!(
            middle
                .wire_ids
                .iter()
                .any(|wire| wire.as_str() == "00000000-0000-4000-8000-000000000008")
        );
        assert!(middle.pins.iter().any(|pin| pin.component_id.as_str()
            == "00000000-0000-4000-8000-000000000003"
            && pin.pin_id == PinId::Pin1));
    }

    #[test]
    fn explicit_and_automatic_references_are_deterministic() {
        let (schematic, geometry) = divider();
        let explicit = derive_netlist(&schematic, &geometry).unwrap();
        assert_eq!(explicit.islands.len(), 1);
        assert_eq!(
            explicit.islands[0].reference_kind,
            Some(ReferenceKind::ExplicitMarker)
        );
        assert!(explicit.diagnostics.is_empty());

        let mut unreferenced = schematic.clone();
        unreferenced.reference_markers.clear();
        let automatic = derive_netlist(&unreferenced, &geometry).unwrap();
        assert_eq!(
            automatic.islands[0].reference_kind,
            Some(ReferenceKind::Automatic)
        );
        let chosen = &automatic.nodes[automatic.islands[0].reference_node.unwrap()].pins[0];
        assert_eq!(
            chosen.component_id.as_str(),
            "00000000-0000-4000-8000-000000000001"
        );
        assert_eq!(chosen.pin_id, PinId::Pin1);
        assert_eq!(
            automatic
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == DiagnosticCode::NoGroundReference)
                .count(),
            1
        );
    }

    #[test]
    fn rerouting_and_input_order_do_not_change_electrical_topology_or_reference() {
        let (schematic, geometry) = divider();
        let expected = derive_netlist(&schematic, &geometry).unwrap();

        let mut rerouted = schematic;
        rerouted.components.reverse();
        rerouted.wires.reverse();
        rerouted
            .wires
            .iter_mut()
            .find(|wire| wire.id.as_str() == "00000000-0000-4000-8000-000000000006")
            .unwrap()
            .points = vec![
            GridPoint { x: 9, y: 0 },
            GridPoint { x: 9, y: -8 },
            GridPoint { x: 1, y: -8 },
            GridPoint { x: 1, y: 0 },
        ];
        rerouted
            .wires
            .iter_mut()
            .find(|wire| wire.id.as_str() == "00000000-0000-4000-8000-000000000004")
            .unwrap()
            .points = vec![
            GridPoint { x: -1, y: 0 },
            GridPoint { x: -1, y: 8 },
            GridPoint { x: 3, y: 8 },
            GridPoint { x: 3, y: 0 },
        ];
        let mut rerouted_geometry = geometry;
        rerouted_geometry.components.reverse();

        let actual = derive_netlist(&rerouted, &rerouted_geometry).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn detached_markers_and_wire_only_islands_are_diagnosed() {
        let (mut schematic, geometry) = divider();
        schematic.reference_markers[0].position = GridPoint { x: 200, y: 200 };
        let errors = derive_netlist(&schematic, &geometry).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|item| item.code == DiagnosticCode::DetachedReferenceMarker)
        );

        let wire_only: Schematic = from_str(
            r#"{"schemaVersion":1,"components":[],"wires":[{"id":"00000000-0000-4000-8000-000000000008","points":[{"x":0,"y":0},{"x":2,"y":0}]}],"junctions":[],"referenceMarkers":[]}"#,
        )
        .unwrap();
        let empty_geometry: GeometryEnvelope = from_str(r#"{"components":[]}"#).unwrap();
        let errors = derive_netlist(&wire_only, &empty_geometry).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|item| item.code == DiagnosticCode::FloatingNode)
        );
    }

    #[test]
    fn invalid_values_and_duplicate_ids_block_topology() {
        let (mut schematic, geometry) = divider();
        if let ComponentSpec::Resistor { parameters, .. } = &mut schematic.components[1] {
            parameters.resistance_ohms = f64::NAN;
        }
        let errors = derive_netlist(&schematic, &geometry).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|item| item.code == DiagnosticCode::InvalidParameter)
        );

        let (mut schematic, geometry) = divider();
        schematic.wires[0].id = schematic.components[0].id().clone();
        let errors = derive_netlist(&schematic, &geometry).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|item| item.code == DiagnosticCode::DuplicateId)
        );
    }
}
