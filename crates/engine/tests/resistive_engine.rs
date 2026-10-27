use open_circuit_engine::netlist::derive_netlist;
use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentSpec, DiagnosticCode,
    DiagnosticReference, GeometryEnvelope, ParameterId, PinId, ResultLayout, ResultQuantity,
    ResultSnapshot, Schematic, SetParameterRequest, SetParameterResult, StableId, Unit,
};
use serde_json::{Value, from_str};

const SCHEMATIC_JSON: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY_JSON: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");
const EXPECTED_JSON: &str = include_str!("../../../tests/golden/stage-1-divider.expected.json");

fn divider_request() -> CompileRequest {
    CompileRequest {
        schematic: from_str(SCHEMATIC_JSON).unwrap(),
        geometry: from_str(GEOMETRY_JSON).unwrap(),
        document_revision: "divider-1".to_owned(),
        faults: None,
    }
}

fn component_value(
    layout: &ResultLayout,
    snapshot: &ResultSnapshot,
    component_id: &str,
    quantity: ResultQuantity,
) -> f64 {
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.component_id.as_str() == component_id && slot.quantity == quantity)
        .expect("result layout contains the requested component value");
    snapshot.values[slot.offset as usize]
}

fn assert_kcl_and_power_balance(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    layout: &ResultLayout,
    snapshot: &ResultSnapshot,
) {
    let netlist = derive_netlist(schematic, geometry).unwrap();
    for node in &netlist.nodes {
        let current_sum = node
            .pins
            .iter()
            .map(|pin| {
                let current = component_value(
                    layout,
                    snapshot,
                    pin.component_id.as_str(),
                    ResultQuantity::Current,
                );
                if pin.pin_id == PinId::Pin1 {
                    current
                } else {
                    -current
                }
            })
            .sum::<f64>();
        assert!(
            current_sum.abs() < 1e-10,
            "KCL residual at node {}: {current_sum}",
            node.index
        );
    }

    let total_power = schematic
        .components
        .iter()
        .map(|component| {
            let id = component.id().as_str();
            component_value(layout, snapshot, id, ResultQuantity::Voltage)
                * component_value(layout, snapshot, id, ResultQuantity::Current)
        })
        .sum::<f64>();
    assert!(
        total_power.abs() < 1e-9,
        "power balance residual: {total_power}"
    );
}

#[test]
fn divider_matches_golden_values_signs_kcl_and_power_balance() {
    let request = divider_request();
    let schematic = request.schematic.clone();
    let geometry = request.geometry.clone();
    let expected: Value = from_str(EXPECTED_JSON).unwrap();
    let mut engine = open_circuit_engine::Engine::default();
    let CompileResult::Success {
        layout,
        initial_snapshot,
        diagnostics,
        ..
    } = engine.compile(request)
    else {
        panic!("the golden divider should compile");
    };

    assert!(diagnostics.is_empty());
    assert_eq!(initial_snapshot.metadata.simulation_step, 0);
    assert_eq!(initial_snapshot.metadata.simulation_time_seconds, 0.0);
    assert_eq!(layout.value_count as usize, layout.slots.len());
    assert!(
        layout
            .slots
            .windows(2)
            .all(|pair| pair[0].offset + 1 == pair[1].offset)
    );
    assert!(layout.slots.iter().all(|slot| match slot.quantity {
        ResultQuantity::Voltage => slot.unit == Unit::Volt,
        ResultQuantity::Current => slot.unit == Unit::Amp,
        ResultQuantity::AccumulatedI2t => slot.unit == Unit::AmpereSquaredSecond,
        ResultQuantity::AngularVelocity => slot.unit == Unit::RadianPerSecond,
    }));

    let source_current = component_value(
        &layout,
        &initial_snapshot,
        "00000000-0000-4000-8000-000000000001",
        ResultQuantity::Current,
    );
    let resistor1_voltage = component_value(
        &layout,
        &initial_snapshot,
        "00000000-0000-4000-8000-000000000002",
        ResultQuantity::Voltage,
    );
    let resistor2_voltage = component_value(
        &layout,
        &initial_snapshot,
        "00000000-0000-4000-8000-000000000003",
        ResultQuantity::Voltage,
    );
    let resistor1_current = component_value(
        &layout,
        &initial_snapshot,
        "00000000-0000-4000-8000-000000000002",
        ResultQuantity::Current,
    );
    let expected_equal = &expected["equalResistors"];
    assert!((source_current + expected_equal["sourceCurrentAmps"].as_f64().unwrap()).abs() <= 1e-9);
    assert!(
        (resistor1_voltage - expected_equal["resistor1VoltageVolts"].as_f64().unwrap()).abs()
            <= 1e-6
    );
    assert!(
        (resistor2_voltage - expected_equal["resistor2VoltageVolts"].as_f64().unwrap()).abs()
            <= 1e-6
    );
    assert!((resistor1_current - resistor1_voltage / 1000.0).abs() < 1e-12);
    assert_kcl_and_power_balance(&schematic, &geometry, &layout, &initial_snapshot);
}

