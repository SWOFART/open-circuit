//! Resolve probe points and model wire spans as edges without changing electrical connectivity.

use std::collections::{HashMap, HashSet};

use super::{
    Conductor, Netlist, PinRef, Segment, make_pins, make_segments, point_on_segment,
    segments_connect_automatically,
};
use crate::{ConductorSpan, DiagnosticCode, GeometryEnvelope, GridPoint, Schematic, StableId};

fn owner_id(owner: &Conductor) -> &StableId {
    match owner {
        Conductor::Wire(id) | Conductor::Rail(id) => id,
    }
}

pub(crate) fn node_at(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    netlist: &Netlist,
    point: GridPoint,
) -> Result<usize, DiagnosticCode> {
    let (segments, _) = make_segments(schematic);
    let owners = segments
        .iter()
        .filter(|segment| point_on_segment(point, segment))
        .map(|segment| owner_id(&segment.owner))
        .collect::<HashSet<_>>();
    let pins = geometry
        .components
        .iter()
        .flat_map(|entry| {
            entry
                .pins
                .iter()
                .filter(move |pin| pin.position == point)
                .map(move |pin| (&entry.component_id, pin.pin_id.as_str()))
        })
        .collect::<HashSet<_>>();
    let mut matching = netlist.nodes.iter().filter(|node| {
        node.wire_ids
            .iter()
            .chain(&node.rail_ids)
            .any(|id| owners.contains(id))
            || node
                .pins
                .iter()
                .any(|pin| pins.contains(&(&pin.component_id, pin.pin_id.as_str())))
    });
    let node = matching.next().ok_or(DiagnosticCode::UnconnectedProbe)?;
    if matching.next().is_some() {
        return Err(DiagnosticCode::AmbiguousProbe);
    }
    if node.pins.is_empty() {
        return Err(DiagnosticCode::UnconnectedProbe);
    }
    Ok(node.index)
}

#[derive(Default)]
struct Vertex {
    neighbors: Vec<(usize, Option<usize>)>,
    pin: Option<PinRef>,
}

#[derive(Default)]
pub(crate) struct ConductorGraph {
    vertices: Vec<Vertex>,
    spans: Vec<(ConductorSpan, usize, usize)>,
    earth_hub: Option<usize>,
}

fn contacts(left: &Segment, right: &Segment) -> Vec<GridPoint> {
    if left.owner != right.owner && !segments_connect_automatically(left, right) {
        return Vec::new();
    }
    let left_vertical = left.start.x == left.end.x;
    let right_vertical = right.start.x == right.end.x;
    if left_vertical != right_vertical {
        let (vertical, horizontal) = if left_vertical {
            (left, right)
        } else {
            (right, left)
        };
        let crossing = GridPoint {
            x: vertical.start.x,
            y: horizontal.start.y,
        };
        return (point_on_segment(crossing, left) && point_on_segment(crossing, right))
            .then_some(crossing)
            .into_iter()
            .collect();
    }
    let mut endpoints = [left.start, left.end, right.start, right.end]
        .into_iter()
        .filter(|point| point_on_segment(*point, left) && point_on_segment(*point, right))
        .collect::<Vec<_>>();
    endpoints.sort_unstable_by_key(|point| (point.x, point.y));
    endpoints.dedup();
    endpoints
}

