use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::pin_groups::{RCD_FOUR_POLES, RCD_TWO_POLES};
use crate::supported_fault_kinds;

/// Stable IDs are serialized exactly as specified by the Stage 1 contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ComponentTypeId.ts")]
pub enum ComponentTypeId {
    Resistor,
    DcVoltageSource,
    AcVoltageSource,
    ThreePhaseSourceStar,
    ThreePhaseSourceDelta,
    AcVoltmeter,
    AcAmmeter,
    Wattmeter,
    Capacitor,
    Inductor,
    RelaySpdt,
    LimitSwitch,
    EmergencyStop,
    SelectorSwitch,
    KeySwitch,
    RelayOnDelay,
    RelayOffDelay,
    Fuse,
    PmdcMotor,
    Voltmeter,
    Ammeter,
    SwitchSpst,
    PushButtonNo,
    PushButtonNc,
    Contactor,
    Diode,
    BjtNpn,
    BjtPnp,
    ContactorThreePole,
    BreakerThreePole,
    ThermalOverloadThreePole,
    InductionMotorThreePhase,
    TransformerThreePhase,
    ThreePhaseLoadStar,
    ThreePhaseLoadDelta,
    EarthElectrode,
    ExposedPart,
    RcdTwoPole,
    RcdFourPole,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "PinId.ts")]
pub enum PinId {
    Pin1,
    Pin2,
    L1,
    L2,
    L3,
    T1,
    T2,
    T3,
    N,
    V1,
    V2,
    I1,
    I2,
    Anode,
    Cathode,
    Collector,
    Base,
    Emitter,
    CoilPositive,
    CoilNegative,
    Common,
    NormallyOpen,
    NormallyClosed,
    A1,
    A2,
    #[serde(rename = "no_1")]
    No1,
    #[serde(rename = "no_2")]
    No2,
    #[serde(rename = "nc_1")]
    Nc1,
    #[serde(rename = "nc_2")]
    Nc2,
    U1,
    U2,
    W1,
    W2,
    P1a,
    P1b,
    P2a,
    P2b,
    P3a,
    P3b,
    S1a,
    S1b,
    S2a,
    S2b,
    S3a,
    S3b,
    Pe,
    Ep,
    L4,
    T4,
    #[serde(rename = "no1")]
    TimerNo1,
    P,
    Com,
    Throw1,
    Throw2,
    #[serde(rename = "no2")]
    TimerNo2,
    #[serde(rename = "nc1")]
    TimerNc1,
    #[serde(rename = "nc2")]
    TimerNc2,
}

impl PinId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pin1 => "pin1",
            Self::P => "p",
            Self::Com => "com",
            Self::Throw1 => "throw1",
            Self::Throw2 => "throw2",
            Self::Pin2 => "pin2",
            Self::L1 => "l1",
            Self::L2 => "l2",
            Self::L3 => "l3",
            Self::T1 => "t1",
            Self::T2 => "t2",
            Self::T3 => "t3",
            Self::N => "n",
            Self::V1 => "v1",
            Self::V2 => "v2",
            Self::I1 => "i1",
            Self::I2 => "i2",
            Self::Anode => "anode",
            Self::Cathode => "cathode",
            Self::Collector => "collector",
            Self::Base => "base",
            Self::Emitter => "emitter",
            Self::CoilPositive => "coil_positive",
            Self::CoilNegative => "coil_negative",
            Self::Common => "common",
            Self::NormallyOpen => "normally_open",
            Self::NormallyClosed => "normally_closed",
            Self::A1 => "a1",
            Self::A2 => "a2",
            Self::TimerNo1 => "no1",
            Self::No1 => "no_1",
            Self::TimerNo2 => "no2",
            Self::No2 => "no_2",
            Self::TimerNc1 => "nc1",
            Self::Nc1 => "nc_1",
            Self::TimerNc2 => "nc2",
            Self::Nc2 => "nc_2",
            Self::U1 => "u1",
            Self::U2 => "u2",
            Self::W1 => "w1",
            Self::W2 => "w2",
            Self::P1a => "p1a",
            Self::P1b => "p1b",
            Self::P2a => "p2a",
            Self::P2b => "p2b",
            Self::P3a => "p3a",
            Self::P3b => "p3b",
            Self::S1a => "s1a",
            Self::S1b => "s1b",
            Self::S2a => "s2a",
            Self::S2b => "s2b",
            Self::S3a => "s3a",
            Self::S3b => "s3b",
            Self::Pe => "pe",
            Self::Ep => "ep",
            Self::L4 => "l4",
            Self::T4 => "t4",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ParameterId.ts")]
pub enum ParameterId {
    ResistanceOhms,
    VoltageVolts,
    VoltageRmsVolts,
    FrequencyHz,
    PhaseOffsetDegrees,
    PhaseVoltageRmsVolts,
    LineVoltageRmsVolts,
    ReversedSequence,
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
    CoilSenseSeconds,
    DelaySeconds,
    InitialPosition,
    InstantaneousTripAmps,
    SenseResistanceOhms,
    RatedCurrentAmps,
    ThermalSenseSeconds,
    TripScaleSeconds,
    CoolSeconds,
    ResetDose,
    StatorResistanceOhms,
    RotorResistanceOhms,
    StatorLeakageHenries,
    RotorLeakageHenries,
    MagnetizingHenries,
    PolePairs,
    TurnsRatio,
    PrimaryResistanceOhms,
    SecondaryResistanceOhms,
    PrimaryLeakageHenries,
    SecondaryLeakageHenries,
    Phase1ResistanceOhms,
    Phase2ResistanceOhms,
    Phase3ResistanceOhms,
    Phase1InductanceHenries,
    Phase2InductanceHenries,
    Phase3InductanceHenries,
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
    ClosedResistanceOhms,
    InitialClosed,
    SaturationCurrentAmps,
    EmissionCoefficient,
    ForwardBeta,
    ReverseBeta,
    EarthResistanceOhms,
    TouchVoltageLimitVolts,
    RatedResidualCurrentAmps,
    TripTimeAtRatedSeconds,
    ResidualSenseSeconds,
    CurveClass,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "Unit.ts")]
pub enum Unit {
    Ohm,
    Volt,
    Hertz,
    Degree,
    Amp,
    Farad,
    Henry,
    AmpereSquaredSecond,
    AmpereSquared,
    Second,
    KilogramMeterSquared,
    NewtonMeter,
    NewtonMeterPerAmp,
    NewtonMeterSecondPerRadian,
    RadianPerSecond,
    Watt,
    VoltAmpere,
    VoltAmpereReactive,
    Dimensionless,
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
    ZeroOrOne,
    ZeroOneOrTwo,
    Integer,
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
            Self::ZeroOrOne => value == 0.0 || value == 1.0,
            Self::ZeroOneOrTwo => value == 0.0 || value == 1.0 || value == 2.0,
            Self::Integer => value.trunc() == value,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub minimum_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub maximum_value: Option<f64>,
}

