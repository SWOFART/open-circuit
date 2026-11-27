use std::f64::consts::TAU;

use open_circuit_engine::{
    CompileRequest, DiagnosticCode, PinId, StableId,
    engine::analyses::{
        AnalysisError, FrequencyResponseRequest, FrequencySpacing, frequency_response,
    },
    netlist::PinRef,
};
use serde_json::{Value, json};

fn id(index: usize) -> StableId {
    StableId::parse(format!("00000000-0000-4000-8000-{index:012x}")).unwrap()
}

fn series(parts: &[(&str, Value)]) -> CompileRequest {
    let components: Vec<_> = parts
        .iter()
        .enumerate()
        .map(|(index, (kind, parameters))| {
            json!({
                "id":id(index + 1), "typeId":kind, "position":{"x":index * 4,"y":4},
                "rotationDeg":0, "parameters":parameters
            })
        })
        .collect();
    let geometry: Vec<_> = parts
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let (left, right) = if index == 0 {
                (1, 0)
            } else {
                (index, (index + 1) % parts.len())
            };
            json!({"componentId":id(index + 1),"pins":[
                {"pinId":"pin1","position":{"x":left * 4,"y":0}},
                {"pinId":"pin2","position":{"x":right * 4,"y":0}}
            ]})
        })
        .collect();
    serde_json::from_value(json!({
        "documentRevision":"ac-1",
        "schematic":{"schemaVersion":3, "components":components, "wires":[], "junctions":[],
            "referenceMarkers":[{"id":id(100),"position":{"x":0,"y":0}}], "rails":[],
            "documentation":{"designators":[],"wireNumbers":[],"terminals":[],"notes":[],
                "titleBlock":{"title":"","drawingNumber":"","revision":"","author":"","company":"","date":""},
                "sheets":[]}
        },
        "geometry":{"components":geometry}
    })).unwrap()
}

fn input(resistance: f64) -> (&'static str, Value) {
    (
        "dc_voltage_source",
        json!({"voltageVolts":7,"internalResistanceOhms":resistance}),
    )
}

fn output(index: usize) -> [PinRef; 2] {
    [PinId::Pin1, PinId::Pin2].map(|pin_id| PinRef {
        component_id: id(index),
        pin_id,
    })
}

fn analysis<'a>(
    circuit: &'a CompileRequest,
    output: &'a [PinRef; 2],
) -> FrequencyResponseRequest<'a> {
    FrequencyResponseRequest {
        schematic: &circuit.schematic,
        geometry: &circuit.geometry,
        document_revision: &circuit.document_revision,
        source_id: circuit.schematic.components[0].id(),
        output: [&output[0], &output[1]],
        start_hz: 100.0,
        stop_hz: 10_000.0,
        point_count: 3,
        spacing: FrequencySpacing::Logarithmic,
    }
}

#[test]
fn rc_rl_and_rlc_match_analytical_resistor_transfer_functions() {
    let resistance = 100.0;
    let source_resistance = 25.0;
    let inductance = 0.01;
    let capacitance = 1.0 / ((TAU * 1000.0).powi(2) * inductance);
    for (has_c, has_l) in [(true, false), (false, true), (true, true)] {
        let mut parts = vec![
            input(source_resistance),
            ("resistor", json!({"resistanceOhms":resistance})),
        ];
        if has_c {
            parts.push((
                "capacitor",
                json!({"capacitanceFarads":capacitance,"initialVoltageVolts":42}),
            ));
        }
        if has_l {
            parts.push((
                "inductor",
                json!({"inductanceHenries":inductance,"initialCurrentAmps":3}),
            ));
        }
        let circuit = series(&parts);
        let output = output(2);
        let result = frequency_response(analysis(&circuit, &output), || false).unwrap();
        assert_eq!(result.document_revision, "ac-1");
        assert_eq!(result.output, output);
        assert_eq!(result.points.len(), 3);
        for (point, hz) in result.points.iter().zip([100.0, 1000.0, 10000.0]) {
            assert_eq!(point.frequency_hz, hz);
            let omega = TAU * hz;
            let resistance_sum = resistance + source_resistance;
            let inductive = if has_l { omega * inductance } else { 0.0 };
            let capacitive = if has_c {
                -1.0 / (omega * capacitance)
            } else {
                0.0
            };
            let reactance = inductive + capacitive;
            let magnitude = resistance / resistance_sum.hypot(reactance);
            let phase = -reactance.atan2(resistance_sum).to_degrees();
            assert!((point.magnitude - magnitude).abs() <= 1e-8 + 1e-5 * magnitude);
            assert!((point.phase_degrees.unwrap() - phase).abs() <= 0.01);
        }
    }
}

#[test]
fn linear_grid_disables_other_sources_and_places_zero_phase_at_null() {
    let circuit = series(&[
        input(10.0),
        input(10.0),
        ("resistor", json!({"resistanceOhms":80.0})),
    ]);
    let observed = output(3);
    let request = FrequencyResponseRequest {
        spacing: FrequencySpacing::Linear,
        start_hz: 0.01,
        stop_hz: 100_000.0,
        ..analysis(&circuit, &observed)
    };
    let result = frequency_response(FrequencyResponseRequest { ..request }, || false).unwrap();
    assert_eq!(result.points[0].frequency_hz, 0.01);
    assert_eq!(result.points[2].frequency_hz, 100_000.0);
    assert!(
        result
            .points
            .iter()
            .all(|point| (point.magnitude - 0.8).abs() < 1e-12)
    );
    let result = frequency_response(
        FrequencyResponseRequest {
            output: [&observed[0], &observed[0]],
            ..request
        },
        || false,
    )
    .unwrap();
    assert!(
        result
            .points
            .iter()
            .all(|point| point.magnitude == 0.0 && point.phase_degrees.is_none())
    );
}

#[test]
fn unsupported_circuits_singular_points_and_invalid_bounds_are_structured() {
    let circuit = series(&[input(1.0), ("ammeter", json!({"burdenResistanceOhms":0.1}))]);
    let output = output(1);
    assert_eq!(
        frequency_response(analysis(&circuit, &output), || false),
        Err(AnalysisError::UnsupportedCircuit(vec![id(2)]))
    );
    let circuit = series(&[input(0.0), input(0.0)]);
    assert!(matches!(
        frequency_response(analysis(&circuit, &output), || false),
        Err(AnalysisError::SolveFailed { point_index: 0, diagnostics })
            if diagnostics[0].code == DiagnosticCode::SingularSystem
    ));
    for (start_hz, stop_hz, point_count) in [
        (0.009, 100.0, 2),
        (1.0, 100_001.0, 2),
        (1.0, 1.0, 2),
        (1.0, 100.0, 1),
        (1.0, 100.0, 258),
    ] {
        assert!(matches!(
            frequency_response(
                FrequencyResponseRequest {
                    start_hz,
                    stop_hz,
                    point_count,
                    ..analysis(&circuit, &output)
                },
                || false
            ),
            Err(AnalysisError::InvalidRequest(_))
        ));
    }
}
