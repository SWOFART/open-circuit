//! The result slots of each component type (stage-1.md section 5, stage-3-dynamics.md section
//! 7.1, stage-4-components.md section 5). The result layout and the validation of an
//! assignment's signal keys read the same table, so they cannot drift apart.

use crate::{ComponentTypeId, ResultQuantity, ResultSignConvention, Unit};

/// One slot of a component type. Its key is `<componentId><separator><suffix>`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SlotDefinition {
    pub quantity: ResultQuantity,
    pub unit: Unit,
    pub separator: char,
    pub suffix: &'static str,
    pub sign_convention: ResultSignConvention,
}

impl SlotDefinition {
    /// The layout key of this slot for a component with the given ID.
    pub fn key(&self, component_id: &str) -> String {
        format!("{component_id}{}{}", self.separator, self.suffix)
    }
}

const fn slot(
    quantity: ResultQuantity,
    unit: Unit,
    separator: char,
    suffix: &'static str,
    sign_convention: ResultSignConvention,
) -> SlotDefinition {
    SlotDefinition {
        quantity,
        unit,
        separator,
        suffix,
        sign_convention,
    }
}

/// The slots of a type, in layout order. A relay has six ports; a fuse also reports its
/// accumulated `I^2 t`, and a motor its angular velocity.
pub(crate) fn slot_definitions(type_id: ComponentTypeId) -> Vec<SlotDefinition> {
    use ResultQuantity::{AccumulatedI2t, AngularVelocity, Current, Voltage};
    use ResultSignConvention::{
        CurrentIntoPin1, PositiveRotationWithPositivePin1Current, VoltagePin1MinusPin2,
    };

    let voltage = slot(Voltage, Unit::Volt, ':', "voltage", VoltagePin1MinusPin2);
    let current = slot(Current, Unit::Amp, ':', "current", CurrentIntoPin1);
    match type_id {
        ComponentTypeId::RelaySpdt => vec![
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "coil/voltage",
                VoltagePin1MinusPin2,
            ),
            slot(Current, Unit::Amp, '/', "coil/current", CurrentIntoPin1),
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "normally_open/voltage",
                VoltagePin1MinusPin2,
            ),
            slot(
                Current,
                Unit::Amp,
                '/',
                "normally_open/current",
                CurrentIntoPin1,
            ),
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "normally_closed/voltage",
                VoltagePin1MinusPin2,
            ),
            slot(
                Current,
                Unit::Amp,
                '/',
                "normally_closed/current",
                CurrentIntoPin1,
            ),
        ],
        ComponentTypeId::Fuse => vec![
            voltage,
            current,
            slot(
                AccumulatedI2t,
                Unit::AmpereSquaredSecond,
                '/',
                "accumulated_i2t",
                CurrentIntoPin1,
            ),
        ],
        ComponentTypeId::PmdcMotor => vec![
            voltage,
            current,
            slot(
                AngularVelocity,
                Unit::RadianPerSecond,
                '/',
                "angular_velocity",
                PositiveRotationWithPositivePin1Current,
            ),
        ],
        ComponentTypeId::Resistor
        | ComponentTypeId::DcVoltageSource
        | ComponentTypeId::Capacitor
        | ComponentTypeId::Inductor
        | ComponentTypeId::Voltmeter
        | ComponentTypeId::Ammeter => vec![voltage, current],
    }
}

/// The slot of a type that a layout key `<componentId><separator><suffix>` names, if any.
/// `rest` is the key after the component ID, separator included.
pub(crate) fn slot_for_key_rest(type_id: ComponentTypeId, rest: &str) -> Option<SlotDefinition> {
    slot_definitions(type_id)
        .into_iter()
        .find(|definition| rest.strip_prefix(definition.separator) == Some(definition.suffix))
}
