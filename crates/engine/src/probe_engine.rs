use std::collections::{HashMap, HashSet};

use super::{CompiledRun, Engine};
use crate::{
    ComponentSpec, ComponentTypeId, ConductorSpanPage, ConductorSpanRequest, Diagnostic,
    DiagnosticCode, DiagnosticReference, PinId, ProbeRequest, ProbeResult, ProbeTarget,
    ResultQuantity, StableId, Unit, component_catalog,
    mna::{MnaSystem, NodeVariables},
    netlist::{
        Netlist, PinRef,
        probe_geometry::{ConductorGraph, node_at},
    },
};

const PAGE_SIZE: usize = 256;
type Reading = Result<Option<(f64, Unit)>, Diagnostic>;

impl Engine {
    /// Read the most recent committed endpoint. This does not advance model time.
    pub fn probe(&self, request: ProbeRequest) -> ProbeResult {
        let run = match self.probe_run(&request.run_id, &request.document_revision) {
            Ok(run) => run,
            Err(diagnostic) => {
                return ProbeResult::Unavailable {
                    metadata: None,
                    diagnostic,
                };
            }
        };
        let metadata = run.snapshot.metadata.clone();
        if request.frame_sequence > metadata.frame_sequence {
            return ProbeResult::Unavailable {
                metadata: None,
                diagnostic: error(DiagnosticCode::InvalidParameter),
            };
        }
        match run.measure(&request.target) {
            Ok(Some((value, unit))) => ProbeResult::Reading {
                metadata,
                value,
                unit,
            },
            Ok(None) => ProbeResult::OpenCircuit { metadata },
            Err(diagnostic) => ProbeResult::Unavailable {
                metadata: Some(metadata),
                diagnostic,
            },
        }
    }

    /// List selectable wire and rail spans in pages tied to the active document.
    pub fn conductor_spans(
        &self,
        request: ConductorSpanRequest,
    ) -> Result<ConductorSpanPage, Diagnostic> {
        let run = self.probe_run(&request.run_id, &request.document_revision)?;
        let graph = run.conductor_graph()?;
        let prefix = format!(
            "{}:{}:",
            run.run_id, run.snapshot.metadata.document_revision
        );
        let offset = match request.cursor {
            None => 0,
            Some(cursor) => cursor
                .strip_prefix(&prefix)
                .and_then(|offset| offset.parse::<usize>().ok())
                .filter(|offset| {
                    *offset > 0 && offset.is_multiple_of(PAGE_SIZE) && *offset < graph.spans().len()
                })
                .ok_or_else(|| error(DiagnosticCode::InvalidParameter))?,
        };
        let spans: Vec<_> = graph
            .spans()
            .skip(offset)
            .take(PAGE_SIZE)
            .cloned()
            .collect();
        let end = offset + spans.len();
        Ok(ConductorSpanPage {
            spans,
            next_cursor: (end < graph.spans().len()).then(|| format!("{prefix}{end}")),
        })
    }

    fn probe_run(&self, run_id: &StableId, revision: &str) -> Result<&CompiledRun, Diagnostic> {
        let run = self
            .compiled
            .as_ref()
            .filter(|run| &run.run_id == run_id)
            .ok_or_else(|| error(DiagnosticCode::StaleRun))?;
        if run.snapshot.metadata.document_revision != revision {
            return Err(error(DiagnosticCode::StaleDocumentRevision));
        }
        Ok(run)
    }
}

fn error(code: DiagnosticCode) -> Diagnostic {
    Diagnostic::error(code, Vec::new())
}

impl CompiledRun {
    fn conductor_graph(&self) -> Result<&ConductorGraph, Diagnostic> {
        self.probe_graph
            .get_or_init(|| ConductorGraph::build(&self.overlay.schematic, &self.overlay.geometry))
            .as_ref()
            .map_err(|&code| error(code))
    }

