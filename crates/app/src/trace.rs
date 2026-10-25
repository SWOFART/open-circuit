//! Binary envelope for one bounded oscilloscope trace response.

use std::fmt;

use open_circuit_engine::TraceBatch;

pub const MAGIC: [u8; 4] = *b"OCT1";
pub const MAX_TRACE_SIGNALS: usize = 8;
pub const MAX_TRACE_POINTS: usize = 2048;
pub const MAX_HEADER_BYTES: usize = 8192;
pub const MAX_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraceEnvelopeError {
    TooManySignals(usize),
    TooManyPoints(usize),
    InvalidPoint,
    HeaderTooLarge(usize),
    ResponseTooLarge(usize),
}

impl fmt::Display for TraceEnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySignals(count) => write!(formatter, "trace has {count} signals"),
            Self::TooManyPoints(count) => write!(formatter, "trace has {count} points"),
            Self::InvalidPoint => {
                formatter.write_str("trace point dimensions or values are invalid")
            }
            Self::HeaderTooLarge(length) => write!(formatter, "trace header has {length} bytes"),
            Self::ResponseTooLarge(length) => {
                write!(formatter, "trace response has {length} bytes")
            }
        }
    }
}

impl std::error::Error for TraceEnvelopeError {}

/// Encodes a bounded Engine trace batch with a JSON metadata header and little-endian values.
pub fn encode_trace(
    batch: &TraceBatch,
    trace_sequence: u64,
) -> Result<Vec<u8>, TraceEnvelopeError> {
    if batch.signals.is_empty() || batch.signals.len() > MAX_TRACE_SIGNALS {
        return Err(TraceEnvelopeError::TooManySignals(batch.signals.len()));
    }
    if batch.points.len() > MAX_TRACE_POINTS {
        return Err(TraceEnvelopeError::TooManyPoints(batch.points.len()));
    }
    if batch.points.iter().any(|point| {
        point.minimum_values.len() != batch.signals.len()
            || point.maximum_values.len() != batch.signals.len()
            || point
                .minimum_values
                .iter()
                .chain(&point.maximum_values)
                .any(|value| !value.is_finite())
    }) {
        return Err(TraceEnvelopeError::InvalidPoint);
    }

    let point_count = u32::try_from(batch.points.len())
        .map_err(|_| TraceEnvelopeError::TooManyPoints(batch.points.len()))?;
    let header = serde_json::to_vec(&serde_json::json!({
        "traceSequence": trace_sequence.to_string(),
        "metadata": {
            "runId": batch.metadata.run_id.as_str(),
            "documentRevision": &batch.metadata.document_revision,
            "layoutVersion": batch.metadata.layout_version,
            "sampleIntervalSeconds": batch.metadata.sample_interval_seconds,
            "historySamples": batch.metadata.history_samples,
            "oldestSampleStep": batch.metadata.oldest_sample_step.map(|step| step.to_string()),
            "latestSampleStep": batch.metadata.latest_sample_step.map(|step| step.to_string()),
            "firstSampleStep": batch.metadata.first_sample_step.map(|step| step.to_string()),
            "lastSampleStep": batch.metadata.last_sample_step.map(|step| step.to_string()),
            "sampleCount": batch.metadata.sample_count,
            "missedSamples": batch.metadata.missed_samples.to_string(),
            "replaced": batch.metadata.replaced,
        },
        "signals": &batch.signals,
        "pointCount": point_count,
    }))
    .map_err(|_| TraceEnvelopeError::InvalidPoint)?;
    if header.len() > MAX_HEADER_BYTES {
        return Err(TraceEnvelopeError::HeaderTooLarge(header.len()));
    }

    let payload_bytes = batch
        .points
        .len()
        .checked_mul(8 + 16 * batch.signals.len())
        .ok_or(TraceEnvelopeError::ResponseTooLarge(usize::MAX))?;
    let payload_offset = (8 + header.len()).next_multiple_of(8);
    let response_bytes = payload_offset
        .checked_add(payload_bytes)
        .ok_or(TraceEnvelopeError::ResponseTooLarge(usize::MAX))?;
    if response_bytes > MAX_RESPONSE_BYTES {
        return Err(TraceEnvelopeError::ResponseTooLarge(response_bytes));
    }

    let header_length = u32::try_from(header.len())
        .map_err(|_| TraceEnvelopeError::HeaderTooLarge(header.len()))?;
    let mut bytes = Vec::with_capacity(response_bytes);
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&header_length.to_le_bytes());
    bytes.extend_from_slice(&header);
    bytes.resize(payload_offset, 0);
    for point in &batch.points {
        bytes.extend_from_slice(&point.simulation_step.to_le_bytes());
        for (minimum, maximum) in point.minimum_values.iter().zip(&point.maximum_values) {
            bytes.extend_from_slice(&minimum.to_le_bytes());
            bytes.extend_from_slice(&maximum.to_le_bytes());
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_circuit_engine::{StableId, TraceBatchMetadata, TracePoint, TraceSignal, Unit};

    fn batch() -> TraceBatch {
        TraceBatch {
            metadata: TraceBatchMetadata {
                run_id: StableId::parse("00000000-0000-4000-8000-000000000001").unwrap(),
                document_revision: "revision-1".to_owned(),
                layout_version: 1,
                sample_interval_seconds: 0.001,
                history_samples: 10_000,
                oldest_sample_step: Some(0),
                latest_sample_step: Some(2),
                first_sample_step: Some(0),
                last_sample_step: Some(0),
                sample_count: 3,
                missed_samples: 0,
                replaced: false,
            },
            signals: vec![TraceSignal {
                key: "00000000-0000-4000-8000-000000000002:voltage".to_owned(),
                unit: Unit::Volt,
            }],
            points: vec![TracePoint {
                simulation_step: 0,
                minimum_values: vec![-1.5],
                maximum_values: vec![2.0],
            }],
        }
    }

    #[test]
    fn response_has_versioned_header_and_little_endian_point_payload() {
        let bytes = encode_trace(&batch(), 7).unwrap();
        assert_eq!(&bytes[..4], b"OCT1");
        let header_length = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let payload_offset = (8 + header_length).next_multiple_of(8);
        assert!(
            bytes[8 + header_length..payload_offset]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(bytes.len(), payload_offset + 24);
        assert_eq!(
            u64::from_le_bytes(
                bytes[payload_offset..payload_offset + 8]
                    .try_into()
                    .unwrap()
            ),
            0
        );
        assert_eq!(
            f64::from_le_bytes(
                bytes[payload_offset + 8..payload_offset + 16]
                    .try_into()
                    .unwrap()
            ),
            -1.5
        );
        assert_eq!(
            f64::from_le_bytes(
                bytes[payload_offset + 16..payload_offset + 24]
                    .try_into()
                    .unwrap()
            ),
            2.0
        );
        let header: serde_json::Value =
            serde_json::from_slice(&bytes[8..8 + header_length]).unwrap();
        assert_eq!(header["traceSequence"], "7");
        assert_eq!(header["pointCount"], 1);
    }

    #[test]
    fn rejects_unbounded_or_non_finite_trace_data() {
        let mut invalid = batch();
        invalid.points[0].maximum_values[0] = f64::INFINITY;
        assert_eq!(
            encode_trace(&invalid, 1),
            Err(TraceEnvelopeError::InvalidPoint)
        );

        let mut oversized = batch();
        oversized.points = vec![oversized.points[0].clone(); MAX_TRACE_POINTS + 1];
        assert_eq!(
            encode_trace(&oversized, 1),
            Err(TraceEnvelopeError::TooManyPoints(MAX_TRACE_POINTS + 1))
        );
    }

    #[test]
    fn maximum_signal_and_point_counts_stay_within_the_response_limit() {
        let mut maximum = batch();
        maximum.signals = (0..MAX_TRACE_SIGNALS)
            .map(|index| TraceSignal {
                key: format!("signal-{index}"),
                unit: Unit::Volt,
            })
            .collect();
        maximum.points = (0..MAX_TRACE_POINTS)
            .map(|step| TracePoint {
                simulation_step: step as u64,
                minimum_values: vec![-1.0; MAX_TRACE_SIGNALS],
                maximum_values: vec![1.0; MAX_TRACE_SIGNALS],
            })
            .collect();

        let response = encode_trace(&maximum, 1).unwrap();
        assert!(response.len() <= MAX_RESPONSE_BYTES);
    }
}
