//! The transport shapes of the extra analyses and the function that runs a request
//! (docs/contracts/stage-17-extra-analyses.md section 5), with the real Engine.

use std::cell::Cell;

use open_circuit_engine::engine::analyses::{
    AnalysisError, AnalysisFailureReason, AnalysisKind, AnalysisOutcome, AnalysisRequest,
    AnalysisResult, DcSweepPointDto, FrequencyResponsePointDto, SpectrumBinDto, failure_from,
    run_analysis,
};
use open_circuit_engine::{Diagnostic, DiagnosticCode, PinId, StableId, Unit};
use serde_json::{Value, json};

const SCHEMATIC: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");
const SOURCE: &str = "00000000-0000-4000-8000-000000000001";

fn sweep(points: u32) -> AnalysisRequest {
    serde_json::from_value(json!({
        "kind": "dcSweep",
        "schematic": serde_json::from_str::<Value>(SCHEMATIC).unwrap(),
        "geometry": serde_json::from_str::<Value>(GEOMETRY).unwrap(),
        "documentRevision": "rev-1",
        "sourceId": SOURCE,
        "signalKey": "00000000-0000-4000-8000-000000000003:voltage",
        "startVolts": 0.0,
        "stopVolts": 5.0,
        "pointCount": points,
    }))
    .unwrap()
}

fn no_capture(
    _: open_circuit_engine::engine::analyses::SpectrumRequest<'_>,
) -> Result<open_circuit_engine::engine::analyses::SpectrumCapture, AnalysisError> {
    panic!("a sweep reads no history");
}

#[test]
fn a_request_names_its_kind_and_the_drawing_it_was_made_from() {
    let request = sweep(3);

    assert_eq!(request.kind(), AnalysisKind::DcSweep);
    assert_eq!(request.document_revision(), "rev-1");
    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(value["kind"], "dcSweep");
    assert_eq!(value["documentRevision"], "rev-1");
    // It decodes back to the same request.
    assert_eq!(
        serde_json::from_value::<AnalysisRequest>(value).unwrap(),
        request
    );
}

#[test]
fn a_spectrum_step_crosses_as_a_decimal_string_that_can_exceed_the_safe_integer_range() {
    let request: AnalysisRequest = serde_json::from_value(json!({
        "kind": "spectrum", "runId": "00000000-0000-4000-8000-0000000000aa",
        "layoutVersion": 1, "documentRevision": "rev", "signalKey": "k",
        "endStep": "18446744073709551615", "sampleCount": 64,
    }))
    .unwrap();
    let seen = Cell::new(0_u64);

    // The largest step a run could name is read exactly, not through a floating-point number.
    let result = run_analysis(
        &request,
        |capture| {
            seen.set(capture.end_step);
            Err(AnalysisError::UnavailableHistory)
        },
        || false,
    );

    assert_eq!(seen.get(), u64::MAX);
    assert_eq!(result, Err(AnalysisError::UnavailableHistory));
}

