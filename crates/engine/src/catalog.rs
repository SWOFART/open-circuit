use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Stable IDs are serialized exactly as specified by the Stage 1 contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ComponentTypeId.ts")]
pub enum ComponentTypeId {
    Resistor,
    DcVoltageSource,
    Capacitor,
    Inductor,
    RelaySpdt,
    Fuse,
    PmdcMotor,
    Voltmeter,
    Ammeter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "PinId.ts")]
pub enum PinId {
    Pin1,
    Pin2,
    CoilPositive,
    CoilNegative,
    Common,
    NormallyOpen,
    NormallyClosed,
}

impl PinId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pin1 => "pin1",
            Self::Pin2 => "pin2",
            Self::CoilPositive => "coil_positive",
            Self::CoilNegative => "coil_negative",
            Self::Common => "common",
            Self::NormallyOpen => "normally_open",
            Self::NormallyClosed => "normally_closed",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ParameterId.ts")]
pub enum ParameterId {
    ResistanceOhms,
    VoltageVolts,
    InternalResistanceOhms,
    CapacitanceFarads,
    InitialVoltageVolts,
    InductanceHenries,
    InitialCurrentAmps,
    CoilResistanceOhms,
    CoilInductanceHenries,
    PickupCurrentAmps,
    DropoutCurrentAmps,
    ClosedContactResistanceOhms,
    IntactResistanceOhms,
    TripI2tAmpereSquaredSeconds,
    ArmatureResistanceOhms,
    ArmatureInductanceHenries,
    MotorConstantNewtonMetersPerAmp,
    RotorInertiaKilogramMetersSquared,
    ViscousFrictionNewtonMeterSecondsPerRadian,
    LoadTorqueNewtonMeters,
    InitialAngularVelocityRadiansPerSecond,
    InitialArmatureCurrentAmps,
    InputResistanceOhms,
    BurdenResistanceOhms,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "Unit.ts")]
pub enum Unit {
    Ohm,
    Volt,
    Amp,
    Farad,
    Henry,
    AmpereSquaredSecond,
    KilogramMeterSquared,
    NewtonMeter,
    NewtonMeterPerAmp,
    NewtonMeterSecondPerRadian,
    RadianPerSecond,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "PinRole.ts")]
pub enum PinRole {
    Unpolarized,
    Positive,
    Negative,
}

/// Every domain includes the finite-f64 requirement from the Stage 1 contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "NumericDomain.ts")]
pub enum NumericDomain {
    Finite,
    StrictlyPositive,
    NonNegative,
}

impl NumericDomain {
    pub const fn accepts(self, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }

        match self {
            Self::Finite => true,
            Self::StrictlyPositive => value > 0.0,
            Self::NonNegative => value >= 0.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "PinDefinition.ts")]
