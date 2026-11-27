use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticCode, Engine, ParameterId,
    SetParameterRequest, SetParameterResult, Unit,
    engine::analyses::{AnalysisError, SpectrumRequest, harmonic_spectrum},
};

#[test]
fn capture_requires_a_current_contiguous_revision_and_survives_later_edits() {
    let mut engine = Engine::default();
    let request = CompileRequest {
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
    };
    let source = request.schematic.components[0].id().clone();
    let key = format!("{}:voltage", request.schematic.components[2].id());
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = engine.compile(request)
    else {
        panic!("divider must compile");
    };
    let offset = layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap()
        .offset as usize;
    let first_value = initial_snapshot.values[offset];
    let spectrum = SpectrumRequest {
        run_id: &run_id,
        layout_version: layout.layout_version,
        document_revision: "rev-1",
        signal_key: &key,
        end_step: 63,
        sample_count: 64,
    };
    assert!(matches!(
        engine.capture_spectrum(SpectrumRequest { ..spectrum }),
        Err(AnalysisError::UnavailableHistory)
    ));
    engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 63,
        })
        .unwrap();
    let captured = engine
        .capture_spectrum(SpectrumRequest { ..spectrum })
        .unwrap();
    let initial = harmonic_spectrum(&captured, || false).unwrap();
    assert_eq!((initial.first_step, initial.end_step), (0, 63));
    assert_eq!(initial.unit, Unit::Volt);
    assert_eq!(initial.bins.len(), 33);
    assert_eq!(initial.bins[32].frequency_hz, 500.0);
    assert!((initial.bins[0].peak_amplitude - first_value).abs() < 1e-12);
    assert_eq!(
        harmonic_spectrum(&captured, || true),
        Err(AnalysisError::Cancelled)
    );

    let update = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "rev-1".into(),
        document_revision: "rev-2".into(),
        component_id: source,
        parameter_id: ParameterId::VoltageVolts,
        value: 10.0,
    });
    assert!(matches!(update, SetParameterResult::Success { .. }));
    assert_eq!(harmonic_spectrum(&captured, || false).unwrap(), initial);
    assert!(matches!(
        engine.capture_spectrum(SpectrumRequest { ..spectrum }),
        Err(AnalysisError::StaleInput(
            DiagnosticCode::StaleDocumentRevision
        ))
    ));
    assert!(matches!(
        engine.capture_spectrum(SpectrumRequest {
            document_revision: "rev-2",
            ..spectrum
        }),
        Err(AnalysisError::UnavailableHistory)
    ));
    let current = SpectrumRequest {
        document_revision: "rev-2",
        ..spectrum
    };
    assert!(matches!(
        engine.capture_spectrum(SpectrumRequest {
            layout_version: 0,
            ..current
        }),
        Err(AnalysisError::StaleInput(DiagnosticCode::StaleRun))
    ));
    assert!(matches!(
        engine.capture_spectrum(SpectrumRequest {
            signal_key: "absent",
            ..current
        }),
        Err(AnalysisError::UnsupportedSignal)
    ));
    for sample_count in [0, 32, 1000, 8192] {
        assert!(matches!(
            engine.capture_spectrum(SpectrumRequest {
                sample_count,
                ..current
            }),
            Err(AnalysisError::InvalidRequest(_))
        ));
    }
    engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 4095,
        })
        .unwrap();
    let maximum = engine
        .capture_spectrum(SpectrumRequest {
            end_step: 4158,
            sample_count: 4096,
            ..current
        })
        .unwrap();
    let result = harmonic_spectrum(&maximum, || false).unwrap();
    assert_eq!((result.first_step, result.end_step), (63, 4158));
    assert_eq!(result.bins.len(), 2049);
    assert_eq!(result.bin_spacing_hz, 1000.0 / 4096.0);
    assert!((result.bins[0].peak_amplitude - 2.0 * first_value).abs() < 1e-10);
}
