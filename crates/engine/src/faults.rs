//! Compile-time faults (docs/contracts/stage-5-assignments.md section 5).
//!
//! A fault is an overlay on a compile request: it never changes the schematic and is never
//! saved. This module validates a fault list and builds the *effective* circuit the Engine
//! solves: parameter faults are applied to a copy of the schematic, a shorted part becomes a
//! fixed resistance, and an open part is marked so that the solver leaves its branch out.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::engine::update_parameter;
use crate::{
    ComponentGeometry, ComponentSpec, ComponentTypeId, Diagnostic, DiagnosticCode,
    DiagnosticReference, GeometryEnvelope, ParameterId, PinId, ResistorParameters,
    ResolvedPinPosition, Schematic, StableId, component_catalog,
};

/// The smallest and largest leakage resistance of a `leak` fault, in ohms (docs/contracts/
/// stage-13-protection-and-safety.md section 3.1). The floor is the bolted-fault resistance.
pub const MIN_LEAK_RESISTANCE_OHMS: f64 = SHORT_FAULT_RESISTANCE_OHMS;
pub const MAX_LEAK_RESISTANCE_OHMS: f64 = 1.0e9;

/// The resistance that replaces a shorted part, the same for every part: 1 mOhm.
pub const SHORT_FAULT_RESISTANCE_OHMS: f64 = 0.001;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "FaultKind.ts")]
pub enum FaultKind {
    /// The component's branch is removed: no current, and the voltage across its pins is
    /// the difference of the potentials of the nodes it would connect.
    Open,
    /// The component's branch is replaced by `SHORT_FAULT_RESISTANCE_OHMS`.
    Short,
    /// One parameter takes the fault's value in place of the schematic's.
    Parameter,
    /// An insulation failure: the named pin is joined to a named exposed part (the frame)
    /// through `value` ohms (docs/contracts/stage-13-protection-and-safety.md section 3).
    Leak,
}

/// One fault of a compile request. It names its component by stable ID.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "FaultSpec.ts")]
pub struct FaultSpec {
    pub id: StableId,
    pub kind: FaultKind,
    pub component_id: StableId,
    /// Required for a `parameter` fault and absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parameter_id: Option<ParameterId>,
    /// Required for a `parameter` fault and absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<f64>,
    /// Required for a `leak` fault and absent otherwise: the pin whose insulation fails.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pin_id: Option<PinId>,
    /// Required for a `leak` fault and absent otherwise: the exposed part it leaks to.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub frame_id: Option<StableId>,
}

/// The fault kinds a component type supports (section 5.2 of the contract).
pub fn supported_fault_kinds(type_id: ComponentTypeId) -> &'static [FaultKind] {
    match type_id {
        ComponentTypeId::Resistor | ComponentTypeId::Capacitor | ComponentTypeId::Inductor => &[
            FaultKind::Open,
            FaultKind::Short,
            FaultKind::Parameter,
            FaultKind::Leak,
        ],
        ComponentTypeId::DcVoltageSource => {
            &[FaultKind::Open, FaultKind::Parameter, FaultKind::Leak]
        }
        ComponentTypeId::RelaySpdt
        | ComponentTypeId::Fuse
        | ComponentTypeId::Voltmeter
        | ComponentTypeId::Ammeter => &[FaultKind::Parameter],
        ComponentTypeId::PmdcMotor => &[FaultKind::Parameter, FaultKind::Leak],
        // Stage 9 and 10 equipment has no other fault kind, but its insulation can fail.
        ComponentTypeId::AcVoltageSource
        | ComponentTypeId::ThreePhaseSourceStar
        | ComponentTypeId::ThreePhaseSourceDelta
        | ComponentTypeId::ContactorThreePole
        | ComponentTypeId::BreakerThreePole
        | ComponentTypeId::ThermalOverloadThreePole
        | ComponentTypeId::InductionMotorThreePhase
        | ComponentTypeId::TransformerThreePhase
        | ComponentTypeId::ThreePhaseLoadStar
        | ComponentTypeId::ThreePhaseLoadDelta => &[FaultKind::Leak],
        // Stage 6 operator controls, the other meters, semiconductors and the Stage 13 earth
        // devices have no fault kinds.
        ComponentTypeId::AcVoltmeter
        | ComponentTypeId::AcAmmeter
        | ComponentTypeId::Wattmeter
        | ComponentTypeId::SwitchSpst
        | ComponentTypeId::PushButtonNo
        | ComponentTypeId::PushButtonNc
        | ComponentTypeId::Contactor
        | ComponentTypeId::Diode
        | ComponentTypeId::BjtNpn
        | ComponentTypeId::BjtPnp
        | ComponentTypeId::EarthElectrode
        | ComponentTypeId::ExposedPart
        | ComponentTypeId::RcdTwoPole
        | ComponentTypeId::RcdFourPole => &[],
    }
}