impl ConductorGraph {
    pub(crate) fn build(
        schematic: &Schematic,
        geometry: &GeometryEnvelope,
    ) -> Result<Self, DiagnosticCode> {
        let (segments, _) = make_segments(schematic);
        let pins = make_pins(schematic, geometry);
        let span_count = segments.len();
        let point_count = pins
            .len()
            .saturating_add(schematic.junctions.len())
            .saturating_add(schematic.reference_markers.len());
        let pairs = span_count.saturating_mul(span_count.saturating_sub(1)) / 2;
        let work = pairs.saturating_add(span_count.saturating_mul(point_count));
        if work > crate::MAX_RAIL_CONTACT_WORK {
            return Err(DiagnosticCode::ModelRangeExceeded);
        }
        let mut splits = segments
            .iter()
            .map(|segment| vec![segment.start, segment.end])
            .collect::<Vec<_>>();
        let mut bridges = Vec::new();
        for left in 0..segments.len() {
            for right in left + 1..segments.len() {
                for point in contacts(&segments[left], &segments[right]) {
                    splits[left].push(point);
                    splits[right].push(point);
                    bridges.push((left, right, point));
                }
            }
        }
        let points = pins
            .iter()
            .map(|pin| pin.position)
            .chain(schematic.junctions.iter().map(|junction| junction.position))
            .chain(
                schematic
                    .reference_markers
                    .iter()
                    .map(|marker| marker.position),
            )
            .map(|point| (point.x, point.y))
            .collect::<HashSet<_>>();
        for (index, segment) in segments.iter().enumerate() {
            splits[index].extend(
                points
                    .iter()
                    .map(|&(x, y)| GridPoint { x, y })
                    .filter(|point| point_on_segment(*point, segment)),
            );
        }

        let mut graph = Self::default();
        let mut ids = HashMap::new();
        // make_segments groups wires then rails; order the published list by stable ID.
        let mut order = (0..segments.len()).collect::<Vec<_>>();
        order.sort_by_key(|&index| owner_id(&segments[index].owner).as_str());
        let mut by_point = HashMap::<(i32, i32), Vec<usize>>::new();
        for index in order {
            let segment = &segments[index];
            let points = &mut splits[index];
            points.sort_by_key(|point| {
                (i64::from(point.x) - i64::from(segment.start.x)).abs()
                    + (i64::from(point.y) - i64::from(segment.start.y)).abs()
            });
            points.dedup();
            for &point in points.iter() {
                let vertex = graph.vertex(None);
                ids.insert((index, point.x, point.y), vertex);
                by_point.entry((point.x, point.y)).or_default().push(vertex);
            }
            for endpoints in points.windows(2) {
                let [start, end] = [endpoints[0], endpoints[1]];
                let [from, to] = [ids[&(index, start.x, start.y)], ids[&(index, end.x, end.y)]];
                let span = ConductorSpan {
                    conductor_id: owner_id(&segment.owner).clone(),
                    start,
                    end,
                };
                let edge = graph.spans.len();
                graph.spans.push((span, from, to));
                graph.connect(from, to, Some(edge));
            }
        }
        for (left, right, point) in bridges {
            graph.connect(
                ids[&(left, point.x, point.y)],
                ids[&(right, point.x, point.y)],
                None,
            );
        }

        let mut attachments = HashMap::new();
        for point in points {
            let vertex = graph.vertex(None);
            attachments.insert(point, vertex);
            for &segment_vertex in by_point.get(&point).into_iter().flatten() {
                graph.connect(vertex, segment_vertex, None);
            }
        }
        for pin in pins {
            let vertex = graph.vertex(Some(pin.reference));
            graph.connect(vertex, attachments[&(pin.position.x, pin.position.y)], None);
        }
        if !schematic.reference_markers.is_empty() {
            let hub = graph.vertex(None);
            if schematic
                .components
                .iter()
                .any(|component| matches!(component, crate::ComponentSpec::EarthElectrode { .. }))
            {
                graph.earth_hub = Some(hub);
            }
            for marker in &schematic.reference_markers {
                graph.connect(
                    hub,
                    attachments[&(marker.position.x, marker.position.y)],
                    None,
                );
            }
        }
        Ok(graph)
    }

    fn vertex(&mut self, pin: Option<PinRef>) -> usize {
        let vertex = self.vertices.len();
        self.vertices.push(Vertex {
            pin,
            ..Vertex::default()
        });
        vertex
    }

    fn connect(&mut self, from: usize, to: usize, span: Option<usize>) {
        self.vertices[from].neighbors.push((to, span));
        self.vertices[to].neighbors.push((from, span));
    }

    pub(crate) fn spans(&self) -> impl ExactSizeIterator<Item = &ConductorSpan> {
        self.spans.iter().map(|(span, _, _)| span)
    }

    fn side(&self, vertex: usize, edge: usize) -> Vec<bool> {
        let mut seen = vec![false; self.vertices.len()];
        seen[vertex] = true;
        let mut pending = vec![vertex];
        while let Some(vertex) = pending.pop() {
            for &(neighbor, skip) in &self.vertices[vertex].neighbors {
                if skip != Some(edge) && !seen[neighbor] {
                    seen[neighbor] = true;
                    pending.push(neighbor);
                }
            }
        }
        seen
    }

    pub(crate) fn current(
        &self,
        target: &ConductorSpan,
        current: impl Fn(&PinRef) -> Result<f64, DiagnosticCode>,
    ) -> Result<f64, DiagnosticCode> {
        let mut spans = self
            .spans
            .iter()
            .enumerate()
            .filter_map(|(edge, (span, from, to))| {
                if span.conductor_id != target.conductor_id {
                    return None;
                }
                if span.start == target.start && span.end == target.end {
                    Some((edge, *from, *to))
                } else if span.start == target.end && span.end == target.start {
                    Some((edge, *to, *from))
                } else {
                    None
                }
            });
        let (edge, from, to) = spans.next().ok_or(DiagnosticCode::UnsupportedProbeTarget)?;
        let first = self.side(from, edge);
        if first[to] || spans.next().is_some() {
            return Err(DiagnosticCode::AmbiguousConductorCurrent);
        }
        let has_terminal = |side: &[bool]| {
            self.vertices
                .iter()
                .zip(side)
                .any(|(vertex, &included)| included && vertex.pin.is_some())
        };
        let second = (!has_terminal(&first)).then(|| self.side(to, edge));
        if second.as_ref().is_some_and(|side| !has_terminal(side)) {
            return Err(DiagnosticCode::UnconnectedProbe);
        }
        let sum = |side: Vec<bool>| {
            if self.earth_hub.is_some_and(|hub| side[hub]) {
                return Err(DiagnosticCode::UnsupportedProbeTarget);
            }
            self.vertices
                .iter()
                .zip(side)
                .filter_map(|(vertex, included)| included.then_some(vertex.pin.as_ref()).flatten())
                .try_fold(0.0, |total, pin| Ok(total + current(pin)?))
        };
        sum(first)
            .map(|value| -value)
            .or_else(|_| sum(second.unwrap_or_else(|| self.side(to, edge))))
    }
}
