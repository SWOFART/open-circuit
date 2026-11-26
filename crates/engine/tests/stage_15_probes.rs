use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ConductorSpan, ConductorSpanRequest,
    DiagnosticCode, Engine, FrameMetadata, GridPoint, ParameterId, PinId, ProbeRequest,
    ProbeResult, ProbeTarget, SetParameterRequest, SetParameterResult, StableId, Unit,
};

fn point(x: i32, y: i32) -> GridPoint {
    GridPoint { x, y }
}

fn id(number: u32) -> StableId {
    StableId::parse(format!("00000000-0000-4000-8000-{number:012}")).unwrap()
}

fn divider() -> CompileRequest {
    CompileRequest {
        schematic: serde_json::from_str(include_str!(
            "../../../tests/golden/stage-1-divider.schematic.json"
        ))
        .unwrap(),
        geometry: serde_json::from_str(include_str!(
            "../../../tests/golden/stage-1-divider.geometry.json"
        ))
        .unwrap(),
        document_revision: "rev-1".into(),
        faults: None,
    }
}

fn start(request: CompileRequest) -> (Engine, FrameMetadata) {
    let mut engine = Engine::default();
    let result = engine.compile(request);
    let CompileResult::Success {
        initial_snapshot, ..
    } = result
    else {
        panic!("fixture must compile: {result:?}");
    };
    (engine, initial_snapshot.metadata)
}

fn probe(engine: &Engine, frame: &FrameMetadata, target: ProbeTarget) -> ProbeResult {
    engine.probe(ProbeRequest {
        run_id: frame.run_id.clone(),
        document_revision: frame.document_revision.clone(),
        frame_sequence: frame.frame_sequence,
        target,
    })
}

fn voltage(positive: GridPoint, negative: GridPoint) -> ProbeTarget {
    ProbeTarget::Voltage { positive, negative }
}

fn assert_reading(result: ProbeResult, expected: f64, quantity: Unit) -> FrameMetadata {
    let ProbeResult::Reading {
        metadata,
        value,
        unit,
    } = result
    else {
        panic!("expected a measurement: {result:?}");
    };
    assert_eq!(unit, quantity);
    assert!(
        (value - expected).abs() <= 1e-8 + expected.abs() * 1e-3,
        "{value} vs {expected}"
    );
    metadata
}

fn assert_refusal(result: ProbeResult, expected: DiagnosticCode) {
    let ProbeResult::Unavailable { diagnostic, .. } = result else {
        panic!("expected a refusal: {result:?}");
    };
    assert_eq!(diagnostic.code, expected);
}

#[test]
fn divider_voltage_terminal_and_wire_currents_use_the_same_committed_endpoint() {
    let (mut engine, frame) = start(divider());
    let amps = 5.0 / 2000.1;
    assert_reading(
        probe(&engine, &frame, voltage(point(6, 0), point(9, 0))),
        1000.0 * amps,
        Unit::Volt,
    );
    assert_reading(
        probe(&engine, &frame, voltage(point(9, 0), point(6, 0))),
        -1000.0 * amps,
        Unit::Volt,
    );
    for (pin_id, expected) in [(PinId::Pin1, amps), (PinId::Pin2, -amps)] {
        assert_reading(
            probe(
                &engine,
                &frame,
                ProbeTarget::TerminalCurrent {
                    component_id: id(2),
                    pin_id,
                },
            ),
            expected,
            Unit::Amp,
        );
    }
    let spans = engine
        .conductor_spans(ConductorSpanRequest {
            run_id: frame.run_id.clone(),
            document_revision: frame.document_revision.clone(),
            cursor: None,
        })
        .unwrap();
    let span = spans
        .spans
        .into_iter()
        .find(|span| span.conductor_id == id(5))
        .unwrap();
    assert_eq!((span.start, span.end), (point(5, 0), point(7, 0)));
    assert_reading(
        probe(
            &engine,
            &frame,
            ProbeTarget::ConductorCurrent { span: span.clone() },
        ),
        amps,
        Unit::Amp,
    );
    assert_reading(
        probe(
            &engine,
            &frame,
            ProbeTarget::ConductorCurrent {
                span: ConductorSpan {
                    start: span.end,
                    end: span.start,
                    ..span
                },
            },
        ),
        -amps,
        Unit::Amp,
    );

    let advanced = engine
        .advance(AdvanceRequest {
            run_id: frame.run_id.clone(),
            step_count: 1,
        })
        .unwrap();
    let actual = assert_reading(
        probe(&engine, &frame, voltage(point(6, 0), point(9, 0))),
        1000.0 * amps,
        Unit::Volt,
    );
    assert_eq!(actual, advanced.metadata);
    let mut ahead = actual.clone();
    ahead.frame_sequence += 1;
    assert_refusal(
        probe(&engine, &ahead, voltage(point(6, 0), point(9, 0))),
        DiagnosticCode::InvalidParameter,
    );

    let SetParameterResult::Success { snapshot, .. } = engine.set_parameter(SetParameterRequest {
        run_id: frame.run_id.clone(),
        expected_document_revision: frame.document_revision.clone(),
        document_revision: "rev-2".into(),
        component_id: id(3),
        parameter_id: ParameterId::ResistanceOhms,
        value: 3000.0,
    }) else {
        panic!("resistor edit must succeed")
    };
    assert_refusal(
        probe(&engine, &frame, voltage(point(6, 0), point(9, 0))),
        DiagnosticCode::StaleDocumentRevision,
    );
    assert_reading(
        probe(
            &engine,
            &snapshot.metadata,
            voltage(point(6, 0), point(9, 0)),
        ),
        5.0 * 3000.0 / 4000.1,
        Unit::Volt,
    );
}