/// An ordered branch in a component's pin list.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "PinGroupDefinition.ts")]
pub struct PinGroupDefinition {
    pub id: String,
    pub pins: [PinId; 2],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ComponentCatalogEntry.ts")]
pub struct ComponentCatalogEntry {
    pub type_id: ComponentTypeId,
    pub display_name_key: String,
    // The drawer category (docs/contracts/stage-11-drawer-and-editing.md section 2).
    pub category: CategoryId,
    // Message keys of alternative names and related words for search.
    pub search_keyword_keys: Vec<String>,
    // The capital letters a new part's designator starts with (docs/contracts/
    // stage-12-cabinet-documentation.md section 3.2): the owner's identification letter.
    pub designator_prefix: String,
    pub pins: Vec<PinDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pin_groups: Option<Vec<PinGroupDefinition>>,
    pub parameters: Vec<ParameterDefinition>,
    // The fault kinds this type supports (docs/contracts/stage-5-assignments.md section 5.2).
    pub fault_kinds: Vec<crate::FaultKind>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ComponentCatalog.ts")]
pub struct ComponentCatalog {
    pub component_types: Vec<ComponentCatalogEntry>,
    // The categories in display order, each once; a category with no part is not listed.
    pub categories: Vec<CategoryId>,
}

/// The drawer category of a part, in display order.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "CategoryId.ts")]
pub enum CategoryId {
    Sources,
    Passive,
    Protection,
    Switching,
    Measurement,
    MotorsAndLoads,
    Semiconductors,
}

impl CategoryId {
    const ALL: [Self; 7] = [
        Self::Sources,
        Self::Passive,
        Self::Protection,
        Self::Switching,
        Self::Measurement,
        Self::MotorsAndLoads,
        Self::Semiconductors,
    ];
}

impl ParameterDefinition {
    pub fn accepts(&self, value: f64) -> bool {
        self.domain.accepts(value)
            && self.minimum_value.is_none_or(|minimum| value >= minimum)
            && self.maximum_value.is_none_or(|maximum| value <= maximum)
    }
}

fn transistor_entry(type_id: ComponentTypeId, name: &str) -> ComponentCatalogEntry {
    ComponentCatalogEntry {
        type_id,
        display_name_key: format!("component.{name}.name"),
        category: CategoryId::Semiconductors,
        search_keyword_keys: vec![format!("component.{name}.keywords")],
        designator_prefix: "Q".to_owned(),
        pins: [PinId::Collector, PinId::Base, PinId::Emitter]
            .map(|id| PinDefinition {
                id,
                role: PinRole::Unpolarized,
            })
            .to_vec(),
        parameters: [
            (
                ParameterId::SaturationCurrentAmps,
                Unit::Amp,
                1e-14,
                1e-18,
                1e-9,
                "saturationCurrent",
            ),
            (
                ParameterId::ForwardBeta,
                Unit::Dimensionless,
                100.0,
                1.0,
                1000.0,
                "forwardBeta",
            ),
            (
                ParameterId::ReverseBeta,
                Unit::Dimensionless,
                1.0,
                0.01,
                100.0,
                "reverseBeta",
            ),
        ]
        .map(
            |(id, unit, default_value, minimum, maximum, key)| ParameterDefinition {
                id,
                unit,
                default_value,
                domain: NumericDomain::StrictlyPositive,
                description_key: format!("component.{name}.{key}"),
                optional: false,
                initial_condition: false,
                minimum_value: Some(minimum),
                maximum_value: Some(maximum),
            },
        )
        .to_vec(),
        pin_groups: None,
        fault_kinds: vec![],
    }
}

fn operator_contact_entry(type_id: ComponentTypeId, name: &str) -> ComponentCatalogEntry {
    let selector = type_id == ComponentTypeId::SelectorSwitch;
    let pins: &[PinId] = if selector {
        &[PinId::Com, PinId::Throw1, PinId::Throw2]
    } else {
        &[PinId::P, PinId::N]
    };
    let mut parameters = vec![ParameterDefinition {
        id: ParameterId::ClosedResistanceOhms,
        unit: Unit::Ohm,
        default_value: 0.01,
        domain: NumericDomain::StrictlyPositive,
        description_key: "component.operatorContact.closedResistance".into(),
        optional: false,
        initial_condition: false,
        minimum_value: Some(1e-6),
        maximum_value: Some(100.0),
    }];
    if selector || type_id == ComponentTypeId::KeySwitch {
        parameters.push(ParameterDefinition {
            id: ParameterId::InitialPosition,
            unit: Unit::Dimensionless,
            default_value: f64::from(selector),
            domain: if selector {
                NumericDomain::ZeroOneOrTwo
            } else {
                NumericDomain::ZeroOrOne
            },
            description_key: "component.operatorContact.initialPosition".into(),
            optional: true,
            initial_condition: true,
            minimum_value: None,
            maximum_value: None,
        });
    }
    ComponentCatalogEntry {
        type_id,
        display_name_key: format!("component.{name}.name"),
        category: CategoryId::Switching,
        search_keyword_keys: vec![format!("component.{name}.keywords")],
        designator_prefix: "S".into(),
        pins: pins
            .iter()
            .map(|&id| PinDefinition {
                id,
                role: PinRole::Unpolarized,
            })
            .collect(),
        parameters,
        pin_groups: None,
        fault_kinds: Vec::new(),
    }
}

fn timed_relay_entry(type_id: ComponentTypeId, name: &str) -> ComponentCatalogEntry {
    let parameter =
        |id, unit, default_value, minimum_value, maximum_value, label: &str| ParameterDefinition {
            id,
            unit,
            default_value,
            minimum_value: Some(minimum_value),
            maximum_value: Some(maximum_value),
            domain: if minimum_value > 0.0 {
                NumericDomain::StrictlyPositive
            } else {
                NumericDomain::NonNegative
            },
            description_key: format!("component.timeRelay.{label}"),
            optional: false,
            initial_condition: false,
        };
    ComponentCatalogEntry {
        type_id,
        display_name_key: format!("component.{name}.name"),
        category: CategoryId::Switching,
        search_keyword_keys: vec![format!("component.{name}.keywords")],
        designator_prefix: "K".into(),
        pins: [
            (PinId::A1, PinRole::Positive),
            (PinId::A2, PinRole::Negative),
            (PinId::TimerNo1, PinRole::Unpolarized),
            (PinId::TimerNo2, PinRole::Unpolarized),
            (PinId::TimerNc1, PinRole::Unpolarized),
            (PinId::TimerNc2, PinRole::Unpolarized),
        ]
        .into_iter()
        .map(|(id, role)| PinDefinition { id, role })
        .collect(),
        parameters: vec![
            parameter(
                ParameterId::CoilResistanceOhms,
                Unit::Ohm,
                240.0,
                1.0,
                1e6,
                "coilResistance",
            ),
            parameter(
                ParameterId::PickupCurrentAmps,
                Unit::Amp,
                0.08,
                1e-6,
                100.0,
                "pickupCurrent",
            ),
            parameter(
                ParameterId::DropoutCurrentAmps,
                Unit::Amp,
                0.04,
                0.0,
                100.0,
                "dropoutCurrent",
            ),
            parameter(
                ParameterId::ClosedResistanceOhms,
                Unit::Ohm,
                0.01,
                1e-6,
                100.0,
                "closedResistance",
            ),
            parameter(
                ParameterId::DelaySeconds,
                Unit::Second,
                0.1,
                0.0,
                3600.0,
                "delay",
            ),
        ],
        pin_groups: None,
        fault_kinds: Vec::new(),
    }
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

    let mut catalog = ComponentCatalog {
        component_types: vec![
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Resistor,
                display_name_key: "component.resistor.name".to_owned(),
                category: CategoryId::Passive,
                search_keyword_keys: vec!["component.resistor.keywords".to_owned()],
                designator_prefix: "R".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![ParameterDefinition {
                    id: ParameterId::ResistanceOhms,
                    unit: Unit::Ohm,
                    default_value: 1000.0,
                    domain: NumericDomain::StrictlyPositive,
                    description_key: "component.resistor.resistance".to_owned(),
                    optional: false,
                    initial_condition: false,
                    minimum_value: None,
                    maximum_value: None,
                }],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::Resistor).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::DcVoltageSource,
                display_name_key: "component.dcVoltageSource.name".to_owned(),
                category: CategoryId::Sources,
                search_keyword_keys: vec!["component.dcVoltageSource.keywords".to_owned()],
                designator_prefix: "G".to_owned(),
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
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::InternalResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.1,
                        domain: NumericDomain::NonNegative,
                        description_key: "component.dcVoltageSource.internalResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                ],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::DcVoltageSource).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Capacitor,
                display_name_key: "component.capacitor.name".to_owned(),
                category: CategoryId::Passive,
                search_keyword_keys: vec!["component.capacitor.keywords".to_owned()],
                designator_prefix: "C".to_owned(),
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
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::InitialVoltageVolts,
                        unit: Unit::Volt,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.capacitor.initialVoltage".to_owned(),
                        optional: true,
                        initial_condition: true,
                        minimum_value: None,
                        maximum_value: None,
                    },
                ],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::Capacitor).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Inductor,
                display_name_key: "component.inductor.name".to_owned(),
                category: CategoryId::Passive,
                search_keyword_keys: vec!["component.inductor.keywords".to_owned()],
                designator_prefix: "R".to_owned(),
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
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::InitialCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.inductor.initialCurrent".to_owned(),
                        optional: true,
                        initial_condition: true,
                        minimum_value: None,
                        maximum_value: None,
                    },
                ],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::Inductor).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::RelaySpdt,
                display_name_key: "component.relaySpdt.name".to_owned(),
                category: CategoryId::Switching,
                search_keyword_keys: vec!["component.relaySpdt.keywords".to_owned()],
                designator_prefix: "K".to_owned(),
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
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::CoilInductanceHenries,
                        unit: Unit::Henry,
                        default_value: 0.01,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.relaySpdt.coilInductance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::PickupCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.04,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.relaySpdt.pickupCurrent".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::DropoutCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.02,
                        domain: NumericDomain::NonNegative,
                        description_key: "component.relaySpdt.dropoutCurrent".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::ClosedContactResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.relaySpdt.closedContactResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                ],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::RelaySpdt).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Fuse,
                display_name_key: "component.fuse.name".to_owned(),
                category: CategoryId::Protection,
                search_keyword_keys: vec!["component.fuse.keywords".to_owned()],
                designator_prefix: "F".to_owned(),
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
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::TripI2tAmpereSquaredSeconds,
                        unit: Unit::AmpereSquaredSecond,
                        default_value: 0.004,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.fuse.tripI2t".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                ],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::Fuse).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::PmdcMotor,
                display_name_key: "component.pmdcMotor.name".to_owned(),
                category: CategoryId::MotorsAndLoads,
                search_keyword_keys: vec!["component.pmdcMotor.keywords".to_owned()],
                designator_prefix: "M".to_owned(),
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
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::ArmatureInductanceHenries,
                        unit: Unit::Henry,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.pmdcMotor.armatureInductance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::MotorConstantNewtonMetersPerAmp,
                        unit: Unit::NewtonMeterPerAmp,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.pmdcMotor.motorConstant".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::RotorInertiaKilogramMetersSquared,
                        unit: Unit::KilogramMeterSquared,
                        default_value: 0.01,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.pmdcMotor.rotorInertia".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::ViscousFrictionNewtonMeterSecondsPerRadian,
                        unit: Unit::NewtonMeterSecondPerRadian,
                        default_value: 0.01,
                        domain: NumericDomain::NonNegative,
                        description_key: "component.pmdcMotor.viscousFriction".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::LoadTorqueNewtonMeters,
                        unit: Unit::NewtonMeter,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.pmdcMotor.loadTorque".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::InitialAngularVelocityRadiansPerSecond,
                        unit: Unit::RadianPerSecond,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.pmdcMotor.initialAngularVelocity".to_owned(),
                        optional: true,
                        initial_condition: true,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::InitialArmatureCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.pmdcMotor.initialArmatureCurrent".to_owned(),
                        optional: true,
                        initial_condition: true,
                        minimum_value: None,
                        maximum_value: None,
                    },
                ],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::PmdcMotor).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Voltmeter,
                display_name_key: "component.voltmeter.name".to_owned(),
                category: CategoryId::Measurement,
                search_keyword_keys: vec!["component.voltmeter.keywords".to_owned()],
                designator_prefix: "P".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![ParameterDefinition {
                    id: ParameterId::InputResistanceOhms,
                    unit: Unit::Ohm,
                    default_value: 10_000_000.0,
                    domain: NumericDomain::StrictlyPositive,
                    description_key: "component.voltmeter.inputResistance".to_owned(),
                    optional: false,
                    initial_condition: false,
                    minimum_value: None,
                    maximum_value: None,
                }],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::Voltmeter).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Ammeter,
                display_name_key: "component.ammeter.name".to_owned(),
                category: CategoryId::Measurement,
                search_keyword_keys: vec!["component.ammeter.keywords".to_owned()],
                designator_prefix: "P".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![ParameterDefinition {
                    id: ParameterId::BurdenResistanceOhms,
                    unit: Unit::Ohm,
                    default_value: 0.1,
                    domain: NumericDomain::StrictlyPositive,
                    description_key: "component.ammeter.burdenResistance".to_owned(),
                    optional: false,
                    initial_condition: false,
                    minimum_value: None,
                    maximum_value: None,
                }],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::Ammeter).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::SwitchSpst,
                display_name_key: "component.switchSpst.name".to_owned(),
                category: CategoryId::Switching,
                search_keyword_keys: vec!["component.switchSpst.keywords".to_owned()],
                designator_prefix: "S".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::ClosedResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.01,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.switchSpst.closedResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::InitialClosed,
                        unit: Unit::Dimensionless,
                        default_value: 0.0,
                        domain: NumericDomain::ZeroOrOne,
                        description_key: "component.switchSpst.initialClosed".to_owned(),
                        optional: true,
                        initial_condition: true,
                        minimum_value: None,
                        maximum_value: None,
                    },
                ],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::SwitchSpst).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::PushButtonNo,
                display_name_key: "component.pushButtonNo.name".to_owned(),
                category: CategoryId::Switching,
                search_keyword_keys: vec!["component.pushButtonNo.keywords".to_owned()],
                designator_prefix: "S".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![ParameterDefinition {
                    id: ParameterId::ClosedResistanceOhms,
                    unit: Unit::Ohm,
                    default_value: 0.01,
                    domain: NumericDomain::StrictlyPositive,
                    description_key: "component.pushButtonNo.closedResistance".to_owned(),
                    optional: false,
                    initial_condition: false,
                    minimum_value: None,
                    maximum_value: None,
                }],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::PushButtonNo).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::PushButtonNc,
                display_name_key: "component.pushButtonNc.name".to_owned(),
                category: CategoryId::Switching,
                search_keyword_keys: vec!["component.pushButtonNc.keywords".to_owned()],
                designator_prefix: "S".to_owned(),
                pins: two_unpolarized_pins(),
                parameters: vec![ParameterDefinition {
                    id: ParameterId::ClosedResistanceOhms,
                    unit: Unit::Ohm,
                    default_value: 0.01,
                    domain: NumericDomain::StrictlyPositive,
                    description_key: "component.pushButtonNc.closedResistance".to_owned(),
                    optional: false,
                    initial_condition: false,
                    minimum_value: None,
                    maximum_value: None,
                }],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::PushButtonNc).to_vec(),
            },
            operator_contact_entry(ComponentTypeId::LimitSwitch, "limitSwitch"),
            operator_contact_entry(ComponentTypeId::EmergencyStop, "emergencyStop"),
            operator_contact_entry(ComponentTypeId::SelectorSwitch, "selectorSwitch"),
            operator_contact_entry(ComponentTypeId::KeySwitch, "keySwitch"),
            timed_relay_entry(ComponentTypeId::RelayOnDelay, "onDelayRelay"),
            timed_relay_entry(ComponentTypeId::RelayOffDelay, "offDelayRelay"),
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Contactor,
                display_name_key: "component.contactor.name".to_owned(),
                category: CategoryId::Switching,
                search_keyword_keys: vec!["component.contactor.keywords".to_owned()],
                designator_prefix: "Q".to_owned(),
                pins: vec![
                    PinDefinition {
                        id: PinId::A1,
                        role: PinRole::Positive,
                    },
                    PinDefinition {
                        id: PinId::A2,
                        role: PinRole::Negative,
                    },
                    PinDefinition {
                        id: PinId::No1,
                        role: PinRole::Unpolarized,
                    },
                    PinDefinition {
                        id: PinId::No2,
                        role: PinRole::Unpolarized,
                    },
                    PinDefinition {
                        id: PinId::Nc1,
                        role: PinRole::Unpolarized,
                    },
                    PinDefinition {
                        id: PinId::Nc2,
                        role: PinRole::Unpolarized,
                    },
                ],
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::CoilResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 100.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.contactor.coilResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::CoilInductanceHenries,
                        unit: Unit::Henry,
                        default_value: 0.01,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.contactor.coilInductance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::PickupCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.04,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.contactor.pickupCurrent".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::DropoutCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 0.02,
                        domain: NumericDomain::NonNegative,
                        description_key: "component.contactor.dropoutCurrent".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::ClosedContactResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.contactor.closedContactResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                ],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::Contactor).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Diode,
                display_name_key: "component.diode.name".into(),
                category: CategoryId::Semiconductors,
                search_keyword_keys: vec!["component.diode.keywords".to_owned()],
                designator_prefix: "R".to_owned(),
                pins: vec![
                    PinDefinition {
                        id: PinId::Anode,
                        role: PinRole::Positive,
                    },
                    PinDefinition {
                        id: PinId::Cathode,
                        role: PinRole::Negative,
                    },
                ],
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::SaturationCurrentAmps,
                        unit: Unit::Amp,
                        default_value: 1e-14,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.diode.saturationCurrent".into(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: Some(1e-18),
                        maximum_value: Some(1e-9),
                    },
                    ParameterDefinition {
                        id: ParameterId::EmissionCoefficient,
                        unit: Unit::Dimensionless,
                        default_value: 1.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.diode.emissionCoefficient".into(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: Some(1.0),
                        maximum_value: Some(2.0),
                    },
                ],
                pin_groups: None,
                fault_kinds: vec![],
            },
            transistor_entry(ComponentTypeId::BjtNpn, "bjtNpn"),
            transistor_entry(ComponentTypeId::BjtPnp, "bjtPnp"),
            ComponentCatalogEntry {
                type_id: ComponentTypeId::AcVoltageSource,
                display_name_key: "component.acVoltageSource.name".to_owned(),
                category: CategoryId::Sources,
                search_keyword_keys: vec!["component.acVoltageSource.keywords".to_owned()],
                designator_prefix: "G".to_owned(),
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
                        id: ParameterId::VoltageRmsVolts,
                        unit: Unit::Volt,
                        default_value: 230.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.acVoltageSource.voltageRms".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: Some(0.001),
                        maximum_value: Some(100_000.0),
                    },
                    ParameterDefinition {
                        id: ParameterId::FrequencyHz,
                        unit: Unit::Hertz,
                        default_value: 50.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.acVoltageSource.frequency".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: Some(1.0),
                        maximum_value: Some(400.0),
                    },
                    ParameterDefinition {
                        id: ParameterId::PhaseOffsetDegrees,
                        unit: Unit::Degree,
                        default_value: 0.0,
                        domain: NumericDomain::Finite,
                        description_key: "component.acVoltageSource.phaseOffset".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: Some(-360.0),
                        maximum_value: Some(360.0),
                    },
                    ParameterDefinition {
                        id: ParameterId::InternalResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.1,
                        domain: NumericDomain::NonNegative,
                        description_key: "component.acVoltageSource.internalResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                ],
                pin_groups: None,
                fault_kinds: supported_fault_kinds(ComponentTypeId::AcVoltageSource).to_vec(),
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::ThreePhaseSourceStar,
                display_name_key: "component.threePhaseSourceStar.name".to_owned(),
                category: CategoryId::Sources,
                search_keyword_keys: vec!["component.threePhaseSourceStar.keywords".to_owned()],
                designator_prefix: "G".to_owned(),
                pins: [PinId::L1, PinId::L2, PinId::L3, PinId::N]
                    .map(|id| PinDefinition {
                        id,
                        role: PinRole::Unpolarized,
                    })
                    .to_vec(),
                parameters: [
                    (
                        ParameterId::PhaseVoltageRmsVolts,
                        Unit::Volt,
                        230.0,
                        NumericDomain::StrictlyPositive,
                        Some(0.001),
                        Some(100_000.0),
                        "phaseVoltageRms",
                    ),
                    (
                        ParameterId::FrequencyHz,
                        Unit::Hertz,
                        50.0,
                        NumericDomain::StrictlyPositive,
                        Some(1.0),
                        Some(400.0),
                        "frequency",
                    ),
                    (
                        ParameterId::PhaseOffsetDegrees,
                        Unit::Degree,
                        0.0,
                        NumericDomain::Finite,
                        Some(-360.0),
                        Some(360.0),
                        "phaseOffset",
                    ),
                    (
                        ParameterId::InternalResistanceOhms,
                        Unit::Ohm,
                        0.1,
                        NumericDomain::NonNegative,
                        None,
                        None,
                        "internalResistance",
                    ),
                    (
                        ParameterId::ReversedSequence,
                        Unit::Dimensionless,
                        0.0,
                        NumericDomain::ZeroOrOne,
                        Some(0.0),
                        Some(1.0),
                        "reversedSequence",
                    ),
                ]
                .map(
                    |(id, unit, default_value, domain, minimum_value, maximum_value, key)| {
                        ParameterDefinition {
                            id,
                            unit,
                            default_value,
                            domain,
                            description_key: format!("component.threePhaseSourceStar.{key}"),
                            optional: false,
                            initial_condition: false,
                            minimum_value,
                            maximum_value,
                        }
                    },
                )
                .to_vec(),
                pin_groups: None,
                fault_kinds: vec![],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::ThreePhaseSourceDelta,
                display_name_key: "component.threePhaseSourceDelta.name".to_owned(),
                category: CategoryId::Sources,
                search_keyword_keys: vec!["component.threePhaseSourceDelta.keywords".to_owned()],
                designator_prefix: "G".to_owned(),
                pins: [PinId::L1, PinId::L2, PinId::L3]
                    .map(|id| PinDefinition {
                        id,
                        role: PinRole::Unpolarized,
                    })
                    .to_vec(),
                parameters: [
                    (
                        ParameterId::LineVoltageRmsVolts,
                        Unit::Volt,
                        400.0,
                        NumericDomain::StrictlyPositive,
                        Some(0.001),
                        Some(100_000.0),
                        "lineVoltageRms",
                    ),
                    (
                        ParameterId::FrequencyHz,
                        Unit::Hertz,
                        50.0,
                        NumericDomain::StrictlyPositive,
                        Some(1.0),
                        Some(400.0),
                        "frequency",
                    ),
                    (
                        ParameterId::PhaseOffsetDegrees,
                        Unit::Degree,
                        0.0,
                        NumericDomain::Finite,
                        Some(-360.0),
                        Some(360.0),
                        "phaseOffset",
                    ),
                    (
                        ParameterId::InternalResistanceOhms,
                        Unit::Ohm,
                        0.1,
                        NumericDomain::StrictlyPositive,
                        None,
                        None,
                        "internalResistance",
                    ),
                    (
                        ParameterId::ReversedSequence,
                        Unit::Dimensionless,
                        0.0,
                        NumericDomain::ZeroOrOne,
                        Some(0.0),
                        Some(1.0),
                        "reversedSequence",
                    ),
                ]
                .map(
                    |(id, unit, default_value, domain, minimum_value, maximum_value, key)| {
                        ParameterDefinition {
                            id,
                            unit,
                            default_value,
                            domain,
                            description_key: format!("component.threePhaseSourceDelta.{key}"),
                            optional: false,
                            initial_condition: false,
                            minimum_value,
                            maximum_value,
                        }
                    },
                )
                .to_vec(),
                pin_groups: None,
                fault_kinds: vec![],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::AcVoltmeter,
                display_name_key: "component.acVoltmeter.name".to_owned(),
                category: CategoryId::Measurement,
                search_keyword_keys: vec!["component.acVoltmeter.keywords".to_owned()],
                designator_prefix: "P".to_owned(),
                pins: [PinId::Pin1, PinId::Pin2]
                    .map(|id| PinDefinition {
                        id,
                        role: PinRole::Unpolarized,
                    })
                    .to_vec(),
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::InputResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 10_000_000.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.acVoltmeter.inputResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::FrequencyHz,
                        unit: Unit::Hertz,
                        default_value: 50.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.acVoltmeter.frequency".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: Some(1.0),
                        maximum_value: Some(400.0),
                    },
                ],
                pin_groups: None,
                fault_kinds: vec![],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::AcAmmeter,
                display_name_key: "component.acAmmeter.name".to_owned(),
                category: CategoryId::Measurement,
                search_keyword_keys: vec!["component.acAmmeter.keywords".to_owned()],
                designator_prefix: "P".to_owned(),
                pins: [PinId::Pin1, PinId::Pin2]
                    .map(|id| PinDefinition {
                        id,
                        role: PinRole::Unpolarized,
                    })
                    .to_vec(),
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::BurdenResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.acAmmeter.burdenResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::FrequencyHz,
                        unit: Unit::Hertz,
                        default_value: 50.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.acAmmeter.frequency".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: Some(1.0),
                        maximum_value: Some(400.0),
                    },
                ],
                pin_groups: None,
                fault_kinds: vec![],
            },
            ComponentCatalogEntry {
                type_id: ComponentTypeId::Wattmeter,
                display_name_key: "component.wattmeter.name".to_owned(),
                category: CategoryId::Measurement,
                search_keyword_keys: vec!["component.wattmeter.keywords".to_owned()],
                designator_prefix: "P".to_owned(),
                pins: [PinId::V1, PinId::V2, PinId::I1, PinId::I2]
                    .map(|id| PinDefinition {
                        id,
                        role: PinRole::Unpolarized,
                    })
                    .to_vec(),
                parameters: vec![
                    ParameterDefinition {
                        id: ParameterId::InputResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 10_000_000.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.wattmeter.inputResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::BurdenResistanceOhms,
                        unit: Unit::Ohm,
                        default_value: 0.1,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.wattmeter.burdenResistance".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: None,
                        maximum_value: None,
                    },
                    ParameterDefinition {
                        id: ParameterId::FrequencyHz,
                        unit: Unit::Hertz,
                        default_value: 50.0,
                        domain: NumericDomain::StrictlyPositive,
                        description_key: "component.wattmeter.frequency".to_owned(),
                        optional: false,
                        initial_condition: false,
                        minimum_value: Some(1.0),
                        maximum_value: Some(400.0),
                    },
                ],
                pin_groups: None,
                fault_kinds: vec![],
            },
        ],
        categories: Vec::new(),
    };
    catalog.component_types.extend(stage10_equipment_catalog());
    catalog.component_types.extend(stage13_protection_catalog());
    // One source of truth for the fault kinds of every type (docs/contracts/
    // stage-13-protection-and-safety.md section 3.2 adds `leak`).
    for entry in &mut catalog.component_types {
        entry.fault_kinds = supported_fault_kinds(entry.type_id).to_vec();
    }
    catalog.categories = CategoryId::ALL
        .into_iter()
        .filter(|category| {
            catalog
                .component_types
                .iter()
                .any(|entry| entry.category == *category)
        })
        .collect();
    catalog
}

