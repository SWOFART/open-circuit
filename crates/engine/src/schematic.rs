use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    ComponentCatalogEntry, ComponentTypeId, Diagnostic, DiagnosticCode, DiagnosticReference,
    ParameterId, StableId, component_catalog,
};

pub const SCHEMA_VERSION: u32 = 3;

/// Integer world-grid coordinates shared by the schematic and editor layers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "GridPoint.ts")]
pub struct GridPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "BjtParameters.ts")]
pub struct BjtParameters {
    pub saturation_current_amps: f64,
    pub forward_beta: f64,
    pub reverse_beta: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "DiodeParameters.ts")]
pub struct DiodeParameters {
    pub saturation_current_amps: f64,
    pub emission_coefficient: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ResistorParameters.ts")]
pub struct ResistorParameters {
    pub resistance_ohms: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "DcVoltageSourceParameters.ts")]
pub struct DcVoltageSourceParameters {
    pub voltage_volts: f64,
    pub internal_resistance_ohms: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "AcVoltageSourceParameters.ts")]
pub struct AcVoltageSourceParameters {
    pub voltage_rms_volts: f64,
    pub frequency_hz: f64,
    pub phase_offset_degrees: f64,
    pub internal_resistance_ohms: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ThreePhaseSourceStarParameters.ts")]
pub struct ThreePhaseSourceStarParameters {
    pub phase_voltage_rms_volts: f64,
    pub frequency_hz: f64,
    pub phase_offset_degrees: f64,
    pub internal_resistance_ohms: f64,
    pub reversed_sequence: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ThreePhaseSourceDeltaParameters.ts")]
pub struct ThreePhaseSourceDeltaParameters {
    pub line_voltage_rms_volts: f64,
    pub frequency_hz: f64,
    pub phase_offset_degrees: f64,
    pub internal_resistance_ohms: f64,
    pub reversed_sequence: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "AcVoltmeterParameters.ts")]
pub struct AcVoltmeterParameters {
    pub input_resistance_ohms: f64,
    pub frequency_hz: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "AcAmmeterParameters.ts")]
pub struct AcAmmeterParameters {
    pub burden_resistance_ohms: f64,
    pub frequency_hz: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "EarthElectrodeParameters.ts")]
pub struct EarthElectrodeParameters {
    pub earth_resistance_ohms: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ExposedPartParameters.ts")]
pub struct ExposedPartParameters {
    pub frequency_hz: f64,
    pub touch_voltage_limit_volts: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "WattmeterParameters.ts")]
pub struct WattmeterParameters {
    pub input_resistance_ohms: f64,
    pub burden_resistance_ohms: f64,
    pub frequency_hz: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "CapacitorParameters.ts")]
pub struct CapacitorParameters {
    pub capacitance_farads: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub initial_voltage_volts: Option<f64>,
}

fn deserialize_optional_non_null_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    f64::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CapacitorParametersInput {
    capacitance_farads: f64,
    #[serde(default, deserialize_with = "deserialize_optional_non_null_f64")]
    initial_voltage_volts: Option<f64>,
}

impl<'de> Deserialize<'de> for CapacitorParameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let input = CapacitorParametersInput::deserialize(deserializer)?;
        Ok(Self {
            capacitance_farads: input.capacitance_farads,
            initial_voltage_volts: input.initial_voltage_volts,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "InductorParameters.ts")]
pub struct InductorParameters {
    pub inductance_henries: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub initial_current_amps: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "RelayParameters.ts")]
pub struct RelayParameters {
    pub coil_resistance_ohms: f64,
    pub coil_inductance_henries: f64,
    pub pickup_current_amps: f64,
    pub dropout_current_amps: f64,
    pub closed_contact_resistance_ohms: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ContactorThreePoleParameters.ts")]
pub struct ContactorThreePoleParameters {
    pub coil_resistance_ohms: f64,
    pub coil_inductance_henries: f64,
    pub pickup_current_amps: f64,
    pub dropout_current_amps: f64,
    pub closed_contact_resistance_ohms: f64,
    pub coil_sense_seconds: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "BreakerThreePoleParameters.ts")]
pub struct BreakerThreePoleParameters {
    pub closed_contact_resistance_ohms: f64,
    pub instantaneous_trip_amps: f64,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null_f64",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub initial_closed: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "RcdParameters.ts")]
pub struct RcdParameters {
    pub rated_residual_current_amps: f64,
    pub trip_time_at_rated_seconds: f64,
    pub residual_sense_seconds: f64,
    pub closed_contact_resistance_ohms: f64,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null_f64",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub initial_closed: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ThermalOverloadThreePoleParameters.ts")]
pub struct ThermalOverloadThreePoleParameters {
    pub closed_contact_resistance_ohms: f64,
    pub sense_resistance_ohms: f64,
    pub rated_current_amps: f64,
    pub thermal_sense_seconds: f64,
    pub trip_scale_seconds: f64,
    pub cool_seconds: f64,
    pub reset_dose: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "InductionMotorThreePhaseParameters.ts")]
pub struct InductionMotorThreePhaseParameters {
    pub stator_resistance_ohms: f64,
    pub rotor_resistance_ohms: f64,
    pub stator_leakage_henries: f64,
    pub rotor_leakage_henries: f64,
    pub magnetizing_henries: f64,
    pub pole_pairs: f64,
    pub rotor_inertia_kilogram_meters_squared: f64,
    pub viscous_friction_newton_meter_seconds_per_radian: f64,
    pub load_torque_newton_meters: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "FuseParameters.ts")]
pub struct FuseParameters {
    pub intact_resistance_ohms: f64,
    pub trip_i2t_ampere_squared_seconds: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "TransformerThreePhaseParameters.ts")]
