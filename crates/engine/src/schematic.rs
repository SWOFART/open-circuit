use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    ComponentCatalogEntry, ComponentTypeId, Diagnostic, DiagnosticCode, DiagnosticReference,
    ParameterId, StableId, component_catalog,
};

pub const SCHEMA_VERSION: u32 = 1;

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
#[ts(export, export_to = "FuseParameters.ts")]
pub struct FuseParameters {
    pub intact_resistance_ohms: f64,
    pub trip_i2t_ampere_squared_seconds: f64,
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

/// Saved schema version 1 supports resistors, independent voltage sources,
/// capacitors, inductors, relays, and fuses.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(
    tag = "typeId",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, export_to = "ComponentSpec.ts")]
pub enum ComponentSpec {
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
}

impl ComponentSpec {
    pub fn id(&self) -> &StableId {
        match self {
            Self::Resistor { id, .. }
            | Self::DcVoltageSource { id, .. }
            | Self::Capacitor { id, .. }
            | Self::Inductor { id, .. }
            | Self::RelaySpdt { id, .. }
            | Self::Fuse { id, .. } => id,
        }
    }

    pub const fn type_id(&self) -> ComponentTypeId {
        match self {
            Self::Resistor { .. } => ComponentTypeId::Resistor,
            Self::DcVoltageSource { .. } => ComponentTypeId::DcVoltageSource,
            Self::Capacitor { .. } => ComponentTypeId::Capacitor,
            Self::Inductor { .. } => ComponentTypeId::Inductor,
            Self::RelaySpdt { .. } => ComponentTypeId::RelaySpdt,
            Self::Fuse { .. } => ComponentTypeId::Fuse,
        }
    }

    fn parameter_values(&self) -> Vec<(ParameterId, f64)> {
        match self {
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
            Self::RelaySpdt { parameters, .. } => vec![
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
    #[ts(type = "1")]
    pub schema_version: u32,
    pub components: Vec<ComponentSpec>,
    pub wires: Vec<WireSpec>,
    pub junctions: Vec<JunctionSpec>,
    pub reference_markers: Vec<ReferenceMarkerSpec>,
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
        if self.schema_version != SCHEMA_VERSION {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidSchemaVersion,
                Vec::new(),
            ));
        }

        let mut ids = HashSet::new();
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

            if let ComponentSpec::Resistor { rotation_deg, .. }
            | ComponentSpec::DcVoltageSource { rotation_deg, .. }
            | ComponentSpec::Capacitor { rotation_deg, .. }
            | ComponentSpec::Inductor { rotation_deg, .. }
            | ComponentSpec::RelaySpdt { rotation_deg, .. }
            | ComponentSpec::Fuse { rotation_deg, .. } = component
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
                        Some(value) => parameter.domain.accepts(value),
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

            if let ComponentSpec::RelaySpdt { parameters, .. } = component
                && parameters.dropout_current_amps >= parameters.pickup_current_amps
            {
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

        diagnostics
    }
}

impl GeometryEnvelope {
    /// Validates the component and pin set against the authoritative Rust catalog.
    pub fn validate_for(&self, schematic: &Schematic) -> Vec<Diagnostic> {
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
    ids: &mut HashSet<String>,
    id: &StableId,
    reference: DiagnosticReference,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ids.insert(id.as_str().to_owned()) {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::DuplicateId,
            vec![reference],
        ));
    }
}