/// The Stage 13 devices (docs/contracts/stage-13-protection-and-safety.md sections 2 and 7):
/// the earth electrode and the exposed part, each with one pin.
fn stage13_protection_catalog() -> [ComponentCatalogEntry; 4] {
    let one_pin = |id| {
        vec![PinDefinition {
            id,
            role: PinRole::Unpolarized,
        }]
    };
    let parameter = |id, unit, default_value, min, max, key: &str| ParameterDefinition {
        id,
        unit,
        default_value,
        domain: NumericDomain::StrictlyPositive,
        description_key: key.to_owned(),
        optional: false,
        initial_condition: false,
        minimum_value: Some(min),
        maximum_value: Some(max),
    };

    [
        ComponentCatalogEntry {
            type_id: ComponentTypeId::EarthElectrode,
            display_name_key: "component.earthElectrode.name".to_owned(),
            category: CategoryId::Protection,
            search_keyword_keys: vec!["component.earthElectrode.keywords".to_owned()],
            designator_prefix: "X".to_owned(),
            pins: one_pin(PinId::Pe),
            pin_groups: None,
            parameters: vec![parameter(
                ParameterId::EarthResistanceOhms,
                Unit::Ohm,
                10.0,
                0.001,
                1_000_000.0,
                "component.earthElectrode.earthResistance",
            )],
            fault_kinds: vec![],
        },
        ComponentCatalogEntry {
            type_id: ComponentTypeId::ExposedPart,
            display_name_key: "component.exposedPart.name".to_owned(),
            category: CategoryId::Measurement,
            search_keyword_keys: vec!["component.exposedPart.keywords".to_owned()],
            designator_prefix: "P".to_owned(),
            pins: one_pin(PinId::Ep),
            pin_groups: None,
            parameters: vec![
                parameter(
                    ParameterId::FrequencyHz,
                    Unit::Hertz,
                    50.0,
                    1.0,
                    400.0,
                    "component.exposedPart.frequency",
                ),
                parameter(
                    ParameterId::TouchVoltageLimitVolts,
                    Unit::Volt,
                    50.0,
                    1.0,
                    1000.0,
                    "component.exposedPart.touchVoltageLimit",
                ),
            ],
            fault_kinds: vec![],
        },
        rcd_entry(ComponentTypeId::RcdTwoPole, "rcdTwoPole", &RCD_TWO_POLES),
        rcd_entry(ComponentTypeId::RcdFourPole, "rcdFourPole", &RCD_FOUR_POLES),
    ]
}