pub struct TransformerThreePhaseParameters {
    pub turns_ratio: f64,
    pub primary_resistance_ohms: f64,
    pub secondary_resistance_ohms: f64,
    pub primary_leakage_henries: f64,
    pub secondary_leakage_henries: f64,
    pub magnetizing_henries: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ThreePhaseLoadParameters.ts")]
pub struct ThreePhaseLoadParameters {
    pub phase1_resistance_ohms: f64,
    pub phase1_inductance_henries: f64,
    pub phase2_resistance_ohms: f64,
    pub phase2_inductance_henries: f64,
    pub phase3_resistance_ohms: f64,
    pub phase3_inductance_henries: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "PmdcMotorParameters.ts")]
pub struct PmdcMotorParameters {
    pub armature_resistance_ohms: f64,
    pub armature_inductance_henries: f64,
    pub motor_constant_newton_meters_per_amp: f64,
    pub rotor_inertia_kilogram_meters_squared: f64,
    pub viscous_friction_newton_meter_seconds_per_radian: f64,
    pub load_torque_newton_meters: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub initial_angular_velocity_radians_per_second: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub initial_armature_current_amps: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "VoltmeterParameters.ts")]
pub struct VoltmeterParameters {
    pub input_resistance_ohms: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "AmmeterParameters.ts")]
pub struct AmmeterParameters {
    pub burden_resistance_ohms: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PmdcMotorParametersInput {
    armature_resistance_ohms: f64,
    armature_inductance_henries: f64,
    motor_constant_newton_meters_per_amp: f64,
    rotor_inertia_kilogram_meters_squared: f64,
    viscous_friction_newton_meter_seconds_per_radian: f64,
    load_torque_newton_meters: f64,
    #[serde(default, deserialize_with = "deserialize_optional_non_null_f64")]
    initial_angular_velocity_radians_per_second: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null_f64")]
    initial_armature_current_amps: Option<f64>,
}

impl<'de> Deserialize<'de> for PmdcMotorParameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let input = PmdcMotorParametersInput::deserialize(deserializer)?;
        Ok(Self {
            armature_resistance_ohms: input.armature_resistance_ohms,
            armature_inductance_henries: input.armature_inductance_henries,
            motor_constant_newton_meters_per_amp: input.motor_constant_newton_meters_per_amp,
            rotor_inertia_kilogram_meters_squared: input.rotor_inertia_kilogram_meters_squared,
            viscous_friction_newton_meter_seconds_per_radian: input
                .viscous_friction_newton_meter_seconds_per_radian,
            load_torque_newton_meters: input.load_torque_newton_meters,
            initial_angular_velocity_radians_per_second: input
                .initial_angular_velocity_radians_per_second,
            initial_armature_current_amps: input.initial_armature_current_amps,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InductorParametersInput {
    inductance_henries: f64,
    #[serde(default, deserialize_with = "deserialize_optional_non_null_f64")]
    initial_current_amps: Option<f64>,
}

impl<'de> Deserialize<'de> for InductorParameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let input = InductorParametersInput::deserialize(deserializer)?;
        Ok(Self {
            inductance_henries: input.inductance_henries,
            initial_current_amps: input.initial_current_amps,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "SwitchParameters.ts")]
pub struct SwitchParameters {
    pub closed_resistance_ohms: f64,
    // An initial condition: 1 draws the switch closed, 0 or absent open (Stage 6 contract
    // section 4.5). The position reached while running is never a parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub initial_closed: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SwitchParametersInput {
    closed_resistance_ohms: f64,
    #[serde(default, deserialize_with = "deserialize_optional_non_null_f64")]
    initial_closed: Option<f64>,
}

impl<'de> Deserialize<'de> for SwitchParameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let input = SwitchParametersInput::deserialize(deserializer)?;
        Ok(Self {
            closed_resistance_ohms: input.closed_resistance_ohms,
            initial_closed: input.initial_closed,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "PushButtonParameters.ts")]
pub struct PushButtonParameters {
    pub closed_resistance_ohms: f64,
}

/// A typed component understood by the current Engine. Persisted project versions and
/// assignment support have their own compatibility boundaries.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(
    tag = "typeId",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, export_to = "ComponentSpec.ts")]
pub enum ComponentSpec {
    BjtNpn {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: BjtParameters,
    },
    BjtPnp {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: BjtParameters,
    },

