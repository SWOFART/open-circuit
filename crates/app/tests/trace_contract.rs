//! Trace envelope checks. The golden fixture was produced by an independent encoder
//! written from the specification (docs/contracts/stage-1.md section 8.3); the
//! TypeScript decoder test (`src/store/traceCodec.test.ts`) reads the same file.

use open_circuit_app_lib::trace::encode_trace;
use open_circuit_engine::{
    StableId, TraceBatch, TraceBatchMetadata, TracePoint, TraceSignal, Unit,
};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../tests/golden/stage-3-trace.json");

fn from_hex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("valid hex"))
        .collect()
}

fn text(value: &Value, field: &str) -> String {
    value[field].as_str().expect("a string field").to_owned()
}

fn step(value: &Value, field: &str) -> Option<u64> {
    value[field]
        .as_str()
        .map(|digits| digits.parse().expect("a base-10 step"))
}

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|number| number.as_f64().expect("a number"))
        .collect()
}

/// The batch the fixture describes, built from its own header and points.
fn batch(fixture: &Value) -> TraceBatch {
    let header = &fixture["header"];
    let metadata = &header["metadata"];

    TraceBatch {
        metadata: TraceBatchMetadata {
            run_id: StableId::parse(text(metadata, "runId")).expect("a valid run ID"),
            document_revision: text(metadata, "documentRevision"),
            layout_version: metadata["layoutVersion"].as_u64().unwrap() as u32,
            sample_interval_seconds: metadata["sampleIntervalSeconds"].as_f64().unwrap(),
            history_samples: metadata["historySamples"].as_u64().unwrap() as u32,
            oldest_sample_step: step(metadata, "oldestSampleStep"),
            latest_sample_step: step(metadata, "latestSampleStep"),
            first_sample_step: step(metadata, "firstSampleStep"),
            last_sample_step: step(metadata, "lastSampleStep"),
            sample_count: metadata["sampleCount"].as_u64().unwrap() as u32,
            missed_samples: step(metadata, "missedSamples").unwrap(),
            replaced: metadata["replaced"].as_bool().unwrap(),
        },
        signals: header["signals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|signal| TraceSignal {
                key: text(signal, "key"),
                unit: match text(signal, "unit").as_str() {
                    "volt" => Unit::Volt,
                    "amp" => Unit::Amp,
                    other => panic!("unexpected unit {other}"),
                },
            })
            .collect(),
        points: fixture["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| TracePoint {
                simulation_step: point["simulationStep"].as_u64().unwrap(),
                minimum_values: numbers(&point["minimumValues"]),
                maximum_values: numbers(&point["maximumValues"]),
            })
            .collect(),
    }
}

#[test]
fn encoder_agrees_with_the_independent_golden_encoding() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("the golden fixture must parse");
    let expected = from_hex(fixture["hex"].as_str().unwrap());
    let sequence: u64 = fixture["header"]["traceSequence"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let encoded = encode_trace(&batch(&fixture), sequence).expect("the fixture batch encodes");

    assert_eq!(&encoded[..4], b"OCT1");
    assert_eq!(encoded[..4], expected[..4]);

    // The header is JSON, so it is compared as a value: key order is not part of the
    // contract. The padding and the binary payload are compared byte for byte.
    let header_length = |bytes: &[u8]| u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let actual_length = header_length(&encoded);
    let expected_length = header_length(&expected);
    let actual_header: Value = serde_json::from_slice(&encoded[8..8 + actual_length]).unwrap();
    let expected_header: Value = serde_json::from_slice(&expected[8..8 + expected_length]).unwrap();

    assert_eq!(actual_header, expected_header);
    assert_eq!(actual_header, fixture["header"]);

    let payload_offset = (8 + actual_length).next_multiple_of(8);

    assert_eq!(
        payload_offset,
        (8 + expected_length).next_multiple_of(8),
        "the payload must start at the same 8-byte aligned offset"
    );
    assert_eq!(encoded[payload_offset..], expected[payload_offset..]);
    assert!(
        encoded[8 + actual_length..payload_offset]
            .iter()
            .all(|byte| *byte == 0)
    );
}