/// A residual-current device with `groups` as its poles (docs/contracts/
/// stage-13-protection-and-safety.md section 4).
fn rcd_entry(
    type_id: ComponentTypeId,
    name: &str,
    groups: &[crate::pin_groups::PinGroup],
) -> ComponentCatalogEntry {
    let parameter = |id, unit, key: &str, default_value, min, max| ParameterDefinition {
        id,
        unit,
        default_value,
        domain: NumericDomain::StrictlyPositive,
        description_key: format!("component.{name}.{key}"),
        optional: false,
        initial_condition: false,
        minimum_value: Some(min),
        maximum_value: Some(max),
    };
    ComponentCatalogEntry {
        type_id,
        display_name_key: format!("component.{name}.name"),
        category: CategoryId::Protection,
        search_keyword_keys: vec![format!("component.{name}.keywords")],
        // The owner's table puts residual-current devices with the direct protection (F).
        designator_prefix: "F".to_owned(),
        pins: groups
            .iter()
            .flat_map(|group| group.pins)
            .map(|id| PinDefinition {
                id,
                role: PinRole::Unpolarized,
            })
            .collect(),
        pin_groups: Some(
            groups
                .iter()
                .map(|group| PinGroupDefinition {
                    id: group.id.to_owned(),
                    pins: group.pins,
                })
                .collect(),
        ),
        parameters: vec![
            parameter(
                ParameterId::RatedResidualCurrentAmps,
                Unit::Amp,
                "ratedResidualCurrent",
                0.03,
                0.003,
                1.0,
            ),
            parameter(
                ParameterId::TripTimeAtRatedSeconds,
                Unit::Second,
                "tripTimeAtRated",
                0.1,
                0.01,
                2.0,
            ),
            parameter(
                ParameterId::ResidualSenseSeconds,
                Unit::Second,
                "residualSense",
                0.02,
                0.001,
                0.2,
            ),
            parameter(
                ParameterId::ClosedContactResistanceOhms,
                Unit::Ohm,
                "closedContactResistance",
                0.01,
                0.0001,
                10.0,
            ),
            ParameterDefinition {
                id: ParameterId::InitialClosed,
                unit: Unit::Dimensionless,
                default_value: 0.0,
                domain: NumericDomain::ZeroOrOne,
                description_key: format!("component.{name}.initialClosed"),
                optional: true,
                initial_condition: true,
                minimum_value: None,
                maximum_value: None,
            },
        ],
        fault_kinds: vec![],
    }
}