    Diode {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: DiodeParameters,
    },
    Resistor {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: ResistorParameters,
    },
    DcVoltageSource {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: DcVoltageSourceParameters,
    },
    AcVoltageSource {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: AcVoltageSourceParameters,
    },
    ThreePhaseSourceStar {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: ThreePhaseSourceStarParameters,
    },
    ThreePhaseSourceDelta {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: ThreePhaseSourceDeltaParameters,
    },
    AcVoltmeter {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: AcVoltmeterParameters,
    },
    AcAmmeter {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: AcAmmeterParameters,
    },
    Wattmeter {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: WattmeterParameters,
    },
    Capacitor {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: CapacitorParameters,
    },
    Inductor {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: InductorParameters,
    },
    RelaySpdt {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: RelayParameters,
    },
    Fuse {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: FuseParameters,
    },
    PmdcMotor {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: PmdcMotorParameters,
    },
    Voltmeter {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: VoltmeterParameters,
    },
    Ammeter {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: AmmeterParameters,
    },
    SwitchSpst {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: SwitchParameters,
    },
    PushButtonNo {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: PushButtonParameters,
    },
    PushButtonNc {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: PushButtonParameters,
    },
    Contactor {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: RelayParameters,
    },
    ContactorThreePole {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: ContactorThreePoleParameters,
    },
    BreakerThreePole {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: BreakerThreePoleParameters,
    },
    ThermalOverloadThreePole {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: ThermalOverloadThreePoleParameters,
    },
    InductionMotorThreePhase {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: InductionMotorThreePhaseParameters,
    },
    TransformerThreePhase {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: TransformerThreePhaseParameters,
    },
    ThreePhaseLoadStar {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: ThreePhaseLoadParameters,
    },
    ThreePhaseLoadDelta {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: ThreePhaseLoadParameters,
    },
    EarthElectrode {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: EarthElectrodeParameters,
    },
    ExposedPart {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: ExposedPartParameters,
    },
    RcdTwoPole {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: RcdParameters,
    },
    RcdFourPole {
        id: StableId,
        position: GridPoint,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation_deg: u16,
        parameters: RcdParameters,
    },
}

impl ComponentSpec {
    pub fn id(&self) -> &StableId {
        match self {
            Self::BjtNpn { id, .. }
            | Self::BjtPnp { id, .. }
            | Self::Diode { id, .. }
            | Self::Resistor { id, .. }
            | Self::DcVoltageSource { id, .. }
            | Self::AcVoltageSource { id, .. }
            | Self::ThreePhaseSourceStar { id, .. }
            | Self::ThreePhaseSourceDelta { id, .. }
            | Self::AcVoltmeter { id, .. }
            | Self::AcAmmeter { id, .. }
            | Self::Wattmeter { id, .. }
            | Self::Capacitor { id, .. }
            | Self::Inductor { id, .. }
            | Self::RelaySpdt { id, .. }
            | Self::Fuse { id, .. }
            | Self::PmdcMotor { id, .. }
            | Self::Voltmeter { id, .. }
            | Self::Ammeter { id, .. }
            | Self::SwitchSpst { id, .. }
            | Self::PushButtonNo { id, .. }
            | Self::PushButtonNc { id, .. }
            | Self::Contactor { id, .. }
            | Self::ContactorThreePole { id, .. }
            | Self::BreakerThreePole { id, .. }
            | Self::ThermalOverloadThreePole { id, .. }
            | Self::InductionMotorThreePhase { id, .. }
            | Self::TransformerThreePhase { id, .. }
            | Self::ThreePhaseLoadStar { id, .. }
            | Self::EarthElectrode { id, .. }
            | Self::ExposedPart { id, .. }
            | Self::RcdTwoPole { id, .. }
            | Self::RcdFourPole { id, .. }
            | Self::ThreePhaseLoadDelta { id, .. } => id,
        }
    }

    pub fn position(&self) -> GridPoint {
        match self {
            Self::BjtNpn { position, .. }
            | Self::BjtPnp { position, .. }
            | Self::Diode { position, .. }
            | Self::Resistor { position, .. }
            | Self::DcVoltageSource { position, .. }
            | Self::AcVoltageSource { position, .. }
            | Self::ThreePhaseSourceStar { position, .. }
            | Self::ThreePhaseSourceDelta { position, .. }
            | Self::AcVoltmeter { position, .. }
            | Self::AcAmmeter { position, .. }
            | Self::Wattmeter { position, .. }
            | Self::Capacitor { position, .. }
            | Self::Inductor { position, .. }
            | Self::RelaySpdt { position, .. }
            | Self::Fuse { position, .. }
            | Self::PmdcMotor { position, .. }
            | Self::Voltmeter { position, .. }
            | Self::Ammeter { position, .. }
            | Self::SwitchSpst { position, .. }
            | Self::PushButtonNo { position, .. }
            | Self::PushButtonNc { position, .. }
            | Self::Contactor { position, .. }
            | Self::ContactorThreePole { position, .. }
            | Self::BreakerThreePole { position, .. }
            | Self::ThermalOverloadThreePole { position, .. }
            | Self::InductionMotorThreePhase { position, .. }
            | Self::TransformerThreePhase { position, .. }
            | Self::ThreePhaseLoadStar { position, .. }
            | Self::EarthElectrode { position, .. }
            | Self::ExposedPart { position, .. }
            | Self::RcdTwoPole { position, .. }
            | Self::RcdFourPole { position, .. }
            | Self::ThreePhaseLoadDelta { position, .. } => *position,
        }
    }

    pub fn rotation_deg(&self) -> u16 {
        match self {
            Self::BjtNpn { rotation_deg, .. }
            | Self::BjtPnp { rotation_deg, .. }
            | Self::Diode { rotation_deg, .. }
            | Self::Resistor { rotation_deg, .. }
            | Self::DcVoltageSource { rotation_deg, .. }
            | Self::AcVoltageSource { rotation_deg, .. }
            | Self::ThreePhaseSourceStar { rotation_deg, .. }
            | Self::ThreePhaseSourceDelta { rotation_deg, .. }
            | Self::AcVoltmeter { rotation_deg, .. }
            | Self::AcAmmeter { rotation_deg, .. }
            | Self::Wattmeter { rotation_deg, .. }
            | Self::Capacitor { rotation_deg, .. }
            | Self::Inductor { rotation_deg, .. }
            | Self::RelaySpdt { rotation_deg, .. }
            | Self::Fuse { rotation_deg, .. }
            | Self::PmdcMotor { rotation_deg, .. }
            | Self::Voltmeter { rotation_deg, .. }
            | Self::Ammeter { rotation_deg, .. }
            | Self::SwitchSpst { rotation_deg, .. }
            | Self::PushButtonNo { rotation_deg, .. }
            | Self::PushButtonNc { rotation_deg, .. }
            | Self::Contactor { rotation_deg, .. }
            | Self::ContactorThreePole { rotation_deg, .. }
            | Self::BreakerThreePole { rotation_deg, .. }
            | Self::ThermalOverloadThreePole { rotation_deg, .. }
            | Self::InductionMotorThreePhase { rotation_deg, .. }
            | Self::TransformerThreePhase { rotation_deg, .. }
            | Self::ThreePhaseLoadStar { rotation_deg, .. }
            | Self::EarthElectrode { rotation_deg, .. }
            | Self::ExposedPart { rotation_deg, .. }
            | Self::RcdTwoPole { rotation_deg, .. }
            | Self::RcdFourPole { rotation_deg, .. }
            | Self::ThreePhaseLoadDelta { rotation_deg, .. } => *rotation_deg,
        }
    }