/// The circuit the Engine solves for a schematic and a list of faults.
#[derive(Clone, Debug)]
pub(crate) struct FaultOverlay {
    /// The schematic with parameter faults applied and shorted parts replaced by resistors.
    /// An open part stays in it, without initial conditions, so that its pins keep a place
    /// in the netlist.
    pub schematic: Schematic,
    /// Components whose branch the solver leaves out.
    pub open: HashSet<StableId>,
    /// The pin geometry of `schematic`: the request's, plus the pins of the resistor that each
    /// `leak` fault adds. Empty when the overlay was built without geometry (validation only).
    pub geometry: GeometryEnvelope,
}

impl FaultOverlay {
    /// Validates `faults` against `schematic` and builds the effective circuit. Any
    /// violation is an `InvalidFault` error that references the component.
    pub(crate) fn new(
        schematic: &Schematic,
        geometry: Option<&GeometryEnvelope>,
        faults: &[FaultSpec],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut effective_geometry = geometry.cloned().unwrap_or(GeometryEnvelope {
            components: Vec::new(),
        });
        if faults.is_empty() {
            return Ok(Self {
                schematic: schematic.clone(),
                open: HashSet::new(),
                geometry: effective_geometry,
            });
        }

        let types = schematic
            .components
            .iter()
            .map(|component| (component.id().clone(), component.type_id()))
            .collect::<HashMap<_, _>>();
        let mut invalid = Vec::new();
        let mut seen_components = HashSet::new();
        let mut seen_ids = HashSet::new();

        for fault in faults {
            let reference = vec![DiagnosticReference::Component {
                component_id: fault.component_id.clone(),
            }];
            let mut reject = || {
                invalid.push(Diagnostic::error(
                    DiagnosticCode::InvalidFault,
                    reference.clone(),
                ));
            };
            let Some(type_id) = types.get(&fault.component_id) else {
                reject();
                continue;
            };
            if !supported_fault_kinds(*type_id).contains(&fault.kind)
                || !seen_components.insert(fault.component_id.clone())
                || !seen_ids.insert(fault.id.clone())
                // A leak adds a branch of its own, which takes the fault's ID.
                || (fault.kind == FaultKind::Leak && types.contains_key(&fault.id))
            {
                reject();
                continue;
            }
            let has_parameter = fault.parameter_id.is_some() && fault.value.is_some();
            let is_parameter_fault = fault.kind == FaultKind::Parameter;
            let well_formed = match fault.kind {
                FaultKind::Parameter => has_parameter && fault.value.is_some_and(f64::is_finite),
                FaultKind::Leak => {
                    fault.parameter_id.is_none()
                        && fault.pin_id.is_some()
                        && fault.value.is_some_and(|value| {
                            (MIN_LEAK_RESISTANCE_OHMS..=MAX_LEAK_RESISTANCE_OHMS).contains(&value)
                        })
                        && fault
                            .frame_id
                            .as_ref()
                            .and_then(|frame| types.get(frame))
                            .is_some_and(|frame| *frame == ComponentTypeId::ExposedPart)
                        && fault.pin_id.is_some_and(|pin| {
                            component_catalog()
                                .component_types
                                .iter()
                                .find(|entry| entry.type_id == *type_id)
                                .is_some_and(|entry| entry.pins.iter().any(|p| p.id == pin))
                        })
                }
                FaultKind::Short | FaultKind::Open => {
                    fault.parameter_id.is_none()
                        && fault.value.is_none()
                        && fault.pin_id.is_none()
                        && fault.frame_id.is_none()
                }
            };
            let leak_fields_absent =
                !is_parameter_fault || (fault.pin_id.is_none() && fault.frame_id.is_none());
            if !well_formed || !leak_fields_absent {
                reject();
            }
        }
        if !invalid.is_empty() {
            return Err(invalid);
        }

        let mut effective = schematic.clone();
        let mut open = HashSet::new();

        for fault in faults {
            match fault.kind {
                FaultKind::Parameter => {
                    let (Some(parameter_id), Some(value)) = (fault.parameter_id, fault.value)
                    else {
                        unreachable!("a well-formed parameter fault has both fields");
                    };
                    // `update_parameter` refuses a parameter the type does not have and every
                    // initial condition, which a fault may not override.
                    if !update_parameter(&mut effective, &fault.component_id, parameter_id, value) {
                        return Err(vec![Diagnostic::error(
                            DiagnosticCode::InvalidFault,
                            vec![DiagnosticReference::Component {
                                component_id: fault.component_id.clone(),
                            }],
                        )]);
                    }
                }
                FaultKind::Short => {
                    for component in &mut effective.components {
                        if component.id() == &fault.component_id {
                            *component = shorted(component);
                        }
                    }
                }
                FaultKind::Leak => {
                    // Without geometry (validation only) there is nothing to build.
                    if geometry.is_some() {
                        let (Some(pin_id), Some(frame_id), Some(value)) =
                            (fault.pin_id, fault.frame_id.as_ref(), fault.value)
                        else {
                            unreachable!("a well-formed leak fault has all three fields");
                        };
                        let Some((leak, pin_position, frame_position)) =
                            leak_branch(schematic, &effective_geometry, fault, pin_id, frame_id)
                        else {
                            return Err(vec![Diagnostic::error(
                                DiagnosticCode::InvalidFault,
                                vec![DiagnosticReference::Component {
                                    component_id: fault.component_id.clone(),
                                }],
                            )]);
                        };
                        effective.components.push(ComponentSpec::Resistor {
                            id: fault.id.clone(),
                            position: pin_position,
                            rotation_deg: 0,
                            parameters: ResistorParameters {
                                resistance_ohms: value,
                            },
                        });
                        effective_geometry.components.push(ComponentGeometry {
                            component_id: leak,
                            pins: vec![
                                ResolvedPinPosition {
                                    pin_id: PinId::Pin1.as_str().to_owned(),
                                    position: pin_position,
                                },
                                ResolvedPinPosition {
                                    pin_id: PinId::Pin2.as_str().to_owned(),
                                    position: frame_position,
                                },
                            ],
                        });
                    }
                }
                FaultKind::Open => {
                    open.insert(fault.component_id.clone());
                    for component in &mut effective.components {
                        if component.id() == &fault.component_id {
                            clear_initial_condition(component);
                        }
                    }
                }
            }
        }

        // A parameter fault whose value leaves the catalog domain, or breaks a rule between
        // the parameters of one component, is a fault error, not a parameter error.
        let before = schematic.validate();
        let after = effective.validate();
        let parameter_targets = faults
            .iter()
            .filter(|fault| fault.kind == FaultKind::Parameter)
            .map(|fault| fault.component_id.clone())
            .collect::<HashSet<_>>();
        let mut caused = Vec::new();

        for diagnostic in &after {
            if before.contains(diagnostic) {
                continue;
            }
            for reference in &diagnostic.references {
                if let DiagnosticReference::Component { component_id } = reference
                    && parameter_targets.contains(component_id)
                {
                    caused.push(Diagnostic::error(
                        DiagnosticCode::InvalidFault,
                        vec![DiagnosticReference::Component {
                            component_id: component_id.clone(),
                        }],
                    ));
                }
            }
        }
        if !caused.is_empty() {
            caused.dedup();
            return Err(caused);
        }

        Ok(Self {
            schematic: effective,
            open,
            geometry: effective_geometry,
        })
    }
}