pub struct PinDefinition {
    pub id: PinId,
    pub role: PinRole,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ParameterDefinition.ts")]
pub struct ParameterDefinition {
    pub id: ParameterId,
    pub unit: Unit,
    pub default_value: f64,
    pub domain: NumericDomain,
    pub description_key: String,
    pub optional: bool,
    pub initial_condition: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ComponentCatalogEntry.ts")]
pub struct ComponentCatalogEntry {
    pub type_id: ComponentTypeId,
    pub display_name_key: String,
    pub pins: Vec<PinDefinition>,
    pub parameters: Vec<ParameterDefinition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ComponentCatalog.ts")]
pub struct ComponentCatalog {
    pub component_types: Vec<ComponentCatalogEntry>,
}

/// Returns the authoritative Engine catalog. Values use SI units.
pub fn component_catalog() -> ComponentCatalog {
    let two_unpolarized_pins = || {
        vec![
            PinDefinition {
                id: PinId::Pin1,
                role: PinRole::Unpolarized,
            },
            PinDefinition {
                id: PinId::Pin2,
                role: PinRole::Unpolarized,
            },
        ]
    };

    ComponentCatalog {
        component_types: vec![
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Resistor,
                display_name_key: "component.resistor.name".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![ParameterDefinition {
                    id: ParameterId::ResistanceOhms,
                    unit: Unit::Ohm,
                    default_value: 1000.0,
                    domain: NumericDomain::StrictlyPositive,
                    description_key: "component.resistor.resistance".to_owned(),
                    optional: false,
                    initial_condition: false,
                }],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::DcVoltageSource,
                display_name_key: "component.dcVoltageSource.name".to_owned(),
                pins: vec![
                    PinDefinition {
                        id: PinId::Pin1,
                        role: PinRole::Positive,
                    },
                    PinDefinition {
                        id: PinId::Pin2,
                        role: PinRole::Negative,
                    },
                ],
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::VoltageVolts,
                        unit: Unit::Volt,
                        default_value: 5.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.dcVoltageSource.voltage".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::InternalResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.1,
                        domain: NumericDomain::NonNegative,
                        description_key: "component.dcVoltageSource.internalResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                ],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Capacitor,
                display_name_key: "component.capacitor.name".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::CapacitanceFarads,
                        unit: Unit::Farad,
                        default_value: 1e-5,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.capacitor.capacitance".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::InitialVoltageVolts,
                        unit: Unit::Volt,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.capacitor.initialVoltage".to_owned(),
                        optional: true,
                        initial_condition: true,
                    },
                ],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Inductor,
                display_name_key: "component.inductor.name".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::InductanceHenries,
                        unit: Unit::Henry,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.inductor.inductance".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::InitialCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.inductor.initialCurrent".to_owned(),
                        optional: true,
                        initial_condition: true,
                    },
                ],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::RelaySpdt,
                display_name_key: "component.relaySpdt.name".to_owned(),
                pins: vec![
                    PinDefinition {
                        id: PinId::CoilPositive,
                        role: PinRole::Positive,
                    },
                    PinDefinition {
                        id: PinId::CoilNegative,
                        role: PinRole::Negative,
                    },
                    PinDefinition {
                        id: PinId::Common,
                        role: PinRole::Unpolarized,
                    },
                    PinDefinition {
                        id: PinId::NormallyOpen,
                        role: PinRole::Unpolarized,
                    },
                    PinDefinition {
                        id: PinId::NormallyClosed,
                        role: PinRole::Unpolarized,
                    },
                ],
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::CoilResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 100.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.relaySpdt.coilResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::CoilInductanceHenries,
                        unit: Unit::Henry,
                        default_value: 0.01,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.relaySpdt.coilInductance".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::PickupCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.04,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.relaySpdt.pickupCurrent".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::DropoutCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.02,
                        domain: NumericDomain::NonNegative,
                        description_key: "component.relaySpdt.dropoutCurrent".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::ClosedContactResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.relaySpdt.closedContactResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                ],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Fuse,
                display_name_key: "component.fuse.name".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::IntactResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.01,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.fuse.intactResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::TripI2tAmpereSquaredSeconds,
                        unit: Unit::AmpereSquaredSecond,
                        default_value: 0.004,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.fuse.tripI2t".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                ],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::PmdcMotor,
                display_name_key: "component.pmdcMotor.name".to_owned(),
                pins: vec![
                    PinDefinition {
                        id: PinId::Pin1,
                        role: PinRole::Positive,
                    },
                    PinDefinition {
                        id: PinId::Pin2,
                        role: PinRole::Negative,
                    },
                ],
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::ArmatureResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 2.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.pmdcMotor.armatureResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::ArmatureInductanceHenries,
                        unit: Unit::Henry,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.pmdcMotor.armatureInductance".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::MotorConstantNewtonMetersPerAmp,
                        unit: Unit::NewtonMeterPerAmp,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.pmdcMotor.motorConstant".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::RotorInertiaKilogramMetersSquared,
                        unit: Unit::KilogramMeterSquared,
                        default_value: 0.01,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.pmdcMotor.rotorInertia".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::ViscousFrictionNewtonMeterSecondsPerRadian,
                        unit: Unit::NewtonMeterSecondPerRadian,
                        default_value: 0.01,
                        domain: NumericDomain::NonNegative,
                        description_key: "component.pmdcMotor.viscousFriction".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::LoadTorqueNewtonMeters,
                        unit: Unit::NewtonMeter,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.pmdcMotor.loadTorque".to_owned(),
                        optional: false,
                        initial_condition: false,
                    },
                    ParameterDefinition {
                        id: ParameterId::InitialAngularVelocityRadiansPerSecond,
                        unit: Unit::RadianPerSecond,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.pmdcMotor.initialAngularVelocity".to_owned(),
                        optional: true,
                        initial_condition: true,
                    },
                    ParameterDefinition {
                        id: ParameterId::InitialArmatureCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.pmdcMotor.initialArmatureCurrent".to_owned(),
                        optional: true,
                        initial_condition: true,
                    },
                ],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Voltmeter,
                display_name_key: "component.voltmeter.name".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![ParameterDefinition {
                    id: ParameterId::InputResistanceOhms,
                    unit: Unit::Ohm,
                    default_value: 10_000_000.0,
                    domain: NumericDomain::StrictlyPositive,
                    description_key: "component.voltmeter.inputResistance".to_owned(),
                    optional: false,
                    initial_condition: false,
                }],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Ammeter,
                display_name_key: "component.ammeter.name".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![ParameterDefinition {
                    id: ParameterId::BurdenResistanceOhms,
                    unit: Unit::Ohm,
                    default_value: 0.1,
                    domain: NumericDomain::StrictlyPositive,
                    description_key: "component.ammeter.burdenResistance".to_owned(),
                    optional: false,
                    initial_condition: false,
                }],
            },
        ],
    }
}
