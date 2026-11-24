//! Operator state (docs/contracts/stage-6-control-circuits.md section 4).
//!
//! The position of a switch or a push button is an input of a run. It is owned by the run, not
//! by the schematic: a compile starts every position at its initial one, and nothing but a
//! `set_operator_state` request changes it. A contact that does not conduct is left out of the
//! circuit exactly like a component with an `open` fault, so the solver needs no new device.

use std::collections::{HashMap, HashSet};

use crate::{
    ComponentSpec, DeviceState, EmergencyStopPosition, KeySwitchPosition, OperatorPosition,
    PushButtonPosition, Schematic, SelectorPosition, StableId, SwitchPosition,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContactKind {
    Switch,
    PushButtonNo,
    PushButtonNc,
}

#[derive(Clone, Copy, Debug)]
struct Contact {
    kind: ContactKind,
    /// A switch is closed, a push button is pressed.
    actuated: bool,
    last_transition_time_seconds: Option<f64>,
}

impl Contact {
    fn conducting(&self) -> bool {
        match self.kind {
            ContactKind::Switch | ContactKind::PushButtonNo => self.actuated,
            ContactKind::PushButtonNc => !self.actuated,
        }
    }
}

/// The positions of all switches and push buttons of the active run.
#[derive(Clone, Debug, Default)]
pub(crate) struct OperatorState {
    contacts: HashMap<StableId, Contact>,
    additional: HashMap<StableId, AdditionalContact>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum AdditionalPosition {
    Limit(PushButtonPosition),
    Emergency(EmergencyStopPosition),
    Selector(SelectorPosition),
    Key(KeySwitchPosition),
}

#[derive(Clone, Copy, Debug)]
struct AdditionalContact {
    position: AdditionalPosition,
    last_transition_time_seconds: Option<f64>,
}

impl OperatorState {
    /// Every position at its initial one: a switch from `initialClosed` (open when omitted),
    /// a push button released.
    pub(crate) fn new(schematic: &Schematic) -> Self {
        let contacts = schematic
            .components
            .iter()
            .filter_map(|component| {
                let (id, kind, actuated) = match component {
                    ComponentSpec::SwitchSpst { id, parameters, .. } => (
                        id,
                        ContactKind::Switch,
                        parameters.initial_closed == Some(1.0),
                    ),
                    ComponentSpec::PushButtonNo { id, .. } => {
                        (id, ContactKind::PushButtonNo, false)
                    }
                    ComponentSpec::PushButtonNc { id, .. } => {
                        (id, ContactKind::PushButtonNc, false)
                    }
                    _ => return None,
                };
                Some((
                    id.clone(),
                    Contact {
                        kind,
                        actuated,
                        last_transition_time_seconds: None,
                    },
                ))
            })
            .collect();
        let additional = schematic
            .components
            .iter()
            .filter_map(|component| {
                let position = match component {
                    ComponentSpec::LimitSwitch { .. } => {
                        AdditionalPosition::Limit(PushButtonPosition::Released)
                    }
                    ComponentSpec::EmergencyStop { .. } => {
                        AdditionalPosition::Emergency(EmergencyStopPosition::Armed)
                    }
                    ComponentSpec::SelectorSwitch { parameters, .. } => {
                        AdditionalPosition::Selector(match parameters.initial_position {
                            Some(0.0) => SelectorPosition::Left,
                            Some(2.0) => SelectorPosition::Right,
                            _ => SelectorPosition::Center,
                        })
                    }
                    ComponentSpec::KeySwitch { parameters, .. } => {
                        AdditionalPosition::Key(if parameters.initial_position == Some(1.0) {
                            KeySwitchPosition::On
                        } else {
                            KeySwitchPosition::Off
                        })
                    }
                    _ => return None,
                };
                Some((
                    component.id().clone(),
                    AdditionalContact {
                        position,
                        last_transition_time_seconds: None,
                    },
                ))
            })
            .collect();
        Self {
            contacts,
            additional,
        }
    }

    /// The components whose branch the solver leaves out: `fault_open` plus every contact
    /// that does not conduct in its position.
    pub(crate) fn effective_open(&self, fault_open: &HashSet<StableId>) -> HashSet<StableId> {
        let mut open = fault_open.clone();
        open.extend(
            self.contacts
                .iter()
                .filter(|(_, contact)| !contact.conducting())
                .map(|(id, _)| id.clone()),
        );
        open.extend(
            self.additional
                .iter()
                .filter_map(|(id, contact)| match contact.position {
                    AdditionalPosition::Limit(PushButtonPosition::Released)
                    | AdditionalPosition::Emergency(EmergencyStopPosition::Latched)
                    | AdditionalPosition::Key(KeySwitchPosition::Off) => Some(id.clone()),
                    _ => None,
                }),
        );
        open
    }

    pub(crate) fn is_additional(&self, id: &StableId) -> bool {
        self.additional.contains_key(id)
    }

    /// Update the throw represented by the transient solver's private schematic copy.
    /// The authored schematic and initial position are left untouched.
    pub(crate) fn project_selectors(&self, solver_schematic: &mut Schematic) {
        for component in &mut solver_schematic.components {
            if let ComponentSpec::SelectorSwitch { id, parameters, .. } = component
                && let Some(AdditionalContact {
                    position: AdditionalPosition::Selector(position),
                    ..
                }) = self.additional.get(id)
            {
                parameters.initial_position = Some(match position {
                    SelectorPosition::Left => 0.0,
                    SelectorPosition::Center => 1.0,
                    SelectorPosition::Right => 2.0,
                });
            }
        }
    }

    /// Moves one device to `position` at `time_seconds`. Returns whether the position changed,
    /// or `Err` when the component cannot be operated or the position is not one of its type.
    pub(crate) fn set_position(
        &mut self,
        component_id: &StableId,
        position: OperatorPosition,
        time_seconds: f64,
    ) -> Result<bool, ()> {
        if let Some(contact) = self.additional.get_mut(component_id) {
            let next = match (contact.position, position) {
                (AdditionalPosition::Limit(_), OperatorPosition::Pressed) => {
                    AdditionalPosition::Limit(PushButtonPosition::Pressed)
                }
                (AdditionalPosition::Limit(_), OperatorPosition::Released) => {
                    AdditionalPosition::Limit(PushButtonPosition::Released)
                }
                (AdditionalPosition::Emergency(_), OperatorPosition::Pressed) => {
                    AdditionalPosition::Emergency(EmergencyStopPosition::Latched)
                }
                (AdditionalPosition::Emergency(_), OperatorPosition::Released) => return Ok(false),
                (AdditionalPosition::Emergency(_), OperatorPosition::Reset) => {
                    AdditionalPosition::Emergency(EmergencyStopPosition::Armed)
                }
                (AdditionalPosition::Selector(_), OperatorPosition::Left) => {
                    AdditionalPosition::Selector(SelectorPosition::Left)
                }
                (AdditionalPosition::Selector(_), OperatorPosition::Center) => {
                    AdditionalPosition::Selector(SelectorPosition::Center)
                }
                (AdditionalPosition::Selector(_), OperatorPosition::Right) => {
                    AdditionalPosition::Selector(SelectorPosition::Right)
                }
                (AdditionalPosition::Key(_), OperatorPosition::Open) => {
                    AdditionalPosition::Key(KeySwitchPosition::Off)
                }
                (AdditionalPosition::Key(_), OperatorPosition::Closed) => {
                    AdditionalPosition::Key(KeySwitchPosition::On)
                }
                _ => return Err(()),
            };
            let changed = contact.position != next;
            if changed {
                contact.position = next;
                contact.last_transition_time_seconds = Some(time_seconds);
            }
            return Ok(changed);
        }
        let contact = self.contacts.get_mut(component_id).ok_or(())?;
        let actuated = match (contact.kind, position) {
            (ContactKind::Switch, OperatorPosition::Closed)
            | (ContactKind::PushButtonNo | ContactKind::PushButtonNc, OperatorPosition::Pressed) => {
                true
            }
            (ContactKind::Switch, OperatorPosition::Open)
            | (ContactKind::PushButtonNo | ContactKind::PushButtonNc, OperatorPosition::Released) => {
                false
            }
            _ => return Err(()),
        };
        if contact.actuated == actuated {
            return Ok(false);
        }
        contact.actuated = actuated;
        contact.last_transition_time_seconds = Some(time_seconds);
        Ok(true)
    }

    /// The typed state records of the switches and push buttons, in component-ID order.
    pub(crate) fn device_states(&self) -> Vec<DeviceState> {
        let mut states = self
            .contacts
            .iter()
            .map(|(component_id, contact)| match contact.kind {
                ContactKind::Switch => DeviceState::Switch {
                    component_id: component_id.clone(),
                    position: if contact.actuated {
                        SwitchPosition::Closed
                    } else {
                        SwitchPosition::Open
                    },
                    conducting: contact.conducting(),
                    last_transition_time_seconds: contact.last_transition_time_seconds,
                },
                ContactKind::PushButtonNo | ContactKind::PushButtonNc => DeviceState::PushButton {
                    component_id: component_id.clone(),
                    position: if contact.actuated {
                        PushButtonPosition::Pressed
                    } else {
                        PushButtonPosition::Released
                    },
                    conducting: contact.conducting(),
                    last_transition_time_seconds: contact.last_transition_time_seconds,
                },
            })
            .collect::<Vec<_>>();
        states.extend(self.additional.iter().map(|(id, contact)| {
            let component_id = id.clone();
            let last_transition_time_seconds = contact.last_transition_time_seconds;
            match contact.position {
                AdditionalPosition::Limit(position) => DeviceState::LimitSwitch {
                    component_id,
                    position,
                    last_transition_time_seconds,
                },
                AdditionalPosition::Emergency(position) => DeviceState::EmergencyStop {
                    component_id,
                    position,
                    last_transition_time_seconds,
                },
                AdditionalPosition::Selector(position) => DeviceState::SelectorSwitch {
                    component_id,
                    position,
                    last_transition_time_seconds,
                },
                AdditionalPosition::Key(position) => DeviceState::KeySwitch {
                    component_id,
                    position,
                    last_transition_time_seconds,
                },
            }
        }));
        states.sort_by(|left, right| {
            left.component_id()
                .as_str()
                .cmp(right.component_id().as_str())
        });
        states
    }
}