#[test]
fn a_crossing_is_ambiguous_and_a_parallel_route_has_no_unique_current() {
    let mut request = divider();
    request.schematic.wires.push(open_circuit_engine::WireSpec {
        id: id(10),
        points: vec![point(5, 0), point(5, 5), point(0, 5), point(0, 3)],
    });
    let (engine, frame) = start(request);
    assert_refusal(
        probe(&engine, &frame, voltage(point(0, 4), point(9, 0))),
        DiagnosticCode::AmbiguousProbe,
    );
    assert_refusal(
        probe(&engine, &frame, voltage(point(50, 50), point(9, 0))),
        DiagnosticCode::UnconnectedProbe,
    );

    let mut request = divider();
    request.schematic.wires.push(open_circuit_engine::WireSpec {
        id: id(10),
        points: vec![point(5, 0), point(5, 2), point(7, 2), point(7, 0)],
    });
    let (engine, frame) = start(request);
    assert_refusal(
        probe(
            &engine,
            &frame,
            ProbeTarget::ConductorCurrent {
                span: ConductorSpan {
                    conductor_id: id(5),
                    start: point(5, 0),
                    end: point(7, 0),
                },
            },
        ),
        DiagnosticCode::AmbiguousConductorCurrent,
    );
}

#[test]
fn equivalent_resistance_uses_the_passive_network_and_refuses_a_source() {
    let measure = |first, second| ProbeTarget::Resistance { first, second };
    let request = divider();
    let (engine, frame) = start(request.clone());
    assert_refusal(
        probe(&engine, &frame, measure(point(3, 0), point(9, 0))),
        DiagnosticCode::ResistanceUnavailable,
    );

    let mut passive = request;
    passive.schematic.components.remove(0);
    passive.geometry.components.remove(0);
    passive.schematic.wires.retain(|wire| wire.id == id(5));
    passive.schematic.reference_markers.clear();
    let (engine, frame) = start(passive.clone());
    assert_reading(
        probe(&engine, &frame, measure(point(3, 0), point(9, 0))),
        2000.0,
        Unit::Ohm,
    );
    assert_reading(
        probe(&engine, &frame, measure(point(5, 0), point(7, 0))),
        0.0,
        Unit::Ohm,
    );

    let mut parallel = passive.clone();
    parallel.schematic.wires[0].points = vec![point(3, 0), point(3, 2), point(7, 2), point(7, 0)];
    parallel
        .schematic
        .wires
        .push(open_circuit_engine::WireSpec {
            id: id(6),
            points: vec![point(5, 0), point(5, -2), point(9, -2), point(9, 0)],
        });
    let (engine, frame) = start(parallel);
    assert_reading(
        probe(&engine, &frame, measure(point(3, 0), point(9, 0))),
        500.0,
        Unit::Ohm,
    );
    passive.schematic.wires.clear();
    let (engine, frame) = start(passive);
    assert!(matches!(
        probe(&engine, &frame, measure(point(3, 0), point(9, 0))),
        ProbeResult::OpenCircuit { .. }
    ));
    assert_refusal(
        probe(&engine, &frame, voltage(point(3, 0), point(9, 0))),
        DiagnosticCode::IncomparableProbeReferences,
    );
}