#[test]
fn bridge_network_matches_analytical_node_voltages_and_conservation() {
    let (schematic, geometry) = bridge_fixture();
    let mut engine = open_circuit_engine::Engine::default();
    let CompileResult::Success {
        layout,
        initial_snapshot,
        ..
    } = engine.compile(CompileRequest {
        schematic: schematic.clone(),
        geometry: geometry.clone(),
        document_revision: "bridge-1".to_owned(),
        faults: None,
    })
    else {
        panic!("the resistive bridge should compile");
    };

    let left_node_voltage = component_value(
        &layout,
        &initial_snapshot,
        "00000000-0000-4000-8000-000000000003",
        ResultQuantity::Voltage,
    );
    let right_node_voltage = component_value(
        &layout,
        &initial_snapshot,
        "00000000-0000-4000-8000-000000000005",
        ResultQuantity::Voltage,
    );
    assert!((left_node_voltage - 204.0 / 31.0).abs() < 1e-9);
    assert!((right_node_voltage - 184.0 / 31.0).abs() < 1e-9);
    assert_kcl_and_power_balance(&schematic, &geometry, &layout, &initial_snapshot);
}

#[test]
fn parameter_update_recalculates_the_operating_point_and_advance_uses_fixed_steps() {
    let request = divider_request();
    let mut engine = open_circuit_engine::Engine::default();
    let CompileResult::Success { run_id, layout, .. } = engine.compile(request) else {
        panic!("the golden divider should compile");
    };
    let component_id = StableId::parse("00000000-0000-4000-8000-000000000002").unwrap();
    let SetParameterResult::Success { snapshot, .. } = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "divider-1".to_owned(),
        document_revision: "divider-2".to_owned(),
        component_id: component_id.clone(),
        parameter_id: ParameterId::ResistanceOhms,
        value: 3000.0,
    }) else {
        panic!("valid resistance update should succeed");
    };
    let expected: Value = from_str(EXPECTED_JSON).unwrap();
    let changed = &expected["resistor1At3000Ohms"];
    let source_current = component_value(
        &layout,
        &snapshot,
        "00000000-0000-4000-8000-000000000001",
        ResultQuantity::Current,
    );
    let resistor2_voltage = component_value(
        &layout,
        &snapshot,
        "00000000-0000-4000-8000-000000000003",
        ResultQuantity::Voltage,
    );
    assert!((source_current + changed["sourceCurrentAmps"].as_f64().unwrap()).abs() <= 1e-9);
    assert!((resistor2_voltage - changed["resistor2VoltageVolts"].as_f64().unwrap()).abs() <= 1e-6);
    assert_eq!(snapshot.metadata.run_id, run_id);
    assert_eq!(snapshot.metadata.document_revision, "divider-2");
    assert_eq!(snapshot.metadata.layout_version, 1);
    assert_eq!(snapshot.metadata.frame_sequence, 1);

    let valid_snapshot = snapshot.clone();
    let invalid = engine.set_parameter(SetParameterRequest {
        run_id: snapshot.metadata.run_id.clone(),
        expected_document_revision: "divider-2".to_owned(),
        document_revision: "divider-3".to_owned(),
        component_id,
        parameter_id: ParameterId::ResistanceOhms,
        value: 0.0,
    });
    assert!(matches!(invalid, SetParameterResult::Failure { .. }));
    assert_eq!(engine.latest_snapshot(), Some(&valid_snapshot));

    let advanced = engine
        .advance(AdvanceRequest {
            run_id: snapshot.metadata.run_id,
            step_count: 3,
        })
        .unwrap();
    assert_eq!(advanced.metadata.simulation_step, 3);
    assert_eq!(advanced.metadata.simulation_time_seconds, 0.003);
    assert_eq!(advanced.metadata.frame_sequence, 2);
    assert_eq!(advanced.values, valid_snapshot.values);
}