/// The branch a `leak` fault adds: its ID, the position of the leaking pin and the position of
/// the frame's pin. A resistor with its pins at those two points joins their nets exactly as a
/// drawn resistor would. `None` when the geometry does not hold one of the two pins.
fn leak_branch(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    fault: &FaultSpec,
    pin_id: PinId,
    frame_id: &StableId,
) -> Option<(StableId, crate::GridPoint, crate::GridPoint)> {
    let position_of = |component_id: &StableId, pin: PinId| {
        geometry
            .components
            .iter()
            .find(|entry| entry.component_id == *component_id)?
            .pins
            .iter()
            .find(|entry| entry.pin_id == pin.as_str())
            .map(|entry| entry.position)
    };
    let pin_position = position_of(&fault.component_id, pin_id)?;
    let frame = schematic
        .components
        .iter()
        .find(|component| component.id() == frame_id)?;
    let frame_position = position_of(frame.id(), PinId::Ep)?;
    Some((fault.id.clone(), pin_position, frame_position))
}

/// A shorted part is a plain resistor of the fixed fault resistance, on the same pins.
fn shorted(component: &ComponentSpec) -> ComponentSpec {
    ComponentSpec::Resistor {
        id: component.id().clone(),
        position: component.position(),
        rotation_deg: component.rotation_deg(),
        parameters: ResistorParameters {
            resistance_ohms: SHORT_FAULT_RESISTANCE_OHMS,
        },
    }
}

/// An open part has no state, so an explicit initial condition does not apply to it.
fn clear_initial_condition(component: &mut ComponentSpec) {
    match component {
        ComponentSpec::Capacitor { parameters, .. } => parameters.initial_voltage_volts = None,
        ComponentSpec::Inductor { parameters, .. } => parameters.initial_current_amps = None,
        _ => {}
    }
}