#[test]
fn a_rail_span_reports_current_and_paging_is_run_and_revision_scoped() {
    let mut request = divider();
    request.schematic.wires.retain(|wire| wire.id != id(5));
    request.schematic.rails.push(open_circuit_engine::RailSpec {
        id: id(5),
        name: "MID".into(),
        start: point(6, 0),
        end: point(6, 2),
    });
    request.schematic.wires.extend([
        open_circuit_engine::WireSpec {
            id: id(10),
            points: vec![point(5, 0), point(6, 0)],
        },
        open_circuit_engine::WireSpec {
            id: id(11),
            points: vec![point(6, 2), point(8, 2), point(8, 0), point(7, 0)],
        },
    ]);
    let (engine, frame) = start(request);
    let page = engine
        .conductor_spans(ConductorSpanRequest {
            run_id: frame.run_id.clone(),
            document_revision: frame.document_revision.clone(),
            cursor: None,
        })
        .unwrap();
    let span = page
        .spans
        .into_iter()
        .find(|span| span.conductor_id == id(5))
        .unwrap();
    assert_reading(
        probe(&engine, &frame, ProbeTarget::ConductorCurrent { span }),
        5.0 / 2000.1,
        Unit::Amp,
    );

    let mut request = divider();
    let wire = request
        .schematic
        .wires
        .iter_mut()
        .find(|wire| wire.id == id(5))
        .unwrap();
    let mut route = vec![point(5, 0)];
    let mut x = 5;
    for y in 1..=135 {
        route.push(point(x, -y));
        x = if x == 5 { 6 } else { 5 };
        route.push(point(x, -y));
    }
    route.push(point(7, -135));
    route.push(point(7, 0));
    wire.points = route;
    let (engine, frame) = start(request);
    let mut request = ConductorSpanRequest {
        run_id: frame.run_id,
        document_revision: frame.document_revision,
        cursor: None,
    };
    let first = engine.conductor_spans(request.clone()).unwrap();
    assert_eq!(first.spans.len(), 256);
    request.cursor = first.next_cursor;
    let second = engine.conductor_spans(request.clone()).unwrap();
    assert!(!second.spans.is_empty());
    assert!(second.next_cursor.is_none());
    assert!(first.spans.iter().all(|span| !second.spans.contains(span)));
    request.document_revision = "stale".into();
    assert_eq!(
        engine.conductor_spans(request.clone()).unwrap_err().code,
        DiagnosticCode::StaleDocumentRevision
    );
    request.run_id = id(999);
    assert_eq!(
        engine.conductor_spans(request).unwrap_err().code,
        DiagnosticCode::StaleRun
    );
}

#[test]
fn a_self_crossing_wire_is_split_and_cannot_report_a_unique_loop_current() {
    let mut request = divider();
    request
        .schematic
        .wires
        .iter_mut()
        .find(|wire| wire.id == id(5))
        .unwrap()
        .points = vec![
        point(5, 0),
        point(5, 3),
        point(8, 3),
        point(8, 1),
        point(4, 1),
        point(4, -1),
        point(7, -1),
        point(7, 0),
    ];
    let (engine, frame) = start(request);
    let scope = ConductorSpanRequest {
        run_id: frame.run_id.clone(),
        document_revision: frame.document_revision.clone(),
        cursor: None,
    };
    let found = engine.conductor_spans(scope).unwrap().spans;
    let span = found
        .into_iter()
        .find(|span| {
            span.conductor_id == id(5) && span.start == point(5, 1) && span.end == point(5, 3)
        })
        .expect("span at the self-crossing");
    assert_refusal(
        probe(&engine, &frame, ProbeTarget::ConductorCurrent { span }),
        DiagnosticCode::AmbiguousConductorCurrent,
    );
    assert_reading(
        probe(
            &engine,
            &frame,
            ProbeTarget::ConductorCurrent {
                span: ConductorSpan {
                    conductor_id: id(5),
                    start: point(5, 0),
                    end: point(5, 1),
                },
            },
        ),
        5.0 / 2000.1,
        Unit::Amp,
    );
}