    fn measure(&self, target: &ProbeTarget) -> Reading {
        let schematic = &self.overlay.schematic;
        let geometry = &self.overlay.geometry;
        let netlist = self
            .states
            .solver_memory
            .probe_netlist
            .as_deref()
            .ok_or_else(|| error(DiagnosticCode::UnconnectedProbe))?;
        let point_node = |position| {
            node_at(schematic, geometry, netlist, position).map_err(|code| {
                Diagnostic::error(code, vec![DiagnosticReference::Point { position }])
            })
        };
        let reading = match target {
            ProbeTarget::Voltage { positive, negative } => {
                let a = point_node(*positive)?;
                let b = point_node(*negative)?;
                let volts = self.voltage(a, b).map_err(|code| {
                    Diagnostic::error(
                        code,
                        vec![
                            DiagnosticReference::Point {
                                position: *positive,
                            },
                            DiagnosticReference::Point {
                                position: *negative,
                            },
                        ],
                    )
                })?;
                Some((volts, Unit::Volt))
            }
            ProbeTarget::TerminalCurrent {
                component_id,
                pin_id,
            } => {
                let pin = PinRef {
                    component_id: component_id.clone(),
                    pin_id: *pin_id,
                };
                let fail = |code| {
                    Diagnostic::error(
                        code,
                        vec![DiagnosticReference::pin(component_id.clone(), *pin_id)],
                    )
                };
                let connected = netlist.nodes.iter().any(|node| {
                    node.pins.contains(&pin)
                        && (node.pins.len() > 1
                            || !node.wire_ids.is_empty()
                            || !node.rail_ids.is_empty())
                });
                if !connected {
                    return Err(fail(DiagnosticCode::UnconnectedProbe));
                }
                Some((self.pin_current(&pin).map_err(fail)?, Unit::Amp))
            }
            ProbeTarget::ConductorCurrent { span } => {
                let current = self
                    .conductor_graph()?
                    .current(span, |pin| self.pin_current(pin))
                    .map_err(|code| {
                        let id = span.conductor_id.clone();
                        let reference = if schematic.rails.iter().any(|rail| rail.id == id) {
                            DiagnosticReference::Rail { rail_id: id }
                        } else {
                            DiagnosticReference::Wire { wire_id: id }
                        };
                        Diagnostic::error(code, vec![reference])
                    })?;
                Some((current, Unit::Amp))
            }
            ProbeTarget::Resistance { first, second } => {
                let a = point_node(*first)?;
                let b = point_node(*second)?;
                self.resistance(netlist, a, b)
                    .map_err(|code| {
                        Diagnostic::error(
                            code,
                            vec![
                                DiagnosticReference::Point { position: *first },
                                DiagnosticReference::Point { position: *second },
                            ],
                        )
                    })?
                    .map(|ohms| (ohms, Unit::Ohm))
            }
        };
        if reading.is_some_and(|(value, _)| !value.is_finite()) {
            return Err(error(DiagnosticCode::NonFiniteState));
        }
        Ok(reading)
    }

    fn voltage(&self, a: usize, b: usize) -> Result<f64, DiagnosticCode> {
        if a == b {
            return Ok(0.0);
        }
        let memory = &self.states.solver_memory;
        let (Some(a_domain), Some(b_domain)) = (
            memory.reference_domains.get(&a),
            memory.reference_domains.get(&b),
        ) else {
            return Err(DiagnosticCode::UnconnectedProbe);
        };
        if a_domain != b_domain {
            return Err(DiagnosticCode::IncomparableProbeReferences);
        }
        Ok(memory.nodes[&a] - memory.nodes[&b])
    }

    fn resistance(
        &self,
        netlist: &Netlist,
        a: usize,
        b: usize,
    ) -> Result<Option<f64>, DiagnosticCode> {
        let unavailable = DiagnosticCode::ResistanceUnavailable;
        let island_at = |node| {
            netlist
                .islands
                .iter()
                .find(|island| island.node_indices.contains(&node))
        };
        let a_island = island_at(a).ok_or(unavailable)?;
        let b_island = island_at(b).ok_or(unavailable)?;
        let pin_nodes = [a_island, b_island]
            .into_iter()
            .flat_map(|island| &island.node_indices)
            .flat_map(|&node| netlist.nodes[node].pins.iter().map(move |pin| (pin, node)))
            .collect::<HashMap<_, _>>();
        let mut resistors = Vec::new();
        for component in &self.overlay.schematic.components {
            let pin_at = |pin_id: PinId| {
                pin_nodes
                    .get(&PinRef {
                        component_id: component.id().clone(),
                        pin_id,
                    })
                    .copied()
            };
            match component {
                ComponentSpec::Resistor { parameters, .. } => {
                    if let (Some(first), Some(second)) = (pin_at(PinId::Pin1), pin_at(PinId::Pin2))
                        && !self.open.contains(component.id())
                    {
                        resistors.push((first, second, parameters.resistance_ohms));
                    }
                }
                _ if pin_nodes
                    .keys()
                    .any(|pin| &pin.component_id == component.id()) =>
                {
                    return Err(unavailable);
                }
                _ => {}
            }
        }
        if a == b {
            return Ok(Some(0.0));
        }
        if !std::ptr::eq(a_island, b_island) {
            return Ok(None);
        }
        let variables = NodeVariables::for_island(
            netlist.nodes.len(),
            &a_island.node_indices,
            b,
            &HashSet::new(),
        );
        let mut system = MnaSystem::new(variables.voltage_variable_count());
        for (first, second, ohms) in resistors {
            if !system.stamp_conductance(&variables, first, second, 1.0 / ohms) {
                return Err(unavailable);
            }
        }
        system.stamp_current_injection(&variables, a, b, 1.0);
        let solution = system.solve_equilibrated().map_err(|_| unavailable)?;
        let value = solution.values[variables.voltage_variable(a).ok_or(unavailable)?];
        Ok(Some(value))
    }