    pub const fn type_id(&self) -> ComponentTypeId {
        match self {
            Self::BjtNpn { .. } => ComponentTypeId::BjtNpn,
            Self::BjtPnp { .. } => ComponentTypeId::BjtPnp,
            Self::Diode { .. } => ComponentTypeId::Diode,
            Self::Resistor { .. } => ComponentTypeId::Resistor,
            Self::DcVoltageSource { .. } => ComponentTypeId::DcVoltageSource,
            Self::AcVoltageSource { .. } => ComponentTypeId::AcVoltageSource,
            Self::ThreePhaseSourceStar { .. } => ComponentTypeId::ThreePhaseSourceStar,
            Self::ThreePhaseSourceDelta { .. } => ComponentTypeId::ThreePhaseSourceDelta,
            Self::AcVoltmeter { .. } => ComponentTypeId::AcVoltmeter,
            Self::AcAmmeter { .. } => ComponentTypeId::AcAmmeter,
            Self::Wattmeter { .. } => ComponentTypeId::Wattmeter,
            Self::Capacitor { .. } => ComponentTypeId::Capacitor,
            Self::Inductor { .. } => ComponentTypeId::Inductor,
            Self::RelaySpdt { .. } => ComponentTypeId::RelaySpdt,
            Self::Fuse { .. } => ComponentTypeId::Fuse,
            Self::PmdcMotor { .. } => ComponentTypeId::PmdcMotor,
            Self::Voltmeter { .. } => ComponentTypeId::Voltmeter,
            Self::Ammeter { .. } => ComponentTypeId::Ammeter,
            Self::SwitchSpst { .. } => ComponentTypeId::SwitchSpst,
            Self::PushButtonNo { .. } => ComponentTypeId::PushButtonNo,
            Self::PushButtonNc { .. } => ComponentTypeId::PushButtonNc,
            Self::Contactor { .. } => ComponentTypeId::Contactor,
            Self::ContactorThreePole { .. } => ComponentTypeId::ContactorThreePole,
            Self::BreakerThreePole { .. } => ComponentTypeId::BreakerThreePole,
            Self::ThermalOverloadThreePole { .. } => ComponentTypeId::ThermalOverloadThreePole,
            Self::InductionMotorThreePhase { .. } => ComponentTypeId::InductionMotorThreePhase,
            Self::TransformerThreePhase { .. } => ComponentTypeId::TransformerThreePhase,
            Self::ThreePhaseLoadStar { .. } => ComponentTypeId::ThreePhaseLoadStar,
            Self::ThreePhaseLoadDelta { .. } => ComponentTypeId::ThreePhaseLoadDelta,
            Self::EarthElectrode { .. } => ComponentTypeId::EarthElectrode,
            Self::ExposedPart { .. } => ComponentTypeId::ExposedPart,
            Self::RcdTwoPole { .. } => ComponentTypeId::RcdTwoPole,
            Self::RcdFourPole { .. } => ComponentTypeId::RcdFourPole,
        }
    }

