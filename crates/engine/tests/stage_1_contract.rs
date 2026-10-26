use open_circuit_engine::{
    ComponentCatalog, ComponentTypeId, DiagnosticCode, GeometryEnvelope, NumericDomain,
    ParameterId, Schematic, StableId, Unit, component_catalog,
};
use serde_json::{Value, json};

const SCHEMATIC: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");
const EXPECTED: &str = include_str!("../../../tests/golden/stage-1-divider.expected.json");

fn parse_schematic() -> Schematic {
    serde_json::from_str(SCHEMATIC).expect("canonical Stage 1 schematic should deserialize")
}

fn diagnostic_codes(schematic: &Schematic) -> Vec<DiagnosticCode> {
    schematic
        .validate()
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn canonical_fixture_round_trips_and_contains_no_computed_state() {
    let schematic = parse_schematic();
    assert!(schematic.validate().is_empty());

    let encoded = serde_json::to_string(&schematic).expect("schematic should serialize");
    let decoded: Schematic =
        serde_json::from_str(&encoded).expect("serialized schematic should parse");
    assert_eq!(decoded, schematic);

    let document: Value = serde_json::from_str(&encoded).expect("serialized JSON should parse");
    assert_eq!(document["components"][0]["typeId"], "dc_voltage_source");
    assert_eq!(
        document["components"][0]["parameters"]["internalResistanceOhms"],
        0.1
    );
    assert_eq!(
        document
            .as_object()
            .expect("schema is an object")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        [
            "components",
            "junctions",
            "referenceMarkers",
            "schemaVersion",
            "wires"
        ]
    );
}

#[test]
fn rejects_unknown_schema_versions_and_noncanonical_ids() {
    let mut value: Value = serde_json::from_str(SCHEMATIC).unwrap();
    value["schemaVersion"] = json!(2);
    let unsupported: Schematic = serde_json::from_value(value).unwrap();
    assert!(diagnostic_codes(&unsupported).contains(&DiagnosticCode::InvalidSchemaVersion));

    let mut value: Value = serde_json::from_str(SCHEMATIC).unwrap();
    value["components"][0]["id"] = json!("AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA");
    assert!(serde_json::from_value::<Schematic>(value).is_err());
    assert!(StableId::parse("not-a-uuid").is_err());
}

#[test]
fn rejects_duplicate_ids_and_invalid_parameter_domains() {
    let mut duplicate: Value = serde_json::from_str(SCHEMATIC).unwrap();
    duplicate["wires"][0]["id"] = duplicate["components"][0]["id"].clone();
    let duplicate: Schematic = serde_json::from_value(duplicate).unwrap();
    assert!(diagnostic_codes(&duplicate).contains(&DiagnosticCode::DuplicateId));

    let mut invalid: Value = serde_json::from_str(SCHEMATIC).unwrap();
    invalid["components"][1]["parameters"]["resistanceOhms"] = json!(0.0);
    let invalid: Schematic = serde_json::from_value(invalid).unwrap();
    assert!(diagnostic_codes(&invalid).contains(&DiagnosticCode::InvalidParameter));

    let mut invalid: Value = serde_json::from_str(SCHEMATIC).unwrap();
    invalid["components"][0]["parameters"]["internalResistanceOhms"] = json!(-0.1);
    let invalid: Schematic = serde_json::from_value(invalid).unwrap();
    assert!(diagnostic_codes(&invalid).contains(&DiagnosticCode::InvalidParameter));
}

#[test]
fn rejects_unknown_parameters_and_computed_schematic_fields() {
    let mut unknown_parameter: Value = serde_json::from_str(SCHEMATIC).unwrap();
    unknown_parameter["components"][1]["parameters"]["resistanceFoo"] = json!(10.0);
    assert!(serde_json::from_value::<Schematic>(unknown_parameter).is_err());

    let mut computed_state: Value = serde_json::from_str(SCHEMATIC).unwrap();
    computed_state["nodeIds"] = json!(["node-1"]);
    assert!(serde_json::from_value::<Schematic>(computed_state).is_err());

    let mut unknown_type: Value = serde_json::from_str(SCHEMATIC).unwrap();
    unknown_type["components"][0]["typeId"] = json!("capacitor");
    assert!(serde_json::from_value::<Schematic>(unknown_type).is_err());
}

#[test]
fn rejects_invalid_component_rotations_and_wire_paths() {
    let mut invalid_rotation: Value = serde_json::from_str(SCHEMATIC).unwrap();
    invalid_rotation["components"][1]["rotationDeg"] = json!(45);
    let invalid_rotation: Schematic = serde_json::from_value(invalid_rotation).unwrap();
    assert!(diagnostic_codes(&invalid_rotation).contains(&DiagnosticCode::InvalidGeometry));

    let mut invalid_wire: Value = serde_json::from_str(SCHEMATIC).unwrap();
    invalid_wire["wires"][0]["points"][1] = json!({ "x": 0, "y": 3 });
    let invalid_wire: Schematic = serde_json::from_value(invalid_wire).unwrap();
    assert!(diagnostic_codes(&invalid_wire).contains(&DiagnosticCode::InvalidGeometry));
}

#[test]
fn catalog_is_authoritative_for_defaults_units_and_domains() {
    let catalog: ComponentCatalog = component_catalog();
    assert_eq!(catalog.component_types.len(), 5);

    let resistor = catalog
        .component_types
        .iter()
        .find(|entry| entry.type_id == ComponentTypeId::Resistor)
        .unwrap();
    assert_eq!(resistor.pins.len(), 2);
    assert_eq!(resistor.parameters[0].id, ParameterId::ResistanceOhms);
    assert_eq!(resistor.parameters[0].unit, Unit::Ohm);
    assert_eq!(resistor.parameters[0].default_value, 1000.0);
    assert_eq!(
        resistor.parameters[0].domain,
        NumericDomain::StrictlyPositive
    );

    let source = catalog
        .component_types
        .iter()
        .find(|entry| entry.type_id == ComponentTypeId::DcVoltageSource)
        .unwrap();
    assert_eq!(source.parameters[0].default_value, 5.0);
    assert_eq!(source.parameters[1].default_value, 0.1);
    assert_eq!(source.parameters[1].domain, NumericDomain::NonNegative);

    let capacitor = catalog
        .component_types
        .iter()
        .find(|entry| entry.type_id == ComponentTypeId::Capacitor)
        .unwrap();
    assert_eq!(capacitor.parameters[0].id, ParameterId::CapacitanceFarads);
    assert_eq!(capacitor.parameters[0].unit, Unit::Farad);
    assert_eq!(capacitor.parameters[0].default_value, 1e-5);
    assert_eq!(
        capacitor.parameters[0].domain,
        NumericDomain::StrictlyPositive
    );
    assert!(!capacitor.parameters[0].optional);
    assert!(capacitor.parameters[1].optional);
    assert!(capacitor.parameters[1].initial_condition);
    assert_eq!(capacitor.parameters[1].id, ParameterId::InitialVoltageVolts);
    assert_eq!(capacitor.parameters[1].unit, Unit::Volt);

    let inductor = catalog
        .component_types
        .iter()
        .find(|entry| entry.type_id == ComponentTypeId::Inductor)
        .unwrap();
    assert_eq!(inductor.parameters[0].id, ParameterId::InductanceHenries);
    assert_eq!(inductor.parameters[0].unit, Unit::Henry);
    assert_eq!(inductor.parameters[0].default_value, 0.1);
    assert_eq!(
        inductor.parameters[0].domain,
        NumericDomain::StrictlyPositive
    );
    assert!(inductor.parameters[1].optional);
    assert!(inductor.parameters[1].initial_condition);
    assert_eq!(inductor.parameters[1].id, ParameterId::InitialCurrentAmps);
    assert_eq!(inductor.parameters[1].unit, Unit::Amp);

    let relay = catalog
        .component_types
        .iter()
        .find(|entry| entry.type_id == ComponentTypeId::RelaySpdt)
        .unwrap();
    assert_eq!(relay.pins.len(), 5);
    assert_eq!(relay.pins[0].id.as_str(), "coil_positive");
    assert_eq!(relay.pins[1].id.as_str(), "coil_negative");
    assert_eq!(relay.pins[2].id.as_str(), "common");
    assert_eq!(relay.pins[3].id.as_str(), "normally_open");
    assert_eq!(relay.pins[4].id.as_str(), "normally_closed");
    assert_eq!(relay.parameters.len(), 5);
    assert_eq!(relay.parameters[0].default_value, 100.0);
    assert_eq!(relay.parameters[0].unit, Unit::Ohm);
    assert_eq!(relay.parameters[1].default_value, 0.01);
    assert_eq!(relay.parameters[1].unit, Unit::Henry);
    assert_eq!(relay.parameters[2].default_value, 0.04);
    assert_eq!(relay.parameters[2].unit, Unit::Amp);
    assert_eq!(relay.parameters[3].default_value, 0.02);
    assert_eq!(relay.parameters[4].default_value, 0.1);
    assert_eq!(relay.parameters[4].unit, Unit::Ohm);

    assert!(NumericDomain::NonNegative.accepts(0.0));
    assert!(NumericDomain::Finite.accepts(-5.0));
    assert!(!NumericDomain::Finite.accepts(f64::INFINITY));

    let serialized = serde_json::to_value(&catalog).unwrap();
    assert_eq!(serialized["componentTypes"][0]["typeId"], "resistor");
    assert_eq!(serialized["componentTypes"][1]["pins"][0]["id"], "pin1");
    assert_eq!(
        serialized["componentTypes"][0]["parameters"][0]["id"],
        "resistanceOhms"
    );
}

#[test]
fn canonical_geometry_has_the_catalog_pin_set_and_rejects_unknown_pins() {
    let schematic = parse_schematic();
    let mut geometry: GeometryEnvelope = serde_json::from_str(GEOMETRY).unwrap();
    assert!(geometry.validate_for(&schematic).is_empty());

    geometry.components[0].pins[0].pin_id = "pin3".to_owned();
    let diagnostics = geometry.validate_for(&schematic);
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::UnknownPin)
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::InvalidGeometry)
    );
}

#[test]
fn golden_results_remain_separate_and_match_contract_values() {
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    let equal = &expected["equalResistors"];
    let expected_equal = 5.0 / (0.1 + 1000.0 + 1000.0);
    assert!((equal["sourceCurrentAmps"].as_f64().unwrap() - expected_equal).abs() < 1e-15);
    assert!(
        (equal["resistor2VoltageVolts"].as_f64().unwrap() - 1000.0 * expected_equal).abs() < 1e-12
    );

    let changed = &expected["resistor1At3000Ohms"];
    let expected_changed = 5.0 / (0.1 + 3000.0 + 1000.0);
    assert!((changed["sourceCurrentAmps"].as_f64().unwrap() - expected_changed).abs() < 1e-15);
    assert!(
        (changed["resistor2VoltageVolts"].as_f64().unwrap() - 1000.0 * expected_changed).abs()
            < 1e-12
    );
}