    fn pin_current(&self, pin: &PinRef) -> Result<f64, DiagnosticCode> {
        let unavailable = DiagnosticCode::UnsupportedProbeTarget;
        let component = self
            .overlay
            .schematic
            .components
            .iter()
            .find(|component| component.id() == &pin.component_id)
            .ok_or(unavailable)?;
        if self.open.contains(component.id()) {
            return Ok(0.0);
        }
        let catalog = probe_catalog();
        let definition = catalog
            .component_types
            .iter()
            .find(|entry| entry.type_id == component.type_id())
            .ok_or(unavailable)?;
        let id = pin.component_id.as_str();
        let number = |key: &str| {
            self.layout
                .slots
                .iter()
                .find(|slot| slot.key == key)
                .and_then(|slot| self.snapshot.values.get(slot.offset as usize).copied())
                .ok_or(unavailable)
        };
        if let [first, second] = definition.pins.as_slice() {
            let mut slots = self.layout.slots.iter().filter(|slot| {
                slot.component_id == pin.component_id && slot.quantity == ResultQuantity::Current
            });
            let slot = slots.next().ok_or(unavailable)?;
            if slots.next().is_some() {
                return Err(unavailable);
            }
            return match pin.pin_id {
                p if p == first.id => number(&slot.key),
                p if p == second.id => number(&slot.key).map(|value| -value),
                _ => Err(unavailable),
            };
        }
        let modeled_groups = definition.pin_groups.as_deref().unwrap_or_default();
        let branch = |key: &str, first: PinId, second: PinId| {
            if pin.pin_id == first {
                Some(number(&format!("{id}/{key}")))
            } else if pin.pin_id == second {
                Some(number(&format!("{id}/{key}")).map(|value| -value))
            } else {
                None
            }
        };
        let branches = |groups: &[(&str, PinId, PinId)]| {
            groups
                .iter()
                .filter_map(|&(key, first, second)| branch(key, first, second))
                .try_fold(0.0, |total, value| Ok(total + value?))
        };
        use ComponentTypeId::*;
        use PinId::*;
        match component.type_id() {
            ThreePhaseSourceStar | ThreePhaseSourceDelta => {
                number(&format!("{id}/{}/current", pin.pin_id.as_str()))
            }
            BjtNpn | BjtPnp => number(&format!("{id}/{}/current", pin.pin_id.as_str())),
            RelaySpdt => branches(&[
                ("coil/current", CoilPositive, CoilNegative),
                ("normally_open/current", Common, NormallyOpen),
                ("normally_closed/current", Common, NormallyClosed),
            ]),
            Contactor => branches(&[
                ("coil/current", A1, A2),
                ("no/current", No1, No2),
                ("nc/current", Nc1, Nc2),
            ]),
            RelayOnDelay | RelayOffDelay => branches(&[
                ("coilCurrentAmps", A1, A2),
                ("noCurrentAmps", TimerNo1, TimerNo2),
                ("ncCurrentAmps", TimerNc1, TimerNc2),
            ]),
            SelectorSwitch => branches(&[
                ("throw1CurrentAmps", Com, Throw1),
                ("throw2CurrentAmps", Com, Throw2),
            ]),
            FrequencyConverter => branches(&[
                ("inputCurrentAmps", InP, InN),
                ("uCurrentAmps", U, Neutral),
                ("vCurrentAmps", V, Neutral),
                ("wCurrentAmps", W, Neutral),
            ]),
            MosfetN if pin.pin_id == D => number(&format!("{id}/drainCurrentAmps")),
            OperationalAmplifier if pin.pin_id == Out => number(&format!("{id}/outputCurrentAmps")),
            Wattmeter => branch("current", I1, I2).unwrap_or(Err(unavailable)),
            _ if modeled_groups
                .iter()
                .any(|group| group.pins.contains(&pin.pin_id)) =>
            {
                modeled_groups
                    .iter()
                    .filter_map(|group| {
                        branch(
                            &format!("{}/current", group.id),
                            group.pins[0],
                            group.pins[1],
                        )
                    })
                    .try_fold(0.0, |total, value| Ok(total + value?))
            }
            _ => Err(unavailable),
        }
    }
}

fn probe_catalog() -> &'static crate::ComponentCatalog {
    static CATALOG: std::sync::OnceLock<crate::ComponentCatalog> = std::sync::OnceLock::new();
    CATALOG.get_or_init(component_catalog)
}