    pub(crate) fn parameter_values(&self) -> Vec<(ParameterId, f64)> {
        match self {
            Self::ContactorThreePole { parameters, .. } => vec![
                (
                    ParameterId::CoilResistanceOhms,
                    parameters.coil_resistance_ohms,
                ),
                (
                    ParameterId::CoilInductanceHenries,
                    parameters.coil_inductance_henries,
                ),
                (
                    ParameterId::PickupCurrentAmps,
                    parameters.pickup_current_amps,
                ),
                (
                    ParameterId::DropoutCurrentAmps,
                    parameters.dropout_current_amps,
                ),
                (
                    ParameterId::ClosedContactResistanceOhms,
                    parameters.closed_contact_resistance_ohms,
                ),
                (ParameterId::CoilSenseSeconds, parameters.coil_sense_seconds),
            ],
            Self::TransformerThreePhase { parameters, .. } => vec![
                (ParameterId::TurnsRatio, parameters.turns_ratio),
                (
                    ParameterId::PrimaryResistanceOhms,
                    parameters.primary_resistance_ohms,
                ),
                (
                    ParameterId::SecondaryResistanceOhms,
                    parameters.secondary_resistance_ohms,
                ),
                (
                    ParameterId::PrimaryLeakageHenries,
                    parameters.primary_leakage_henries,
                ),
                (
                    ParameterId::SecondaryLeakageHenries,
                    parameters.secondary_leakage_henries,
                ),
                (
                    ParameterId::MagnetizingHenries,
                    parameters.magnetizing_henries,
                ),
            ],
            Self::ThreePhaseLoadStar { parameters, .. }
            | Self::ThreePhaseLoadDelta { parameters, .. } => vec![
                (
                    ParameterId::Phase1ResistanceOhms,
                    parameters.phase1_resistance_ohms,
                ),
                (
                    ParameterId::Phase1InductanceHenries,
                    parameters.phase1_inductance_henries,
                ),
                (
                    ParameterId::Phase2ResistanceOhms,
                    parameters.phase2_resistance_ohms,
                ),
                (
                    ParameterId::Phase2InductanceHenries,
                    parameters.phase2_inductance_henries,
                ),
                (
                    ParameterId::Phase3ResistanceOhms,
                    parameters.phase3_resistance_ohms,
                ),
                (
                    ParameterId::Phase3InductanceHenries,
                    parameters.phase3_inductance_henries,
                ),
            ],
            Self::InductionMotorThreePhase { parameters, .. } => vec![
                (
                    ParameterId::StatorResistanceOhms,
                    parameters.stator_resistance_ohms,
                ),
                (
                    ParameterId::RotorResistanceOhms,
                    parameters.rotor_resistance_ohms,
                ),
                (
                    ParameterId::StatorLeakageHenries,
                    parameters.stator_leakage_henries,
                ),
                (
                    ParameterId::RotorLeakageHenries,
                    parameters.rotor_leakage_henries,
                ),
                (
                    ParameterId::MagnetizingHenries,
                    parameters.magnetizing_henries,
                ),
                (ParameterId::PolePairs, parameters.pole_pairs),
                (
                    ParameterId::RotorInertiaKilogramMetersSquared,
                    parameters.rotor_inertia_kilogram_meters_squared,
                ),
                (
                    ParameterId::ViscousFrictionNewtonMeterSecondsPerRadian,
                    parameters.viscous_friction_newton_meter_seconds_per_radian,
                ),
                (
                    ParameterId::LoadTorqueNewtonMeters,
                    parameters.load_torque_newton_meters,
                ),
            ],
            Self::ThermalOverloadThreePole { parameters, .. } => vec![
                (
                    ParameterId::ClosedContactResistanceOhms,
                    parameters.closed_contact_resistance_ohms,
                ),
                (
                    ParameterId::SenseResistanceOhms,
                    parameters.sense_resistance_ohms,
                ),
                (ParameterId::RatedCurrentAmps, parameters.rated_current_amps),
                (
                    ParameterId::ThermalSenseSeconds,
                    parameters.thermal_sense_seconds,
                ),
                (ParameterId::TripScaleSeconds, parameters.trip_scale_seconds),
                (ParameterId::CoolSeconds, parameters.cool_seconds),
                (ParameterId::ResetDose, parameters.reset_dose),
            ],
            Self::RcdTwoPole { parameters, .. } | Self::RcdFourPole { parameters, .. } => vec![
                (
                    ParameterId::RatedResidualCurrentAmps,
                    parameters.rated_residual_current_amps,
                ),
                (
                    ParameterId::TripTimeAtRatedSeconds,
                    parameters.trip_time_at_rated_seconds,
                ),
                (
                    ParameterId::ResidualSenseSeconds,
                    parameters.residual_sense_seconds,
                ),
                (
                    ParameterId::ClosedContactResistanceOhms,
                    parameters.closed_contact_resistance_ohms,
                ),
            ]
            .into_iter()
            .chain(
                parameters
                    .initial_closed
                    .map(|value| (ParameterId::InitialClosed, value)),
            )
            .collect(),
            Self::BreakerThreePole { parameters, .. } => vec![
                (
                    ParameterId::ClosedContactResistanceOhms,
                    parameters.closed_contact_resistance_ohms,
                ),
                (
                    ParameterId::InstantaneousTripAmps,
                    parameters.instantaneous_trip_amps,
                ),
            ]
            .into_iter()
            .chain(
                parameters
                    .initial_closed
                    .map(|value| (ParameterId::InitialClosed, value)),
            )
            .collect(),
            Self::BjtNpn { parameters, .. } | Self::BjtPnp { parameters, .. } => vec![
                (
                    ParameterId::SaturationCurrentAmps,
                    parameters.saturation_current_amps,
                ),
                (ParameterId::ForwardBeta, parameters.forward_beta),
                (ParameterId::ReverseBeta, parameters.reverse_beta),
            ],
            Self::Diode { parameters, .. } => vec![
                (
                    ParameterId::SaturationCurrentAmps,
                    parameters.saturation_current_amps,
                ),
                (
                    ParameterId::EmissionCoefficient,
                    parameters.emission_coefficient,
                ),
            ],
            Self::Resistor { parameters, .. } => {
                vec![(ParameterId::ResistanceOhms, parameters.resistance_ohms)]
            }
            Self::DcVoltageSource { parameters, .. } => vec![
                (ParameterId::VoltageVolts, parameters.voltage_volts),
                (
                    ParameterId::InternalResistanceOhms,
                    parameters.internal_resistance_ohms,
                ),
            ],
            Self::AcVoltageSource { parameters, .. } => vec![
                (ParameterId::VoltageRmsVolts, parameters.voltage_rms_volts),
                (ParameterId::FrequencyHz, parameters.frequency_hz),
                (
                    ParameterId::PhaseOffsetDegrees,
                    parameters.phase_offset_degrees,
                ),
                (
                    ParameterId::InternalResistanceOhms,
                    parameters.internal_resistance_ohms,
                ),
            ],
            Self::ThreePhaseSourceStar { parameters, .. } => vec![
                (
                    ParameterId::PhaseVoltageRmsVolts,
                    parameters.phase_voltage_rms_volts,
                ),
                (ParameterId::FrequencyHz, parameters.frequency_hz),
                (
                    ParameterId::PhaseOffsetDegrees,
                    parameters.phase_offset_degrees,
                ),
                (
                    ParameterId::InternalResistanceOhms,
                    parameters.internal_resistance_ohms,
                ),
                (ParameterId::ReversedSequence, parameters.reversed_sequence),
            ],
            Self::ThreePhaseSourceDelta { parameters, .. } => vec![
                (
                    ParameterId::LineVoltageRmsVolts,
                    parameters.line_voltage_rms_volts,
                ),
                (ParameterId::FrequencyHz, parameters.frequency_hz),
                (
                    ParameterId::PhaseOffsetDegrees,
                    parameters.phase_offset_degrees,
                ),
                (
                    ParameterId::InternalResistanceOhms,
                    parameters.internal_resistance_ohms,
                ),
                (ParameterId::ReversedSequence, parameters.reversed_sequence),
            ],
            Self::AcVoltmeter { parameters, .. } => vec![
                (
                    ParameterId::InputResistanceOhms,
                    parameters.input_resistance_ohms,
                ),
                (ParameterId::FrequencyHz, parameters.frequency_hz),
            ],
            Self::AcAmmeter { parameters, .. } => vec![
                (
                    ParameterId::BurdenResistanceOhms,
                    parameters.burden_resistance_ohms,
                ),
                (ParameterId::FrequencyHz, parameters.frequency_hz),
            ],
            Self::EarthElectrode { parameters, .. } => vec![(
                ParameterId::EarthResistanceOhms,
                parameters.earth_resistance_ohms,
            )],
            Self::ExposedPart { parameters, .. } => vec![
                (ParameterId::FrequencyHz, parameters.frequency_hz),
                (
                    ParameterId::TouchVoltageLimitVolts,
                    parameters.touch_voltage_limit_volts,
                ),
            ],
            Self::Wattmeter { parameters, .. } => vec![
                (
                    ParameterId::InputResistanceOhms,
                    parameters.input_resistance_ohms,
                ),
                (
                    ParameterId::BurdenResistanceOhms,
                    parameters.burden_resistance_ohms,
                ),
                (ParameterId::FrequencyHz, parameters.frequency_hz),
            ],
            Self::Capacitor { parameters, .. } => {
                let mut values = vec![(
                    ParameterId::CapacitanceFarads,
                    parameters.capacitance_farads,
                )];
                if let Some(value) = parameters.initial_voltage_volts {
                    values.push((ParameterId::InitialVoltageVolts, value));
                }
                values
            }
            Self::Inductor { parameters, .. } => {
                let mut values = vec![(
                    ParameterId::InductanceHenries,
                    parameters.inductance_henries,
                )];
                if let Some(value) = parameters.initial_current_amps {
                    values.push((ParameterId::InitialCurrentAmps, value));
                }
                values
            }
            Self::RelaySpdt { parameters, .. } | Self::Contactor { parameters, .. } => vec![
                (
                    ParameterId::CoilResistanceOhms,
                    parameters.coil_resistance_ohms,
                ),
                (
                    ParameterId::CoilInductanceHenries,
                    parameters.coil_inductance_henries,
                ),
                (
                    ParameterId::PickupCurrentAmps,
                    parameters.pickup_current_amps,
                ),
                (
                    ParameterId::DropoutCurrentAmps,
                    parameters.dropout_current_amps,
                ),
                (
                    ParameterId::ClosedContactResistanceOhms,
                    parameters.closed_contact_resistance_ohms,
                ),
            ],
            Self::Fuse { parameters, .. } => vec![
                (
                    ParameterId::IntactResistanceOhms,
                    parameters.intact_resistance_ohms,
                ),
                (
                    ParameterId::TripI2tAmpereSquaredSeconds,
                    parameters.trip_i2t_ampere_squared_seconds,
                ),
            ],
            Self::PmdcMotor { parameters, .. } => {
                let mut values = vec![
                    (
                        ParameterId::ArmatureResistanceOhms,
                        parameters.armature_resistance_ohms,
                    ),
                    (
                        ParameterId::ArmatureInductanceHenries,
                        parameters.armature_inductance_henries,
                    ),
                    (
                        ParameterId::MotorConstantNewtonMetersPerAmp,
                        parameters.motor_constant_newton_meters_per_amp,
                    ),
                    (
                        ParameterId::RotorInertiaKilogramMetersSquared,
                        parameters.rotor_inertia_kilogram_meters_squared,
                    ),
                    (
                        ParameterId::ViscousFrictionNewtonMeterSecondsPerRadian,
                        parameters.viscous_friction_newton_meter_seconds_per_radian,
                    ),
                    (
                        ParameterId::LoadTorqueNewtonMeters,
                        parameters.load_torque_newton_meters,
                    ),
                ];
                if let Some(value) = parameters.initial_angular_velocity_radians_per_second {
                    values.push((ParameterId::InitialAngularVelocityRadiansPerSecond, value));
                }
                if let Some(value) = parameters.initial_armature_current_amps {
                    values.push((ParameterId::InitialArmatureCurrentAmps, value));
                }
                values
            }
            Self::Voltmeter { parameters, .. } => {
                vec![(
                    ParameterId::InputResistanceOhms,
                    parameters.input_resistance_ohms,
                )]
            }
            Self::Ammeter { parameters, .. } => {
                vec![(
                    ParameterId::BurdenResistanceOhms,
                    parameters.burden_resistance_ohms,
                )]
            }
            Self::SwitchSpst { parameters, .. } => {
                let mut values = vec![(
                    ParameterId::ClosedResistanceOhms,
                    parameters.closed_resistance_ohms,
                )];
                if let Some(value) = parameters.initial_closed {
                    values.push((ParameterId::InitialClosed, value));
                }
                values
            }
            Self::PushButtonNo { parameters, .. } | Self::PushButtonNc { parameters, .. } => {
                vec![(
                    ParameterId::ClosedResistanceOhms,
                    parameters.closed_resistance_ohms,
                )]
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "WireSpec.ts")]
pub struct WireSpec {
    pub id: StableId,
    pub points: Vec<GridPoint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "JunctionSpec.ts")]
pub struct JunctionSpec {
    pub id: StableId,
    pub position: GridPoint,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ReferenceMarkerSpec.ts")]
pub struct ReferenceMarkerSpec {
    pub id: StableId,
    pub position: GridPoint,
}

/// Persisted user-drawn content only; topology, solver values, and snapshots are not fields.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "Schematic.ts")]
pub struct Schematic {
    #[ts(type = "3")]
    pub schema_version: u32,
    pub components: Vec<ComponentSpec>,
    pub wires: Vec<WireSpec>,
    pub junctions: Vec<JunctionSpec>,
    pub reference_markers: Vec<ReferenceMarkerSpec>,
    pub rails: Vec<crate::RailSpec>,
    /// Designators, wire numbers, terminals, notes, a title block and sheets. The Engine
    /// ignores it when it compiles and runs a circuit.
    pub documentation: crate::ProjectDocumentation,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "GeometryEnvelope.ts")]
pub struct GeometryEnvelope {
    pub components: Vec<ComponentGeometry>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ComponentGeometry.ts")]
pub struct ComponentGeometry {
    pub component_id: StableId,
    pub pins: Vec<ResolvedPinPosition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ResolvedPinPosition.ts")]
pub struct ResolvedPinPosition {
    pub pin_id: String,
    pub position: GridPoint,
}

impl Schematic {
    /// Checks version, cross-document stable IDs, geometry path shape, and catalog parameter
    /// domains. It deliberately does not build topology or solve the circuit.
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if self.rails.len() > crate::MAX_RAIL_COUNT {
            return vec![Diagnostic::error(
                DiagnosticCode::RailLimitExceeded,
                Vec::new(),
            )];
        }
        if self.schema_version != SCHEMA_VERSION {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidSchemaVersion,
                Vec::new(),
            ));
        }