#[test]
fn a_step_that_is_not_plain_digits_or_does_not_fit_is_an_invalid_request() {
    for bad in [
        "",
        "-1",
        "+1",
        " 1",
        "1 ",
        "1e3",
        "0x10",
        "18446744073709551616",
    ] {
        let request: AnalysisRequest = serde_json::from_value(json!({
            "kind": "spectrum", "runId": "00000000-0000-4000-8000-0000000000aa",
            "layoutVersion": 1, "documentRevision": "rev", "signalKey": "k",
            "endStep": bad, "sampleCount": 64,
        }))
        .unwrap();

        let result = run_analysis(&request, |_| panic!("no capture"), || false);

        assert!(
            matches!(result, Err(AnalysisError::InvalidRequest(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn a_cancel_before_anything_runs_returns_cancelled_without_solving() {
    assert_eq!(
        run_analysis(&sweep(257), no_capture, || true),
        Err(AnalysisError::Cancelled)
    );
}

#[test]
fn a_cancel_accepted_before_publication_discards_a_finished_result() {
    // Count how often the analysis asks, then cancel at the very last question: the points are
    // all computed, and the result is still not returned.
    let asked = Cell::new(0_u32);
    let finished = run_analysis(&sweep(5), no_capture, || {
        asked.set(asked.get() + 1);
        false
    });
    assert!(finished.is_ok());

    let calls = Cell::new(0_u32);
    let cancelled = run_analysis(&sweep(5), no_capture, || {
        calls.set(calls.get() + 1);
        calls.get() == asked.get()
    });

    assert_eq!(cancelled, Err(AnalysisError::Cancelled));
}

#[test]
fn every_failure_has_a_machine_readable_reason_and_a_cancel_is_not_one() {
    let diagnostic = Diagnostic::error(DiagnosticCode::InvalidParameter, Vec::new());
    let component = StableId::parse(SOURCE).unwrap();

    let cases = [
        (
            AnalysisError::InvalidRequest(vec![diagnostic.clone()]),
            AnalysisFailureReason::InvalidRequest,
        ),
        (
            AnalysisError::UnsupportedCircuit(vec![component.clone()]),
            AnalysisFailureReason::UnsupportedCircuit,
        ),
        (
            AnalysisError::UnsupportedSignal,
            AnalysisFailureReason::UnsupportedSignal,
        ),
        (
            AnalysisError::StaleInput(DiagnosticCode::StaleRun),
            AnalysisFailureReason::StaleInput,
        ),
        (
            AnalysisError::UnavailableHistory,
            AnalysisFailureReason::UnavailableHistory,
        ),
        (
            AnalysisError::SolveFailed {
                point_index: 4,
                diagnostics: vec![diagnostic],
            },
            AnalysisFailureReason::SolveFailed,
        ),
    ];
    for (error, reason) in cases {
        assert_eq!(failure_from(error).unwrap().reason, reason);
    }
    assert!(failure_from(AnalysisError::Cancelled).is_none());

    let unsupported = failure_from(AnalysisError::UnsupportedCircuit(vec![component.clone()]));
    assert_eq!(unsupported.unwrap().component_ids, [component]);
    let solve = failure_from(AnalysisError::SolveFailed {
        point_index: 4,
        diagnostics: Vec::new(),
    })
    .unwrap();
    assert_eq!(solve.point_index, Some(4));
}

#[test]
fn a_failed_job_carries_no_points_and_a_success_carries_the_revision_and_selection() {
    let failed = serde_json::to_value(AnalysisOutcome::Failed {
        failure: failure_from(AnalysisError::UnavailableHistory).unwrap(),
    })
    .unwrap();
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["failure"]["reason"], "unavailableHistory");
    assert!(failed.get("result").is_none());

    let result = run_analysis(&sweep(3), no_capture, || false).unwrap();
    let AnalysisResult::DcSweep {
        document_revision,
        source_id,
        signal_key,
        unit,
        points,
    } = &result
    else {
        panic!("a sweep result");
    };
    assert_eq!(document_revision, "rev-1");
    assert_eq!(source_id.as_str(), SOURCE);
    assert_eq!(signal_key, "00000000-0000-4000-8000-000000000003:voltage");
    assert_eq!(*unit, Unit::Volt);
    assert_eq!(points.len(), 3);
    let value = serde_json::to_value(AnalysisOutcome::Succeeded { result }).unwrap();
    assert_eq!(value["status"], "succeeded");
    assert_eq!(value["result"]["kind"], "dcSweep");
    assert_eq!(value["result"]["unit"], "volt");
}

#[test]
fn a_result_with_a_number_that_is_not_finite_is_found_by_its_point() {
    let sweep = |reading: f64| AnalysisResult::DcSweep {
        document_revision: "r".into(),
        source_id: StableId::parse(SOURCE).unwrap(),
        signal_key: "k".into(),
        unit: Unit::Volt,
        points: vec![
            DcSweepPointDto {
                source_volts: 0.0,
                reading: 0.0,
            },
            DcSweepPointDto {
                source_volts: 1.0,
                reading,
            },
        ],
    };
    assert_eq!(sweep(2.0).first_non_finite_point(), None);
    assert_eq!(sweep(f64::NAN).first_non_finite_point(), Some(1));
    assert_eq!(sweep(f64::INFINITY).first_non_finite_point(), Some(1));

    let response = |magnitude: f64, phase: Option<f64>| AnalysisResult::FrequencyResponse {
        document_revision: "r".into(),
        source_id: StableId::parse(SOURCE).unwrap(),
        output: [
            open_circuit_engine::engine::analyses::AnalysisTerminal {
                component_id: StableId::parse(SOURCE).unwrap(),
                pin_id: PinId::Pin1,
            },
            open_circuit_engine::engine::analyses::AnalysisTerminal {
                component_id: StableId::parse(SOURCE).unwrap(),
                pin_id: PinId::Pin2,
            },
        ],
        points: vec![FrequencyResponsePointDto {
            frequency_hz: 1.0,
            magnitude,
            phase_degrees: phase,
        }],
    };
    // A missing phase (an exact zero magnitude) is allowed; a phase that is not finite is not.
    assert_eq!(response(0.0, None).first_non_finite_point(), None);
    assert_eq!(
        response(1.0, Some(f64::NAN)).first_non_finite_point(),
        Some(0)
    );
    assert_eq!(
        response(f64::NAN, Some(0.0)).first_non_finite_point(),
        Some(0)
    );

    let spectrum = |amplitude: f64| AnalysisResult::Spectrum {
        run_id: StableId::parse(SOURCE).unwrap(),
        layout_version: 1,
        document_revision: "r".into(),
        signal_key: "k".into(),
        first_step: "0".into(),
        end_step: "63".into(),
        bin_spacing_hz: 15.625,
        unit: Unit::Volt,
        bins: vec![SpectrumBinDto {
            frequency_hz: 0.0,
            peak_amplitude: amplitude,
        }],
    };
    assert_eq!(spectrum(1.0).first_non_finite_point(), None);
    assert_eq!(spectrum(f64::INFINITY).first_non_finite_point(), Some(0));
}
