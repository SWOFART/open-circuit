//! Operator state (docs/contracts/stage-6-control-circuits.md section 4).
//!
//! The position of a switch or a push button is an input of a run. It is owned by the run, not
//! by the schematic: a compile starts every position at its initial one, and nothing but a
//! `set_operator_state` request changes it. A contact that does not conduct is left out of the
//! circuit exactly like a component with an `open` fault, so the solver needs no new device.

use std::collections::{HashMap, HashSet};

use crate::{
    ComponentSpec, DeviceState, OperatorPosition, PushButtonPosition, Schematic, StableId,
    SwitchPosition,
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
        Self { contacts }
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
        open
    }

    /// Moves one device to `position` at `time_seconds`. Returns whether the position changed,
    /// or `Err` when the component cannot be operated or the position is not one of its type.
    pub(crate) fn set_position(
        &mut self,
        component_id: &StableId,
        position: OperatorPosition,
        time_seconds: f64,
    ) -> Result<bool, ()> {
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
        states.sort_by(|left, right| {
            left.component_id()
                .as_str()
                .cmp(right.component_id().as_str())
        });
        states
    }
}
