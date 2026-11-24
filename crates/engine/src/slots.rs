//! The result slots of each component type (stage-1.md section 5, stage-3-dynamics.md section
//! 7.1, stage-4-components.md section 5). The result layout and the validation of an
//! assignment's signal keys read the same table, so they cannot drift apart.

use crate::{
    ComponentTypeId, ResultQuantity, ResultSignConvention, Unit,
    pin_groups::{CONTACTOR_AUXILIARIES, CONTACTOR_COIL, POWER_POLES, PinGroup},
};

/// One slot of a component type. Its key is `<componentId><separator><suffix>`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SlotDefinition {
    pub quantity: ResultQuantity,
    pub unit: Unit,
    pub separator: char,
    pub suffix: &'static str,
    group: Option<&'static str>,
    pub sign_convention: ResultSignConvention,
}

impl SlotDefinition {
    /// The layout key of this slot for a component with the given ID.
    pub fn key(&self, component_id: &str) -> String {
        match self.group {
            Some(group) => format!("{component_id}/{group}/{}", self.suffix),
            None => format!("{component_id}{}{}", self.separator, self.suffix),
        }
    }

    fn matches_rest(self, rest: &str) -> bool {
        let Some(rest) = rest.strip_prefix(self.separator) else {
            return false;
        };
        match self.group {
            Some(group) => rest.split_once('/') == Some((group, self.suffix)),
            None => rest == self.suffix,
        }
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
        group: None,
        sign_convention,
    }
}

pub(crate) fn group_slots(group: PinGroup) -> [SlotDefinition; 2] {
    let mut slots = [
        slot(
            ResultQuantity::Voltage,
            Unit::Volt,
            '/',
            "voltage",
            ResultSignConvention::VoltagePin1MinusPin2,
        ),
        slot(
            ResultQuantity::Current,
            Unit::Amp,
            '/',
            "current",
            ResultSignConvention::CurrentIntoPin1,
        ),
    ];
    for slot in &mut slots {
        slot.group = Some(group.id);
    }
    slots
}

/// The slots of a type, in layout order. A relay has six ports; a fuse also reports its
/// accumulated `I^2 t`, and a motor its angular velocity.
/// The slots of one component. A breaker with a trip class (1 to 3) also has its thermal dose
/// after `tripped`; every other component has the slots of its type.
pub(crate) fn component_slot_definitions(component: &crate::ComponentSpec) -> Vec<SlotDefinition> {
    let mut definitions = slot_definitions(component.type_id());
    if let crate::ComponentSpec::BreakerThreePole { parameters, .. } = component
        && parameters.thermal_magnetic()
    {
        definitions.push(breaker_thermal_dose_slot());
    }
    definitions
}

fn breaker_thermal_dose_slot() -> SlotDefinition {
    slot(
        ResultQuantity::ThermalDose,
        Unit::Dimensionless,
        '/',
        "thermal_dose",
        ResultSignConvention::Unsigned,
    )
}

