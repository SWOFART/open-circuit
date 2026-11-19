use crate::{Diagnostic, DiagnosticCode, StableId, pin_groups::PinGroup, slots::group_slots};

use super::{Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

#[derive(Clone, Copy, Debug)]
pub(crate) struct LinkedPole {
    pub group: PinGroup,
    pub terminals: Terminals,
    pub closed_when_active: bool,
}

/// Every pole in a solve sees the same committed or tentative actuator state.
pub(crate) struct LinkedContacts<'a> {
    pub poles: &'a [LinkedPole],
    pub active: bool,
}

impl LinkedContacts<'_> {
    pub(crate) fn conducting(&self) -> impl Iterator<Item = Terminals> + '_ {
        self.poles
            .iter()
            .filter(|pole| pole.closed_when_active == self.active)
            .map(|pole| pole.terminals)
    }

    pub(crate) fn stamp(
        &self,
        id: &StableId,
        resistance: f64,
        system: &mut MnaSystem,
        variables: &NodeVariables,
    ) -> Result<(), Diagnostic> {
        for terminals in self.conducting() {
            if !system.stamp_conductance(
                variables,
                terminals.pin1,
                terminals.pin2,
                1.0 / resistance,
            ) {
                return Err(component_diagnostic(DiagnosticCode::InvalidParameter, id));
            }
        }
        Ok(())
    }

    pub(crate) fn append_results(
        &self,
        id: &StableId,
        resistance: f64,
        node_voltages: &[f64],
        values: &mut Vec<(String, f64)>,
    ) -> Result<(), Diagnostic> {
        let original_len = values.len();
        for pole in self.poles {
            let voltage = node_voltages[pole.terminals.pin1] - node_voltages[pole.terminals.pin2];
            let current = if self.active == pole.closed_when_active {
                voltage / resistance
            } else {
                0.0
            };
            if !voltage.is_finite() || !current.is_finite() {
                values.truncate(original_len);
                return Err(component_diagnostic(DiagnosticCode::NonFiniteState, id));
            }
            let [voltage_slot, current_slot] = group_slots(pole.group);
            values.push((voltage_slot.key(id.as_str()), voltage));
            values.push((current_slot.key(id.as_str()), current));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PinId;

    #[test]
    fn three_poles_share_one_state_and_keep_declaration_order() {
        let id = StableId::parse("00000000-0000-4000-8000-000000000001").unwrap();
        let poles = [("pole1", 1), ("pole2", 2), ("pole3", 3)].map(|(name, pin1)| {
            let group = PinGroup {
                id: name,
                pins: [PinId::Pin1, PinId::Pin2],
            };
            let terminals = group
                .resolve(|pin| match pin {
                    PinId::Pin1 => Some(pin1),
                    PinId::Pin2 => Some(0),
                    _ => None,
                })
                .unwrap();
            LinkedPole {
                group,
                terminals,
                closed_when_active: true,
            }
        });
        let voltages = [0.0, 2.0, -4.0, 8.0];
        for active in [false, true, true, false] {
            let contacts = LinkedContacts {
                poles: &poles,
                active,
            };
            assert_eq!(contacts.conducting().count(), if active { 3 } else { 0 });
            let variables = NodeVariables::for_island(4, &[0, 1, 2, 3], 0, &Default::default());
            let mut system = MnaSystem::new(3);
            for node in 1..=3 {
                assert!(system.stamp_conductance(&variables, node, 0, 0.5));
                assert!(system.stamp_current_injection(&variables, node, 0, 1.0));
            }
            contacts.stamp(&id, 2.0, &mut system, &variables).unwrap();
            assert_eq!(
                system.solve().unwrap().values,
                vec![if active { 1.0 } else { 2.0 }; 3]
            );
            let mut results = Vec::new();
            contacts
                .append_results(&id, 2.0, &voltages, &mut results)
                .unwrap();
            let keys: Vec<_> = results
                .iter()
                .map(|(key, _)| key.as_str().strip_prefix(id.as_str()).unwrap())
                .collect();
            assert_eq!(
                keys,
                [
                    "/pole1/voltage",
                    "/pole1/current",
                    "/pole2/voltage",
                    "/pole2/current",
                    "/pole3/voltage",
                    "/pole3/current"
                ]
            );
            let values: Vec<_> = results.iter().map(|(_, value)| *value).collect();
            assert_eq!(
                values,
                if active {
                    [2.0, 1.0, -4.0, -2.0, 8.0, 4.0]
                } else {
                    [2.0, 0.0, -4.0, 0.0, 8.0, 0.0]
                }
            );
        }
        let contacts = LinkedContacts {
            poles: &poles,
            active: true,
        };
        let mut results = vec![("earlier".to_owned(), 1.0)];
        let error = contacts
            .append_results(&id, 2.0, &[0.0, 2.0, -4.0, f64::INFINITY], &mut results)
            .unwrap_err();
        assert_eq!(error.code, DiagnosticCode::NonFiniteState);
        assert_eq!(results, [("earlier".to_owned(), 1.0)]);
    }
}