        let mut ids = HashMap::new();
        let catalog = component_catalog();
        for component in &self.components {
            record_id(
                &mut ids,
                component.id(),
                DiagnosticReference::Component {
                    component_id: component.id().clone(),
                },
                &mut diagnostics,
            );

            if let ComponentSpec::BjtNpn { rotation_deg, .. }
            | ComponentSpec::BjtPnp { rotation_deg, .. }
            | ComponentSpec::Diode { rotation_deg, .. }
            | ComponentSpec::Resistor { rotation_deg, .. }
            | ComponentSpec::DcVoltageSource { rotation_deg, .. }
            | ComponentSpec::AcVoltageSource { rotation_deg, .. }
            | ComponentSpec::ThreePhaseSourceStar { rotation_deg, .. }
            | ComponentSpec::ThreePhaseSourceDelta { rotation_deg, .. }
            | ComponentSpec::AcVoltmeter { rotation_deg, .. }
            | ComponentSpec::AcAmmeter { rotation_deg, .. }
            | ComponentSpec::Wattmeter { rotation_deg, .. }
            | ComponentSpec::Capacitor { rotation_deg, .. }
            | ComponentSpec::Inductor { rotation_deg, .. }
            | ComponentSpec::RelaySpdt { rotation_deg, .. }
            | ComponentSpec::Fuse { rotation_deg, .. }
            | ComponentSpec::PmdcMotor { rotation_deg, .. }
            | ComponentSpec::Voltmeter { rotation_deg, .. }
            | ComponentSpec::Ammeter { rotation_deg, .. }
            | ComponentSpec::SwitchSpst { rotation_deg, .. }
            | ComponentSpec::PushButtonNo { rotation_deg, .. }
            | ComponentSpec::PushButtonNc { rotation_deg, .. }
            | ComponentSpec::Contactor { rotation_deg, .. }
            | ComponentSpec::ContactorThreePole { rotation_deg, .. }
            | ComponentSpec::BreakerThreePole { rotation_deg, .. }
            | ComponentSpec::ThermalOverloadThreePole { rotation_deg, .. }
            | ComponentSpec::InductionMotorThreePhase { rotation_deg, .. }
            | ComponentSpec::TransformerThreePhase { rotation_deg, .. }
            | ComponentSpec::ThreePhaseLoadStar { rotation_deg, .. }
            | ComponentSpec::EarthElectrode { rotation_deg, .. }
            | ComponentSpec::ExposedPart { rotation_deg, .. }
            | ComponentSpec::RcdTwoPole { rotation_deg, .. }
            | ComponentSpec::RcdFourPole { rotation_deg, .. }
            | ComponentSpec::ThreePhaseLoadDelta { rotation_deg, .. } = component
                && !matches!(rotation_deg, 0 | 90 | 180 | 270)
            {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidGeometry,
                    vec![DiagnosticReference::Component {
                        component_id: component.id().clone(),
                    }],
                ));
            }
            let catalog_entry = catalog
                .component_types
                .iter()
                .find(|entry| entry.type_id == component.type_id());
            if let Some(ComponentCatalogEntry { parameters, .. }) = catalog_entry {
                let values = component.parameter_values();
                for parameter in parameters {
                    let parameter_value = values
                        .iter()
                        .find(|(parameter_id, _)| *parameter_id == parameter.id)
                        .map(|(_, value)| *value);
                    let valid = match parameter_value {
                        Some(value) => parameter.accepts(value),
                        None => parameter.optional,
                    };
                    if !valid {
                        diagnostics.push(Diagnostic::error(
                            DiagnosticCode::InvalidParameter,
                            vec![DiagnosticReference::Component {
                                component_id: component.id().clone(),
                            }],
                        ));
                    }
                }

                if values.iter().any(|(parameter_id, _)| {
                    !parameters
                        .iter()
                        .any(|parameter| parameter.id == *parameter_id)
                }) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidParameter,
                        vec![DiagnosticReference::Component {
                            component_id: component.id().clone(),
                        }],
                    ));
                }
            }

            let thresholds = match component {
                ComponentSpec::RelaySpdt { parameters, .. }
                | ComponentSpec::Contactor { parameters, .. } => Some((
                    parameters.dropout_current_amps,
                    parameters.pickup_current_amps,
                )),
                ComponentSpec::ContactorThreePole { parameters, .. } => Some((
                    parameters.dropout_current_amps,
                    parameters.pickup_current_amps,
                )),
                _ => None,
            };
            if thresholds.is_some_and(|(dropout, pickup)| dropout >= pickup) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    vec![DiagnosticReference::Component {
                        component_id: component.id().clone(),
                    }],
                ));
            }
        }

        for wire in &self.wires {
            record_id(
                &mut ids,
                &wire.id,
                DiagnosticReference::Wire {
                    wire_id: wire.id.clone(),
                },
                &mut diagnostics,
            );

            let valid_path = wire.points.len() >= 2
                && wire.points.windows(2).all(|pair| {
                    pair[0] != pair[1] && (pair[0].x == pair[1].x || pair[0].y == pair[1].y)
                });
            if !valid_path {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidGeometry,
                    vec![DiagnosticReference::Wire {
                        wire_id: wire.id.clone(),
                    }],
                ));
            }
        }

        for junction in &self.junctions {
            record_id(
                &mut ids,
                &junction.id,
                DiagnosticReference::Junction {
                    junction_id: junction.id.clone(),
                },
                &mut diagnostics,
            );
        }

        for marker in &self.reference_markers {
            record_id(
                &mut ids,
                &marker.id,
                DiagnosticReference::ReferenceMarker {
                    reference_marker_id: marker.id.clone(),
                },
                &mut diagnostics,
            );
        }

        diagnostics.extend(self.validate_rails());
        for rail in &self.rails {
            record_id(
                &mut ids,
                &rail.id,
                DiagnosticReference::Rail {
                    rail_id: rail.id.clone(),
                },
                &mut diagnostics,
            );
        }
        for terminal in &self.documentation.terminals {
            record_id(
                &mut ids,
                &terminal.id,
                DiagnosticReference::Terminal {
                    terminal_id: terminal.id.clone(),
                },
                &mut diagnostics,
            );
        }
        for note in &self.documentation.notes {
            record_id(
                &mut ids,
                &note.id,
                DiagnosticReference::Note {
                    note_id: note.id.clone(),
                },
                &mut diagnostics,
            );
        }
        for sheet in &self.documentation.sheets {
            record_id(
                &mut ids,
                &sheet.id,
                DiagnosticReference::Sheet {
                    sheet_id: sheet.id.clone(),
                },
                &mut diagnostics,
            );
        }
        diagnostics.extend(self.validate_documentation());
        diagnostics
    }
}