pub(crate) fn slot_definitions(type_id: ComponentTypeId) -> Vec<SlotDefinition> {
    use ResultQuantity::{
        AccumulatedI2t, AngularVelocity, ApparentPower, Current, Frequency, MeanCurrent,
        MeanVoltage, PeakCurrent, PeakVoltage, PhaseAngle, PowerFactor, ReactivePower, RealPower,
        RmsCurrent, RmsVoltage, Valid, Voltage,
    };
    use ResultSignConvention::{
        CurrentIntoPin1, Flag, LoadConvention, PositiveRotationWithPositivePin1Current, Unsigned,
        VoltagePin1MinusPin2,
    };

    let voltage = slot(Voltage, Unit::Volt, ':', "voltage", VoltagePin1MinusPin2);
    let current = slot(Current, Unit::Amp, ':', "current", CurrentIntoPin1);
    match type_id {
        ComponentTypeId::BjtNpn | ComponentTypeId::BjtPnp => vec![
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "base_emitter/voltage",
                VoltagePin1MinusPin2,
            ),
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "collector_emitter/voltage",
                VoltagePin1MinusPin2,
            ),
            slot(
                Current,
                Unit::Amp,
                '/',
                "collector/current",
                CurrentIntoPin1,
            ),
            slot(Current, Unit::Amp, '/', "base/current", CurrentIntoPin1),
            slot(Current, Unit::Amp, '/', "emitter/current", CurrentIntoPin1),
        ],
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
        ComponentTypeId::Contactor => std::iter::once(CONTACTOR_COIL)
            .chain(CONTACTOR_AUXILIARIES)
            .flat_map(group_slots)
            .collect(),
        ComponentTypeId::RelayOnDelay | ComponentTypeId::RelayOffDelay => vec![
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "coilVoltageVolts",
                VoltagePin1MinusPin2,
            ),
            slot(Current, Unit::Amp, '/', "coilCurrentAmps", CurrentIntoPin1),
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "noVoltageVolts",
                VoltagePin1MinusPin2,
            ),
            slot(Current, Unit::Amp, '/', "noCurrentAmps", CurrentIntoPin1),
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "ncVoltageVolts",
                VoltagePin1MinusPin2,
            ),
            slot(Current, Unit::Amp, '/', "ncCurrentAmps", CurrentIntoPin1),
            slot(
                ResultQuantity::ElapsedTime,
                Unit::Second,
                '/',
                "timerElapsedSeconds",
                Unsigned,
            ),
        ],
        ComponentTypeId::ContactorThreePole => group_slots(CONTACTOR_COIL)
            .into_iter()
            .chain(std::iter::once(slot(
                ResultQuantity::MeanSquareCurrent,
                Unit::AmpereSquared,
                '/',
                "coil/current_squared",
                Unsigned,
            )))
            .chain(
                POWER_POLES
                    .into_iter()
                    .chain(CONTACTOR_AUXILIARIES)
                    .flat_map(group_slots),
            )
            .collect(),
        ComponentTypeId::RcdTwoPole | ComponentTypeId::RcdFourPole => {
            let groups: &[crate::pin_groups::PinGroup] = if type_id == ComponentTypeId::RcdTwoPole {
                &crate::pin_groups::RCD_TWO_POLES
            } else {
                &crate::pin_groups::RCD_FOUR_POLES
            };
            groups
                .iter()
                .copied()
                .flat_map(group_slots)
                .chain([
                    slot(Valid, Unit::Dimensionless, '/', "tripped", Flag),
                    slot(RmsCurrent, Unit::Amp, '/', "residual_rms", Unsigned),
                    slot(
                        ResultQuantity::ThermalDose,
                        Unit::Dimensionless,
                        '/',
                        "trip_dose",
                        Unsigned,
                    ),
                ])
                .collect()
        }
        ComponentTypeId::BreakerThreePole => POWER_POLES
            .into_iter()
            .flat_map(group_slots)
            .chain(std::iter::once(slot(
                Valid,
                Unit::Dimensionless,
                '/',
                "tripped",
                Flag,
            )))
            .collect(),
        ComponentTypeId::ThermalOverloadThreePole => POWER_POLES
            .into_iter()
            .chain(CONTACTOR_AUXILIARIES)
            .flat_map(group_slots)
            .chain(std::iter::once(slot(
                ResultQuantity::ThermalDose,
                Unit::Dimensionless,
                '/',
                "thermal_dose",
                Unsigned,
            )))
            .collect(),
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
        ComponentTypeId::InductionMotorThreePhase => crate::pin_groups::INDUCTION_WINDINGS
            .into_iter()
            .flat_map(group_slots)
            .chain([
                slot(
                    ResultQuantity::AngularVelocity,
                    Unit::RadianPerSecond,
                    '/',
                    "angular_velocity",
                    ResultSignConvention::PositivePhaseSequence,
                ),
                slot(
                    ResultQuantity::ElectromagneticTorque,
                    Unit::NewtonMeter,
                    '/',
                    "electromagnetic_torque",
                    ResultSignConvention::PositivePhaseSequence,
                ),
            ])
            .collect(),
        ComponentTypeId::TransformerThreePhase => crate::pin_groups::TRANSFORMER_WINDINGS
            .into_iter()
            .flat_map(group_slots)
            .collect(),
        ComponentTypeId::ThreePhaseLoadStar => crate::pin_groups::STAR_LOAD_WINDINGS
            .into_iter()
            .flat_map(group_slots)
            .chain([slot(Current, Unit::Amp, '/', "n/current", CurrentIntoPin1)])
            .collect(),
        ComponentTypeId::ThreePhaseLoadDelta => crate::pin_groups::DELTA_LOAD_WINDINGS
            .into_iter()
            .flat_map(group_slots)
            .chain(
                ["l1/current", "l2/current", "l3/current"]
                    .map(|name| slot(Current, Unit::Amp, '/', name, CurrentIntoPin1)),
            )
            .collect(),
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
        // The earth electrode has no results. The exposed part reports the touch voltage over the
        // window of its frequency, the hazard flag and one validity flag (docs/contracts/
        // stage-13-protection-and-safety.md section 7.3); it has no instantaneous slot.
        ComponentTypeId::EarthElectrode => Vec::new(),
        ComponentTypeId::ExposedPart => vec![
            slot(RmsVoltage, Unit::Volt, '/', "touch_voltage_rms", Unsigned),
            slot(PeakVoltage, Unit::Volt, '/', "touch_voltage_peak", Unsigned),
            slot(Valid, Unit::Dimensionless, '/', "hazard", Flag),
            slot(Valid, Unit::Dimensionless, '/', "valid", Flag),
        ],
        ComponentTypeId::Diode
        | ComponentTypeId::Resistor
        | ComponentTypeId::DcVoltageSource
        | ComponentTypeId::AcVoltageSource
        | ComponentTypeId::Capacitor
        | ComponentTypeId::Inductor
        | ComponentTypeId::Voltmeter
        | ComponentTypeId::Ammeter
        | ComponentTypeId::SwitchSpst
        | ComponentTypeId::PushButtonNo
        | ComponentTypeId::PushButtonNc => vec![voltage, current],
        ComponentTypeId::ThreePhaseSourceStar => vec![
            slot(Voltage, Unit::Volt, '/', "l1/voltage", VoltagePin1MinusPin2),
            slot(Current, Unit::Amp, '/', "l1/current", CurrentIntoPin1),
            slot(Voltage, Unit::Volt, '/', "l2/voltage", VoltagePin1MinusPin2),
            slot(Current, Unit::Amp, '/', "l2/current", CurrentIntoPin1),
            slot(Voltage, Unit::Volt, '/', "l3/voltage", VoltagePin1MinusPin2),
            slot(Current, Unit::Amp, '/', "l3/current", CurrentIntoPin1),
            slot(Current, Unit::Amp, '/', "n/current", CurrentIntoPin1),
        ],
        ComponentTypeId::ThreePhaseSourceDelta => vec![
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "l12/voltage",
                VoltagePin1MinusPin2,
            ),
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "l23/voltage",
                VoltagePin1MinusPin2,
            ),
            slot(
                Voltage,
                Unit::Volt,
                '/',
                "l31/voltage",
                VoltagePin1MinusPin2,
            ),
            slot(Current, Unit::Amp, '/', "l1/current", CurrentIntoPin1),
            slot(Current, Unit::Amp, '/', "l2/current", CurrentIntoPin1),
            slot(Current, Unit::Amp, '/', "l3/current", CurrentIntoPin1),
        ],
        ComponentTypeId::AcVoltmeter => vec![
            voltage,
            current,
            slot(RmsVoltage, Unit::Volt, '/', "rms_voltage", Unsigned),
            slot(
                MeanVoltage,
                Unit::Volt,
                '/',
                "mean_voltage",
                VoltagePin1MinusPin2,
            ),
            slot(PeakVoltage, Unit::Volt, '/', "peak_voltage", Unsigned),
            slot(Frequency, Unit::Hertz, '/', "frequency", Unsigned),
            slot(Valid, Unit::Dimensionless, '/', "frequency_valid", Flag),
            slot(Valid, Unit::Dimensionless, '/', "valid", Flag),
        ],
        ComponentTypeId::AcAmmeter => vec![
            voltage,
            current,
            slot(RmsCurrent, Unit::Amp, '/', "rms_current", Unsigned),
            slot(MeanCurrent, Unit::Amp, '/', "mean_current", CurrentIntoPin1),
            slot(PeakCurrent, Unit::Amp, '/', "peak_current", Unsigned),
            slot(Frequency, Unit::Hertz, '/', "frequency", Unsigned),
            slot(Valid, Unit::Dimensionless, '/', "frequency_valid", Flag),
            slot(Valid, Unit::Dimensionless, '/', "valid", Flag),
        ],
        ComponentTypeId::Wattmeter => vec![
            slot(Voltage, Unit::Volt, '/', "voltage", VoltagePin1MinusPin2),
            slot(Current, Unit::Amp, '/', "current", CurrentIntoPin1),
            slot(RmsVoltage, Unit::Volt, '/', "rms_voltage", Unsigned),
            slot(RmsCurrent, Unit::Amp, '/', "rms_current", Unsigned),
            slot(RealPower, Unit::Watt, '/', "real_power", LoadConvention),
            slot(
                ReactivePower,
                Unit::VoltAmpereReactive,
                '/',
                "reactive_power",
                LoadConvention,
            ),
            slot(
                ApparentPower,
                Unit::VoltAmpere,
                '/',
                "apparent_power",
                Unsigned,
            ),
            slot(
                PowerFactor,
                Unit::Dimensionless,
                '/',
                "power_factor",
                LoadConvention,
            ),
            slot(PhaseAngle, Unit::Degree, '/', "phase_angle", LoadConvention),
            slot(Valid, Unit::Dimensionless, '/', "valid", Flag),
        ],
    }
}

/// The slot of a type that a layout key `<componentId><separator><suffix>` names, if any.
/// `rest` is the key after the component ID, separator included.
pub(crate) fn slot_for_key_rest(type_id: ComponentTypeId, rest: &str) -> Option<SlotDefinition> {
    let mut definitions = slot_definitions(type_id);
    // A breaker's thermal dose exists only for a trip class; the key names it for any breaker.
    if type_id == ComponentTypeId::BreakerThreePole {
        definitions.push(breaker_thermal_dose_slot());
    }
    definitions
        .into_iter()
        .find(|definition| definition.matches_rest(rest))
}