#[test]
fn automatic_reference_warning_does_not_prevent_a_valid_operating_point() {
    let mut request = divider_request();
    request.schematic.reference_markers.clear();
    let mut engine = open_circuit_engine::Engine::default();
    let CompileResult::Success {
        diagnostics,
        initial_snapshot,
        ..
    } = engine.compile(request)
    else {
        panic!("a local numerical reference should solve the unmarked island");
    };

    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::NoGroundReference)
    );
    assert!(
        initial_snapshot
            .values
            .iter()
            .all(|value| value.is_finite())
    );
}

#[test]
fn ideal_source_conflicts_singular_loops_and_non_finite_values_fail_with_diagnostics() {
    let (mut schematic, mut geometry) = bridge_fixture();
    let second_source_id = "00000000-0000-4000-8000-000000000008";
    schematic.components.push(ComponentSpec::DcVoltageSource {
        id: StableId::parse(second_source_id).unwrap(),
        position: open_circuit_engine::GridPoint { x: 0, y: 0 },
        rotation_deg: 0,
        parameters: open_circuit_engine::DcVoltageSourceParameters {
            voltage_volts: 10.0,
            internal_resistance_ohms: 0.0,
        },
    });
    geometry
        .components
        .push(open_circuit_engine::ComponentGeometry {
            component_id: StableId::parse(second_source_id).unwrap(),
            pins: vec![
                open_circuit_engine::ResolvedPinPosition {
                    pin_id: "pin1".to_owned(),
                    position: open_circuit_engine::GridPoint { x: 0, y: 10 },
                },
                open_circuit_engine::ResolvedPinPosition {
                    pin_id: "pin2".to_owned(),
                    position: open_circuit_engine::GridPoint { x: 0, y: 0 },
                },
            ],
        });

    let mut engine = open_circuit_engine::Engine::default();
    let consistent_ideal_loop = engine.compile(CompileRequest {
        schematic: schematic.clone(),
        geometry: geometry.clone(),
        document_revision: "ideal-loop".to_owned(),
        faults: None,
    });
    let CompileResult::Failure { diagnostics } = consistent_ideal_loop else {
        panic!("parallel ideal-source currents are underdetermined");
    };
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::SingularSystem)
    );

    if let Some(ComponentSpec::DcVoltageSource { parameters, .. }) = schematic
        .components
        .iter_mut()
        .find(|component| component.id().as_str() == second_source_id)
    {
        parameters.voltage_volts = 11.0;
    }
    let inconsistent_loop = engine.compile(CompileRequest {
        schematic: schematic.clone(),
        geometry: geometry.clone(),
        document_revision: "inconsistent-loop".to_owned(),
        faults: None,
    });
    let CompileResult::Failure { diagnostics } = inconsistent_loop else {
        panic!("conflicting ideal sources must fail");
    };
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::InconsistentIdealConstraint)
    );
    assert!(diagnostics.iter().any(|item| item.references.iter().any(|reference| {
        matches!(reference, DiagnosticReference::Component { component_id } if component_id.as_str() == second_source_id)
    })));

    let (mut short_schematic, mut short_geometry) = bridge_fixture();
    short_geometry.components[0].pins[0].position = open_circuit_engine::GridPoint { x: 0, y: 0 };
    let shorted_source = engine.compile(CompileRequest {
        schematic: short_schematic.clone(),
        geometry: short_geometry,
        document_revision: "shorted-source".to_owned(),
        faults: None,
    });
    let CompileResult::Failure { diagnostics } = shorted_source else {
        panic!("an ideal source short must fail");
    };
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::ShortCircuit)
    );

    if let ComponentSpec::Resistor { parameters, .. } = &mut short_schematic.components[1] {
        parameters.resistance_ohms = 0.0;
    }
    let invalid_resistance = engine.compile(CompileRequest {
        schematic: short_schematic,
        geometry: bridge_fixture().1,
        document_revision: "invalid-resistance".to_owned(),
        faults: None,
    });
    let CompileResult::Failure { diagnostics } = invalid_resistance else {
        panic!("zero resistance must not be accepted");
    };
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::InvalidParameter)
    );

    let mut non_finite = divider_request();
    if let ComponentSpec::DcVoltageSource { parameters, .. } =
        &mut non_finite.schematic.components[0]
    {
        parameters.voltage_volts = f64::NAN;
    }
    let invalid = engine.compile(non_finite);
    let CompileResult::Failure { diagnostics } = invalid else {
        panic!("non-finite input must never produce a result");
    };
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::InvalidParameter)
    );
}