fn stage10_equipment_catalog() -> [ComponentCatalogEntry; 7] {
    use crate::pin_groups::{
        CONTACTOR_AUXILIARIES, CONTACTOR_COIL, DELTA_LOAD_WINDINGS, INDUCTION_WINDINGS,
        POWER_POLES, PinGroup, STAR_LOAD_WINDINGS, TRANSFORMER_WINDINGS,
    };
    use ParameterId::*;

    let entry = |type_id, name: &str, groups: Vec<PinGroup>, parameters| ComponentCatalogEntry {
        type_id,
        display_name_key: format!("component.{name}.name"),
        category: match type_id {
            ComponentTypeId::BreakerThreePole | ComponentTypeId::ThermalOverloadThreePole => {
                CategoryId::Protection
            }
            ComponentTypeId::InductionMotorThreePhase
            | ComponentTypeId::TransformerThreePhase
            | ComponentTypeId::ThreePhaseLoadStar
            | ComponentTypeId::ThreePhaseLoadDelta => CategoryId::MotorsAndLoads,
            _ => CategoryId::Switching,
        },
        search_keyword_keys: vec![format!("component.{name}.keywords")],
        designator_prefix: match type_id {
            ComponentTypeId::ContactorThreePole | ComponentTypeId::BreakerThreePole => "Q",
            ComponentTypeId::ThermalOverloadThreePole => "F",
            ComponentTypeId::InductionMotorThreePhase => "M",
            ComponentTypeId::TransformerThreePhase => "T",
            _ => "R",
        }
        .to_owned(),
        pins: match type_id {
            ComponentTypeId::ThreePhaseLoadStar => vec![PinId::L1, PinId::L2, PinId::L3, PinId::N],
            ComponentTypeId::ThreePhaseLoadDelta => vec![PinId::L1, PinId::L2, PinId::L3],
            _ => groups.iter().flat_map(|group| group.pins).collect(),
        }
        .into_iter()
        .map(|id| PinDefinition {
            id,
            role: PinRole::Unpolarized,
        })
        .collect(),
        pin_groups: Some(
            groups
                .iter()
                .map(|group| PinGroupDefinition {
                    id: group.id.to_owned(),
                    pins: group.pins,
                })
                .collect(),
        ),
        parameters,
        fault_kinds: vec![],
    };
    let parameter =
        |name: &str, key: &str, id, unit, min, max, default_value| ParameterDefinition {
            id,
            unit,
            default_value,
            domain: NumericDomain::StrictlyPositive,
            description_key: format!("component.{name}.{key}"),
            optional: false,
            initial_condition: false,
            minimum_value: Some(min),
            maximum_value: Some(max),
        };
    let motor = "inductionMotorThreePhase";
    let transformer = "transformerThreePhase";
    let load_entry = |type_id, name, windings: [PinGroup; 3]| {
        let parameters = [
            (
                "phase1Resistance",
                Phase1ResistanceOhms,
                Unit::Ohm,
                0.001,
                1_000_000.0,
                100.0,
                NumericDomain::StrictlyPositive,
            ),
            (
                "phase2Resistance",
                Phase2ResistanceOhms,
                Unit::Ohm,
                0.001,
                1_000_000.0,
                100.0,
                NumericDomain::StrictlyPositive,
            ),
            (
                "phase3Resistance",
                Phase3ResistanceOhms,
                Unit::Ohm,
                0.001,
                1_000_000.0,
                100.0,
                NumericDomain::StrictlyPositive,
            ),
            (
                "phase1Inductance",
                Phase1InductanceHenries,
                Unit::Henry,
                0.0,
                10.0,
                0.0,
                NumericDomain::NonNegative,
            ),
            (
                "phase2Inductance",
                Phase2InductanceHenries,
                Unit::Henry,
                0.0,
                10.0,
                0.0,
                NumericDomain::NonNegative,
            ),
            (
                "phase3Inductance",
                Phase3InductanceHenries,
                Unit::Henry,
                0.0,
                10.0,
                0.0,
                NumericDomain::NonNegative,
            ),
        ]
        .map(
            |(key, id, unit, min, max, default, domain)| ParameterDefinition {
                domain,
                ..parameter(name, key, id, unit, min, max, default)
            },
        )
        .to_vec();
        entry(type_id, name, windings.to_vec(), parameters)
    };
    let contactor = "contactorThreePole";
    let breaker = "breakerThreePole";
    let closed_resistance = |name| {
        parameter(
            name,
            "closedContactResistance",
            ClosedContactResistanceOhms,
            Unit::Ohm,
            0.0001,
            10.0,
            0.01,
        )
    };
    [
        entry(
            ComponentTypeId::ContactorThreePole,
            contactor,
            std::iter::once(CONTACTOR_COIL)
                .chain(POWER_POLES)
                .chain(CONTACTOR_AUXILIARIES)
                .collect(),
            vec![
                parameter(
                    contactor,
                    "coilResistance",
                    CoilResistanceOhms,
                    Unit::Ohm,
                    0.1,
                    100_000.0,
                    230.0,
                ),
                parameter(
                    contactor,
                    "coilInductance",
                    CoilInductanceHenries,
                    Unit::Henry,
                    0.00001,
                    100.0,
                    0.2,
                ),
                parameter(
                    contactor,
                    "pickupCurrent",
                    PickupCurrentAmps,
                    Unit::Amp,
                    0.001,
                    100.0,
                    0.7,
                ),
                parameter(
                    contactor,
                    "dropoutCurrent",
                    DropoutCurrentAmps,
                    Unit::Amp,
                    0.0001,
                    100.0,
                    0.4,
                ),
                parameter(
                    contactor,
                    "coilSense",
                    CoilSenseSeconds,
                    Unit::Second,
                    0.005,
                    2.0,
                    0.02,
                ),
                closed_resistance(contactor),
            ],
        ),
        entry(
            ComponentTypeId::BreakerThreePole,
            breaker,
            POWER_POLES.to_vec(),
            vec![
                closed_resistance(breaker),
                parameter(
                    breaker,
                    "instantaneousTrip",
                    InstantaneousTripAmps,
                    Unit::Amp,
                    0.1,
                    100_000.0,
                    100.0,
                ),
                ParameterDefinition {
                    id: InitialClosed,
                    unit: Unit::Dimensionless,
                    default_value: 0.0,
                    domain: NumericDomain::ZeroOrOne,
                    description_key: "component.breakerThreePole.initialClosed".to_owned(),
                    optional: true,
                    initial_condition: true,
                    minimum_value: None,
                    maximum_value: None,
                },
                // The optional thermal-magnetic trip class and its inputs (docs/contracts/
                // stage-13-protection-and-safety.md section 5). Absent, the breaker is the
                // Stage 10 instantaneous breaker.
                ParameterDefinition {
                    id: CurveClass,
                    unit: Unit::Dimensionless,
                    default_value: 0.0,
                    domain: NumericDomain::Integer,
                    description_key: "component.breakerThreePole.curveClass".to_owned(),
                    optional: true,
                    initial_condition: false,
                    minimum_value: Some(0.0),
                    maximum_value: Some(3.0),
                },
                ParameterDefinition {
                    optional: true,
                    ..parameter(
                        breaker,
                        "ratedCurrent",
                        RatedCurrentAmps,
                        Unit::Amp,
                        0.1,
                        10_000.0,
                        crate::schematic::DEFAULT_BREAKER_RATED_CURRENT_AMPS,
                    )
                },
                ParameterDefinition {
                    optional: true,
                    ..parameter(
                        breaker,
                        "thermalSense",
                        ThermalSenseSeconds,
                        Unit::Second,
                        0.005,
                        2.0,
                        crate::schematic::DEFAULT_BREAKER_THERMAL_SENSE_SECONDS,
                    )
                },
                ParameterDefinition {
                    optional: true,
                    ..parameter(
                        breaker,
                        "tripScale",
                        TripScaleSeconds,
                        Unit::Second,
                        0.01,
                        10_000.0,
                        crate::schematic::DEFAULT_BREAKER_TRIP_SCALE_SECONDS,
                    )
                },
                ParameterDefinition {
                    optional: true,
                    ..parameter(
                        breaker,
                        "cool",
                        CoolSeconds,
                        Unit::Second,
                        0.01,
                        10_000.0,
                        crate::schematic::DEFAULT_BREAKER_COOL_SECONDS,
                    )
                },
            ],
        ),
        entry(
            ComponentTypeId::ThermalOverloadThreePole,
            "thermalOverloadThreePole",
            POWER_POLES
                .into_iter()
                .chain(CONTACTOR_AUXILIARIES)
                .collect(),
            vec![
                closed_resistance("thermalOverloadThreePole"),
                parameter(
                    "thermalOverloadThreePole",
                    "senseResistance",
                    SenseResistanceOhms,
                    Unit::Ohm,
                    0.0001,
                    10.0,
                    0.005,
                ),
                parameter(
                    "thermalOverloadThreePole",
                    "ratedCurrent",
                    RatedCurrentAmps,
                    Unit::Amp,
                    0.1,
                    10_000.0,
                    10.0,
                ),
                parameter(
                    "thermalOverloadThreePole",
                    "thermalSense",
                    ThermalSenseSeconds,
                    Unit::Second,
                    0.005,
                    2.0,
                    0.02,
                ),
                parameter(
                    "thermalOverloadThreePole",
                    "tripScale",
                    TripScaleSeconds,
                    Unit::Second,
                    0.01,
                    10_000.0,
                    10.0,
                ),
                parameter(
                    "thermalOverloadThreePole",
                    "cool",
                    CoolSeconds,
                    Unit::Second,
                    0.01,
                    10_000.0,
                    30.0,
                ),
                ParameterDefinition {
                    domain: NumericDomain::NonNegative,
                    ..parameter(
                        "thermalOverloadThreePole",
                        "resetDose",
                        ResetDose,
                        Unit::Dimensionless,
                        0.0,
                        0.99,
                        0.25,
                    )
                },
            ],
        ),
        entry(
            ComponentTypeId::InductionMotorThreePhase,
            motor,
            INDUCTION_WINDINGS.to_vec(),
            [
                (
                    "statorResistance",
                    StatorResistanceOhms,
                    Unit::Ohm,
                    0.01,
                    10_000.0,
                    1.0,
                    NumericDomain::StrictlyPositive,
                ),
                (
                    "rotorResistance",
                    RotorResistanceOhms,
                    Unit::Ohm,
                    0.01,
                    10_000.0,
                    1.0,
                    NumericDomain::StrictlyPositive,
                ),
                (
                    "statorLeakage",
                    StatorLeakageHenries,
                    Unit::Henry,
                    0.00001,
                    10.0,
                    0.005,
                    NumericDomain::StrictlyPositive,
                ),
                (
                    "rotorLeakage",
                    RotorLeakageHenries,
                    Unit::Henry,
                    0.00001,
                    10.0,
                    0.005,
                    NumericDomain::StrictlyPositive,
                ),
                (
                    "magnetizing",
                    MagnetizingHenries,
                    Unit::Henry,
                    0.00001,
                    10.0,
                    0.1,
                    NumericDomain::StrictlyPositive,
                ),
                (
                    "polePairs",
                    PolePairs,
                    Unit::Dimensionless,
                    1.0,
                    8.0,
                    2.0,
                    NumericDomain::Integer,
                ),
                (
                    "inertia",
                    RotorInertiaKilogramMetersSquared,
                    Unit::KilogramMeterSquared,
                    0.0001,
                    100.0,
                    0.02,
                    NumericDomain::StrictlyPositive,
                ),
                (
                    "friction",
                    ViscousFrictionNewtonMeterSecondsPerRadian,
                    Unit::NewtonMeterSecondPerRadian,
                    0.0,
                    100.0,
                    0.01,
                    NumericDomain::NonNegative,
                ),
                (
                    "loadTorque",
                    LoadTorqueNewtonMeters,
                    Unit::NewtonMeter,
                    -1000.0,
                    1000.0,
                    0.0,
                    NumericDomain::Finite,
                ),
            ]
            .map(
                |(key, id, unit, min, max, default, domain)| ParameterDefinition {
                    domain,
                    ..parameter(motor, key, id, unit, min, max, default)
                },
            )
            .to_vec(),
        ),
        entry(
            ComponentTypeId::TransformerThreePhase,
            transformer,
            TRANSFORMER_WINDINGS.to_vec(),
            [
                (
                    "turnsRatio",
                    TurnsRatio,
                    Unit::Dimensionless,
                    0.01,
                    100.0,
                    2.0,
                ),
                (
                    "primaryResistance",
                    PrimaryResistanceOhms,
                    Unit::Ohm,
                    0.001,
                    10_000.0,
                    1.0,
                ),
                (
                    "secondaryResistance",
                    SecondaryResistanceOhms,
                    Unit::Ohm,
                    0.001,
                    10_000.0,
                    1.0,
                ),
                (
                    "primaryLeakage",
                    PrimaryLeakageHenries,
                    Unit::Henry,
                    0.00001,
                    10.0,
                    0.001,
                ),
                (
                    "secondaryLeakage",
                    SecondaryLeakageHenries,
                    Unit::Henry,
                    0.00001,
                    10.0,
                    0.001,
                ),
                (
                    "magnetizing",
                    MagnetizingHenries,
                    Unit::Henry,
                    0.00001,
                    10.0,
                    1.0,
                ),
            ]
            .map(|(key, id, unit, min, max, default)| {
                parameter(transformer, key, id, unit, min, max, default)
            })
            .to_vec(),
        ),
        load_entry(
            ComponentTypeId::ThreePhaseLoadStar,
            "threePhaseLoadStar",
            STAR_LOAD_WINDINGS,
        ),
        load_entry(
            ComponentTypeId::ThreePhaseLoadDelta,
            "threePhaseLoadDelta",
            DELTA_LOAD_WINDINGS,
        ),
    ]
}