impl GeometryEnvelope {
    /// Validates the component and pin set against the authoritative Rust catalog.
    pub fn validate_for(&self, schematic: &Schematic) -> Vec<Diagnostic> {
        let pins = self
            .components
            .iter()
            .try_fold(0usize, |sum, c| sum.checked_add(c.pins.len()));
        if pins.is_none() || !schematic.rail_work_is_bounded(pins.unwrap_or(usize::MAX)) {
            return vec![schematic.rail_limit_diagnostic()];
        }
        let catalog = component_catalog();
        let mut diagnostics = Vec::new();
        let mut seen_components = HashSet::new();

        for geometry in &self.components {
            let Some(component) = schematic
                .components
                .iter()
                .find(|component| component.id() == &geometry.component_id)
            else {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidGeometry,
                    vec![DiagnosticReference::Component {
                        component_id: geometry.component_id.clone(),
                    }],
                ));
                continue;
            };

            if !seen_components.insert(geometry.component_id.as_str()) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::DuplicateId,
                    vec![DiagnosticReference::Component {
                        component_id: geometry.component_id.clone(),
                    }],
                ));
            }

            let Some(definition) = catalog
                .component_types
                .iter()
                .find(|definition| definition.type_id == component.type_id())
            else {
                continue;
            };

            let mut seen_pins = HashSet::new();
            for pin in &geometry.pins {
                if !seen_pins.insert(pin.pin_id.as_str()) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidGeometry,
                        vec![DiagnosticReference::Component {
                            component_id: geometry.component_id.clone(),
                        }],
                    ));
                }

                if !definition
                    .pins
                    .iter()
                    .any(|expected| expected.id.as_str() == pin.pin_id)
                {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::UnknownPin,
                        vec![DiagnosticReference::Pin {
                            component_id: geometry.component_id.clone(),
                            pin_id: pin.pin_id.clone(),
                        }],
                    ));
                }
            }

            for expected_pin in &definition.pins {
                if !seen_pins.contains(expected_pin.id.as_str()) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidGeometry,
                        vec![DiagnosticReference::pin(
                            geometry.component_id.clone(),
                            expected_pin.id,
                        )],
                    ));
                }
            }
        }

        for component in &schematic.components {
            if !seen_components.contains(component.id().as_str()) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidGeometry,
                    vec![DiagnosticReference::Component {
                        component_id: component.id().clone(),
                    }],
                ));
            }
        }

        diagnostics
    }
}

fn record_id(
    ids: &mut HashMap<String, DiagnosticReference>,
    id: &StableId,
    reference: DiagnosticReference,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(previous) = ids.get(id.as_str()) {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::DuplicateId,
            vec![previous.clone(), reference],
        ));
    } else {
        ids.insert(id.as_str().to_owned(), reference);
    }
}