fn bridge_fixture() -> (Schematic, GeometryEnvelope) {
    let schematic: Schematic = from_str(
        r#"{
          "schemaVersion":1,
          "components":[
            {"id":"00000000-0000-4000-8000-000000000001","typeId":"dc_voltage_source","position":{"x":0,"y":0},"rotationDeg":0,"parameters":{"voltageVolts":10,"internalResistanceOhms":0}},
            {"id":"00000000-0000-4000-8000-000000000002","typeId":"resistor","position":{"x":1,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":1000}},
            {"id":"00000000-0000-4000-8000-000000000003","typeId":"resistor","position":{"x":2,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":2000}},
            {"id":"00000000-0000-4000-8000-000000000004","typeId":"resistor","position":{"x":3,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":3000}},
            {"id":"00000000-0000-4000-8000-000000000005","typeId":"resistor","position":{"x":4,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":4000}},
            {"id":"00000000-0000-4000-8000-000000000006","typeId":"resistor","position":{"x":5,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":5000}}
          ],"wires":[],"junctions":[],
          "referenceMarkers":[{"id":"00000000-0000-4000-8000-000000000007","position":{"x":0,"y":0}}]
        }"#,
    )
    .unwrap();
    let geometry: GeometryEnvelope = from_str(
        r#"{
          "components":[
            {"componentId":"00000000-0000-4000-8000-000000000001","pins":[{"pinId":"pin1","position":{"x":0,"y":10}},{"pinId":"pin2","position":{"x":0,"y":0}}]},
            {"componentId":"00000000-0000-4000-8000-000000000002","pins":[{"pinId":"pin1","position":{"x":0,"y":10}},{"pinId":"pin2","position":{"x":4,"y":6}}]},
            {"componentId":"00000000-0000-4000-8000-000000000003","pins":[{"pinId":"pin1","position":{"x":4,"y":6}},{"pinId":"pin2","position":{"x":0,"y":0}}]},
            {"componentId":"00000000-0000-4000-8000-000000000004","pins":[{"pinId":"pin1","position":{"x":0,"y":10}},{"pinId":"pin2","position":{"x":8,"y":6}}]},
            {"componentId":"00000000-0000-4000-8000-000000000005","pins":[{"pinId":"pin1","position":{"x":8,"y":6}},{"pinId":"pin2","position":{"x":0,"y":0}}]},
            {"componentId":"00000000-0000-4000-8000-000000000006","pins":[{"pinId":"pin1","position":{"x":4,"y":6}},{"pinId":"pin2","position":{"x":8,"y":6}}]}
          ]
        }"#,
    )
    .unwrap();
    (schematic, geometry)
}
